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

describe("notes and deletion", () => {
  function drawn(id: string): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id, x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 50 });
    return reducer(s, { type: "END_DRAG" });
  }

  it("SET_NOTE updates note text and records the previous value in history", () => {
    const s = reducer(drawn("r1"), { type: "SET_NOTE", id: "r1", note: "hello" });
    expect(s.rects[0].note).toBe("hello");
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({ kind: "noteEdit", id: "r1", previousNote: "" });
  });

  it("repeated SET_NOTE on the same rect collapses history (no entry per keystroke)", () => {
    let s = reducer(drawn("r1"), { type: "SET_NOTE", id: "r1", note: "h" });
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "he" });
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "hel" });
    const edits = s.history.filter((h) => h.kind === "noteEdit" && h.id === "r1");
    expect(edits).toHaveLength(1);
    expect(edits[0]).toEqual({ kind: "noteEdit", id: "r1", previousNote: "" });
  });

  it("DELETE_RECT removes the rect and records it in history with its original index", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "BEGIN_DRAG", id: "r2", x: 200, y: 0 });
    s = reducer(s, { type: "END_DRAG" });
    s = reducer(s, { type: "DELETE_RECT", id: "r1" });
    expect(s.rects.map((r) => r.id)).toEqual(["r2"]);
    const last = s.history[s.history.length - 1];
    expect(last.kind).toBe("delete");
    if (last.kind === "delete") {
      expect(last.rect.id).toBe("r1");
      expect(last.index).toBe(0);
    }
  });

  it("CLEAR returns to initial state", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "hi" });
    s = reducer(s, { type: "CLEAR" });
    expect(s).toEqual(initialState);
  });
});
