import type { Plugin } from "@opencode-ai/plugin";
import { RTKDaemonClient } from "./client";
import { onToolExecuteBefore } from "./hooks/tool-before";
import { onToolExecuteAfter } from "./hooks/tool-after";
import { onSessionIdle } from "./hooks/session";
import { startCleanupTimer } from "./state";
import { isDaemonRunning, autoStartDaemon } from "./spawn";
import {
  resolveModelRuntimePolicy,
} from "./model-detection";
import type {
  ModelRuntimePolicy,
  PostExecutionCompressionMode,
  PreExecutionMode,
} from "./types";

import * as os from "os";
import * as path from "path";
import * as fs from "fs";

let startPromise: Promise<boolean> | null = null;

const isWindows = os.platform() === "win32";
const RTK_SOCKET_PATH =
  process.env.RTK_DAEMON_ADDR ||
  (isWindows ? "127.0.0.1:9876" : "/tmp/opencode-rtk.sock");

// Resolve binary path:
// 1. Check RTK_DAEMON_PATH environment variable (for production)
// 2. Fall back to relative path from plugin to target/release (for development)
// 3. Fall back to binary name (relies on PATH)
const DEV_BINARY = path.join(
  __dirname,
  "..",
  "..",
  "target",
  "release",
  isWindows ? "opencode-rtk.exe" : "opencode-rtk",
);

const RTK_BINARY = process.env.RTK_DAEMON_PATH ||
  (fs.existsSync(DEV_BINARY)
    ? DEV_BINARY
    : (isWindows ? "opencode-rtk.exe" : "opencode-rtk"));

const MAX_RESTARTS_PER_SESSION = 3;
const FAILED_HEALTH_RECHECK_MS = 1000;
const HEALTHY_HEALTH_RECHECK_MS = 1500;
const PRE_EXECUTION_MODE: PreExecutionMode = resolvePreExecutionMode();
const EXPLICIT_POST_EXECUTION_MODE: PostExecutionCompressionMode | null =
  resolveExplicitPostExecutionCompressionMode();
const ACTIVE_MODEL_POLICY: ModelRuntimePolicy = resolveModelRuntimePolicy(
  EXPLICIT_POST_EXECUTION_MODE,
);

let lastHealthCheckTime = 0;
let lastHealthCheckResult = false;
let daemonRestartCount = 0;

export function shouldUseCachedHealth(
  elapsedMs: number,
  lastResult: boolean,
  forceCheck: boolean = false,
): boolean {
  if (forceCheck) {
    return false;
  }

  if (lastResult) {
    return elapsedMs < HEALTHY_HEALTH_RECHECK_MS;
  }

  return elapsedMs < FAILED_HEALTH_RECHECK_MS;
}

// Mutex-like pattern to prevent concurrent restart attempts
let restartPromise: Promise<boolean> | null = null;

async function ensureDaemonRunning(
  client: RTKDaemonClient,
  forceCheck: boolean = false
): Promise<boolean> {
  const now = Date.now();
  const elapsed = now - lastHealthCheckTime;

  // Throttle health checks; recheck sooner after recent failures.
  if (shouldUseCachedHealth(elapsed, lastHealthCheckResult, forceCheck)) {
    return lastHealthCheckResult;
  }

  const isHealthy = await client.health();
  lastHealthCheckTime = Date.now();
  lastHealthCheckResult = isHealthy;

  if (isHealthy) {
    return true;
  }

  // Daemon appears down, attempt restart with mutex-like protection
  if (daemonRestartCount < MAX_RESTARTS_PER_SESSION) {
    if (!restartPromise) {
      console.warn("[RTK] Daemon appears down, attempting restart...");
      daemonRestartCount++;

      restartPromise = (async () => {
        try {
          const restarted = await autoStartDaemon(RTK_BINARY, client);
          if (restarted) {
            console.log("[RTK] Daemon restarted successfully");
            lastHealthCheckTime = Date.now();
            lastHealthCheckResult = true;
            daemonRestartCount = 0;
            return true;
          }

          console.error("[RTK] Failed to restart daemon");
          lastHealthCheckTime = Date.now();
          lastHealthCheckResult = false;
          return false;
        } finally {
          restartPromise = null;
        }
      })();
    } else {
      console.log("[RTK] Daemon restart already in progress, waiting...");
    }

    return await restartPromise;
  }

  console.error(
    `[RTK] Max restart attempts (${MAX_RESTARTS_PER_SESSION}) reached`
  );
  lastHealthCheckTime = Date.now();
  lastHealthCheckResult = false;
  return false;
}

export { ensureDaemonRunning };

function resolvePreExecutionMode(): PreExecutionMode {
  const enabled = process.env.RTK_ENABLE_PRE_EXECUTION_FLAGS;
  if (!enabled) {
    return "off";
  }

  const normalized = enabled.trim().toLowerCase();
  return normalized === "1" || normalized === "true" || normalized === "yes"
    ? "rewrite"
    : "off";
}

function resolveExplicitPostExecutionCompressionMode(): PostExecutionCompressionMode | null {
  const rawMode = process.env.RTK_POST_EXECUTION_MODE;
  if (!rawMode) {
    return "replace_output"; // Default to replacing the output to actually save tokens!
  }

  const normalized = rawMode.trim().toLowerCase();
  if (normalized === "off") {
    return "off";
  }

  if (normalized === "metadata_only") {
    return "metadata_only";
  }

  if (normalized === "replace" || normalized === "replace_output") {
    return "replace_output";
  }

  console.warn(
    `[RTK] Invalid RTK_POST_EXECUTION_MODE='${rawMode}', defaulting to replace_output`
  );
  return "replace_output";
}

export const RTKPlugin: Plugin = async ({ directory, worktree: _worktree }) => {
  const client = new RTKDaemonClient(RTK_SOCKET_PATH);
  
  // Start periodic cleanup of expired pending commands
  startCleanupTimer();
  
  // Auto-start daemon if not running, with promise-based lock to prevent race conditions
  let isHealthy = await isDaemonRunning(client);
  
  if (!isHealthy) {
    if (startPromise) {
      console.log("[RTK] Waiting for existing daemon startup...");
      isHealthy = await startPromise;
    } else {
      startPromise = (async () => {
        try {
          return await autoStartDaemon(RTK_BINARY, client);
        } finally {
          startPromise = null;
        }
      })();
      
      isHealthy = await startPromise;
    }
  } else {
    console.log("[RTK] Daemon already running");
  }
  
  return {
    // Hook: Pre-tool execution
    "tool.execute.before": async (input, output) => {
      await onToolExecuteBefore(input, output, client, PRE_EXECUTION_MODE);
    },
    
    // Hook: Post-tool execution
    "tool.execute.after": async (input, output) => {
        await onToolExecuteAfter(
          input,
          output,
          client,
          directory,
          ACTIVE_MODEL_POLICY
        );
      },
    
    // Hook: Session complete
    event: async ({ event }) => {
      if (event.type === "session.idle") {
        await onSessionIdle(event, client);
      }
    },
  };
};

export default RTKPlugin;
