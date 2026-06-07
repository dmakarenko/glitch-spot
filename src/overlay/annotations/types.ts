// ABOUTME: Shared annotation types for the overlay's drawing layer + Rust IPC payload.

export type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

export type Rect = {
  id: string;
  // CSS-pixel coordinates inside the overlay's content area.
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
  kind: 'annotation' | 'redaction';
};

export type ActiveGesture =
  | { kind: 'create'; id: string; anchorX: number; anchorY: number }
  | { kind: 'move'; id: string; pointerStart: { x: number; y: number }; rectStart: { x: number; y: number } }
  | {
      kind: 'resize';
      id: string;
      handle: Handle;
      rectStart: { x: number; y: number; w: number; h: number };
    };

export type AnnotationsState = {
  rects: Rect[];
  // Stack of reversible actions; the head is the most recent.
  history: HistoryEntry[];
  active: ActiveGesture | null;
  lastCreatedId: string | null;
};

export type HistoryEntry =
  | { kind: 'create'; id: string }
  | { kind: 'delete'; rect: Rect; index: number }
  | { kind: 'noteEdit'; id: string; previousNote: string }
  | { kind: 'move'; id: string; previous: { x: number; y: number } }
  | { kind: 'resize'; id: string; previous: { x: number; y: number; w: number; h: number } };

export type AnnotationsAction =
  | { type: 'BEGIN_CREATE'; id: string; x: number; y: number; kind: 'annotation' | 'redaction' }
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
      handle: Handle;
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
      // When set (window mode), a moved rect is clamped to these bounds rather
      // than the full viewport, keeping it inside the selected window.
      clip?: { x: number; y: number; w: number; h: number };
    }
  | { type: 'END_GESTURE' }
  | { type: 'SET_NOTE'; id: string; note: string }
  | { type: 'DELETE_RECT'; id: string }
  | { type: 'UNDO' }
  | { type: 'CLEAR' };
