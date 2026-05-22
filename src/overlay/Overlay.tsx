// ABOUTME: Overlay window component. Supports a live transparent overlay (default)
// ABOUTME: and a frozen mode that displays a captured screenshot as backdrop.
import { useEffect, useReducer, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { DrawingLayer } from "./annotations/DrawingLayer";
import { DiscardModal } from "./annotations/DiscardModal";
import { initialState, reducer } from "./annotations/state";
import { Toolbar } from "./Toolbar";

type Frame = {
  width: number;
  height: number;
  scale_factor: number;
  png: number[];
  display_index: number;
};

type Mode = "live" | "frozen";

// Photoshop-style transparency checker drawn at low opacity so the overlay is
// visually distinguishable from the real desktop without obscuring it.
const CHECKER_BG =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'>" +
  "<rect width='10' height='10' fill='%23999999'/>" +
  "<rect x='10' y='10' width='10' height='10' fill='%23999999'/>" +
  "<rect x='10' y='0' width='10' height='10' fill='%23ffffff'/>" +
  "<rect x='0' y='10' width='10' height='10' fill='%23ffffff'/>" +
  "</svg>\")";

function frameToObjectUrl(frame: Frame): string {
  const bytes = new Uint8Array(frame.png);
  const blob = new Blob([bytes], { type: "image/png" });
  return URL.createObjectURL(blob);
}

export function Overlay() {
  // The Rust side calls __glitchSpotPresent via WebviewWindow::eval before
  // showing the window, so by the time we render the first frame for a new
  // session the mode is correct and imageUrl is clean.
  const [mode, setMode] = useState<Mode>("live");
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [annotations, dispatch] = useReducer(reducer, initialState);
  const [showDiscardModal, setShowDiscardModal] = useState(false);

  useEffect(() => {
    let currentUrl: string | null = null;

    (
      window as unknown as { __glitchSpotPresent?: (mode: Mode) => void }
    ).__glitchSpotPresent = (newMode) => {
      if (currentUrl) URL.revokeObjectURL(currentUrl);
      currentUrl = null;
      setImageUrl(null);
      setMode(newMode);
    };

    const setFrame = (frame: Frame) => {
      // Frames are only ever produced by the frozen-overlay path. If we are
      // receiving one then the user definitely pressed the frozen hotkey,
      // even when the Rust-side `__glitchSpotPresent('frozen')` eval was a
      // no-op (e.g. on the first ever show, before useEffect has installed
      // the global, or after a webview reload). Force frozen mode here so
      // the backdrop actually renders the screenshot.
      setMode("frozen");

      const next = frameToObjectUrl(frame);
      // Decode off-screen before swapping the visible background so the new
      // image appears atomically rather than after a perceptible decode delay.
      const img = new window.Image();
      img.onload = () => {
        if (currentUrl) URL.revokeObjectURL(currentUrl);
        currentUrl = next;
        setImageUrl(next);
      };
      img.onerror = () => {
        URL.revokeObjectURL(next);
      };
      img.src = next;
    };

    invoke<Frame | null>("get_latest_frame")
      .then((frame) => {
        if (frame) setFrame(frame);
      })
      .catch(() => {
        /* no cached frame yet; the listener below picks up the next one */
      });

    const unlisten = listen<Frame>("glitch-spot://frame", (event) => {
      setFrame(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
      if (currentUrl) URL.revokeObjectURL(currentUrl);
      delete (window as unknown as { __glitchSpotPresent?: (mode: Mode) => void })
        .__glitchSpotPresent;
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        if (annotations.rects.length === 0) {
          invoke("hide_overlay");
        } else {
          setShowDiscardModal(true);
        }
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        dispatch({ type: "UNDO" });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [annotations.rects.length]);

  // In live mode, the body stays transparent — the user sees the real desktop
  // through the overlay. In frozen mode, the cropped screenshot fills the
  // overlay 1:1 (capture is sized to the work area).
  const backdrop =
    mode === "frozen" && imageUrl
      ? `center / 100% 100% no-repeat url(${imageUrl})`
      : "transparent";

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: backdrop,
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage: CHECKER_BG,
          backgroundRepeat: "repeat",
          opacity: 0.12,
          pointerEvents: "none",
        }}
      />
      <DrawingLayer state={annotations} dispatch={dispatch} />
      <Toolbar rectCount={annotations.rects.length} onSave={() => { /* M11 wires this */ }} />
      {showDiscardModal && (
        <DiscardModal
          onDiscard={() => {
            setShowDiscardModal(false);
            dispatch({ type: "CLEAR" });
            invoke("hide_overlay");
          }}
          onSave={() => { setShowDiscardModal(false); /* M11 wires the save flow */ }}
          onCancel={() => setShowDiscardModal(false)}
        />
      )}
    </div>
  );
}
