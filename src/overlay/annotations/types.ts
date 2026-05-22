// ABOUTME: Shared annotation types for the overlay's drawing layer + Rust IPC payload.

export type Rect = {
  id: string;
  // CSS-pixel coordinates inside the overlay's content area.
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
  // Drag anchor — present only while the rect is the dragging target. Cleared on END_DRAG.
  anchorX?: number;
  anchorY?: number;
};

export type AnnotationsState = {
  rects: Rect[];
  // Stack of reversible actions; the head is the most recent.
  history: HistoryEntry[];
  // The id of the rectangle currently being dragged into existence, if any.
  draggingId: string | null;
};

export type HistoryEntry =
  | { kind: "create"; id: string }
  | { kind: "delete"; rect: Rect; index: number }
  | { kind: "noteEdit"; id: string; previousNote: string };

export type AnnotationsAction =
  | { type: "BEGIN_DRAG"; id: string; x: number; y: number }
  | { type: "DRAG_TO"; x: number; y: number }
  | { type: "END_DRAG" }
  | { type: "SET_NOTE"; id: string; note: string }
  | { type: "DELETE_RECT"; id: string }
  | { type: "UNDO" }
  | { type: "CLEAR" };
