// ABOUTME: Floating textarea anchored to a rectangle. Auto-positions below the rect,
// ABOUTME: or above if it would overflow the viewport.

import { useEffect, useRef } from "react";
import type { Rect } from "./types";
import { noteVariant } from "./noteVariant";

const NOTE_WIDTH = 240;
const NOTE_OFFSET = 6;
const NOTE_MIN_HEIGHT = 56;

// Translucent backgrounds rotated per annotation: neutral, warm red, cool cyan.
// Paired with a blur so the captured backdrop reads softly behind the text.
const VARIANT_BACKGROUNDS = [
  "rgba(22, 22, 22, 0.80)",
  "rgba(28, 18, 16, 0.80)",
  "rgba(16, 22, 24, 0.80)",
] as const;
const NOTE_BLUR = "blur(8px)";

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

  const background = VARIANT_BACKGROUNDS[noteVariant(rect.id)];

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
        background,
        backdropFilter: NOTE_BLUR,
        WebkitBackdropFilter: NOTE_BLUR,
        color: "#aaaaaa",
        border: "1px solid rgba(238, 102, 51, 0.28)",
        borderRadius: 6,
        boxShadow: "0 2px 10px rgba(0,0,0,0.5)",
        outline: "none",
        resize: "none",
        userSelect: "text",
        WebkitUserSelect: "text",
      }}
    />
  );
}
