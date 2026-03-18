import type {
  ModelRuntimePolicy,
  PostExecutionCompressionMode,
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
 * 3. Replaces output when policy allows and daemon recommends
 * 4. On failure, saves to tee file if enabled
 */
export async function onToolExecuteAfter(
  input: ToolExecuteAfterInput,
  output: ToolExecuteAfterOutput,
  client: RTKDaemonClient,
  cwd: string,
  modelPolicy: ModelRuntimePolicy,
): Promise<void> {
  if (input.tool !== "bash") {
    return;
  }

  const mode: PostExecutionCompressionMode = modelPolicy.postExecutionMode;

  // Retrieve context stored by tool-before hook and delete atomically
  // This uses the SAME Map instance as tool-before via shared state module
  // Atomic get-and-delete to prevent race conditions with concurrent executions
  const context = pendingCommands.get(input.callID);
  pendingCommands.delete(input.callID);

  if (!context) {
    return;
  }

  const command = context.optimizedCommand;
  const rawOutput = typeof output.output === "string" ? output.output : "";

  output.metadata = output.metadata || {};

  if (mode === "off") {
    output.metadata.rtk_mode = mode;
    output.metadata.rtk_compression_skipped = true;
    output.metadata.rtk_skip_reason = "post_execution_compression_disabled";
    return;
  }

  const sensitiveReason = getSensitiveSkipReason(rawOutput);
  if (sensitiveReason) {
    output.metadata.rtk_mode = mode;
    output.metadata.rtk_compression_skipped = true;
    output.metadata.rtk_skip_reason = sensitiveReason;
    attachPreExecutionMetadata(output, context);
    return;
  }

  // Skip if output is too large (>1MB)
  if (rawOutput.length > 1000000) {
    console.warn(
      `RTK: Skipping compression (output too large: ${rawOutput.length} bytes)`
    );
    output.metadata.rtk_mode = mode;
    output.metadata.rtk_compression_skipped = true;
    output.metadata.rtk_skip_reason = "output_too_large";
    attachPreExecutionMetadata(output, context);
    return;
  }

  // Ensure daemon is running with auto-restart capability
  const isHealthy = await ensureDaemonRunning(client);

  if (!isHealthy) {
    console.warn("RTK: Daemon not available, skipping compression");
    output.metadata.rtk_mode = mode;
    output.metadata.rtk_compression_skipped = true;
    output.metadata.rtk_skip_reason = "daemon_unavailable";
    attachPreExecutionMetadata(output, context);
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
        model_id: modelPolicy.modelId,
        model_category: modelPolicy.modelCategory,
        policy_mode: modelPolicy.postExecutionMode,
        compression_aggressiveness: modelPolicy.compressionAggressiveness,
        strip_reasoning: modelPolicy.stripReasoning,
      },
    };

    const response = await client.compress(request);

    output.metadata.rtk_mode = mode;
    output.metadata.rtk_strategy = response.strategy;
    output.metadata.rtk_module = response.module;
    output.metadata.rtk_saved_tokens = response.saved_tokens;
    output.metadata.rtk_savings_pct = response.savings_pct;

    const replaceRecommended = response.replace_recommended ?? true;
    if (mode === "replace_output" && response.saved_tokens > 0 && replaceRecommended) {
      output.output = response.compressed;
      output.metadata.rtk_compressed = true;
      output.metadata.rtk_output_replaced = true;
      console.log(
        `📊 RTK: Saved ${response.saved_tokens} tokens (${response.savings_pct.toFixed(
          1
        )}%) using ${response.strategy}`
      );
    } else {
      output.metadata.rtk_compressed = false;
      output.metadata.rtk_output_replaced = false;
    }

    attachPreExecutionMetadata(output, context);
  } catch (error) {
    console.error("RTK: Compression failed:", error);
    output.metadata.rtk_compression_failed = true;
    output.metadata.rtk_mode = mode;
    output.metadata.rtk_skip_reason = "compression_error";
    attachPreExecutionMetadata(output, context);

    // Save to tee file on failure (if enabled)
    try {
      const teeResult = await client.saveTee(
        context.originalCommand,
        rawOutput
      );
      console.error(
        `[RTK] Compression failed, saved original output to: ${teeResult.path}`
      );

      output.metadata.rtk_tee_path = teeResult.path;
      output.metadata.rtk_compression_failed = true;
    } catch (teeError) {
      console.error("[RTK] Failed to save tee file:", teeError);
    }
  }
}

export function getSensitiveSkipReason(rawOutput: string): string | null {
  if (rawOutput.includes("{{") || rawOutput.includes("}}")) {
    return "template_markers_detected";
  }

  if (rawOutput.includes("```")) {
    return "markdown_code_fence_detected";
  }

  if (/<details[\s>]/i.test(rawOutput) || /<summary[\s>]/i.test(rawOutput)) {
    return "html_details_block_detected";
  }

  if (/(^|\n)\s*\|.+\|\s*\n\s*\|[-:| ]+\|/m.test(rawOutput)) {
    return "markdown_table_detected";
  }

  return null;
}

function attachPreExecutionMetadata(
  output: ToolExecuteAfterOutput,
  context: {
    originalCommand: string;
    flagsAdded: string[];
  }
): void {
  if (!output.metadata) {
    output.metadata = {};
  }

  if (context.flagsAdded.length > 0) {
    output.metadata.rtk_pre_execution_flags = context.flagsAdded;
    output.metadata.rtk_original_command = context.originalCommand;
  }
}
