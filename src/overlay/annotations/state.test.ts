// ABOUTME: Reducer tests for the overlay's annotation state. Pure data, no DOM.
import { describe, expect, it } from "vitest";
import { reducer, initialState } from "./state";

describe("annotations reducer", () => {
  it("starts with no rects, no history, no drag", () => {
    expect(initialState).toEqual({ rects: [], history: [], draggingId: null });
  });

  it("BEGIN_DRAG creates a zero-size rect anchored at the start point", () => {
    const s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    expect(s.rects).toHaveLength(1);
    expect(s.rects[0]).toMatchObject({ id: "r1", x: 100, y: 50, w: 0, h: 0, note: "" });
    expect(s.draggingId).toBe("r1");
    expect(s.history).toEqual([{ kind: "create", id: "r1" }]);
  });

  it("DRAG_TO updates the dragging rect's bottom-right while drag is active", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "DRAG_TO", x: 200, y: 100 });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("DRAG_TO handles drags up and to the left by flipping origin", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 200, y: 100 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 50 });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("END_DRAG clears draggingId without removing the rect", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "DRAG_TO", x: 200, y: 100 });
    s = reducer(s, { type: "END_DRAG" });
    expect(s.draggingId).toBeNull();
    expect(s.rects).toHaveLength(1);
  });

  it("DRAG_TO is a no-op when there is no active drag", () => {
    const s = reducer(initialState, { type: "DRAG_TO", x: 10, y: 10 });
    expect(s).toEqual(initialState);
  });
});
