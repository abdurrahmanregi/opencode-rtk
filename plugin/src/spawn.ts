import * as cp from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { RTKDaemonClient } from "./client";

const isWindows = os.platform() === "win32";

export interface DaemonSpawnResult {
  process: cp.ChildProcess | null;
  success: boolean;
  error?: string;
}

/**
 * Validate that binary path is reasonable.
 * 
 * For absolute paths, validates file exists and has correct extension/permissions.
 * For relative paths or names (e.g., "opencode-rtk"), skips validation
 * to allow PATH resolution by the OS.
 * 
 * @param binaryPath - Path to binary or binary name
 * @returns true if valid, false otherwise
 */
function validateBinaryPath(binaryPath: string): boolean {
  if (!binaryPath || binaryPath.trim() === "") {
    return false;
  }
  
  // If it's not an absolute path, let the OS resolve it via PATH
  // This handles cases like "opencode-rtk" or "./opencode-rtk"
  if (!path.isAbsolute(binaryPath)) {
    return true;
  }
  
  try {
    const stats = fs.statSync(binaryPath);
    if (!stats.isFile()) {
      return false;
    }
    
    if (isWindows) {
      const lowerPath = binaryPath.toLowerCase();
      return lowerPath.endsWith(".exe") || 
             lowerPath.endsWith(".bat") ||
             lowerPath.endsWith(".cmd");
    } else {
      return (stats.mode & 0o111) !== 0;
    }
  } catch (error) {
    if (error instanceof Error && 'code' in error && (error as NodeJS.ErrnoException).code === 'ENOENT') {
      console.error(`[RTK] Binary not found: ${binaryPath}`);
    }
    return false;
  }
}

/**
 * Spawn opencode-rtk daemon as a background process.
 * 
 * On Windows: spawns with windowsHide=true to prevent console popup
 * On Unix: spawns with detached=true and setsid
 * 
 * @param binaryPath - Path to opencode-rtk binary (or name if in PATH)
 * @returns DaemonSpawnResult with process and success status
 */
export function spawnDaemon(binaryPath: string): DaemonSpawnResult {
  try {
    if (!validateBinaryPath(binaryPath)) {
      return {
        process: null,
        success: false,
        error: `Invalid binary path: ${binaryPath}`
      };
    }
    
    let child: cp.ChildProcess;
    const spawnOptions: cp.SpawnOptions = {
      detached: true,
      stdio: ["ignore", "ignore", "pipe"],
      windowsHide: isWindows,
    };
    
    // Spawn without manual quoting - cp.spawn() handles this internally
    child = cp.spawn(binaryPath, [], spawnOptions);
    
    // Capture stderr for debugging
    if (child.stderr) {
      child.stderr.on('data', (data) => {
        console.error(`[RTK] Daemon stderr: ${data}`);
      });
    }
    
    // Attach event listeners for async errors
    const errorListener = (error: Error) => {
      console.error(`[RTK] Daemon failed to start: ${error.message}`);
    };
    
    const exitListener = (code: number | null, signal: string | null) => {
      if (code !== null && code !== 0) {
        console.error(`[RTK] Daemon exited with code ${code}`);
      } else if (signal !== null) {
        console.error(`[RTK] Daemon terminated by signal ${signal}`);
      }
    };
    
    child.on('error', errorListener);
    child.on('exit', exitListener);
    
    // Store listener references for cleanup
    (child as any)._rtkListeners = { errorListener, exitListener };
    
    return {
      process: child,
      success: true
    };
  } catch (error) {
    const errMsg = error instanceof Error ? error.message : String(error);
    console.error(`[RTK] Failed to spawn daemon: ${errMsg}`);
    return {
      process: null,
      success: false,
      error: errMsg
    };
  }
}

/**
 * Check if daemon is running and healthy.
 * 
 * @param client - RTKDaemonClient instance
 * @param timeoutMs - Timeout in milliseconds (default 5000ms)
 * @returns true if daemon responds to health check
 */
