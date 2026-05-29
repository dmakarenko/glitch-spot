// ABOUTME: Parity test loads tests/fixtures/hit-test-cases.json and asserts the
// ABOUTME: TypeScript hitTest matches the Rust window_picker::hit_test on every case.

import { describe, expect, it } from "vitest";
import fixture from "../../tests/fixtures/hit-test-cases.json";
import { hitTest, type PickableBounds } from "./hitTest";

type Case = {
  name: string;
  windows: { window_id: number; bounds: { x: number; y: number; w: number; h: number } }[];
  cursor: { x: number; y: number };
  expected: number | null;
};

describe("hitTest (JS twin)", () => {
  for (const c of (fixture as { cases: Case[] }).cases) {
    it(`case: ${c.name}`, () => {
      const ws: PickableBounds[] = c.windows.map((w) => ({ window_id: w.window_id, bounds: w.bounds }));
      expect(hitTest(ws, c.cursor.x, c.cursor.y)).toBe(c.expected);
    });
  }
});
