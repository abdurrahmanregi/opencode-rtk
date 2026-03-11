import * as net from "net";
import * as os from "os";

const isWindows = os.platform() === "win32";

export interface CompressRequest {
  command: string;
  output: string;
  context?: {
    cwd?: string;
    exit_code?: number;
    tool?: string;
    session_id?: string;
  };
}

export interface CompressResponse {
  compressed: string;
  original_tokens: number;
  compressed_tokens: number;
  saved_tokens: number;
  savings_pct: number;
  strategy: string;
  module: string;
}

export interface StatsResponse {
  command_count: number;
  total_original_tokens: number;
  total_compressed_tokens: number;
  total_saved_tokens: number;
  savings_pct: number;
}

export interface OptimizeResponse {
  original: string;
  optimized: string;
  flags_added: string[];
  skipped: boolean;
  skip_reason?: string;
}

export interface TeeSaveResponse {
  path: string;
  size: number;
}

export interface TeeListResponse {
  files: TeeFileInfo[];
  total: number;
}

export interface TeeFileInfo {
  path: string;
  command: string;
  timestamp: string;
  size: number;
}

export interface TeeReadResponse {
  content: string;
  size: number;
}

/**
 * Client for communicating with the RTK daemon via Unix socket.
 *
 * Uses JSON-RPC 2.0 protocol over newline-delimited JSON.
 */
const MAX_BUFFER_SIZE = 10 * 1024 * 1024; // 10MB
const REQUEST_TIMEOUT_MS = 5000;

export class RTKDaemonClient {
  private socketPath: string;
  private connection: net.Socket | null = null;
  private requestId = 0;
  private isTcp: boolean;

  constructor(socketPath: string) {
    this.socketPath = socketPath;
    // On Windows, always use TCP (no Unix socket support)
    // On Unix, use TCP if address contains ":" (host:port format)
    this.isTcp = isWindows || socketPath.includes(":");
  }

  async compress(request: CompressRequest): Promise<CompressResponse> {
    const response = await this.call("compress", request);
    return response as CompressResponse;
  }

  async health(): Promise<boolean> {
    try {
      const response = await this.call("health", {});
      return (response as { status: string }).status === "ok";
    } catch {
      return false;
    }
  }

  async stats(sessionId?: string): Promise<StatsResponse> {
    const params = sessionId ? { session_id: sessionId } : {};
    const response = await this.call("stats", params);
    return response as StatsResponse;
  }

  /**
   * Optimize a command by adding appropriate flags
   *
   * @param command - The command string to optimize
   * @returns Optimization result with original, optimized, and flags added
   */
  async optimizeCommand(command: string): Promise<OptimizeResponse> {
    const response = await this.call("optimize", { command });
    return response as OptimizeResponse;
  }

  /**
   * Save output to tee file
   */
  async saveTee(command: string, output: string): Promise<TeeSaveResponse> {
    const response = await this.call("tee_save", { command, output });
    return response as TeeSaveResponse;
  }

  /**
   * List tee files
   */
  async listTee(): Promise<TeeListResponse> {
    const response = await this.call("tee_list", {});
    return response as TeeListResponse;
  }

  /**
   * Read tee file content
   */
  async readTee(path: string): Promise<TeeReadResponse> {
    const response = await this.call("tee_read", { path });
    return response as TeeReadResponse;
  }

  /**
   * Clear all tee files
   */
  async clearTee(): Promise<{ deleted: number }> {
    const response = await this.call("tee_clear", {});
    return response as { deleted: number };
  }

  /**
   * Make a JSON-RPC call to the daemon.
   *
   * Fixed issues:
   * - Removed async from Promise executor (anti-pattern)
   * - Added proper data buffering for incomplete chunks
   * - Handles newline-delimited JSON protocol
   */
  private async call(method: string, params: unknown): Promise<unknown> {
    // Connect first (outside Promise to avoid async in executor)
    const socket = await this.connect();

    return new Promise((resolve, reject) => {
      const timeout =       setTimeout(() => {
        reject(new Error("Request timeout"));
        socket.destroy();
        this.connection = null;
      }, REQUEST_TIMEOUT_MS);

      const request = {
        jsonrpc: "2.0",
        id: ++this.requestId,
        method,
        params,
      };

      let buffer = "";

      const onData = (data: Buffer): void => {
        buffer += data.toString();

        if (buffer.length > MAX_BUFFER_SIZE) {
          clearTimeout(timeout);
          reject(new Error("Response too large"));
          socket.destroy();
          this.connection = null;
          return;
        }

        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          if (!line.trim()) {
            continue; // Skip empty lines
          }

          try {
            const response = JSON.parse(line);

            clearTimeout(timeout);
            socket.off("data", onData);
            socket.off("error", onError);

            if (response.error) {
              reject(new Error(response.error.message || "Unknown error"));
            } else {
              resolve(response.result);
            }
            return; // Only process first complete response
          } catch (parseError) {
            // If we can't parse, maybe incomplete - continue buffering
            // Log for debugging but don't reject yet
            console.warn("RTK: Failed to parse response chunk:", parseError);
          }
        }
      };

      const onError = (error: Error): void => {
        clearTimeout(timeout);
        this.connection = null;
        socket.off("data", onData);
        reject(error);
      };

      socket.on("data", onData);
      socket.once("error", onError);

      // Send request with newline terminator for newline-delimited JSON
      socket.write(JSON.stringify(request) + "\n");
    });
  }

  private async connect(): Promise<net.Socket> {
    if (this.connection && !this.connection.destroyed) {
      return this.connection;
    }

    return new Promise((resolve, reject) => {
      let socket: net.Socket;

      if (this.isTcp) {
        const parts = this.socketPath.split(":");
        if (parts.length !== 2) {
          throw new Error(`Invalid TCP address: ${this.socketPath}`);
        }
        const [host, portStr] = parts;
        const port = parseInt(portStr, 10);
        if (isNaN(port) || port <= 0 || port > 65535) {
          throw new Error(`Invalid port: ${portStr}`);
        }
        socket = net.createConnection(port, host);
      } else {
        socket = net.createConnection(this.socketPath);
      }

      // Attach error handler BEFORE connect to handle immediate failures
      const errorHandler = (error: Error) => {
        this.connection = null;
        reject(error);
      };

      const connectHandler = () => {
        this.connection = socket;
        socket.off("error", errorHandler);
        resolve(socket);
      };

      socket.once("error", errorHandler);
      socket.once("connect", connectHandler);

      socket.once("close", () => {
        this.connection = null;
      });
    });
  }

  disconnect(): void {
    if (this.connection) {
      this.connection.destroy();
      this.connection = null;
    }
  }
}
