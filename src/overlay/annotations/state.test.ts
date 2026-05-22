// ABOUTME: Reducer tests for the overlay's annotation state. Pure data, no DOM.
import { describe, expect, it } from "vitest";
import { reducer, initialState, MIN_RECT_DIMENSION } from "./state";

describe("annotations reducer", () => {
  it("starts with no rects, no history, no drag", () => {
    expect(initialState).toEqual({
      rects: [],
      history: [],
      draggingId: null,
      lastCreatedId: null,
    });
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

  it("BEGIN_DRAG sets lastCreatedId so the new rect's note can auto-focus", () => {
    const s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 0, y: 0 });
    expect(s.lastCreatedId).toBe("r1");
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
    s = reducer(s, { type: "DRAG_TO", x: 300, y: 80 });
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

describe("undo", () => {
  function drawn(id: string, state = initialState): import("./types").AnnotationsState {
    let s = reducer(state, { type: "BEGIN_DRAG", id, x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 50 });
    return reducer(s, { type: "END_DRAG" });
  }

  it("UNDO on empty history is a no-op", () => {
    const s = reducer(initialState, { type: "UNDO" });
    expect(s).toEqual(initialState);
  });

  it("UNDO of a create removes the rect", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "UNDO" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
  });

  it("UNDO of a delete restores the rect at its original index", () => {
    let s = drawn("r1");
    s = drawn("r2", s);
    s = reducer(s, { type: "DELETE_RECT", id: "r1" });
    expect(s.rects.map((r) => r.id)).toEqual(["r2"]);
    s = reducer(s, { type: "UNDO" });
    expect(s.rects.map((r) => r.id)).toEqual(["r1", "r2"]);
  });

  it("UNDO of a note edit restores the prior note text", () => {
    let s = drawn("r1");
    s = reducer(s, { type: "SET_NOTE", id: "r1", note: "later" });
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0].note).toBe("");
  });

  it("UNDO of a create clears lastCreatedId when undoing the most recent create", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 50 });
    s = reducer(s, { type: "END_DRAG" });
    s = reducer(s, { type: "UNDO" });
    expect(s.lastCreatedId).toBeNull();
  });
});

describe("min-size guard on END_DRAG", () => {
  it("MIN_RECT_DIMENSION matches the X-button visibility threshold (25)", () => {
    expect(MIN_RECT_DIMENSION).toBe(25);
  });

  it("END_DRAG after a pure click (no drag) discards the zero-size rect entirely", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "END_DRAG" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
    expect(s.draggingId).toBeNull();
    expect(s.lastCreatedId).toBeNull();
  });

  it("END_DRAG discards a sub-MIN_RECT_DIMENSION drag (10x10)", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "DRAG_TO", x: 110, y: 60 });
    s = reducer(s, { type: "END_DRAG" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
  });

  it("END_DRAG keeps a rect that meets the threshold (25x25)", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 25, y: 25 });
    s = reducer(s, { type: "END_DRAG" });
    expect(s.rects).toHaveLength(1);
    expect(s.rects[0]).toMatchObject({ w: 25, h: 25 });
  });

  it("END_DRAG discards a wide-but-flat rect (60x10) because height is below the threshold", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 60, y: 10 });
    s = reducer(s, { type: "END_DRAG" });
    expect(s.rects).toEqual([]);
  });

  it("discarding a too-small rect leaves any previously committed rectangles untouched", () => {
    let s = reducer(initialState, { type: "BEGIN_DRAG", id: "r1", x: 0, y: 0 });
    s = reducer(s, { type: "DRAG_TO", x: 100, y: 80 });
    s = reducer(s, { type: "END_DRAG" });
    s = reducer(s, { type: "BEGIN_DRAG", id: "r2", x: 200, y: 200 });
    s = reducer(s, { type: "END_DRAG" });
    expect(s.rects.map((r) => r.id)).toEqual(["r1"]);
    expect(s.history).toEqual([{ kind: "create", id: "r1" }]);
  });
});
