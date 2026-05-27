// ABOUTME: Mouse-driven drawing layer. Click-drag to draw rectangles; renders all
// ABOUTME: rectangles as absolutely-positioned DOM divs the parent can wire to notes.

import { useCallback, useEffect, useRef, useState } from "react";
import type { AnnotationsAction, AnnotationsState } from "./types";
import { MIN_RECT_DIMENSION } from "./state";
import { RectangleNote } from "./RectangleNote";
import { formatLabel } from "./label";

const RECT_BORDER = "1.5px solid rgba(238, 102, 51, 0.9)";
const RECT_FILL = "rgba(238, 102, 51, 0.05)";

let _nextId = 1;
export function nextRectId(): string {
  return `r${_nextId++}`;
}

type ClipRect = { x: number; y: number; w: number; h: number };

type Props = {
  state: AnnotationsState;
  dispatch: (action: AnnotationsAction) => void;
  clipRect?: ClipRect;
};

export function DrawingLayer({ state, dispatch, clipRect }: Props) {
  const ref = useRef<HTMLDivElement | null>(null);

  const [containerHeight, setContainerHeight] = useState<number>(
    typeof window !== "undefined" ? window.innerHeight : 0
  );

  useEffect(() => {
    const update = () => setContainerHeight(window.innerHeight);
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  // Returns the point if it is inside clipRect (or if no clipRect is set),
  // null otherwise. Used to gate BEGIN_DRAG.
  const clamp = (x: number, y: number): { x: number; y: number } | null => {
    if (!clipRect) return { x, y };
    const inside =
      x >= clipRect.x &&
      x < clipRect.x + clipRect.w &&
      y >= clipRect.y &&
      y < clipRect.y + clipRect.h;
    if (!inside) return null;
    const clampedX = Math.max(clipRect.x, Math.min(x, clipRect.x + clipRect.w));
    const clampedY = Math.max(clipRect.y, Math.min(y, clipRect.y + clipRect.h));
    return { x: clampedX, y: clampedY };
  };

  const onMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      // Only respond to clicks that started on the drawing surface itself, not
      // on a child element (e.g., a note input or a delete X). Children stop
      // propagation via their own handlers.
      if (e.target !== e.currentTarget) return;
      // Ignore non-primary buttons.
      if (e.button !== 0) return;
      // Prevent the browser from starting a text selection that would flash
      // across existing notes and rect contents as the drag passes over them.
      e.preventDefault();
      const rect = ref.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const startInside = clamp(x, y);
      if (!startInside) return;
      dispatch({ type: "BEGIN_DRAG", id: nextRectId(), x: startInside.x, y: startInside.y });
    },
    [dispatch, clipRect]
  );

  const onMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!state.draggingId) return;
      const rect = ref.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      let nx = x;
      let ny = y;
      if (clipRect) {
        nx = Math.max(clipRect.x, Math.min(x, clipRect.x + clipRect.w));
        ny = Math.max(clipRect.y, Math.min(y, clipRect.y + clipRect.h));
      }
      dispatch({ type: "DRAG_TO", x: nx, y: ny });
    },
    [dispatch, state.draggingId, clipRect]
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
        userSelect: "none",
        WebkitUserSelect: "none",
      }}
    >
      {state.rects.map((r, idx) => (
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
            boxShadow: "-2px 0 0 rgba(255, 0, 60, 0.4), 2px 0 0 rgba(0, 245, 255, 0.4)",
          }}
        >
          {r.w >= MIN_RECT_DIMENSION && r.h >= MIN_RECT_DIMENSION && (
            <>
              <div
                aria-hidden
                style={{
                  position: "absolute",
                  top: -9,
                  left: -13,
                  width: 26,
                  height: 18,
                  borderRadius: 9,
                  background: "#ee6633",
                  color: "#ffffff",
                  font: "bold 11px sans-serif",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  pointerEvents: "none",
                  boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
                }}
              >
                {formatLabel(idx)}
              </div>
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
            </>
          )}
        </div>
      ))}
      {state.rects.map((r) => (
        <RectangleNote
          key={`${r.id}-note`}
          rect={r}
          containerHeight={containerHeight}
          autoFocus={!state.draggingId && r.id === state.lastCreatedId}
          onChange={(note) => dispatch({ type: "SET_NOTE", id: r.id, note })}
        />
      ))}
    </div>
  );
}
