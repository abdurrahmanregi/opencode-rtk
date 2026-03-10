import { RTKDaemonClient } from "../client";
import type { SessionIdleEvent } from "../types";

export async function onSessionIdle(
  event: SessionIdleEvent,
  client: RTKDaemonClient
): Promise<void> {
  const sessionId = event.session_id;
  
  if (!sessionId) {
    return;
  }
  
  try {
    const isHealthy = await client.health();
    
    if (!isHealthy) {
      return;
    }
    
    const stats = await client.stats(sessionId);
    
    if (stats.total_saved_tokens > 0) {
      console.log(
        `📊 RTK Session Summary: Saved ${stats.total_saved_tokens} tokens (${stats.savings_pct.toFixed(1)}% reduction) across ${stats.command_count} commands`
      );
    }
  } catch (error) {
    console.error("RTK: Failed to get session stats:", error);
  }
}
