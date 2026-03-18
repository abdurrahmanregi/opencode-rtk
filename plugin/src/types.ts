export interface CompressRequest {
  command: string;
  output: string;
  context?: {
    cwd?: string;
    exit_code?: number;
    tool?: string;
    session_id?: string;
    model_id?: string;
    model_category?: ModelCategory;
    policy_mode?: PostExecutionCompressionMode;
    compression_aggressiveness?: CompressionAggressiveness;
    strip_reasoning?: boolean;
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
  replace_recommended?: boolean;
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
 * Request for the optimize method
 */
export interface OptimizeRequest {
  command: string;
}

/**
 * Response from the optimize method
 */
export interface OptimizeResponse {
  /** Original command string */
  original: string;
  /** Optimized command string with flags */
  optimized: string;
  /** Flags that were added */
  flags_added: string[];
  /** Whether optimization was skipped */
  skipped: boolean;
  /** Reason for skipping (if applicable) */
  skip_reason?: string;
}

export type PreExecutionMode = "off" | "rewrite";

export type PostExecutionCompressionMode =
  | "off"
  | "metadata_only"
  | "replace_output";

export type ModelCategory = "reasoning" | "instruct" | "compact";

export type CompressionAggressiveness = "low" | "medium" | "high";

export interface ModelRuntimePolicy {
  modelId: string;
  modelCategory: ModelCategory;
  postExecutionMode: PostExecutionCompressionMode;
  compressionAggressiveness: CompressionAggressiveness;
  stripReasoning: boolean;
}

/**
 * Request for tee_save method
 */
export interface TeeSaveRequest {
  command: string;
  output: string;
}

/**
 * Response from tee_save method
 */
export interface TeeSaveResponse {
  path: string;
  size: number;
}

/**
 * Response from tee_list method
 */
export interface TeeListResponse {
  files: TeeFileInfo[];
  total: number;
}

/**
 * Information about a tee file
 */
export interface TeeFileInfo {
  path: string;
  command: string;
  timestamp: string;
  size: number;
}

/**
 * Request for tee_read method
 */
export interface TeeReadRequest {
  path: string;
}

/**
 * Response from tee_read method
 */
export interface TeeReadResponse {
  content: string;
  size: number;
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
