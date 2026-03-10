/**
 * Shared state for RTK plugin
 * 
 * This module provides shared state between hook files, ensuring
 * that context stored in tool-before is available in tool-after.
 */

/**
 * Context for a pending command awaiting output
 */
export interface PendingCommand {
  /** The command string that was executed */
  command: string;
  /** Working directory where command was executed */
  cwd: string;
  /** Timestamp when command started (for TTL cleanup) */
  timestamp: number;
}

/**
 * Shared map of pending commands, keyed by callID.
 * 
 * IMPORTANT: This MUST be a singleton shared between hook files.
 * Each file imports from this module to access the same Map instance.
 */
export const pendingCommands = new Map<string, PendingCommand>();

/**
 * Default TTL for pending commands in milliseconds (1 minute)
 */
const DEFAULT_TTL_MS = 60 * 1000;

/**
 * Clean up expired pending commands.
 * 
 * Call this periodically to prevent memory leaks from abandoned commands
 * (e.g., when tool execution fails or is cancelled).
 * 
 * @param ttlMs - Maximum age in milliseconds (default: 60000)
 * @returns Number of entries removed
 */
export function cleanupExpiredCommands(ttlMs: number = DEFAULT_TTL_MS): number {
  const now = Date.now();
  let removed = 0;
  
  for (const [callId, context] of pendingCommands.entries()) {
    if (now - context.timestamp > ttlMs) {
      pendingCommands.delete(callId);
      removed++;
    }
  }
  
  return removed;
}

/**
 * Start periodic cleanup of expired commands.
 * 
 * @param intervalMs - Cleanup interval in milliseconds (default: 30000)
 * @param ttlMs - Maximum age for commands (default: 60000)
 * @returns Timer handle for cleanup job
 */
export function startCleanupTimer(
  intervalMs: number = 30000,
  ttlMs: number = DEFAULT_TTL_MS
): NodeJS.Timeout {
  return setInterval(() => {
    const removed = cleanupExpiredCommands(ttlMs);
    if (removed > 0) {
      console.log(`RTK: Cleaned up ${removed} expired pending commands`);
    }
  }, intervalMs);
}
