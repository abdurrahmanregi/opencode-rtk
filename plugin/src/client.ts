import * as net from "net";
import * as os from "os";
import { StringDecoder } from "string_decoder";
import { isTcpAddress, parseTcpAddress } from "./address";

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
 * Client for communicating with the RTK daemon via TCP or Unix socket.
 *
 * Uses JSON-RPC 2.0 protocol over newline-delimited JSON.
 */
const MAX_BUFFER_SIZE = 10 * 1024 * 1024; // 10MB
// Windows needs longer timeout due to slower network stack and firewall checks
const REQUEST_TIMEOUT_MS = isWindows ? 8000 : 5000;
const CONNECT_TIMEOUT_MS = isWindows ? 3000 : 2000; // Connection-level timeout
const MAX_RECONNECT_ATTEMPTS = 3;
const RECONNECT_DELAY_MS = 100;
const PROBE_TIMEOUT_MS = 1000; // Increased from 500ms for reliability

export class RTKDaemonClient {
  private socketPath: string;
  private connection: net.Socket | null = null;
  private requestId = 0;
  private isTcp: boolean;
  private requestQueue: Promise<unknown> = Promise.resolve();

  constructor(socketPath: string) {
    this.socketPath = socketPath;
    // On Windows, always use TCP (no Unix socket support).
    // On Unix, use strict host:port / [ipv6]:port parsing.
    this.isTcp = isWindows || isTcpAddress(socketPath);
  }

  getSocketPath(): string {
    return this.socketPath;
  }

  /**
   * Quick probe to check if existing connection is alive.
   *
   * This is a lightweight check that tests socket health without making a full API call,
   * avoiding circular dependency (probeConnection -> call -> connect -> probeConnection).
   *
   * Trade-off: Speed vs. Accuracy
   * - Quick socket state check (no network round-trip)
   * - May return true for connections that are about to close
   * - Acceptable because the next actual call will fail and trigger reconnect
   */
  private async probeConnection(): Promise<boolean> {
    if (!this.connection || this.connection.destroyed) {
      return false;
    }

    try {
      // Quick socket state validation with timeout
      return await new Promise<boolean>((resolve) => {
        const timeout = setTimeout(() => resolve(false), PROBE_TIMEOUT_MS);

        try {
          // Check writable, readable, and readyState for health indicators
          const socket = this.connection!;
          const isHealthy =
            !socket.destroyed &&
            socket.writable &&
            socket.readable &&
            socket.readyState === "open";

          clearTimeout(timeout);
          resolve(isHealthy);
        } catch {
          clearTimeout(timeout);
          resolve(false);
        }
      });
    } catch {
      return false;
    }
  }

  async compress(request: CompressRequest): Promise<CompressResponse> {
    const response = await this.call("compress", request);
    return response as CompressResponse;
  }

