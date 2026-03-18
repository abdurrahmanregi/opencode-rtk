import { describe, expect, test } from "bun:test";
import { shouldUseCachedHealth } from "../index";
import {
  detectModelCategory,
  resolveModelIdFromEnv,
  resolveModelRuntimePolicy,
} from "../model-detection";

describe("shouldUseCachedHealth", () => {
  test("uses full throttle window for recent healthy result", () => {
    expect(shouldUseCachedHealth(1000, true)).toBe(true);
    expect(shouldUseCachedHealth(1499, true)).toBe(true);
    expect(shouldUseCachedHealth(1500, true)).toBe(false);
  });

  test("rechecks sooner for failed result", () => {
    expect(shouldUseCachedHealth(100, false)).toBe(true);
    expect(shouldUseCachedHealth(999, false)).toBe(true);
    expect(shouldUseCachedHealth(1000, false)).toBe(false);
  });

  test("force check bypasses cache", () => {
    expect(shouldUseCachedHealth(100, true, true)).toBe(false);
    expect(shouldUseCachedHealth(100, false, true)).toBe(false);
  });
});

describe("model detection", () => {
  test("detects reasoning and instruct categories", () => {
    expect(detectModelCategory("openai/gpt-oss-safeguard-20b")).toBe("reasoning");
    expect(detectModelCategory("meta-llama/llama-3.1-8b-instruct")).toBe("compact");
    expect(detectModelCategory("openai/gpt-4o")).toBe("instruct");
  });

  test("resolves model id from env priority chain", () => {
    const env = {
      RTK_ACTIVE_MODEL: "",
      OPENROUTER_MODEL: "openai/gpt-oss-safeguard-20b",
      OPENAI_MODEL: "gpt-4o",
    } as NodeJS.ProcessEnv;

    expect(resolveModelIdFromEnv(env)).toBe("openai/gpt-oss-safeguard-20b");
  });

  test("builds runtime policy with explicit mode override", () => {
    const env = {
      OPENROUTER_MODEL: "openai/gpt-oss-safeguard-20b",
      RTK_COMPRESSION_AGGRESSIVENESS: "low",
      RTK_STRIP_REASONING: "true",
    } as NodeJS.ProcessEnv;

    const policy = resolveModelRuntimePolicy("replace_output", env);
    expect(policy.modelCategory).toBe("reasoning");
    expect(policy.postExecutionMode).toBe("replace_output");
    expect(policy.compressionAggressiveness).toBe("low");
    expect(policy.stripReasoning).toBe(true);
  });
});
