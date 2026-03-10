export interface CompressRequest {
  command: string;
  output: string;
  context?: {
    cwd?: string;
    exit_code?: number;
    tool?: string;
    session_id?: string;
  };
}

export interface CompressResponse {
  compressed: string;
  original_tokens: number;
  compressed_tokens: number;
  saved_tokens: number;
  savings_pct: number;
  strategy: string;
  module: string;
}

export interface StatsResponse {
  command_count: number;
  total_original_tokens: number;
  total_compressed_tokens: number;
  total_saved_tokens: number;
  savings_pct: number;
}

export interface HealthResponse {
  status: string;
  version: string;
}

/**
 * Hook type definitions (extracted from @opencode-ai/plugin Hooks interface)
 */

/**
 * Input for tool.execute.before hook
 */
export interface ToolExecuteBeforeInput {
  tool: string;
  sessionID: string;
  callID: string;
}

/**
 * Output for tool.execute.before hook
 */
export interface ToolExecuteBeforeOutput {
  args?: Record<string, unknown>;
}

/**
 * Input for tool.execute.after hook
 */
export interface ToolExecuteAfterInput {
  tool: string;
  sessionID: string;
  callID: string;
  args?: Record<string, unknown>;
}

/**
 * Output for tool.execute.after hook
 */
export interface ToolExecuteAfterOutput {
  title?: string;
  output?: string;
  metadata?: {
    exitCode?: number;
    [key: string]: unknown;
  };
}

/**
 * Session idle event from OpenCode
 */
export interface SessionIdleEvent {
  type: "session.idle";
  // OpenCode EventSessionIdle structure
  session_id?: string;
  [key: string]: unknown;
}
