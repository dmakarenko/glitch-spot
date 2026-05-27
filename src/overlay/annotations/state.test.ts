// ABOUTME: Reducer tests for the overlay's annotation state. Pure data, no DOM.
import { describe, expect, it } from "vitest";
import { reducer, initialState, MIN_RECT_DIMENSION } from "./state";

const VW = 1000;
const VH = 800;

function pointerTo(x: number, y: number) {
  return { type: "POINTER_TO" as const, x, y, viewportW: VW, viewportH: VH };
}

describe("annotations reducer", () => {
  it("starts with no rects, no history, no active gesture", () => {
    expect(initialState).toEqual({
      rects: [],
      history: [],
      active: null,
      lastCreatedId: null,
    });
  });

  it("BEGIN_CREATE creates a zero-size rect anchored at the start point", () => {
    const s = reducer(initialState, {
      type: "BEGIN_CREATE",
      id: "r1",
      x: 100,
      y: 50,
    });
    expect(s.rects).toHaveLength(1);
    expect(s.rects[0]).toMatchObject({
      id: "r1",
      x: 100,
      y: 50,
      w: 0,
      h: 0,
      note: "",
    });
    expect(s.active).toEqual({
      kind: "create",
      id: "r1",
      anchorX: 100,
      anchorY: 50,
    });
    expect(s.history).toEqual([{ kind: "create", id: "r1" }]);
  });

  it("POINTER_TO updates the dragging rect's bottom-right while create is active", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, pointerTo(200, 100));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("POINTER_TO handles drags up and to the left by flipping origin", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 200, y: 100 });
    s = reducer(s, pointerTo(100, 50));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 50, w: 100, h: 50 });
  });

  it("END_GESTURE clears active without removing the rect", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, pointerTo(200, 100));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.active).toBeNull();
    expect(s.rects).toHaveLength(1);
  });

  it("POINTER_TO is a no-op when there is no active gesture", () => {
    const s = reducer(initialState, pointerTo(10, 10));
    expect(s).toEqual(initialState);
  });

  it("BEGIN_CREATE sets lastCreatedId so the new rect's note can auto-focus", () => {
    const s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    expect(s.lastCreatedId).toBe("r1");
  });
});

describe("notes and deletion", () => {
  function drawn(id: string): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id, x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 50));
    return reducer(s, { type: "END_GESTURE" });
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
    s = reducer(s, { type: "BEGIN_CREATE", id: "r2", x: 200, y: 0 });
    s = reducer(s, pointerTo(300, 80));
    s = reducer(s, { type: "END_GESTURE" });
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
    let s = reducer(state, { type: "BEGIN_CREATE", id, x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 50));
    return reducer(s, { type: "END_GESTURE" });
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
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 50));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, { type: "UNDO" });
    expect(s.lastCreatedId).toBeNull();
  });

  it("UNDO of a move restores the previous position", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 100 });
    s = reducer(s, pointerTo(150, 150));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 120,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(220, 220));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects[0]).toMatchObject({ x: 200, y: 200 });
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 50, h: 50 });
  });

  it("UNDO of a resize restores the full previous geometry", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 100 });
    s = reducer(s, pointerTo(300, 250));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(400, 300));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 300, h: 200 });
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 200, h: 150 });
  });

  it("UNDO walks back through move → resize in reverse order", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 100 });
    s = reducer(s, pointerTo(200, 200));
    s = reducer(s, { type: "END_GESTURE" });
    // first move (100,100) -> (150,150)
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 120,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(170, 170));
    s = reducer(s, { type: "END_GESTURE" });
    // then resize from (150,150,100,100) to wider
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "se",
      pointerX: 250,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(300, 300));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects[0]).toMatchObject({ x: 150, y: 150, w: 150, h: 150 });
    // Undo the resize first.
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 150, y: 150, w: 100, h: 100 });
    // Then undo the move.
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 100, h: 100 });
  });
});

