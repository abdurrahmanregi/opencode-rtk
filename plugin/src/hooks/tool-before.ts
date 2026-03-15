import type {
  PreExecutionMode,
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
  client: RTKDaemonClient,
  mode: PreExecutionMode = "off"
): Promise<void> {
  // Only process bash commands
  if (input.tool !== "bash") {
    return;
  }

  const commandArg = output.args?.command;
  if (typeof commandArg !== "string") {
    return;
  }

  const originalCommand = commandArg;

  // Skip empty commands
  if (!originalCommand.trim()) {
    return;
  }

  try {
    const optimized =
      mode === "rewrite"
        ? await client.optimizeCommand(originalCommand)
        : {
          original: originalCommand,
          optimized: originalCommand,
          flags_added: [] as string[],
          skipped: true,
          skip_reason: "pre-execution rewrite disabled",
        };

    if (mode === "rewrite" && !optimized.skipped && optimized.flags_added.length > 0) {
      output.args = output.args || {};
      output.args.command = optimized.optimized;
      console.log(
        `[RTK] Pre-execution: Added flags [${optimized.flags_added.join(
          ", "
        )}] to "${originalCommand}"`
      );
    }

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
