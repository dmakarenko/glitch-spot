// ABOUTME: Floating textarea anchored to a rectangle. Auto-positions below the rect,
// ABOUTME: or above if it would overflow the viewport.

import { useEffect, useRef } from "react";
import type { Rect } from "./types";

const NOTE_WIDTH = 240;
const NOTE_OFFSET = 6;
const NOTE_MIN_HEIGHT = 56;

type Props = {
  rect: Rect;
  containerHeight: number;
  onChange: (note: string) => void;
  autoFocus: boolean;
};

export function RectangleNote({ rect, containerHeight, onChange, autoFocus }: Props) {
  const ref = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);

  // Auto-flip vertically if placing below would overflow.
  const belowY = rect.y + rect.h + NOTE_OFFSET;
  const wouldOverflowBelow = belowY + NOTE_MIN_HEIGHT > containerHeight;
  const top = wouldOverflowBelow ? rect.y - NOTE_MIN_HEIGHT - NOTE_OFFSET : belowY;

  // Keep within left/right bounds.
  const left = Math.max(0, Math.min(rect.x, window.innerWidth - NOTE_WIDTH));

  return (
    <textarea
      ref={ref}
      value={rect.note}
      onChange={(e) => onChange(e.target.value)}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      placeholder="Note (optional)"
      maxLength={500}
      rows={3}
      style={{
        position: "absolute",
        top,
        left,
        width: NOTE_WIDTH,
        minHeight: NOTE_MIN_HEIGHT,
        padding: "6px 8px",
        font: "13px/1.4 system-ui",
        background: "rgba(255,255,255,0.96)",
        color: "#111",
        border: "1px solid rgba(0,0,0,0.18)",
        borderRadius: 6,
        boxShadow: "0 4px 16px rgba(0,0,0,0.18)",
        outline: "none",
        resize: "none",
      }}
    />
  );
}
