import type { ToolExecuteAfterInput, ToolExecuteAfterOutput } from "../types";
import { RTKDaemonClient } from "../client";
import type { CompressRequest } from "../types";
import { pendingCommands } from "../state";

export async function onToolExecuteAfter(
  input: ToolExecuteAfterInput,
  output: ToolExecuteAfterOutput,
  client: RTKDaemonClient,
  cwd: string
): Promise<void> {
  if (input.tool !== "bash") {
    return;
  }
  
  // Retrieve context stored by tool-before hook
  // This uses the SAME Map instance as tool-before via shared state module
  const context = pendingCommands.get(input.callID);
  
  if (!context) {
    return;
  }
  
  // Clean up after retrieving (one-shot use)
  pendingCommands.delete(input.callID);
  
  const command = context.command;
  const rawOutput = output.output || "";
  
  // Skip if output is too large (>1MB)
  if (rawOutput.length > 1000000) {
    console.warn(`RTK: Skipping compression (output too large: ${rawOutput.length} bytes)`);
    return;
  }
  
  // Check if daemon is healthy
  const isHealthy = await client.health();
  
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
      
      // Log savings
      console.log(
        `📊 RTK: Saved ${response.saved_tokens} tokens (${response.savings_pct.toFixed(1)}%) using ${response.strategy}`
      );
    }
  } catch (error) {
    console.error("RTK: Compression failed:", error);
    // Fall back to original output (already in place)
  }
}
