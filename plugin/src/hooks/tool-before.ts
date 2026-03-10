import type { ToolExecuteBeforeInput, ToolExecuteBeforeOutput } from "../types";
import { RTKDaemonClient } from "../client";
import { pendingCommands } from "../state";

export async function onToolExecuteBefore(
  input: ToolExecuteBeforeInput,
  output: ToolExecuteBeforeOutput,
  client: RTKDaemonClient
): Promise<void> {
  if (input.tool !== "bash") {
    return;
  }
  
  // Extract command from args (typed as unknown, needs casting)
  const command = (output.args?.command as string) || "";
  
  // Store context for later compression in tool-after hook
  // This Map is shared between hooks via the state module
  pendingCommands.set(input.callID, {
    command,
    cwd: process.cwd(),
    timestamp: Date.now(),
  });
  
  // Could also check if command is supported before storing
  // const isSupported = await checkIfSupported(command, client);
  // if (!isSupported) {
  //   pendingCommands.delete(input.callID);
  // }
}
