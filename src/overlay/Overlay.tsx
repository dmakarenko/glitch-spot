// ABOUTME: Overlay window component that renders a captured screenshot as a full-screen backdrop.
// ABOUTME: Listens for the glitch-spot://frame event and hides itself when Esc is pressed.
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

export function Overlay() {
  const [imageUrl, setImageUrl] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<Frame>("glitch-spot://frame", (event) => {
      const bytes = new Uint8Array(event.payload.png);
      const blob = new Blob([bytes], { type: "image/png" });
      setImageUrl((prev) => {
        if (prev) URL.revokeObjectURL(prev);
        return URL.createObjectURL(blob);
      });
    });

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") invoke("hide_overlay");
    };
    window.addEventListener("keydown", onKey);

    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  return (
    <div
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
