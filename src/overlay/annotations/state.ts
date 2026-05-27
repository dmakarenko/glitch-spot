// ABOUTME: useReducer state for the overlay's annotation layer. No React deps here —
// ABOUTME: keep it pure so the logic stays testable in plain Node/vitest.

import type {
  ActiveGesture,
  AnnotationsAction,
  AnnotationsState,
  HistoryEntry,
  Rect,
} from "./types";
import { clampMoveDelta, clampResize } from "./transform";

// Smallest committed-rectangle dimension. Aligned with DrawingLayer's delete-X
// visibility threshold so every saved rectangle has a working delete button,
// and a stray click without a drag is discarded as accidental.
export const MIN_RECT_DIMENSION = 25;

export const initialState: AnnotationsState = {
  rects: [],
  history: [],
  active: null,
  lastCreatedId: null,
};

export function reducer(
  state: AnnotationsState,
  action: AnnotationsAction,
): AnnotationsState {
  switch (action.type) {
    case "BEGIN_CREATE": {
      const rect: Rect = {
        id: action.id,
        x: action.x,
        y: action.y,
        w: 0,
        h: 0,
        note: "",
      };
      const active: ActiveGesture = {
        kind: "create",
        id: action.id,
        anchorX: action.x,
        anchorY: action.y,
      };
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "create", id: action.id },
      ];
      return {
        ...state,
        rects: [...state.rects, rect],
        history,
        active,
        lastCreatedId: action.id,
      };
    }

    case "BEGIN_MOVE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const active: ActiveGesture = {
        kind: "move",
        id: action.id,
        pointerStart: { x: action.pointerX, y: action.pointerY },
        rectStart: { x: rect.x, y: rect.y },
      };
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "move", id: action.id, previous: { x: rect.x, y: rect.y } },
      ];
      return { ...state, active, history };
    }

    case "BEGIN_RESIZE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const active: ActiveGesture = {
        kind: "resize",
        id: action.id,
        handle: action.handle,
        rectStart: { x: rect.x, y: rect.y, w: rect.w, h: rect.h },
      };
      const history: HistoryEntry[] = [
        ...state.history,
        {
          kind: "resize",
          id: action.id,
          previous: { x: rect.x, y: rect.y, w: rect.w, h: rect.h },
        },
      ];
      return { ...state, active, history };
    }

    case "POINTER_TO": {
      if (!state.active) return state;
      if (state.active.kind === "create") {
        const { id, anchorX, anchorY } = state.active;
        const rects = state.rects.map((r) =>
          r.id === id ? extendTo(r, action.x, action.y, anchorX, anchorY) : r,
        );
        return { ...state, rects };
      }
      if (state.active.kind === "move") {
        const { id, pointerStart, rectStart } = state.active;
        const rect = state.rects.find((r) => r.id === id);
        if (!rect) return state;
        const deltaX = action.x - pointerStart.x;
        const deltaY = action.y - pointerStart.y;
        const startRect = { ...rect, x: rectStart.x, y: rectStart.y };
        const { x, y } = clampMoveDelta(
          startRect,
          deltaX,
          deltaY,
          { w: action.viewportW, h: action.viewportH },
        );
        const rects = state.rects.map((r) => (r.id === id ? { ...r, x, y } : r));
        return { ...state, rects };
      }
      if (state.active.kind === "resize") {
        const { id, handle, rectStart } = state.active;
        const rect = state.rects.find((r) => r.id === id);
        if (!rect) return state;
        const geom = clampResize(
          handle,
          rectStart,
          { x: action.x, y: action.y },
          { w: action.viewportW, h: action.viewportH },
        );
        const rects = state.rects.map((r) => (r.id === id ? { ...r, ...geom } : r));
        return { ...state, rects };
      }
      return state;
    }

    case "END_GESTURE": {
      if (!state.active) return state;
      if (state.active.kind === "create") {
        const id = state.active.id;
        const drawn = state.rects.find((r) => r.id === id);
        if (!drawn) {
          return { ...state, active: null };
        }
        const tooSmall =
          drawn.w < MIN_RECT_DIMENSION || drawn.h < MIN_RECT_DIMENSION;
        if (tooSmall) {
          return {
            ...state,
            rects: state.rects.filter((r) => r.id !== id),
            history: state.history.filter(
              (h) => !(h.kind === "create" && h.id === id),
            ),
            active: null,
            lastCreatedId:
              state.lastCreatedId === id ? null : state.lastCreatedId,
          };
        }
        return { ...state, active: null };
      }
      if (state.active.kind === "move") {
        const { id } = state.active;
        const rect = state.rects.find((r) => r.id === id);
        const last = state.history[state.history.length - 1];
        if (
          rect &&
          last &&
          last.kind === "move" &&
          last.id === id &&
          last.previous.x === rect.x &&
          last.previous.y === rect.y
        ) {
          // No-op move (mousedown without movement). Pop the history entry.
          const history = state.history.slice(0, -1);
          return { ...state, active: null, history };
        }
        return { ...state, active: null };
      }
      if (state.active.kind === "resize") {
        const { id } = state.active;
        const rect = state.rects.find((r) => r.id === id);
        const last = state.history[state.history.length - 1];
        if (
          rect &&
          last &&
          last.kind === "resize" &&
          last.id === id &&
          last.previous.x === rect.x &&
          last.previous.y === rect.y &&
          last.previous.w === rect.w &&
          last.previous.h === rect.h
        ) {
          // No-op resize (rect ended at its starting geometry). Pop the history entry.
          const history = state.history.slice(0, -1);
          return { ...state, active: null, history };
        }
        return { ...state, active: null };
      }
      return { ...state, active: null };
    }

    case "SET_NOTE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const rects = state.rects.map((r) =>
        r.id === action.id ? { ...r, note: action.note } : r,
      );
      const alreadyEditing = state.history.some(
        (h) => h.kind === "noteEdit" && h.id === action.id,
      );
      const history: HistoryEntry[] = alreadyEditing
        ? state.history
        : [
            ...state.history,
            { kind: "noteEdit", id: action.id, previousNote: rect.note },
          ];
      return { ...state, rects, history };
    }

    case "DELETE_RECT": {
      const index = state.rects.findIndex((r) => r.id === action.id);
      if (index < 0) return state;
      const rect = state.rects[index];
      const rects = state.rects.filter((r) => r.id !== action.id);
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "delete", rect, index },
      ];
      return { ...state, rects, history };
    }

    case "CLEAR":
      return initialState;

    case "UNDO": {
      const history = [...state.history];
      const last = history.pop();
      if (!last) return state;
      switch (last.kind) {
        case "create":
          return {
            ...state,
            rects: state.rects.filter((r) => r.id !== last.id),
            history,
            lastCreatedId:
              last.id === state.lastCreatedId ? null : state.lastCreatedId,
          };
        case "delete": {
          const rects = [...state.rects];
          const insertAt = Math.min(last.index, rects.length);
          rects.splice(insertAt, 0, last.rect);
          return { ...state, rects, history };
        }
        case "noteEdit": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, note: last.previousNote } : r,
          );
          return { ...state, rects, history };
        }
        case "move": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, x: last.previous.x, y: last.previous.y } : r,
          );
          return { ...state, rects, history };
        }
        case "resize": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, ...last.previous } : r,
          );
          return { ...state, rects, history };
        }
      }
      assertNever(last);
    }

    default:
      return state;
  }
}

function assertNever(x: never): never {
  throw new Error(`Unhandled history kind: ${JSON.stringify(x)}`);
}

function extendTo(
  rect: Rect,
  cursorX: number,
  cursorY: number,
  anchorX: number,
  anchorY: number,
): Rect {
  const x = Math.min(anchorX, cursorX);
  const y = Math.min(anchorY, cursorY);
  const w = Math.abs(cursorX - anchorX);
  const h = Math.abs(cursorY - anchorY);
  return { ...rect, x, y, w, h };
}

