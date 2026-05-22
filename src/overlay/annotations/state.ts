// ABOUTME: useReducer state for the overlay's annotation layer. No React deps here —
// ABOUTME: keep it pure so the logic stays testable in plain Node/vitest.

import type {
  AnnotationsAction,
  AnnotationsState,
  HistoryEntry,
  Rect,
} from "./types";

export const initialState: AnnotationsState = {
  rects: [],
  history: [],
  draggingId: null,
};

export function reducer(
  state: AnnotationsState,
  action: AnnotationsAction
): AnnotationsState {
  switch (action.type) {
    case "BEGIN_DRAG": {
      const rect: Rect = {
        id: action.id,
        x: action.x,
        y: action.y,
        w: 0,
        h: 0,
        note: "",
        anchorX: action.x,
        anchorY: action.y,
      };
      const history: HistoryEntry[] = [
        ...state.history,
        { kind: "create", id: action.id },
      ];
      return { rects: [...state.rects, rect], history, draggingId: action.id };
    }

    case "DRAG_TO": {
      if (!state.draggingId) return state;
      const rects = state.rects.map((r) =>
        r.id === state.draggingId ? extendTo(r, action.x, action.y) : r
      );
      return { ...state, rects };
    }

    case "END_DRAG": {
      const rects = state.rects.map((r) =>
        r.id === state.draggingId ? stripAnchor(r) : r
      );
      return { ...state, rects, draggingId: null };
    }

    case "SET_NOTE": {
      const rect = state.rects.find((r) => r.id === action.id);
      if (!rect) return state;
      const rects = state.rects.map((r) =>
        r.id === action.id ? { ...r, note: action.note } : r
      );
      const alreadyEditing = state.history.some(
        (h) => h.kind === "noteEdit" && h.id === action.id
      );
      const history: HistoryEntry[] = alreadyEditing
        ? state.history
        : [...state.history, { kind: "noteEdit", id: action.id, previousNote: rect.note }];
      return { ...state, rects, history };
    }

    case "DELETE_RECT": {
      const index = state.rects.findIndex((r) => r.id === action.id);
      if (index < 0) return state;
      const rect = state.rects[index];
      const rects = state.rects.filter((r) => r.id !== action.id);
      const history: HistoryEntry[] = [...state.history, { kind: "delete", rect, index }];
      return { ...state, rects, history };
    }

    case "CLEAR":
      return initialState;

    default:
      return state;
  }
}

function extendTo(rect: Rect, cursorX: number, cursorY: number): Rect {
  const ax = rect.anchorX ?? rect.x;
  const ay = rect.anchorY ?? rect.y;
  const x = Math.min(ax, cursorX);
  const y = Math.min(ay, cursorY);
  const w = Math.abs(cursorX - ax);
  const h = Math.abs(cursorY - ay);
  return { ...rect, x, y, w, h };
}

function stripAnchor(rect: Rect): Rect {
  const { anchorX: _ax, anchorY: _ay, ...rest } = rect;
  return rest;
}
