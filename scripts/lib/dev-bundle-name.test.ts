// ABOUTME: Unit tests for the debug-bundle productName builder (base + short SHA).
// ABOUTME: Runs under the project's Vitest config (scripts/**/*.test.ts).
import { describe, it, expect } from "vitest";
import { devProductName, devIdentifier } from "./dev-bundle-name.mjs";

describe("devProductName", () => {
  it("appends the short SHA in parentheses", () => {
    expect(devProductName("Glitch Spot", "abc1234")).toBe("Glitch Spot (abc1234)");
  });
  it("marks a dirty working tree", () => {
    expect(devProductName("Glitch Spot", "abc1234", true)).toBe(
      "Glitch Spot (abc1234-dirty)",
    );
  });
  it("requires a base name and a SHA", () => {
    expect(() => devProductName("", "abc1234")).toThrow();
    expect(() => devProductName("Glitch Spot", "")).toThrow();
  });
});

describe("devIdentifier", () => {
  it("appends a .debug suffix so dev builds get their own TCC entry", () => {
    expect(devIdentifier("com.glitchspot.app")).toBe("com.glitchspot.app.debug");
  });
  it("requires a base identifier", () => {
    expect(() => devIdentifier("")).toThrow();
  });
});
