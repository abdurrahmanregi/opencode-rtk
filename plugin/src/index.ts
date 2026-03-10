import type { Plugin } from "@opencode-ai/plugin";
import { RTKDaemonClient } from "./client";
import { onToolExecuteBefore } from "./hooks/tool-before";
import { onToolExecuteAfter } from "./hooks/tool-after";
import { onSessionIdle } from "./hooks/session";
import { startCleanupTimer } from "./state";
import { isDaemonRunning, autoStartDaemon } from "./spawn";

import * as os from "os";

let startPromise: Promise<boolean> | null = null;
let isStarting = false;

const isWindows = os.platform() === "win32";
const RTK_SOCKET_PATH = isWindows ? "127.0.0.1:9876" : "/tmp/opencode-rtk.sock";
const RTK_BINARY = isWindows ? "opencode-rtk.exe" : "opencode-rtk";

export const RTKPlugin: Plugin = async ({ directory, worktree }) => {
  const client = new RTKDaemonClient(RTK_SOCKET_PATH);
  
  // Start periodic cleanup of expired pending commands
  startCleanupTimer();
  
  // Auto-start daemon if not running, with promise-based lock to prevent race conditions
  let isHealthy = await isDaemonRunning(client);
  
  if (!isHealthy) {
    if (startPromise || isStarting) {
      console.log("[RTK] Waiting for existing daemon startup...");
      if (startPromise) {
        isHealthy = await startPromise;
      }
    } else {
      isStarting = true;
      startPromise = (async () => {
        console.log(`[RTK] Daemon not running, starting '${RTK_BINARY}'...`);
        return await autoStartDaemon(RTK_BINARY, client);
      })();
      
      isHealthy = await startPromise;
      
      // Only reset flags after successful startup
      if (isHealthy) {
        startPromise = null;
        isStarting = false;
      }
    }
  } else {
    console.log("[RTK] Daemon already running");
  }
  
  return {
    // Hook: Pre-tool execution
    "tool.execute.before": async (input, output) => {
      await onToolExecuteBefore(input, output, client);
    },
    
    // Hook: Post-tool execution
    "tool.execute.after": async (input, output) => {
      await onToolExecuteAfter(input, output, client, directory);
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
