// ABOUTME: Mouse-driven drawing layer. Click-drag to draw rectangles; renders all
// ABOUTME: rectangles as absolutely-positioned DOM divs the parent can wire to notes.

import { useCallback, useEffect, useRef, useState } from "react";
import type { AnnotationsAction, AnnotationsState, Handle } from "./types";
import { MIN_RECT_DIMENSION } from "./state";
import { RectangleNote } from "./RectangleNote";
import { formatLabel } from "./label";

const RECT_BORDER = "1.5px solid rgba(238, 102, 51, 0.9)";
const RECT_FILL = "rgba(238, 102, 51, 0.05)";

let _nextId = 1;
export function nextRectId(): string {
  return `r${_nextId++}`;
}

type Props = {
  state: AnnotationsState;
  dispatch: (action: AnnotationsAction) => void;
};

const HANDLES: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

function handleStyle(handle: Handle): React.CSSProperties {
  const base: React.CSSProperties = {
    position: "absolute",
    background: "#ee6633",
    boxShadow: "0 0 0 1px rgba(0,0,0,0.35)",
    pointerEvents: "auto",
  };
  switch (handle) {
    case "nw":
      return { ...base, width: 10, height: 10, top: -5, left: -5, cursor: "nwse-resize" };
    case "ne":
      return { ...base, width: 10, height: 10, top: -5, right: -5, cursor: "nesw-resize" };
    case "sw":
      return { ...base, width: 10, height: 10, bottom: -5, left: -5, cursor: "nesw-resize" };
    case "se":
      return { ...base, width: 10, height: 10, bottom: -5, right: -5, cursor: "nwse-resize" };
    case "n":
      return { ...base, width: 20, height: 6, top: -3, left: "50%", marginLeft: -10, cursor: "ns-resize" };
    case "s":
      return { ...base, width: 20, height: 6, bottom: -3, left: "50%", marginLeft: -10, cursor: "ns-resize" };
    case "w":
      return { ...base, width: 6, height: 20, left: -3, top: "50%", marginTop: -10, cursor: "ew-resize" };
    case "e":
      return { ...base, width: 6, height: 20, right: -3, top: "50%", marginTop: -10, cursor: "ew-resize" };
  }
}

function ResizeHandle({
  handle,
  rectId,
  dispatch,
  eventCoords,
}: {
  handle: Handle;
  rectId: string;
  dispatch: (action: AnnotationsAction) => void;
  eventCoords: (e: { clientX: number; clientY: number }) => { x: number; y: number };
}) {
  return (
    <div
      aria-hidden
      style={handleStyle(handle)}
      onMouseDown={(e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();
        const { x: pointerX, y: pointerY } = eventCoords(e);
        dispatch({
          type: "BEGIN_RESIZE",
          id: rectId,
          handle,
          pointerX,
          pointerY,
          viewportW: window.innerWidth,
          viewportH: window.innerHeight,
        });
      }}
    />
  );
}

export function DrawingLayer({ state, dispatch }: Props) {
  const ref = useRef<HTMLDivElement | null>(null);

  const [containerHeight, setContainerHeight] = useState<number>(
    typeof window !== "undefined" ? window.innerHeight : 0
  );

  const [hoveredId, setHoveredId] = useState<string | null>(null);

  const eventCoords = useCallback(
    (e: { clientX: number; clientY: number }) => {
      const surface = ref.current?.getBoundingClientRect();
      const offsetX = surface?.left ?? 0;
      const offsetY = surface?.top ?? 0;
      return { x: e.clientX - offsetX, y: e.clientY - offsetY };
    },
    [],
  );

  useEffect(() => {
    const update = () => setContainerHeight(window.innerHeight);
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  const onMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (e.target !== e.currentTarget) return;
      if (e.button !== 0) return;
      e.preventDefault();
      const { x, y } = eventCoords(e);
      dispatch({ type: "BEGIN_CREATE", id: nextRectId(), x, y });
    },
    [dispatch, eventCoords],
  );

  const onMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!state.active) return;
      const { x, y } = eventCoords(e);
      dispatch({
        type: "POINTER_TO",
        x,
        y,
        viewportW: window.innerWidth,
        viewportH: window.innerHeight,
      });
    },
    [dispatch, eventCoords, state.active],
  );

  const onMouseUp = useCallback(() => {
    if (!state.active) return;
    dispatch({ type: "END_GESTURE" });
  }, [dispatch, state.active]);

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
          onMouseEnter={() => setHoveredId(r.id)}
          onMouseLeave={() => setHoveredId((prev) => (prev === r.id ? null : prev))}
          onMouseDown={(e) => {
            // Only respond if the mousedown originated on the rect body itself, not on
            // a child (handle, delete-X, badge, note textarea). Children stopPropagation
            // on their own mousedowns.
            if (e.target !== e.currentTarget) return;
            if (e.button !== 0) return;
            e.preventDefault();
            e.stopPropagation();
            const { x: pointerX, y: pointerY } = eventCoords(e);
            dispatch({
              type: "BEGIN_MOVE",
              id: r.id,
              pointerX,
              pointerY,
              viewportW: window.innerWidth,
              viewportH: window.innerHeight,
            });
          }}
          style={{
            position: "absolute",
            left: r.x,
            top: r.y,
            width: r.w,
            height: r.h,
            border: RECT_BORDER,
            background: RECT_FILL,
            boxSizing: "border-box",
            pointerEvents: "auto",
            cursor: "move",
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
              {hoveredId === r.id && HANDLES.map((handle) => (
                <ResizeHandle
                  key={handle}
                  handle={handle}
                  rectId={r.id}
                  dispatch={dispatch}
                  eventCoords={eventCoords}
                />
              ))}
            </>
          )}
        </div>
      ))}
      {state.rects.map((r) => (
        <RectangleNote
          key={`${r.id}-note`}
          rect={r}
          containerHeight={containerHeight}
          autoFocus={!state.active && r.id === state.lastCreatedId}
          onChange={(note) => dispatch({ type: "SET_NOTE", id: r.id, note })}
        />
      ))}
    </div>
  );
}
