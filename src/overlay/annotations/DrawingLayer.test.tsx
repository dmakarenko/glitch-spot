// ABOUTME: Tests for the DrawingLayer's clipRect prop — restricts rect creation
// ABOUTME: and drag to a sub-rectangle (used by window mode).

// @vitest-environment jsdom

import { describe, expect, it, afterEach, vi } from "vitest";
import { render, fireEvent, cleanup } from "@testing-library/react";
import { useReducer } from "react";
import { DrawingLayer, nextRectId } from "./DrawingLayer";
import { initialState, reducer } from "./state";
import type { AnnotationsState } from "./types";

afterEach(cleanup);

function Harness({
  clipRect,
  onOutsideClick,
  initialAnnotations,
}: {
  clipRect?: { x: number; y: number; w: number; h: number };
  onOutsideClick?: () => void;
  initialAnnotations?: AnnotationsState;
}) {
  const [state, dispatch] = useReducer(reducer, initialAnnotations ?? initialState);
  return (
    <DrawingLayer
      state={state}
      dispatch={dispatch}
      clipRect={clipRect}
      onOutsideClick={onOutsideClick}
    />
  );
}

describe("DrawingLayer with clipRect", () => {
  it("ignores mouse-down outside clipRect", () => {
    const { container } = render(<Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 50, clientY: 50, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 60, clientY: 60 });
    fireEvent.mouseUp(surface);
    expect(surface.querySelectorAll('[data-rect-id]')).toHaveLength(0);
  });

  it("starts a rect when mouse-down inside clipRect", () => {
    const { container } = render(<Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 150, clientY: 150, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 200, clientY: 200 });
    fireEvent.mouseUp(surface);
    const rects = surface.querySelectorAll('[data-rect-id]');
    expect(rects.length).toBe(1);
  });

  it("clamps rect drag to clipRect bounds", () => {
    const { container } = render(<Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 150, clientY: 150, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 500, clientY: 500 });
    fireEvent.mouseUp(surface);
    const rect = surface.querySelector('[data-rect-id]') as HTMLDivElement | null;
    expect(rect).toBeTruthy();
    const left = parseFloat(rect!.style.left);
    const width = parseFloat(rect!.style.width);
    expect(left + width).toBeLessThanOrEqual(300);
  });

  it("when clipRect is undefined, behaves like the existing layer (no clamp)", () => {
    const { container } = render(<Harness />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 10, clientY: 10, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 100, clientY: 100 });
    fireEvent.mouseUp(surface);
    expect(surface.querySelectorAll('[data-rect-id]').length).toBe(1);
  });

  it("calls onOutsideClick when mouse-down lands outside clipRect", () => {
    const onOutsideClick = vi.fn();
    const { container } = render(
      <Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} onOutsideClick={onOutsideClick} />,
    );
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 50, clientY: 50, button: 0 });
    expect(onOutsideClick).toHaveBeenCalledTimes(1);
    expect(surface.querySelectorAll('[data-rect-id]')).toHaveLength(0);
  });

  it("does not call onOutsideClick when mouse-down is inside clipRect", () => {
    const onOutsideClick = vi.fn();
    const { container } = render(
      <Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} onOutsideClick={onOutsideClick} />,
    );
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 150, clientY: 150, button: 0 });
    expect(onOutsideClick).not.toHaveBeenCalled();
  });

  it("clamps a moved rect to clipRect bounds (not just the pointer)", () => {
    const { container } = render(<Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} />);
    const surface = container.firstChild as HTMLDivElement;
    // Create a 100x100 rect at the clip's top-left corner.
    fireEvent.mouseDown(surface, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 200, clientY: 200 });
    fireEvent.mouseUp(surface);
    const rect = surface.querySelector('[data-rect-id]') as HTMLDivElement;
    expect(rect).toBeTruthy();
    // Grab the rect at its top-left and drag far past the clip's bottom-right.
    fireEvent.mouseDown(rect, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 500, clientY: 500 });
    fireEvent.mouseUp(surface);
    const left = parseFloat(rect.style.left);
    const top = parseFloat(rect.style.top);
    const width = parseFloat(rect.style.width);
    const height = parseFloat(rect.style.height);
    // The moved rect must stay fully within the clip (100..300 on both axes).
    expect(left).toBeGreaterThanOrEqual(100);
    expect(top).toBeGreaterThanOrEqual(100);
    expect(left + width).toBeLessThanOrEqual(300);
    expect(top + height).toBeLessThanOrEqual(300);
  });
});

