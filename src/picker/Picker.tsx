// ABOUTME: Per-display picker overlay. Hover is driven by a Rust-side cursor
// ABOUTME: poller broadcasting `picker://hover` events — macOS only delivers
// ABOUTME: mouseMoved to the key window, so JS mousemove can't drive the
// ABOUTME: non-activation display.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PICKER_CURSOR_CSS } from "./cursor";
import type { PickerStartPayload, WindowBounds } from "./types";

// Minimum size (in CSS px) for a drag to be treated as an intentional rect.
// Matches DrawingLayer's MIN_RECT_DIMENSION so the rect always survives the
// overlay's size check when it arrives pre-committed.
const MIN_ANNOTATION_PX = 25;

// Checker pattern for the window area (same as overlay's window-mode veil).
const CHECKER_BG =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'>" +
  "<rect width='10' height='10' fill='%23999999'/>" +
  "<rect x='10' y='10' width='10' height='10' fill='%23999999'/>" +
  "<rect x='10' y='0' width='10' height='10' fill='%23ffffff'/>" +
  "<rect x='0' y='10' width='10' height='10' fill='%23ffffff'/>" +
  "</svg>\")";

type DrawState = {
  windowId: number;
  anchorX: number;
  anchorY: number;
  curX: number;
  curY: number;
};

type CssRect = { x: number; y: number; w: number; h: number };

function rawRect(ds: DrawState): CssRect {
  return {
    x: Math.min(ds.anchorX, ds.curX),
    y: Math.min(ds.anchorY, ds.curY),
    w: Math.abs(ds.curX - ds.anchorX),
    h: Math.abs(ds.curY - ds.anchorY),
  };
}

function clampRect(rect: CssRect, bounds: CssRect): CssRect {
  const x = Math.max(bounds.x, rect.x);
  const y = Math.max(bounds.y, rect.y);
  const w = Math.min(bounds.x + bounds.w, rect.x + rect.w) - x;
  const h = Math.min(bounds.y + bounds.h, rect.y + rect.h) - y;
  return { x, y, w, h };
}

