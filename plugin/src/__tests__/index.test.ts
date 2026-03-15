import { describe, expect, test } from "bun:test";
import { shouldUseCachedHealth } from "../index";

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
