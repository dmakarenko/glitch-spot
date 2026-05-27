// ABOUTME: useReducer state for the overlay's annotation layer. No React deps here —
// ABOUTME: keep it pure so the logic stays testable in plain Node/vitest.

import type {
  ActiveGesture,
  AnnotationsAction,
  AnnotationsState,
  HistoryEntry,
  Rect,
} from "./types";

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

    case "POINTER_TO": {
      if (!state.active) return state;
      if (state.active.kind === "create") {
        const { id, anchorX, anchorY } = state.active;
        const rects = state.rects.map((r) =>
          r.id === id ? extendTo(r, action.x, action.y, anchorX, anchorY) : r,
        );
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
        case "move":
        case "resize":
          return state;
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
