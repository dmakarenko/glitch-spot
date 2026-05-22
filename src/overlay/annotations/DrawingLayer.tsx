// ABOUTME: Mouse-driven drawing layer. Click-drag to draw rectangles; renders all
// ABOUTME: rectangles as absolutely-positioned DOM divs the parent can wire to notes.

import { useCallback, useEffect, useRef, useState } from "react";
import type { AnnotationsAction, AnnotationsState } from "./types";
import { RectangleNote } from "./RectangleNote";

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

  const [containerHeight, setContainerHeight] = useState<number>(
    typeof window !== "undefined" ? window.innerHeight : 0
  );

  useEffect(() => {
    const update = () => setContainerHeight(window.innerHeight);
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

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
            boxSizing: "border-box",
            pointerEvents: "none",
          }}
        >
          {r.w > 24 && r.h > 24 && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                dispatch({ type: "DELETE_RECT", id: r.id });
              }}
              onMouseDown={(e) => e.stopPropagation()}
              aria-label="Delete rectangle"
              style={{
                position: "absolute",
                top: -10,
                right: -10,
                width: 20,
                height: 20,
                padding: 0,
                border: "none",
                borderRadius: "50%",
                background: "#ff3b30",
                color: "white",
                fontSize: 14,
                lineHeight: "20px",
                textAlign: "center",
                cursor: "pointer",
                pointerEvents: "auto",
                boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
              }}
            >
              ×
            </button>
          )}
        </div>
      ))}
      {state.rects.map((r) => (
        <RectangleNote
          key={`${r.id}-note`}
          rect={r}
          containerHeight={containerHeight}
          autoFocus={r.id === state.lastCreatedId}
          onChange={(note) => dispatch({ type: "SET_NOTE", id: r.id, note })}
        />
      ))}
    </div>
  );
}
