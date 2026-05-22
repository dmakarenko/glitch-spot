// ABOUTME: Overlay window component that renders a captured screenshot as a full-screen backdrop.
// ABOUTME: Pulls the latest frame on mount, listens for fresh frames, dismisses on Esc or click.
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

type Frame = {
  width: number;
  height: number;
  scale_factor: number;
  png: number[];
  display_index: number;
};

function frameToObjectUrl(frame: Frame): string {
  const bytes = new Uint8Array(frame.png);
  const blob = new Blob([bytes], { type: "image/png" });
  return URL.createObjectURL(blob);
}

export function Overlay() {
  const [imageUrl, setImageUrl] = useState<string | null>(null);

  useEffect(() => {
    let currentUrl: string | null = null;
    const setFrame = (frame: Frame) => {
      const next = frameToObjectUrl(frame);
      if (currentUrl) URL.revokeObjectURL(currentUrl);
      currentUrl = next;
      setImageUrl(next);
    };

    // Pull the cached frame BEFORE wiring the listener. On the first hotkey
    // press the Rust side emits `glitch-spot://frame` while the webview is
    // still loading — that event would be dropped — so the backend also
    // caches the frame and we read it here.
    invoke<Frame | null>("get_latest_frame")
      .then((frame) => {
        if (frame) setFrame(frame);
      })
      .catch(() => {
        // No frame cached yet; the listener below will pick up the next one.
      });

    const unlisten = listen<Frame>("glitch-spot://frame", (event) => {
      setFrame(event.payload);
    });

    const dismiss = () => {
      invoke("hide_overlay");
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") dismiss();
    };
    window.addEventListener("keydown", onKey);

    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener("keydown", onKey);
      if (currentUrl) URL.revokeObjectURL(currentUrl);
    };
  }, []);

  // TODO(v0.1-followup): borderless transparent NSWindows on macOS may not
  // become the key window, which would prevent the Esc keydown handler above
  // from firing. Click-anywhere-to-dismiss is the reliable mouse-driven path
  // for v0.1; revisit by either flipping the NSWindow style mask or by adding
  // a Rust-side activate-and-makeKey call after `show()`.
  const onClick = () => {
    invoke("hide_overlay");
  };

  return (
    <div
      onClick={onClick}
      style={{
        position: "fixed",
        inset: 0,
        background: imageUrl
          ? `center / cover no-repeat url(${imageUrl})`
          : "rgba(0,0,0,0.05)",
      }}
    />
  );
}
