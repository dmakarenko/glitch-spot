// ABOUTME: Mouse-driven drawing layer. Click-drag to draw rectangles; renders all
// ABOUTME: rectangles as absolutely-positioned DOM divs the parent can wire to notes.

import { useCallback, useEffect, useRef, useState } from "react";
import type { AnnotationsAction, AnnotationsState, Handle } from "./types";
import { MIN_RECT_DIMENSION } from "./state";
import { PICKER_CURSOR_CSS } from "../../picker/cursor";
import { RectangleNote } from "./RectangleNote";
import { annotationLabels } from "./label";
import type { RedactionStyle } from "../bridge";

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
  // In window mode the layer covers the whole overlay (so notes can render
  // outside the window rect), so a click on the dimmed area outside clipRect
  // must be forwarded here rather than silently swallowed.
  onOutsideClick?: () => void;
  redactMode?: boolean;
  redactionStyle?: RedactionStyle;
};

function redactionPreviewBg(style: RedactionStyle): string {
  if (style === "solid") return "#0c0c0c";
  // A small mosaic pattern stands in for the baked cover (no live-screen pixels
  // in the DOM). Branded uses the accent palette; pixelated greys.
  const a = style === "branded" ? "%23ee6633" : "%23777777";
  const b = style === "branded" ? "%2378341a" : "%23444444";
  return (
    "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12'>" +
    `<rect width='12' height='12' fill='${a}'/>` +
    `<rect width='6' height='6' fill='${b}'/>` +
    `<rect x='6' y='6' width='6' height='6' fill='${b}'/>` +
    "</svg>\")"
  );
}

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
      return { ...base, width: 8, height: 8, top: -4, left: -4, cursor: "nwse-resize" };
    case "ne":
      return { ...base, width: 8, height: 8, top: -4, right: -4, cursor: "nesw-resize" };
    case "sw":
      return { ...base, width: 8, height: 8, bottom: -4, left: -4, cursor: "nesw-resize" };
    case "se":
      return { ...base, width: 8, height: 8, bottom: -4, right: -4, cursor: "nwse-resize" };
    case "n":
      return { ...base, width: 16, height: 4, top: -2, left: "50%", marginLeft: -8, cursor: "ns-resize" };
    case "s":
      return { ...base, width: 16, height: 4, bottom: -2, left: "50%", marginLeft: -8, cursor: "ns-resize" };
    case "w":
      return { ...base, width: 4, height: 16, left: -2, top: "50%", marginTop: -8, cursor: "ew-resize" };
    case "e":
      return { ...base, width: 4, height: 16, right: -2, top: "50%", marginTop: -8, cursor: "ew-resize" };
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

export function DrawingLayer({ state, dispatch, clipRect, onOutsideClick, redactMode = false, redactionStyle = 'branded' }: Props) {
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

  // Returns the point if it is inside clipRect (or if no clipRect is set),
  // null otherwise. Used to gate BEGIN_CREATE so the user can't start a new
  // rectangle outside the selected window in window mode.
  const insideClip = (x: number, y: number): { x: number; y: number } | null => {
    if (!clipRect) return { x, y };
    const inside =
      x >= clipRect.x &&
      x < clipRect.x + clipRect.w &&
      y >= clipRect.y &&
      y < clipRect.y + clipRect.h;
    return inside ? { x, y } : null;
  };

  const onMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (e.target !== e.currentTarget) return;
      if (e.button !== 0) return;
      e.preventDefault();
      const { x, y } = eventCoords(e);
      const startInside = insideClip(x, y);
      if (!startInside) {
        // Click on the dimmed area outside the selected window: cancel or
        // prompt to discard, mirroring the veil's intent.
        onOutsideClick?.();
        return;
      }
      dispatch({ type: "BEGIN_CREATE", id: nextRectId(), x: startInside.x, y: startInside.y, kind: redactMode ? "redaction" : "annotation" });
    },
    [dispatch, eventCoords, clipRect, onOutsideClick, redactMode]
  );

  const onMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!state.active) return;
      let { x, y } = eventCoords(e);
      if (clipRect) {
        x = Math.max(clipRect.x, Math.min(x, clipRect.x + clipRect.w));
        y = Math.max(clipRect.y, Math.min(y, clipRect.y + clipRect.h));
      }
      dispatch({
        type: "POINTER_TO",
        x,
        y,
        viewportW: window.innerWidth,
        viewportH: window.innerHeight,
        clip: clipRect,
      });
    },
    [dispatch, eventCoords, state.active, clipRect],
  );

  const onMouseUp = useCallback(() => {
    if (!state.active) return;
    dispatch({ type: "END_GESTURE" });
  }, [dispatch, state.active]);

  const labels = annotationLabels(state.rects);

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
        cursor: PICKER_CURSOR_CSS,
        userSelect: "none",
        WebkitUserSelect: "none",
      }}
    >
      {state.rects.map((r, idx) => {
        const isRedaction = r.kind === "redaction";
        const label = labels[idx];
        return (
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
              border: isRedaction ? "1.5px solid rgba(238, 102, 51, 0.5)" : RECT_BORDER,
              background: isRedaction ? redactionPreviewBg(redactionStyle) : RECT_FILL,
              backgroundSize: isRedaction && redactionStyle !== "solid" ? "12px 12px" : undefined,
              boxSizing: "border-box",
              pointerEvents: "auto",
              cursor: "move",
              boxShadow: isRedaction ? "none" : "-2px 0 0 rgba(255, 0, 60, 0.4), 2px 0 0 rgba(0, 245, 255, 0.4)",
            }}
          >
            {r.w >= MIN_RECT_DIMENSION && r.h >= MIN_RECT_DIMENSION && (
              <>
                {!isRedaction && (
                  <div
                    aria-hidden
                    style={{
                      position: "absolute",
                      width: 22,
                      height: 14,
                      borderRadius: 7,
                      background: "#ee6633",
                      color: "#ffffff",
                      font: "bold 10px sans-serif",
                      lineHeight: 1,
                      fontVariantNumeric: "tabular-nums",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      pointerEvents: "none",
                      boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
                      top: 4,
                      left: 4,
                    }}
                  >
                    {label}
                  </div>
                )}
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
                    width: 20,
                    height: 20,
                    padding: 0,
                    border: "none",
                    background: "transparent",
                    color: "#ee6633",
                    font: "bold 20px sans-serif",
                    lineHeight: "20px",
                    textAlign: "center",
                    cursor: "pointer",
                    pointerEvents: "auto",
                    textShadow: "0 0 3px rgba(0,0,0,0.8), 0 1px 2px rgba(0,0,0,0.5)",
                    top: 2,
                    right: 2,
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
        )
      })}
      {state.rects.map((r) =>
        r.kind === "redaction" ? null : (
          <RectangleNote
            key={`${r.id}-note`}
            rect={r}
            containerHeight={containerHeight}
            autoFocus={!state.active && r.id === state.lastCreatedId}
            onChange={(note) => dispatch({ type: "SET_NOTE", id: r.id, note })}
          />
        ),
      )}
    </div>
  );
}
