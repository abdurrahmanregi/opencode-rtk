import type { Plugin } from "@opencode-ai/plugin";
import { RTKDaemonClient } from "./client";
import { onToolExecuteBefore } from "./hooks/tool-before";
import { onToolExecuteAfter } from "./hooks/tool-after";
import { onSessionIdle } from "./hooks/session";
import { startCleanupTimer } from "./state";

import * as os from "os";

const isWindows = os.platform() === "win32";
const RTK_SOCKET_PATH = isWindows ? "127.0.0.1:9876" : "/tmp/opencode-rtk.sock";
const RTK_BINARY = "opencode-rtk";

export const RTKPlugin: Plugin = async ({ directory, worktree }) => {
  const client = new RTKDaemonClient(RTK_SOCKET_PATH);
  
  // Start periodic cleanup of expired pending commands
  startCleanupTimer();
  
  // Check if daemon is running
  const isHealthy = await client.health();
  
  if (!isHealthy) {
    console.warn("RTK daemon is not running. Token optimization disabled.");
    console.warn(`Start daemon with: ${RTK_BINARY}`);
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
