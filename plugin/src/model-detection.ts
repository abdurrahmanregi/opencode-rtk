import type {
  CompressionAggressiveness,
  ModelCategory,
  ModelRuntimePolicy,
  PostExecutionCompressionMode,
} from "./types";

const MODEL_ENV_CHAIN = [
  "RTK_ACTIVE_MODEL",
  "OPENROUTER_MODEL",
  "OPENCODE_LLM_MODEL",
  "OPENAI_MODEL",
  "ANTHROPIC_MODEL",
];

const REASONING_PATTERNS = [
  /openai\/gpt-oss/i,
  /reason/i,
  /think/i,
  /(^|[-_/])r1([-.]|$)/i,
];

const COMPACT_PATTERNS = [
  /(^|[-_/])(mini|small|tiny|nano)([-_/]|$)/i,
  /(^|[-_/])8b([-.]|$)/i,
];

export function resolveModelIdFromEnv(env: NodeJS.ProcessEnv = process.env): string {
  for (const key of MODEL_ENV_CHAIN) {
    const value = env[key];
    if (!value) {
      continue;
    }

    const trimmed = value.trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
  }

  return "unknown";
}

export function detectModelCategory(modelId: string): ModelCategory {
  const normalized = modelId.trim().toLowerCase();
  if (!normalized || normalized === "unknown") {
    return "instruct";
  }

  if (REASONING_PATTERNS.some((pattern) => pattern.test(normalized))) {
    return "reasoning";
  }

  if (COMPACT_PATTERNS.some((pattern) => pattern.test(normalized))) {
    return "compact";
  }

  return "instruct";
}

function parseCategory(value: string | undefined): ModelCategory | null {
  if (!value) {
    return null;
  }

  const normalized = value.trim().toLowerCase();
  if (
    normalized === "reasoning" ||
    normalized === "instruct" ||
    normalized === "compact"
  ) {
    return normalized;
  }

  return null;
}

function parseCompressionAggressiveness(
  value: string | undefined,
): CompressionAggressiveness | null {
  if (!value) {
    return null;
  }

  const normalized = value.trim().toLowerCase();
  if (normalized === "low" || normalized === "medium" || normalized === "high") {
    return normalized;
  }

  return null;
}

function parsePostExecutionMode(
  value: string | undefined,
): PostExecutionCompressionMode | null {
  if (!value) {
    return null;
  }

  const normalized = value.trim().toLowerCase();
  if (normalized === "off") {
    return "off";
  }
  if (normalized === "metadata_only") {
    return "metadata_only";
  }
  if (normalized === "replace" || normalized === "replace_output") {
    return "replace_output";
  }
  return null;
}

function parseBoolean(value: string | undefined): boolean | null {
  if (!value) {
    return null;
  }

  const normalized = value.trim().toLowerCase();
  if (normalized === "1" || normalized === "true" || normalized === "yes") {
    return true;
  }
  if (normalized === "0" || normalized === "false" || normalized === "no") {
    return false;
  }
  return null;
}

function defaultModeForCategory(_category: ModelCategory): PostExecutionCompressionMode {
  return "metadata_only";
}

function defaultAggressivenessForCategory(
  category: ModelCategory,
): CompressionAggressiveness {
  if (category === "reasoning") {
    return "low";
  }
  if (category === "compact") {
    return "medium";
  }
  return "high";
}

export function resolveModelRuntimePolicy(
  explicitPostExecutionMode: PostExecutionCompressionMode | null,
  env: NodeJS.ProcessEnv = process.env,
): ModelRuntimePolicy {
  const modelId = resolveModelIdFromEnv(env);
  const detectedCategory = detectModelCategory(modelId);

  const modelCategory = parseCategory(env.RTK_MODEL_CATEGORY) ?? detectedCategory;

  const postExecutionMode =
    explicitPostExecutionMode ??
    parsePostExecutionMode(env.RTK_MODEL_POLICY_MODE) ??
    defaultModeForCategory(modelCategory);

  const compressionAggressiveness =
    parseCompressionAggressiveness(env.RTK_COMPRESSION_AGGRESSIVENESS) ??
    defaultAggressivenessForCategory(modelCategory);

  const stripReasoning = parseBoolean(env.RTK_STRIP_REASONING) ?? true;

  return {
    modelId,
    modelCategory,
    postExecutionMode,
    compressionAggressiveness,
    stripReasoning,
  };
}
