import { describe, expect, test } from "bun:test";
import {
  getSensitiveSkipReason,
  onToolExecuteAfter,
} from "../hooks/tool-after";
import { pendingCommands } from "../state";
import type { ToolExecuteAfterInput, ToolExecuteAfterOutput } from "../types";

describe("getSensitiveSkipReason", () => {
  test("detects template markers", () => {
    expect(getSensitiveSkipReason("hello {{name}}"))
      .toBe("template_markers_detected");
  });

  test("detects markdown code fences", () => {
    expect(getSensitiveSkipReason("```bash\ngit status\n```"))
      .toBe("markdown_code_fence_detected");
  });

  test("detects details blocks", () => {
    expect(getSensitiveSkipReason("<details><summary>More</summary></details>"))
      .toBe("html_details_block_detected");
  });

  test("returns null for plain output", () => {
    expect(getSensitiveSkipReason("plain output line"))
      .toBeNull();
  });

  test("detects markdown tables at start of output", () => {
    expect(getSensitiveSkipReason("|a|b|\n|-|-|\n|1|2|"))
      .toBe("markdown_table_detected");
  });
});

describe("onToolExecuteAfter", () => {
  test("marks compression skipped when mode is off", async () => {
    const input: ToolExecuteAfterInput = {
      tool: "bash",
      sessionID: "s1",
      callID: "after-off",
    };
    const output: ToolExecuteAfterOutput = {
      output: "normal output",
      metadata: {},
    };

    pendingCommands.set(input.callID, {
      originalCommand: "git status",
      optimizedCommand: "git status",
      flagsAdded: [],
      cwd: process.cwd(),
      timestamp: Date.now(),
    });

    const mockClient = {};
    await onToolExecuteAfter(input, output, mockClient as never, process.cwd(), "off");

    expect(output.metadata?.rtk_mode).toBe("off");
    expect(output.metadata?.rtk_compression_skipped).toBe(true);
    expect(output.metadata?.rtk_skip_reason).toBe(
      "post_execution_compression_disabled"
    );
  });

  test("skips sensitive output before compression", async () => {
    const input: ToolExecuteAfterInput = {
      tool: "bash",
      sessionID: "s1",
      callID: "after-sensitive",
    };
    const output: ToolExecuteAfterOutput = {
      output: "{{sensitive-template}}",
      metadata: {},
    };

    pendingCommands.set(input.callID, {
      originalCommand: "docker ps",
      optimizedCommand: "docker ps",
      flagsAdded: [],
      cwd: process.cwd(),
      timestamp: Date.now(),
    });

    const mockClient = {};
    await onToolExecuteAfter(
      input,
      output,
      mockClient as never,
      process.cwd(),
      "metadata_only"
    );

    expect(output.metadata?.rtk_mode).toBe("metadata_only");
    expect(output.metadata?.rtk_compression_skipped).toBe(true);
    expect(output.metadata?.rtk_skip_reason).toBe("template_markers_detected");
  });
});
