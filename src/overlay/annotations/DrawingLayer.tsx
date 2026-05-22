// ABOUTME: Mouse-driven drawing layer. Click-drag to draw rectangles; renders all
// ABOUTME: rectangles as absolutely-positioned DOM divs the parent can wire to notes.

import { useCallback, useRef } from "react";
import type { AnnotationsAction, AnnotationsState } from "./types";

const RECT_BORDER = "2px solid #ff3b30";
const RECT_FILL = "rgba(255, 59, 48, 0.08)";

let _nextId = 1;
export function nextRectId(): string {
  return `r${_nextId++}`;
}

type Props = {
  state: AnnotationsState;
  dispatch: (action: AnnotationsAction) => void;
};

export function DrawingLayer({ state, dispatch }: Props) {
  const ref = useRef<HTMLDivElement | null>(null);

  const onMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      // Only respond to clicks that started on the drawing surface itself, not
      // on a child element (e.g., a note input or a delete X). Children stop
      // propagation via their own handlers.
      if (e.target !== e.currentTarget) return;
      // Ignore non-primary buttons.
      if (e.button !== 0) return;
      const rect = ref.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      dispatch({ type: "BEGIN_DRAG", id: nextRectId(), x, y });
    },
    [dispatch]
  );

  const onMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!state.draggingId) return;
      const rect = ref.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      dispatch({ type: "DRAG_TO", x, y });
    },
    [dispatch, state.draggingId]
  );

  const onMouseUp = useCallback(() => {
    if (!state.draggingId) return;
    dispatch({ type: "END_DRAG" });
  }, [dispatch, state.draggingId]);

  return (
    <div
      ref={ref}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseUp}
      style={{
        position: "absolute",
        inset: 0,
        cursor: "crosshair",
      }}
    >
      {state.rects.map((r) => (
        <div
          key={r.id}
          data-rect-id={r.id}
          style={{
            position: "absolute",
            left: r.x,
            top: r.y,
            width: r.w,
            height: r.h,
            border: RECT_BORDER,
            background: RECT_FILL,
            pointerEvents: "none",
            boxSizing: "border-box",
          }}
        />
      ))}
    </div>
  );
}
