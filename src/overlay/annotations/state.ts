// ABOUTME: useReducer state for the overlay's annotation layer. No React deps here —
// ABOUTME: keep it pure so the logic stays testable in plain Node/vitest.

import type {
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
  draggingId: null,
  lastCreatedId: null,
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
      return { rects: [...state.rects, rect], history, draggingId: action.id, lastCreatedId: action.id };
    }

    case "DRAG_TO": {
      if (!state.draggingId) return state;
      const rects = state.rects.map((r) =>
        r.id === state.draggingId ? extendTo(r, action.x, action.y) : r
      );
      return { ...state, rects };
    }

    case "END_DRAG": {
      const draggingId = state.draggingId;
      if (!draggingId) {
        return { ...state, draggingId: null };
      }
      const dragged = state.rects.find((r) => r.id === draggingId);
      if (!dragged) {
        return { ...state, draggingId: null };
      }
      const tooSmall =
        dragged.w < MIN_RECT_DIMENSION || dragged.h < MIN_RECT_DIMENSION;
      if (tooSmall) {
        return {
          ...state,
          rects: state.rects.filter((r) => r.id !== draggingId),
          history: state.history.filter(
            (h) => !(h.kind === "create" && h.id === draggingId)
          ),
          draggingId: null,
          lastCreatedId:
            state.lastCreatedId === draggingId ? null : state.lastCreatedId,
        };
      }
      const rects = state.rects.map((r) =>
        r.id === draggingId ? stripAnchor(r) : r
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
            lastCreatedId: last.id === state.lastCreatedId ? null : state.lastCreatedId,
          };
        case "delete": {
          const rects = [...state.rects];
          const insertAt = Math.min(last.index, rects.length);
          rects.splice(insertAt, 0, last.rect);
          return { ...state, rects, history };
        }
        case "noteEdit": {
          const rects = state.rects.map((r) =>
            r.id === last.id ? { ...r, note: last.previousNote } : r
          );
          return { ...state, rects, history };
        }
      }
    }

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
