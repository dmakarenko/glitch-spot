// ABOUTME: Tests for the inline-SVG picker cursor data URL and CSS value.
// ABOUTME: Verifies dimensions, accent colours, chromatic rings, and CSS hotspot.

import { describe, expect, it } from "vitest";
import { PICKER_CURSOR_CSS, PICKER_CURSOR_SVG } from "./cursor";

describe("picker cursor", () => {
  it("SVG has a 48×48 viewBox", () => {
    expect(PICKER_CURSOR_SVG).toContain('viewBox="0 0 48 48"');
  });

  it("includes accent orange #ee6633 strokes", () => {
    expect(PICKER_CURSOR_SVG.toLowerCase()).toContain("#ee6633");
  });

  it("includes the chromatic red and cyan offset rings", () => {
    expect(PICKER_CURSOR_SVG).toContain("rgba(255,0,60,");
    expect(PICKER_CURSOR_SVG).toContain("rgba(0,245,255,");
  });

  it("CSS uses a data URL with hotspot 16 16 and a crosshair fallback", () => {
    expect(PICKER_CURSOR_CSS).toMatch(/^url\("data:image\/svg\+xml/);
    expect(PICKER_CURSOR_CSS).toContain("16 16, crosshair");
  });
});
