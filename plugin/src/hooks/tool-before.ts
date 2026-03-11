import type {
  ToolExecuteBeforeInput,
  ToolExecuteBeforeOutput,
} from "../types";
import { RTKDaemonClient } from "../client";
import { pendingCommands } from "../state";

/**
 * Pre-execution hook for tool.execute
 *
 * This hook:
 * 1. Detects bash commands
 * 2. Calls daemon to optimize command with flags
 * 3. Modifies the command to be executed
 * 4. Stores context for post-execution hook
 */
export async function onToolExecuteBefore(
  input: ToolExecuteBeforeInput,
  output: ToolExecuteBeforeOutput,
  client: RTKDaemonClient
): Promise<void> {
  // Only process bash commands
  if (input.tool !== "bash") {
    return;
  }

  // Extract command from args
  const originalCommand = (output.args?.command as string) || "";

  // Skip empty commands
  if (!originalCommand.trim()) {
    return;
  }

  try {
    // Call daemon to optimize command
    const optimized = await client.optimizeCommand(originalCommand);

    // Modify command if optimization was applied
    if (!optimized.skipped && optimized.flags_added.length > 0) {
      output.args = output.args || {};
      output.args.command = optimized.optimized;

      // Log optimization for debugging
      console.log(
        `[RTK] Pre-execution: Added flags [${optimized.flags_added.join(
          ", "
        )}] to "${originalCommand}"`
      );
    }

    // Store context for post-execution hook
    pendingCommands.set(input.callID, {
      originalCommand,
      optimizedCommand: optimized.optimized,
      flagsAdded: optimized.flags_added,
      cwd: process.cwd(),
      timestamp: Date.now(),
    });
  } catch (error) {
    // On error, store original command and continue without optimization
    console.error("[RTK] Pre-execution optimization failed:", error);

    pendingCommands.set(input.callID, {
      originalCommand,
      optimizedCommand: originalCommand,
      flagsAdded: [],
      cwd: process.cwd(),
      timestamp: Date.now(),
    });
  }
}