export async function isDaemonRunning(client: RTKDaemonClient, timeoutMs: number = 5000): Promise<boolean> {
  try {
    const result = await Promise.race([
      client.health(),
      new Promise<boolean>((_, reject) => 
        setTimeout(() => reject(new Error('Health check timeout')), timeoutMs)
      )
    ]);
    return result;
  } catch (e) {
    if ((e as Error).message === 'Health check timeout') {
      console.debug(`[RTK] Health check timeout after ${timeoutMs}ms`);
    }
    return false;
  }
}

/**
 * Wait for daemon to become healthy after spawning.
 * 
 * Polls health check until success or max attempts reached.
 * Uses exponential backoff with jitter for retry delays.
 * 
 * @param client - RTKDaemonClient instance
 * @param maxAttempts - Maximum number of health check attempts
 * @param initialDelayMs - Initial delay between attempts in milliseconds
 * @param maxDelayMs - Maximum backoff delay in milliseconds
 * @returns true if daemon becomes healthy
 */
export async function waitForDaemon(
  client: RTKDaemonClient,
  maxAttempts: number = 15,
  initialDelayMs: number = 200,
  maxDelayMs: number = 2000
): Promise<boolean> {
  for (let i = 0; i < maxAttempts; i++) {
    const isHealthy = await isDaemonRunning(client);
    if (isHealthy) {
      return true;
    }
    
    // Exponential backoff with jitter
    const backoff = Math.min(initialDelayMs * Math.pow(1.5, i), maxDelayMs);
    const jitter = Math.random() * 50;
    const delay = backoff + jitter;
    
    await new Promise(resolve => setTimeout(resolve, delay));
  }
  
  return false;
}

/**
 * Cleanup event listeners on a child process to prevent memory leaks.
 * 
 * @param child - Child process to cleanup
 */
function cleanupProcessListeners(child: cp.ChildProcess): void {
  try {
    const listeners = (child as any)._rtkListeners;
    if (listeners) {
      child.removeListener('error', listeners.errorListener);
      child.removeListener('exit', listeners.exitListener);
    }
    child.removeAllListeners();
    if (child.stderr) {
      child.stderr.removeAllListeners();
    }
  } catch (e) {
    // Ignore cleanup errors
  }
}

/**
 * Attempt to start daemon automatically.
 * 
 * Combines spawning and waiting logic for convenience.
 * Cleans up process if health check fails.
 * 
 * @param binaryPath - Path to opencode-rtk binary
 * @param client - RTKDaemonClient instance
 * @returns true if daemon was started and is healthy
 */
export async function autoStartDaemon(
  binaryPath: string,
  client: RTKDaemonClient
): Promise<boolean> {
  console.log(`[RTK] Daemon not running, starting '${binaryPath}'...`);
  
  const result = spawnDaemon(binaryPath);
  
  if (!result.success) {
    console.warn(`[RTK] Failed to spawn daemon: ${result.error}`);
    return false;
  }
  
  if (!result.process) {
    console.warn(`[RTK] Spawn succeeded but no process returned`);
    return false;
  }
  
  const child = result.process;
  
  const started = await waitForDaemon(client);
  
  if (started) {
    console.log("[RTK] Daemon started successfully");
    if (!isWindows) {
      child.unref();
    }
    return true;
  } else {
    console.warn("[RTK] Daemon start timeout, killing spawned process");
    try {
      child.kill('SIGTERM');
    } catch (e) {
      if (isWindows) {
        console.debug(`[RTK] SIGTERM not supported on Windows, using SIGKILL`);
      } else {
        console.debug(`[RTK] SIGTERM failed: ${e}`);
      }
    }
    
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    try {
      child.kill('SIGKILL');
    } catch (e) {
      // Process already exited
    }
    
    cleanupProcessListeners(child);
    console.warn("[RTK] Token optimization may not work");
    console.error("[RTK] Debugging tips:");
    console.error("  1. Verify 'opencode-rtk' binary exists and is executable");
    console.error("  2. Check port 9876 is available: netstat -an | findstr 9876");
    console.error("  3. Run daemon manually to see error messages");
    return false;
  }
}
