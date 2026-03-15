import * as cp from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as net from "net";
import { RTKDaemonClient } from "./client";
import { parseTcpAddress } from "./address";

const isWindows = os.platform() === "win32";
const isVerboseStartupLogging = process.env.RTK_VERBOSE_STARTUP_LOGS === "1";

function logVerboseStartup(message: string): void {
  if (isVerboseStartupLogging) {
    console.log(message);
  }
}

/**
 * Check if a TCP port is available (not in use).
 * 
 * On Unix: Can use any available port
 * On Windows: Check if port 9876 is available
 * 
 * @param port - Port number to check
 * @param host - Host/interface to test bind on
 * @returns true if port is available, false if in use
 */
async function isPortAvailable(port: number, host: string): Promise<boolean> {
  return new Promise((resolve) => {
    const server = net.createServer();
    
    server.on('error', (err: Error & { code?: string }) => {
      if (err.code === 'EADDRINUSE') {
        resolve(false);
      } else if (err.code === 'EAFNOSUPPORT' || err.code === 'EADDRNOTAVAIL') {
        // Host family/address not supported on this machine; do not block startup.
        resolve(true);
      } else {
        resolve(false);
      }
    });
    
    server.once('listening', () => {
      server.once('close', () => resolve(true));
      server.close();
    });
    
    server.listen(port, host);
  });
}

function isLocalBindHost(host: string): boolean {
  const normalized = host.trim().toLowerCase();
  return (
    normalized === "127.0.0.1" ||
    normalized === "::1" ||
    normalized === "localhost" ||
    normalized === "0.0.0.0" ||
    normalized === "::"
  );
}

/**
 * Extended child process with RTK-specific listener storage
 */