describe("min-size guard on END_GESTURE", () => {
  it("MIN_RECT_DIMENSION matches the X-button visibility threshold (25)", () => {
    expect(MIN_RECT_DIMENSION).toBe(25);
  });

  it("END_GESTURE after a pure click (no drag) discards the zero-size rect entirely", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
    expect(s.active).toBeNull();
    expect(s.lastCreatedId).toBeNull();
  });

  it("END_GESTURE discards a sub-MIN_RECT_DIMENSION drag (10x10)", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 100, y: 50 });
    s = reducer(s, pointerTo(110, 60));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toEqual([]);
    expect(s.history).toEqual([]);
  });

  it("END_GESTURE keeps a rect that meets the threshold (25x25)", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(25, 25));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toHaveLength(1);
    expect(s.rects[0]).toMatchObject({ w: 25, h: 25 });
  });

  it("END_GESTURE discards a wide-but-flat rect (60x10) because height is below the threshold", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(60, 10));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects).toEqual([]);
  });

  it("discarding a too-small rect leaves any previously committed rectangles untouched", () => {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id: "r1", x: 0, y: 0 });
    s = reducer(s, pointerTo(100, 80));
    s = reducer(s, { type: "END_GESTURE" });
    s = reducer(s, { type: "BEGIN_CREATE", id: "r2", x: 200, y: 200 });
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects.map((r) => r.id)).toEqual(["r1"]);
    expect(s.history).toEqual([{ kind: "create", id: "r1" }]);
  });
});

describe("move gesture", () => {
  function drawn(id: string, x: number, y: number, w: number, h: number): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id, x, y });
    s = reducer(s, pointerTo(x + w, y + h));
    return reducer(s, { type: "END_GESTURE" });
  }

  it("BEGIN_MOVE sets active=move and records the previous position in history", () => {
    const start = drawn("r1", 100, 100, 50, 50);
    const s = reducer(start, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.active).toEqual({
      kind: "move",
      id: "r1",
      pointerStart: { x: 120, y: 130 },
      rectStart: { x: 100, y: 100 },
    });
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({ kind: "move", id: "r1", previous: { x: 100, y: 100 } });
  });

  it("POINTER_TO during move translates the rect by the pointer delta", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(200, 200));
    expect(s.rects[0]).toMatchObject({ x: 180, y: 170, w: 50, h: 50 });
  });

  it("POINTER_TO during move clamps to viewport edges", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 110,
      pointerY: 110,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(-500, -500));
    expect(s.rects[0]).toMatchObject({ x: 0, y: 0, w: 50, h: 50 });
  });

  it("END_GESTURE after a move clears active and keeps the move history entry", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(200, 200));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.active).toBeNull();
    expect(s.rects[0]).toMatchObject({ x: 180, y: 170 });
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({ kind: "move", id: "r1", previous: { x: 100, y: 100 } });
  });

  it("END_GESTURE after BEGIN_MOVE with no POINTER_TO pops the no-op history entry", () => {
    let s = drawn("r1", 100, 100, 50, 50);
    const before = s.history.length;
    s = reducer(s, {
      type: "BEGIN_MOVE",
      id: "r1",
      pointerX: 120,
      pointerY: 130,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.history.length).toBe(before + 1);
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.history.length).toBe(before);
    expect(s.history[s.history.length - 1]).not.toMatchObject({ kind: "move" });
  });

  it("BEGIN_MOVE on a non-existent id is a no-op", () => {
    const start = drawn("r1", 100, 100, 50, 50);
    const s = reducer(start, {
      type: "BEGIN_MOVE",
      id: "ghost",
      pointerX: 0,
      pointerY: 0,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s).toEqual(start);
  });
});

