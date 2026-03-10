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

/**
 * Client for communicating with the RTK daemon via Unix socket.
 * 
 * Uses JSON-RPC 2.0 protocol over newline-delimited JSON.
 */
export class RTKDaemonClient {
  private socketPath: string;
  private connection: net.Socket | null = null;
  private requestId = 0;
  private isTcp: boolean;
  
  constructor(socketPath: string = isWindows ? "127.0.0.1:9876" : "/tmp/opencode-rtk.sock") {
    this.socketPath = socketPath;
    this.isTcp = socketPath.includes(":");
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
      const timeout = setTimeout(() => {
        reject(new Error("Request timeout"));
        this.connection?.destroy();
      }, 5000);
      
      const request = {
        jsonrpc: "2.0",
        id: ++this.requestId,
        method,
        params,
      };
      
      // Buffer for incomplete data chunks
      let buffer = "";
      
      const onData = (data: Buffer): void => {
        buffer += data.toString();
        
        // Try to find complete JSON messages (newline-delimited)
        // The daemon should send responses terminated with newline
        const lines = buffer.split("\n");
        
        // Keep the last incomplete line in buffer
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
        const [host, portStr] = this.socketPath.split(":");
        const port = parseInt(portStr, 10);
        socket = net.createConnection(port, host);
      } else {
        socket = net.createConnection(this.socketPath);
      }
      
      socket.once("connect", () => {
        this.connection = socket;
        resolve(socket);
      });
      
      socket.once("error", (error) => {
        this.connection = null;
        reject(error);
      });
      
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
