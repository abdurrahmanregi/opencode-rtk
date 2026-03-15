import { describe, expect, test } from "bun:test";
import { onToolExecuteBefore } from "../hooks/tool-before";
import { pendingCommands } from "../state";
import type { ToolExecuteBeforeInput, ToolExecuteBeforeOutput } from "../types";

describe("onToolExecuteBefore", () => {
  test("does not rewrite command in off mode", async () => {
    const input: ToolExecuteBeforeInput = {
      tool: "bash",
      sessionID: "s1",
      callID: "call-off",
    };
    const output: ToolExecuteBeforeOutput = {
      args: { command: "git status" },
    };

    const mockClient = {
      optimizeCommand: () =>
        Promise.reject(new Error("should not be called in off mode")),
    };

    await onToolExecuteBefore(input, output, mockClient as never, "off");

    expect(output.args?.command).toBe("git status");
    const context = pendingCommands.get("call-off");
    expect(context?.optimizedCommand).toBe("git status");
    expect(context?.flagsAdded).toEqual([]);
    pendingCommands.delete("call-off");
  });

  test("rewrites command in rewrite mode", async () => {
    const input: ToolExecuteBeforeInput = {
      tool: "bash",
      sessionID: "s1",
      callID: "call-rewrite",
    };
    const output: ToolExecuteBeforeOutput = {
      args: { command: "git status" },
    };

    const mockClient = {
      optimizeCommand: () =>
        Promise.resolve({
          original: "git status",
          optimized: "git status --porcelain -b",
          flags_added: ["--porcelain", "-b"],
          skipped: false,
        }),
    };

    await onToolExecuteBefore(input, output, mockClient as never, "rewrite");

    expect(output.args?.command).toBe("git status --porcelain -b");
    const context = pendingCommands.get("call-rewrite");
    expect(context?.optimizedCommand).toBe("git status --porcelain -b");
    expect(context?.flagsAdded).toEqual(["--porcelain", "-b"]);
    pendingCommands.delete("call-rewrite");
  });

  test("ignores non-string command values", async () => {
    const input: ToolExecuteBeforeInput = {
      tool: "bash",
      sessionID: "s1",
      callID: "call-bad-command",
    };
    const output: ToolExecuteBeforeOutput = {
      args: { command: 123 },
    };

    const mockClient = {
      optimizeCommand: () =>
        Promise.reject(new Error("should not be called for non-string command")),
    };

    await onToolExecuteBefore(input, output, mockClient as never, "rewrite");

    expect(pendingCommands.has("call-bad-command")).toBe(false);
  });
});
