// ABOUTME: Shared annotation types for the overlay's drawing layer + Rust IPC payload.

export type Corner = 'nw' | 'ne' | 'sw' | 'se';

export type Rect = {
  id: string;
  // CSS-pixel coordinates inside the overlay's content area.
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
};

export type ActiveGesture =
  | { kind: 'create'; id: string; anchorX: number; anchorY: number }
  | { kind: 'move'; id: string; pointerStart: { x: number; y: number }; rectStart: { x: number; y: number } }
  | { kind: 'resize'; id: string; corner: Corner; anchor: { x: number; y: number } };

export type AnnotationsState = {
  rects: Rect[];
  history: HistoryEntry[];
  active: ActiveGesture | null;
  lastCreatedId: string | null;
};

export type HistoryEntry =
  | { kind: 'create'; id: string }
  | { kind: 'delete'; rect: Rect; index: number }
  | { kind: 'noteEdit'; id: string; previousNote: string }
  | { kind: 'move'; id: string; previousX: number; previousY: number }
  | { kind: 'resize'; id: string; previous: { x: number; y: number; w: number; h: number } };

export type AnnotationsAction =
  | { type: 'BEGIN_CREATE'; id: string; x: number; y: number }
  | {
      type: 'BEGIN_MOVE';
      id: string;
      pointerX: number;
      pointerY: number;
      viewportW: number;
      viewportH: number;
    }
  | {
      type: 'BEGIN_RESIZE';
      id: string;
      corner: Corner;
      pointerX: number;
      pointerY: number;
      viewportW: number;
      viewportH: number;
    }
  | {
      type: 'POINTER_TO';
      x: number;
      y: number;
      viewportW: number;
      viewportH: number;
    }
  | { type: 'END_GESTURE' }
  | { type: 'SET_NOTE'; id: string; note: string }
  | { type: 'DELETE_RECT'; id: string }
  | { type: 'UNDO' }
  | { type: 'CLEAR' };
