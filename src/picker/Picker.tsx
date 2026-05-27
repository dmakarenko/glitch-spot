// ABOUTME: Per-display picker UI. Renders a transparent overlay with a custom
// ABOUTME: cursor; on mousemove hit-tests against the session's window list.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PICKER_CURSOR_CSS } from "./cursor";
import { hitTest } from "./hitTest";
import type { PickerStartPayload, PickableWindow, DisplayBounds } from "./types";

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

  if (!session) return null;

  const myDisplay: DisplayBounds | undefined = session.displays[session.my_display_index];
  const scale = myDisplay?.scale_factor ?? 1;

  const onMouseMove = (e: React.MouseEvent) => {
    if (!myDisplay) return;
    // Convert client (CSS px in *this* picker window) → global physical px.
    const globalPhysX = myDisplay.x + e.clientX * scale;
    const globalPhysY = myDisplay.y + e.clientY * scale;
    const id = hitTest(session.windows, globalPhysX, globalPhysY);
    setHoveredId(id);
  };

  const onMouseLeave = () => setHoveredId(null);

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
      onMouseMove={onMouseMove}
      onMouseLeave={onMouseLeave}
      onClick={onClick}
      style={{
        position: "fixed",
        inset: 0,
        cursor: PICKER_CURSOR_CSS,
        background: "transparent",
      }}
      data-testid="picker-root"
    >
      {highlight && myDisplay && (
        <HighlightRect window={highlight} display={myDisplay} />
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

function HighlightRect({ window: w, display }: { window: PickableWindow; display: DisplayBounds }) {
  const scale = display.scale_factor;
  // Convert window's global physical-px bounds → this picker's display-local CSS px.
  const left = (w.bounds.x - display.x) / scale;
  const top = (w.bounds.y - display.y) / scale;
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
