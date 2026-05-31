// ABOUTME: Per-display picker overlay. Hover is driven by a Rust-side cursor
// ABOUTME: poller broadcasting `picker://hover` events — macOS only delivers
// ABOUTME: mouseMoved to the key window, so JS mousemove can't drive the
// ABOUTME: non-activation display.
import { useEffect, useState } from "react";
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

function drawRect(ds: DrawState) {
  return {
    x: Math.min(ds.anchorX, ds.curX),
    y: Math.min(ds.anchorY, ds.curY),
    w: Math.abs(ds.curX - ds.anchorX),
    h: Math.abs(ds.curY - ds.anchorY),
  };
}

export function Picker() {
  const [session, setSession] = useState<PickerStartPayload | null>(null);
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const [drawState, setDrawState] = useState<DrawState | null>(null);

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

  const onMouseUp = () => {
    if (!drawState) return;
    const { windowId, anchorX, anchorY } = drawState;
    const rect = drawRect(drawState);
    if (rect.w >= MIN_ANNOTATION_PX && rect.h >= MIN_ANNOTATION_PX) {
      invoke("picker_select", {
        windowId,
        initialRectX: Math.round(picker_origin.x + rect.x * picker_scale),
        initialRectY: Math.round(picker_origin.y + rect.y * picker_scale),
        initialRectW: Math.round(rect.w * picker_scale),
        initialRectH: Math.round(rect.h * picker_scale),
      });
    } else {
      invoke("picker_select", { windowId });
    }
    void anchorX; void anchorY; // consumed above
    setDrawState(null);
  };

  const highlight = !drawState && session.windows.find((w) => w.window_id === hoveredId);

  // Window being drawn — needed for the checker veil.
  const drawingWindow = drawState
    ? session.windows.find((w) => w.window_id === drawState.windowId)
    : null;
  const drawingWindowCss = drawingWindow
    ? {
        x: (drawingWindow.bounds.x - picker_origin.x) / picker_scale,
        y: (drawingWindow.bounds.y - picker_origin.y) / picker_scale,
        w: drawingWindow.bounds.w / picker_scale,
        h: drawingWindow.bounds.h / picker_scale,
      }
    : null;
  const activeRect = drawState ? drawRect(drawState) : null;

  return (
    <div
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
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
