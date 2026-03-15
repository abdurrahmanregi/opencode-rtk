import type {
  ToolExecuteAfterInput,
  ToolExecuteAfterOutput,
} from "../types";
import { RTKDaemonClient } from "../client";
import type { CompressRequest } from "../types";
import { pendingCommands } from "../state";
import { ensureDaemonRunning } from "../index";

/**
 * Post-execution hook for tool.execute
 *
 * This hook:
 * 1. Retrieves context from pre-execution hook
 * 2. Calls daemon to compress output
 * 3. Replaces output if savings > 0
 * 4. On failure, saves to tee file if enabled
 */
export async function onToolExecuteAfter(
  input: ToolExecuteAfterInput,
  output: ToolExecuteAfterOutput,
  client: RTKDaemonClient,
  cwd: string
): Promise<void> {
  if (input.tool !== "bash") {
    return;
  }

  // Retrieve context stored by tool-before hook and delete atomically
  // This uses the SAME Map instance as tool-before via shared state module
  // Atomic get-and-delete to prevent race conditions with concurrent executions
  const context = pendingCommands.get(input.callID);
  pendingCommands.delete(input.callID);

  if (!context) {
    return;
  }

  const command = context.optimizedCommand;
  const rawOutput = output.output || "";

  // Skip if output is too large (>1MB)
  if (rawOutput.length > 1000000) {
    console.warn(
      `RTK: Skipping compression (output too large: ${rawOutput.length} bytes)`
    );
    return;
  }

  // Ensure daemon is running with auto-restart capability
  const isHealthy = await ensureDaemonRunning(client);

  if (!isHealthy) {
    console.warn("RTK: Daemon not available, skipping compression");
    return;
  }

  try {
    const request: CompressRequest = {
      command,
      output: rawOutput,
      context: {
        cwd,
        exit_code: output.metadata?.exitCode || 0,
        tool: input.tool,
        session_id: input.sessionID,
      },
    };

    const response = await client.compress(request);

    // Only replace if we actually saved tokens
    if (response.saved_tokens > 0) {
      output.output = response.compressed;

      // Add metadata
      output.metadata = output.metadata || {};
      output.metadata.rtk_compressed = true;
      output.metadata.rtk_strategy = response.strategy;
      output.metadata.rtk_module = response.module;
      output.metadata.rtk_saved_tokens = response.saved_tokens;
      output.metadata.rtk_savings_pct = response.savings_pct;

      // Add pre-execution metadata if flags were added
      if (context.flagsAdded && context.flagsAdded.length > 0) {
        output.metadata.rtk_pre_execution_flags = context.flagsAdded;
        output.metadata.rtk_original_command = context.originalCommand;
      }

      // Log savings
      console.log(
        `📊 RTK: Saved ${response.saved_tokens} tokens (${response.savings_pct.toFixed(
          1
        )}%) using ${response.strategy}`
      );
    }
  } catch (error) {
    console.error("RTK: Compression failed:", error);

    // Save to tee file on failure (if enabled)
    try {
      const teeResult = await client.saveTee(
        context.originalCommand,
        rawOutput
      );
      console.error(
        `[RTK] Compression failed, saved original output to: ${teeResult.path}`
      );

      // Add tee path to metadata
      output.metadata = output.metadata || {};
      output.metadata.rtk_tee_path = teeResult.path;
      output.metadata.rtk_compression_failed = true;
    } catch (teeError) {
      console.error("[RTK] Failed to save tee file:", teeError);
    }
  }
}