  async health(): Promise<boolean> {
    try {
      console.log(`[RTK] Sending health check request...`);
      const response = await this.call("health", {});
      console.log(`[RTK] Health check response:`, response);
      return (response as { status: string }).status === "ok";
    } catch (error) {
      console.error(`[RTK] Health check error: ${error instanceof Error ? error.message : String(error)}`);
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
   * - Added request queue to serialize concurrent calls (prevents race condition)
   */
  private async call(method: string, params: unknown): Promise<unknown> {
    const run = () => this._callInternal(method, params);
    const requestPromise = this.requestQueue.then(run, run);

    // Keep queue healthy even if one request fails.
    // We still return the actual request promise to the caller.
    this.requestQueue = requestPromise.then(
      () => undefined,
      () => undefined,
    );

    return requestPromise;
  }

  /**
   * Internal implementation of JSON-RPC call.
   *
   * This is the actual implementation that gets serialized by the request queue.
   */
  private async _callInternal(method: string, params: unknown): Promise<unknown> {
    // Connect first (outside Promise to avoid async in executor)
    const socket = await this.connect();

    return new Promise((resolve, reject) => {
      let settled = false;

      const cleanup = (): void => {
        clearTimeout(timeout);
        socket.off("data", onData);
        socket.off("error", onError);
        socket.off("close", onClose);
      };

      const finishWithError = (error: Error, resetConnection: boolean): void => {
        if (settled) {
          return;
        }
        settled = true;
        cleanup();
        if (resetConnection) {
          this.connection = null;
          socket.destroy();
        }
        reject(error);
      };

      const finishWithResult = (result: unknown): void => {
        if (settled) {
          return;
        }
        settled = true;
        cleanup();
        resolve(result);
      };

      const timeout = setTimeout(() => {
        finishWithError(new Error("Request timeout"), true);
      }, REQUEST_TIMEOUT_MS);

      const request = {
        jsonrpc: "2.0",
        id: ++this.requestId,
        method,
        params,
      };

      let buffer = "";
      const decoder = new StringDecoder("utf8");

      const onData = (data: Buffer): void => {
        buffer += decoder.write(data);

        // Keep protection in bytes (not UTF-16 code units).
        if (Buffer.byteLength(buffer, "utf8") > MAX_BUFFER_SIZE) {
          finishWithError(new Error("Response too large"), true);
          return;
        }

        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          if (!line.trim()) {
            continue; // Skip empty lines
          }

          try {
            const response = JSON.parse(line) as {
              jsonrpc: string;
              id: number;
              result?: unknown;
              error?: { message?: string };
            };

            if (response.id !== request.id) {
              continue;
            }

            if (response.error) {
              finishWithError(
                new Error(response.error.message || "Unknown error"),
                false,
              );
            } else {
              finishWithResult(response.result);
            }
            return; // Only process first complete response
          } catch (parseError) {
            // We split on newline, so each line is a complete frame.
            // A parse error means malformed daemon output and should fail fast.
            const message = parseError instanceof Error ? parseError.message : String(parseError);
            finishWithError(new Error(`Invalid JSON-RPC response frame: ${message}`), true);
            return;
          }
        }
      };

      const onError = (error: Error): void => {
        console.error(`[RTK] Socket error during request: ${error.message}`);
        finishWithError(error, true);
      };

      const onClose = (): void => {
        finishWithError(new Error("Connection closed before response"), true);
      };

      socket.on("data", onData);
      socket.once("error", onError);
      socket.once("close", onClose);

      // Send request with newline terminator for newline-delimited JSON
      socket.write(JSON.stringify(request) + "\n");
    });
  }

  private async connect(): Promise<net.Socket> {
    // If existing connection is healthy, reuse it
    if (this.connection && !this.connection.destroyed) {
      if (await this.probeConnection()) {
        return this.connection;
      }
      // Connection is stale, reconnect
      this.disconnect();
    }

    // Reconnect with retry logic
    let lastError: Error | null = null;
    for (let attempt = 0; attempt < MAX_RECONNECT_ATTEMPTS; attempt++) {
      try {
        const socket = await this.createConnection();
        return socket;
      } catch (error) {
        lastError = error as Error;
        if (attempt < MAX_RECONNECT_ATTEMPTS - 1) {
          const delay = RECONNECT_DELAY_MS * Math.pow(2, attempt);
          await new Promise((r) => setTimeout(r, delay));
        }
      }
    }

    throw lastError || new Error("Failed to connect after max attempts");
  }

  /**
   * Create a new connection (internal helper).
   */
  private async createConnection(): Promise<net.Socket> {
    return new Promise((resolve, reject) => {
      let socket: net.Socket;
      let handshakeTimer: NodeJS.Timeout | null = null;
      let runtimeErrorHandler: ((error: Error) => void) | null = null;

      if (this.isTcp) {
        const parsed = parseTcpAddress(this.socketPath);
        if (!parsed) {
          throw new Error(`Invalid TCP address: ${this.socketPath}`);
        }
        const { host, port } = parsed;
        console.log(`[RTK] Creating TCP connection to ${host}:${port}...`);
        socket = net.createConnection(port, host);
      } else {
        console.log(`[RTK] Creating Unix socket connection to ${this.socketPath}...`);
        socket = net.createConnection(this.socketPath);
      }

      // Connection-level timeout (separate from request timeout)
      const connectTimeout = setTimeout(() => {
        console.error(`[RTK] Connection timeout after ${CONNECT_TIMEOUT_MS}ms`);
        socket.off("error", errorHandler);
        socket.off("connect", connectHandler);
        if (handshakeTimer) {
          clearTimeout(handshakeTimer);
          handshakeTimer = null;
        }
        socket.destroy();
        reject(new Error(`Connection timeout after ${CONNECT_TIMEOUT_MS}ms`));
      }, CONNECT_TIMEOUT_MS);

      // Attach error handler BEFORE connect to handle immediate failures
      const errorHandler = (error: Error) => {
        clearTimeout(connectTimeout);
        if (handshakeTimer) {
          clearTimeout(handshakeTimer);
          handshakeTimer = null;
        }
        console.error(`[RTK] Connection error: ${error.message}`);
        this.connection = null;
        socket.off("error", errorHandler);
        // Don't call socket.destroy() - let Node.js handle cleanup naturally
        reject(error);
      };

      const connectHandler = () => {
        clearTimeout(connectTimeout);
        
        // CRITICAL: Add handshake delay for Windows TCP stack
        // Windows TCP handshake takes 100-200ms before socket is ready for writes
        const handshakeDelay = isWindows ? 150 : 50;
        
        handshakeTimer = setTimeout(() => {
          if (socket.destroyed) {
            console.error(`[RTK] Socket destroyed during handshake`);
            reject(new Error('Socket destroyed during handshake'));
            return;
          }
          console.log(`[RTK] TCP connection established and ready`);
          this.connection = socket;
          socket.off("error", errorHandler);
          runtimeErrorHandler = (error: Error): void => {
            console.error(`[RTK] Socket runtime error: ${error.message}`);
            this.connection = null;
            if (!socket.destroyed) {
              socket.destroy();
            }
          };
          socket.on("error", runtimeErrorHandler);
          handshakeTimer = null;
          resolve(socket);
        }, handshakeDelay);
      };

      socket.once("error", errorHandler);
      socket.once("connect", connectHandler);

      socket.once("close", () => {
        this.connection = null;
        if (runtimeErrorHandler) {
          socket.off("error", runtimeErrorHandler);
          runtimeErrorHandler = null;
        }
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