describe("DrawingLayer cursor", () => {
  it("uses the glitch crosshair cursor rather than the plain CSS crosshair", () => {
    const { container } = render(<Harness />);
    const surface = container.firstChild as HTMLDivElement;
    expect(surface.style.cursor).not.toBe("crosshair");
    expect(surface.style.cursor).toContain("data:image/svg+xml");
  });
});

function redactionState(_style: "pixelated" | "branded" | "solid"): AnnotationsState {
  const id = nextRectId();
  return {
    rects: [{ id, x: 20, y: 20, w: 120, h: 80, note: "", kind: "redaction" }],
    history: [],
    active: null,
    lastCreatedId: id,
  };
}

function RedactionHarness({ style }: { style: "pixelated" | "branded" | "solid" }) {
  const [state, dispatch] = useReducer(reducer, redactionState(style));
  return (
    <DrawingLayer
      state={state}
      dispatch={dispatch}
      redactMode
      redactionStyle={style}
    />
  );
}

describe("DrawingLayer pixelated redaction preview", () => {
  it("does not paint the mosaic background for pixelated rects", () => {
    const { container } = render(<RedactionHarness style="pixelated" />);
    const rect = container.querySelector('[data-rect-id]') as HTMLDivElement;
    // No grey-checker mosaic and no 12px tiling — the live canvas owns the fill.
    expect(rect.style.background).not.toContain("svg");
    expect(rect.style.backgroundSize).toBe("");
  });

  it("still paints the mosaic background for branded rects", () => {
    const { container } = render(<RedactionHarness style="branded" />);
    const rect = container.querySelector('[data-rect-id]') as HTMLDivElement;
    expect(rect.style.background).toContain("svg");
    expect(rect.style.backgroundSize).toBe("12px 12px");
  });

  it("paints a solid fill for solid rects", () => {
    const { container } = render(<RedactionHarness style="solid" />);
    const rect = container.querySelector('[data-rect-id]') as HTMLDivElement;
    // jsdom decomposes `background: "#241f1b"` into backgroundColor; check either.
    const fill = rect.style.background || rect.style.backgroundColor;
    expect(fill).toContain("rgb(36, 31, 27)");
  });
});

describe("PixelatedRedactionFill live sampling", () => {
  it("renders the canvas and paints one averaged block-fill per block from the sampled source", () => {
    // One fake 2D context backs both the offscreen source canvas and the pixel
    // canvas. getImageData yields all-white pixels → every block mean is 255.
    const fills: Array<{ style: string; x: number; y: number; w: number; h: number }> = [];
    let currentStyle = "";
    const fakeCtx = {
      drawImage: () => {},
      clearRect: () => {},
      getImageData: (_x: number, _y: number, w: number, h: number) => ({
        data: new Uint8ClampedArray(w * h * 4).fill(255),
        width: w,
        height: h,
      }),
      set fillStyle(v: string) { currentStyle = v; },
      get fillStyle() { return currentStyle; },
      fillRect: (x: number, y: number, w: number, h: number) => fills.push({ style: currentStyle, x, y, w, h }),
    };
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      fakeCtx as unknown as CanvasRenderingContext2D,
    );
    class FakeImage {
      naturalWidth = 100; naturalHeight = 100; onload: (() => void) | null = null;
      set src(_s: string) { this.onload?.(); }
    }
    vi.stubGlobal("Image", FakeImage);
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => { cb(0); return 1; });
    vi.stubGlobal("cancelAnimationFrame", () => {});

    const id = nextRectId();
    const state: AnnotationsState = {
      rects: [{ id, x: 10, y: 20, w: 48, h: 36, note: "", kind: "redaction" }],
      history: [],
      active: null,
      lastCreatedId: id,
    };
    function Harness() {
      const [s, dispatch] = useReducer(reducer, state);
      return (
        <DrawingLayer
          state={s}
          dispatch={dispatch}
          redactMode
          redactionStyle="pixelated"
          sourceImageUrl="blob:x"
          imageRect={{ x: 0, y: 0, w: 100, h: 100 }}
        />
      );
    }
    const { container } = render(<Harness />);

    // Live canvas renders for the pixelated redaction once the source decodes.
    expect(container.querySelector('[data-testid="redaction-pixel-canvas"]')).not.toBeNull();

    // scale = source.width(100) / imageRect.w(100) = 1 → block = 12.
    // 48×36 rect → 4 cols × 3 rows = 12 blocks, each the white mean, first at (0,0).
    expect(fills.length).toBe(12);
    expect(fills.every((f) => f.style === "rgba(255, 255, 255, 1.000)")).toBe(true);
    expect(fills[0]).toMatchObject({ x: 0, y: 0, w: 12, h: 12 });

    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });
});

