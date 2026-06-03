// ABOUTME: Tests for RectangleNote vertical placement — below the rect by default,
// ABOUTME: flipped fully above (never overlapping the rect) when below would overflow.

// @vitest-environment jsdom

import { describe, expect, it, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { RectangleNote } from "./RectangleNote";
import type { Rect } from "./types";

afterEach(cleanup);

const NOTE_OFFSET = 6;

function renderNote(rect: Rect, containerHeight: number) {
  const { container } = render(
    <RectangleNote
      rect={rect}
      containerHeight={containerHeight}
      onChange={() => {}}
      autoFocus={false}
    />,
  );
  return container.querySelector("textarea") as HTMLTextAreaElement;
}

const px = (v: string) => parseFloat(v);

describe("RectangleNote placement", () => {
  it("places the note below the rect when there is room", () => {
    const rect: Rect = { id: "r1", x: 10, y: 50, w: 100, h: 30, note: "" };
    const ta = renderNote(rect, 800);
    expect(ta.style.top).toBe(`${rect.y + rect.h + NOTE_OFFSET}px`); // 86px
  });

  it("flips the note fully above the rect when below would overflow, without overlapping it", () => {
    // Rect near the bottom of a 300px-tall container: placing below overflows,
    // so the note flips above. Its bottom edge must sit at or above the rect top.
    const rect: Rect = { id: "r1", x: 10, y: 250, w: 100, h: 30, note: "" };
    const ta = renderNote(rect, 300);

    const top = px(ta.style.top);
    const height = px(ta.style.height);
    expect(height).toBeGreaterThan(0); // a fixed height is set (not just minHeight)
    // The note is entirely above the rect — its bottom is NOTE_OFFSET above rect.y.
    expect(top + height).toBeLessThanOrEqual(rect.y);
    expect(top + height).toBe(rect.y - NOTE_OFFSET);
  });
});
