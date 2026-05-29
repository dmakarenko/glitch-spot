// ABOUTME: Per-display picker overlay. Hover is driven by a Rust-side cursor
// ABOUTME: poller broadcasting `picker://hover` events — macOS only delivers
// ABOUTME: mouseMoved to the key window, so JS mousemove can't drive the
// ABOUTME: non-activation display.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PICKER_CURSOR_CSS } from "./cursor";
import type { PickerStartPayload, PickableWindow } from "./types";

export function Picker() {
  const [session, setSession] = useState<PickerStartPayload | null>(null);
  const [hoveredId, setHoveredId] = useState<number | null>(null);

  // Receive the session payload from Rust via window.eval-injected setter.
  useEffect(() => {
    (window as unknown as { __glitchSpotPickerStart?: (p: PickerStartPayload) => void })
      .__glitchSpotPickerStart = (p) => {
        setSession(p);
        setHoveredId(null);
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

  const onClick = () => {
    if (hoveredId == null) {
      invoke("picker_cancel");
      return;
    }
    invoke("picker_select", { windowId: hoveredId });
  };

  const highlight = session.windows.find((w) => w.window_id === hoveredId);

  return (
    <div
      onClick={onClick}
      style={{
        position: "fixed",
        inset: 0,
        cursor: PICKER_CURSOR_CSS,
        background: "transparent",
      }}
      data-testid="picker-root"
    >
      {highlight && (
        <HighlightRect
          window={highlight}
          pickerOriginX={picker_origin.x}
          pickerOriginY={picker_origin.y}
          scale={picker_scale}
        />
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

function HighlightRect({
  window: w,
  pickerOriginX,
  pickerOriginY,
  scale,
}: {
  window: PickableWindow;
  pickerOriginX: number;
  pickerOriginY: number;
  scale: number;
}) {
  // Convert window's global physical-px bounds → picker-local CSS px.
  const left = (w.bounds.x - pickerOriginX) / scale;
  const top = (w.bounds.y - pickerOriginY) / scale;
  const width = w.bounds.w / scale;
  const height = w.bounds.h / scale;
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