export function Picker() {
  const [session, setSession] = useState<PickerStartPayload | null>(null);
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const [drawState, setDrawState] = useState<DrawState | null>(null);

  // Always-current ref so the global mouseup handler never reads stale state.
  const drawStateRef = useRef<DrawState | null>(null);
  drawStateRef.current = drawState;

  // Receive the session payload from Rust via window.eval-injected setter.
  useEffect(() => {
    (window as unknown as { __glitchSpotPickerStart?: (p: PickerStartPayload) => void })
      .__glitchSpotPickerStart = (p) => {
        setSession(p);
        setHoveredId(null);
        setDrawState(null);
      };
    return () => {
      delete (window as unknown as { __glitchSpotPickerStart?: unknown }).__glitchSpotPickerStart;
    };
  }, []);

  // Esc cancels.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        invoke("picker_cancel");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Cancel when the user swipes to another Space. macOS hides the picker
  // window (we don't set canJoinAllSpaces), which transitions the document's
  // visibilityState to "hidden". Resetting state on that lets a fresh
  // ⌃⌥⌘J on the new Space start a fresh picker session.
  useEffect(() => {
    if (!session) return;
    const onVisibilityChange = () => {
      if (document.visibilityState === "hidden") {
        invoke("picker_cancel");
      }
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => document.removeEventListener("visibilitychange", onVisibilityChange);
  }, [session]);

  // Subscribe to Rust-driven hover updates while a session is active. One
  // emitter broadcasts to every picker, so each picker just mirrors the
  // current window_id.
  useEffect(() => {
    if (!session) return;
    let unlisten: UnlistenFn | undefined;
    listen<{ window_id: number | null }>("picker://hover", (event) => {
      setHoveredId(event.payload.window_id);
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, [session]);

  // Global mouseup listener — catches releases outside the picker div (e.g.
  // dragging across a display boundary onto another picker window). Installed
  // only while a draw gesture is active; reads from drawStateRef so it always
  // sees the latest cursor position even though the closure is created once.
  const isDrawing = drawState !== null;
  useEffect(() => {
    if (!isDrawing || !session) return;

    const { picker_origin, picker_scale } = session;
    // Capture the window's CSS bounds at gesture start — they don't change
    // during the draw and we need them for clamping.
    const win = session.windows.find((w) => w.window_id === drawStateRef.current?.windowId);
    const windowCss: CssRect | null = win
      ? {
          x: (win.bounds.x - picker_origin.x) / picker_scale,
          y: (win.bounds.y - picker_origin.y) / picker_scale,
          w: win.bounds.w / picker_scale,
          h: win.bounds.h / picker_scale,
        }
      : null;

    const commit = () => {
      const ds = drawStateRef.current;
      if (!ds) return;
      const raw = rawRect(ds);
      const rect = windowCss ? clampRect(raw, windowCss) : raw;
      if (rect.w >= MIN_ANNOTATION_PX && rect.h >= MIN_ANNOTATION_PX) {
        invoke("picker_select", {
          windowId: ds.windowId,
          initialRectX: Math.round(picker_origin.x + rect.x * picker_scale),
          initialRectY: Math.round(picker_origin.y + rect.y * picker_scale),
          initialRectW: Math.round(rect.w * picker_scale),
          initialRectH: Math.round(rect.h * picker_scale),
        });
      } else {
        invoke("picker_select", { windowId: ds.windowId });
      }
      setDrawState(null);
    };

    document.addEventListener("mouseup", commit);
    return () => document.removeEventListener("mouseup", commit);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isDrawing]);

  if (!session) return null;

  const { picker_origin, picker_scale } = session;

  const onMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    if (hoveredId == null) {
      invoke("picker_cancel");
      return;
    }
    setDrawState({ windowId: hoveredId, anchorX: e.clientX, anchorY: e.clientY, curX: e.clientX, curY: e.clientY });
  };

  const onMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!drawState) return;
    setDrawState((prev) => prev ? { ...prev, curX: e.clientX, curY: e.clientY } : null);
  };

  const highlight = !drawState && session.windows.find((w) => w.window_id === hoveredId);

  // Window being drawn — for the visual veil and clamped preview rect.
  const drawingWindow = drawState
    ? session.windows.find((w) => w.window_id === drawState.windowId)
    : null;
  const drawingWindowCss: CssRect | null = drawingWindow
    ? {
        x: (drawingWindow.bounds.x - picker_origin.x) / picker_scale,
        y: (drawingWindow.bounds.y - picker_origin.y) / picker_scale,
        w: drawingWindow.bounds.w / picker_scale,
        h: drawingWindow.bounds.h / picker_scale,
      }
    : null;
  // Clamp the preview rect to the window so the visual matches what gets committed.
  const rawActive = drawState ? rawRect(drawState) : null;
  const activeRect = rawActive && drawingWindowCss ? clampRect(rawActive, drawingWindowCss) : rawActive;

  return (
    <div
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      style={{
        position: "fixed",
        inset: 0,
        cursor: PICKER_CURSOR_CSS,
        background: "transparent",
      }}
      data-testid="picker-root"
    >
      {/* Hover highlights — hidden once drawing starts */}
      {highlight &&
        highlight.visible_regions.map((region, i) => (
          <HighlightRegion
            key={i}
            bounds={region}
            pickerOriginX={picker_origin.x}
            pickerOriginY={picker_origin.y}
            scale={picker_scale}
          />
        ))}

      {/* Window-mode overlay rendered inside the picker during drag-to-draw */}
      {drawState && (
        <>
          <div
            style={{ position: "absolute", inset: 0, background: "rgba(0,0,0,0.40)", pointerEvents: "none" }}
          />
          {drawingWindowCss && (
            <div
              style={{
                position: "absolute",
                left: drawingWindowCss.x,
                top: drawingWindowCss.y,
                width: drawingWindowCss.w,
                height: drawingWindowCss.h,
                backgroundImage: CHECKER_BG,
                backgroundRepeat: "repeat",
                opacity: 0.12,
                pointerEvents: "none",
              }}
            />
          )}
          {activeRect && activeRect.w > 0 && activeRect.h > 0 && (
            <div
              style={{
                position: "absolute",
                left: activeRect.x,
                top: activeRect.y,
                width: activeRect.w,
                height: activeRect.h,
                border: "1.5px solid rgba(238, 102, 51, 0.9)",
                background: "rgba(238, 102, 51, 0.05)",
                boxSizing: "border-box",
                boxShadow: "-2px 0 0 rgba(255, 0, 60, 0.4), 2px 0 0 rgba(0, 245, 255, 0.4)",
                pointerEvents: "none",
              }}
            />
          )}
        </>
      )}
    </div>
  );
}

const CYAN_CHECKER =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'>" +
  "<rect width='10' height='10' fill='%2300f5ff' fill-opacity='0.28'/>" +
  "<rect x='10' y='10' width='10' height='10' fill='%2300f5ff' fill-opacity='0.28'/>" +
  "<rect x='10' y='0' width='10' height='10' fill='%23ffffff' fill-opacity='0.12'/>" +
  "<rect x='0' y='10' width='10' height='10' fill='%23ffffff' fill-opacity='0.12'/>" +
  "</svg>\")";

function HighlightRegion({
  bounds,
  pickerOriginX,
  pickerOriginY,
  scale,
}: {
  bounds: WindowBounds;
  pickerOriginX: number;
  pickerOriginY: number;
  scale: number;
}) {
  // Convert physical-px global bounds → picker-local CSS px.
  const left = (bounds.x - pickerOriginX) / scale;
  const top = (bounds.y - pickerOriginY) / scale;
  const width = bounds.w / scale;
  const height = bounds.h / scale;
  return (
    <div
      data-testid="picker-highlight"
      style={{
        position: "absolute",
        left,
        top,
        width,
        height,
        backgroundImage: CYAN_CHECKER,
        backgroundRepeat: "repeat",
        pointerEvents: "none",
      }}
    />
  );
}