interface RTKChildProcess extends cp.ChildProcess {
  _rtkListeners?: {
    errorListener: (error: Error) => void;
    exitListener: (code: number | null, signal: string | null) => void;
  };
}

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
    console.log(`[RTK] Binary path is relative, will use PATH resolution`);
    return true;
  }
  
  try {
    const stats = fs.statSync(binaryPath);
    if (!stats.isFile()) {
      console.error(`[RTK] Path exists but is not a file: ${binaryPath}`);
      return false;
    }
    
    if (isWindows) {
      const lowerPath = binaryPath.toLowerCase();
      const validExt = lowerPath.endsWith(".exe") || 
                       lowerPath.endsWith(".bat") ||
                       lowerPath.endsWith(".cmd");
      if (!validExt) {
        console.error(`[RTK] Binary must have .exe, .bat, or .cmd extension on Windows`);
      }
      return validExt;
    } else {
      const isExecutable = (stats.mode & 0o111) !== 0;
      if (!isExecutable) {
        console.error(`[RTK] Binary is not executable: ${binaryPath}`);
      }
      return isExecutable;
    }
  } catch (error) {
    if (error instanceof Error && 'code' in error && (error as NodeJS.ErrnoException).code === 'ENOENT') {
      console.error(`[RTK] ❌ Binary not found: ${binaryPath}`);
      console.error(`[RTK] Possible solutions:`);
      console.error(`[RTK]   1. Build the binary: cargo build --release`);
      console.error(`[RTK]   2. Set RTK_DAEMON_PATH environment variable`);
      console.error(`[RTK]   3. Add binary to PATH`);
    } else {
      console.error(`[RTK] Failed to validate binary path: ${String(error)}`);
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
    
    // On Windows, spawn daemon directly with CREATE_NEW_PROCESS_GROUP
    // This creates a truly independent process that survives parent exit
    if (isWindows) {
      console.log(`[RTK] Spawning daemon on Windows...`);
      
      // Spawn the daemon directly with detached: true
      // On Windows, this creates a new process group that survives parent exit
      const child = cp.spawn(binaryPath, [], {
        detached: true,
        stdio: 'ignore',
        windowsHide: true,
      }) as RTKChildProcess;

      console.log(`[RTK] Daemon spawn returned process with PID: ${child.pid}`);

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

      child._rtkListeners = { errorListener, exitListener };

      // Immediately unref to allow parent to exit
      child.unref();
      
      return {
        process: child,
        success: true
      };
    }
    
    // Unix: Use standard detached spawn
    const spawnOptions: cp.SpawnOptions = {
      detached: true,
      stdio: "ignore",
    };

    console.log(`[RTK] Spawning daemon with options:`, {
      detached: spawnOptions.detached,
    });

    const child = cp.spawn(binaryPath, [], spawnOptions) as RTKChildProcess;

    console.log(`[RTK] Daemon spawn returned process with PID: ${child.pid}`);
    
    // Immediately unref to allow parent to exit
    child.unref();

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
    child._rtkListeners = { errorListener, exitListener };
    
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
 * @param timeoutMs - Timeout in milliseconds (default 10000ms for Windows, 5000ms for Unix)
 * @returns true if daemon responds to health check
 */
export async function isDaemonRunning(client: RTKDaemonClient, timeoutMs?: number): Promise<boolean> {
  // Windows needs longer timeout due to slower process spawning and TCP overhead
  const effectiveTimeout = timeoutMs ?? (isWindows ? 10000 : 5000);
  console.log(`[RTK] Checking daemon health (timeout: ${effectiveTimeout}ms)...`);

  let timeoutId: NodeJS.Timeout | null = null;
  const probeClient = new RTKDaemonClient(client.getSocketPath());

  try {
    const healthPromise = probeClient.health();
    const timeoutPromise = new Promise<boolean>((resolve) => {
      timeoutId = setTimeout(() => {
        console.error(`[RTK] Health check timed out after ${effectiveTimeout}ms`);
        resolve(false);
      }, effectiveTimeout);
    });

    const result = await Promise.race([healthPromise, timeoutPromise]);

    if (timeoutId) {
      clearTimeout(timeoutId);
    }

    console.log(`[RTK] Health check result: ${result}`);
    return result;
  } catch (e) {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }

    const errMsg = (e as Error).message;
    console.error(`[RTK] Health check failed: ${errMsg}`);
    return false;
  } finally {
    probeClient.disconnect();
  }
}

/**
 * Wait for daemon to become healthy after spawning.
 * 
 * Polls health check until success or max attempts reached.
 * Uses exponential backoff with jitter for retry delays.
 * 
 * @param client - RTKDaemonClient instance
 * @param maxAttempts - Maximum number of health check attempts (default 15)
 * @param initialDelayMs - Initial delay between attempts in milliseconds (default 200ms)
 * @param maxDelayMs - Maximum backoff delay in milliseconds (default 2000ms)
 * @returns true if daemon becomes healthy
 */
export async function waitForDaemon(
  client: RTKDaemonClient,
  maxAttempts: number = 15,
  initialDelayMs: number = isWindows ? 500 : 200,
  maxDelayMs: number = 2000
): Promise<boolean> {
  console.log(`[RTK] waitForDaemon: Starting with maxAttempts=${maxAttempts}`);
  
  for (let i = 0; i < maxAttempts; i++) {
    console.log(`[RTK] waitForDaemon: Attempt ${i+1}/${maxAttempts}`);
    const isHealthy = await isDaemonRunning(client, isWindows ? 3000 : 2000);
    
    if (isHealthy) {
      console.log(`[RTK] waitForDaemon: SUCCESS on attempt ${i+1}`);
      return true;
    }
    
    // Exponential backoff with jitter
    const backoff = Math.min(initialDelayMs * Math.pow(1.5, i), maxDelayMs);
    const jitter = Math.random() * 50;
    const delay = backoff + jitter;
    
    console.log(`[RTK] waitForDaemon: Failed attempt ${i+1}, waiting ${Math.round(delay)}ms before next attempt`);
    
    await new Promise(resolve => setTimeout(resolve, delay));
  }
  
  console.log(`[RTK] waitForDaemon: TIMEOUT after ${maxAttempts} attempts`);
  return false;
}

/**
 * Cleanup event listeners on a child process to prevent memory leaks.
 *
 * @param child - Child process to cleanup
 */
function cleanupProcessListeners(child: cp.ChildProcess): void {
  try {
    const listeners = (child as RTKChildProcess)._rtkListeners;
    if (listeners) {
      child.removeListener('error', listeners.errorListener);
      child.removeListener('exit', listeners.exitListener);
    }
    child.removeAllListeners();
    if (child.stderr) {
      child.stderr.removeAllListeners();
    }
  } catch {
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
  console.log("[RTK] Daemon not running, starting...");
  logVerboseStartup(`[RTK] Binary path: ${binaryPath}`);
  logVerboseStartup(`[RTK] Platform: ${os.platform()}`);
  logVerboseStartup(`[RTK] Current directory: ${process.cwd()}`);
  logVerboseStartup(`[RTK] Node version: ${process.version}`);
  
  // Check if binary exists (for debugging)
  if (path.isAbsolute(binaryPath)) {
    try {
      fs.accessSync(binaryPath, fs.constants.X_OK);
      console.log(`[RTK] Binary exists and is executable`);
    } catch {
      console.warn(`[RTK] Binary may not exist or be executable: ${binaryPath}`);
    }
  }
  
  // Check if port is available (TCP only)
  const socketPath = client.getSocketPath();
  const parsedAddress = parseTcpAddress(socketPath);
  const port = parsedAddress?.port ?? null;
  const host = parsedAddress?.host ?? "127.0.0.1";
  if (port !== null) {
    if (!isLocalBindHost(host)) {
      console.log(`[RTK] Skipping local port precheck for non-local host '${host}'`);
    } else {
    const hostsToCheck = host === "localhost" ? ["127.0.0.1", "::1"] : [host];
    console.log(`[RTK] Checking if port ${port} on ${hostsToCheck.join(", ")} is available...`);

    let portAvailable = true;
    for (const checkHost of hostsToCheck) {
      const availableOnHost = await isPortAvailable(port, checkHost);
      if (!availableOnHost) {
        portAvailable = false;
        break;
      }
    }

    if (!portAvailable) {
      console.error(`[RTK] WARNING: Port ${port} is already in use!`);
      console.error(`[RTK] Checking if existing daemon is responsive...`);
      
      // Try to connect to existing daemon
      const existingHealthy = await isDaemonRunning(client, 3000);
      if (existingHealthy) {
        console.log(`[RTK] Existing daemon is responsive, reusing it`);
        return true;
      } else {
        console.error(`[RTK] Port ${port} is in use but daemon is not responding!`);
        console.error(`[RTK] This indicates a zombie daemon process.`);
        console.error(`[RTK] Manual cleanup required:`);
        if (isWindows) {
          console.error(`[RTK]   1. Find PID: netstat -ano | findstr ${port}`);
          console.error(`[RTK]   2. Kill process: taskkill /F /PID <PID>`);
        } else {
          console.error(`[RTK]   1. Find PID: lsof -i :${port}`);
          console.error(`[RTK]   2. Kill process: kill -9 <PID>`);
        }
        return false;
      }
    } else {
      console.log(`[RTK] Port ${port} is available`);
    }
    }
  }
  
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
  console.log(`[RTK] Daemon process spawned with PID: ${child.pid}`);

  // Monitor process exit early
  let earlyExit = false;
  const earlyExitListener = (code: number | null, signal: string | null) => {
    earlyExit = true;
    console.error(`[RTK] Daemon exited unexpectedly! Code: ${code}, Signal: ${signal}`);
  };
  child.once('exit', earlyExitListener);
  
  // Give daemon a moment to start before first health check
  const startupDelay = isWindows ? 1500 : 500;
  console.log(`[RTK] Waiting ${startupDelay}ms for daemon to initialize...`);
  await new Promise(resolve => setTimeout(resolve, startupDelay));
  
  console.log(`[RTK] autoStartDaemon: Calling waitForDaemon...`);
  const started = await waitForDaemon(client);
  console.log(`[RTK] autoStartDaemon: waitForDaemon returned ${started}`);
  
  // Remove the early exit listener
  child.removeListener('exit', earlyExitListener);
  
  if (started) {
    console.log("[RTK] Daemon started successfully");
    child.unref();  // Unref on ALL platforms for true detachment
    return true;
  } else {
    if (earlyExit) {
      console.error("[RTK] Daemon exited before health check could complete");
      console.error("[RTK] Check daemon logs for startup errors");
    } else {
      console.warn("[RTK] Daemon start timeout, killing spawned process");
    }
    
    try {
      // On Windows, use SIGKILL directly; on Unix, try SIGTERM first
      if (isWindows) {
        child.kill('SIGKILL');
      } else {
        child.kill('SIGTERM');
      }
    } catch (e) {
      console.debug(`[RTK] Kill signal failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    
    // On Unix, wait and try SIGKILL if SIGTERM didn't work
    if (!isWindows) {
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      try {
        child.kill('SIGKILL');
      } catch (e) {
        // Process already exited
      }
    }
    
    cleanupProcessListeners(child);
    console.warn("[RTK] Token optimization may not work");
    console.error("[RTK] Debugging tips:");
    console.error("  1. Verify 'opencode-rtk' binary exists and is executable");
    if (port !== null) {
      console.error(`  2. Check port ${port} is available: netstat -an | findstr ${port}`);
    } else {
      console.error(`  2. Check socket ${socketPath} is not in use`);
    }
    console.error("  3. Run daemon manually to see error messages");
    console.error(`     ${binaryPath}`);
    return false;
  }
}