describe("resize gesture", () => {
  function drawn(id: string, x: number, y: number, w: number, h: number): import("./types").AnnotationsState {
    let s = reducer(initialState, { type: "BEGIN_CREATE", id, x, y });
    s = reducer(s, pointerTo(x + w, y + h));
    return reducer(s, { type: "END_GESTURE" });
  }

  it("BEGIN_RESIZE snapshots the rect into active.rectStart and records previous geometry in history", () => {
    const start = drawn("r1", 100, 100, 200, 150);
    const s = reducer(start, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.active).toEqual({
      kind: "resize",
      id: "r1",
      handle: "se",
      rectStart: { x: 100, y: 100, w: 200, h: 150 },
    });
    const last = s.history[s.history.length - 1];
    expect(last).toEqual({
      kind: "resize",
      id: "r1",
      previous: { x: 100, y: 100, w: 200, h: 150 },
    });
  });

  it("POINTER_TO during SE resize extends width/height with NW pinned", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(400, 300));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 300, h: 200 });
  });

  it("POINTER_TO during NW resize moves the top-left with SE pinned", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "nw",
      pointerX: 100,
      pointerY: 100,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(50, 50));
    expect(s.rects[0]).toMatchObject({ x: 50, y: 50, w: 250, h: 200 });
  });

  it("POINTER_TO past the anchor stops at MIN_RECT_DIMENSION (no flip)", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(50, 50));
    expect(s.rects[0]).toMatchObject({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it("POINTER_TO during resize clamps the dragged corner to viewport", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(VW + 500, VH + 500));
    const r = s.rects[0];
    expect(r.x + r.w).toBe(VW);
    expect(r.y + r.h).toBe(VH);
  });

  it("END_GESTURE after BEGIN_RESIZE with no POINTER_TO pops the no-op history entry", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    const before = s.history.length;
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "se",
      pointerX: 300,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.history.length).toBe(before + 1);
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.history.length).toBe(before);
  });

  it("BEGIN_RESIZE on a non-existent id is a no-op", () => {
    const start = drawn("r1", 100, 100, 200, 150);
    const s = reducer(start, {
      type: "BEGIN_RESIZE",
      id: "ghost",
      handle: "se",
      pointerX: 0,
      pointerY: 0,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s).toEqual(start);
  });

  it("BEGIN_RESIZE on an edge handle stores the full rectStart in active", () => {
    const start = drawn("r1", 100, 100, 200, 150);
    const s = reducer(start, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "n",
      pointerX: 200,
      pointerY: 100,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.active).toEqual({
      kind: "resize",
      id: "r1",
      handle: "n",
      rectStart: { x: 100, y: 100, w: 200, h: 150 },
    });
  });

  it("POINTER_TO during N-edge resize moves only the top edge", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "n",
      pointerX: 200,
      pointerY: 100,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(999, 80));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 80, w: 200, h: 170 });
  });

  it("POINTER_TO during E-edge resize moves only the right edge", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "e",
      pointerX: 300,
      pointerY: 175,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(400, -999));
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 300, h: 150 });
  });

  it("UNDO restores the full geometry after an edge resize", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "w",
      pointerX: 100,
      pointerY: 175,
      viewportW: VW,
      viewportH: VH,
    });
    s = reducer(s, pointerTo(50, 175));
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.rects[0]).toMatchObject({ x: 50, y: 100, w: 250, h: 150 });
    s = reducer(s, { type: "UNDO" });
    expect(s.rects[0]).toMatchObject({ x: 100, y: 100, w: 200, h: 150 });
  });

  it("END_GESTURE after edge BEGIN_RESIZE with no POINTER_TO pops the no-op history entry", () => {
    let s = drawn("r1", 100, 100, 200, 150);
    const before = s.history.length;
    s = reducer(s, {
      type: "BEGIN_RESIZE",
      id: "r1",
      handle: "s",
      pointerX: 200,
      pointerY: 250,
      viewportW: VW,
      viewportH: VH,
    });
    expect(s.history.length).toBe(before + 1);
    s = reducer(s, { type: "END_GESTURE" });
    expect(s.history.length).toBe(before);
  });
});
