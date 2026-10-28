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
  it("falls back to the grey checker mosaic for pixelated rects with no live source", () => {
    // Pure live capture has no decoded backdrop to sample, so the live canvas
    // can't render — fall back to the grey checker so the rect still reads as
    // redacted rather than showing only a border.
    const { container } = render(<RedactionHarness style="pixelated" />);
    const rect = container.querySelector('[data-rect-id]') as HTMLDivElement;
    expect(rect.style.background).toContain("svg");
    expect(rect.style.background).toContain("777777");
    expect(rect.style.backgroundSize).toBe("12px 12px");
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

// One fake 2D context backs both the offscreen source canvas and the pixel
// canvas. getImageData yields red and green from pixelAt (white by default).
function stubLiveSource(pixelAt: (x: number, y: number) => [number, number] = () => [255, 255]) {
  const fills: Array<{ style: string; x: number; y: number; w: number; h: number }> = [];
  const reads = { count: 0 };
  let currentStyle = "";
  const fakeCtx = {
    drawImage: () => {},
    clearRect: () => {},
    getImageData: (x0: number, y0: number, w: number, h: number) => {
      reads.count++;
      const data = new Uint8ClampedArray(w * h * 4).fill(255);
      for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) [data[(y * w + x) * 4], data[(y * w + x) * 4 + 1]] = pixelAt(x0 + x, y0 + y);
      }
      return { data, width: w, height: h };
    },
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
  return { fills, reads };
}

function LivePixelatedHarness({
  withRect = true,
  imageRect = { x: 0, y: 0, w: 100, h: 100 },
}: {
  withRect?: boolean;
  imageRect?: { x: number; y: number; w: number; h: number };
}) {
  const id = "live-pixelated";
  const [s, dispatch] = useReducer(reducer, {
    rects: withRect ? [{ id, x: 10, y: 20, w: 48, h: 36, note: "", kind: "redaction" }] : [],
    history: [],
    active: null,
    lastCreatedId: withRect ? id : null,
  } satisfies AnnotationsState);
  return (
    <DrawingLayer
      state={s}
      dispatch={dispatch}
      redactMode
      redactionStyle="pixelated"
      sourceImageUrl="blob:x"
      imageRect={imageRect}
    />
  );
}

function dragTenMoves(container: HTMLElement) {
  const surface = container.firstChild as HTMLDivElement;
  const rect = container.querySelector("[data-rect-id]") as HTMLDivElement;
  fireEvent.mouseDown(rect, { clientX: 20, clientY: 30, button: 0 });
  for (let i = 1; i <= 10; i++) fireEvent.mouseMove(surface, { clientX: 20 + i, clientY: 30 + i });
  fireEvent.mouseUp(surface);
}

describe("PixelatedRedactionFill live sampling", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("renders the canvas and paints one averaged block-fill per block from the sampled source", () => {
    const { fills } = stubLiveSource();
    const { container } = render(<LivePixelatedHarness />);

    // Live canvas renders for the pixelated redaction once the source decodes.
    expect(container.querySelector('[data-testid="redaction-pixel-canvas"]')).not.toBeNull();

    // scale = source.width(100) / imageRect.w(100) = 1 → block = 12.
    // 48×36 rect → 4 cols × 3 rows = 12 blocks, each the white mean, first at (0,0).
    expect(fills.length).toBe(12);
    expect(fills.every((f) => f.style === "rgba(255, 255, 255, 1.000)")).toBe(true);
    expect(fills[0]).toMatchObject({ x: 0, y: 0, w: 12, h: 12 });
  });

  it("samples the source pixels under the rect", () => {
    // Red = source x, green = source y. Rect starts at (10, 20) with block 12,
    // so the first block covers x 10..21 (mean 15) and y 20..31 (mean 25).
    const { fills } = stubLiveSource((x, y) => [x, y]);
    render(<LivePixelatedHarness />);
    expect(fills[0].style).toBe("rgba(15, 25, 255, 1.000)");
    expect(fills[1].style).toBe("rgba(27, 25, 255, 1.000)");
  });

  it("samples from the clipped origin when the rect extends past the image's top-left", () => {
    // Image starts at (20, 30), rect at (10, 20): 10 px of the rect lie outside on
    // each axis, so the first block paints at (10, 10) from source x 0..11, y 0..11 (mean 5).
    const { fills } = stubLiveSource((x, y) => [x, y]);
    render(<LivePixelatedHarness imageRect={{ x: 20, y: 30, w: 100, h: 100 }} />);
    expect(fills[0]).toMatchObject({ style: "rgba(5, 5, 255, 1.000)", x: 10, y: 10, w: 12, h: 12 });
  });

  it("reads the frame back at decode, before any pixelated rect exists", () => {
    const { reads } = stubLiveSource();
    render(<LivePixelatedHarness withRect={false} />);
    expect(reads.count).toBe(1);
  });

  it("pixelate drag reads back once", () => {
    const { reads, fills } = stubLiveSource();
    const { container } = render(<LivePixelatedHarness />);
    const fillsBeforeDrag = fills.length;
    dragTenMoves(container);
    // The moves still repaint; only the source readback is shared.
    expect(fills.length).toBeGreaterThan(fillsBeforeDrag);
    expect(reads.count).toBeLessThanOrEqual(1);
  });

  it("pixelate drag keeps one canvas", () => {
    stubLiveSource();
    const widthSets = vi.spyOn(HTMLCanvasElement.prototype, "width", "set");
    const heightSets = vi.spyOn(HTMLCanvasElement.prototype, "height", "set");
    const { container } = render(<LivePixelatedHarness />);
    const widthBefore = widthSets.mock.calls.length;
    const heightBefore = heightSets.mock.calls.length;
    dragTenMoves(container);
    expect(widthSets.mock.calls.length - widthBefore).toBeLessThanOrEqual(1);
    expect(heightSets.mock.calls.length - heightBefore).toBeLessThanOrEqual(1);
  });
});
