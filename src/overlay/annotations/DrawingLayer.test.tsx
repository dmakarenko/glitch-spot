// ABOUTME: Tests for the DrawingLayer's clipRect prop — restricts rect creation
// ABOUTME: and drag to a sub-rectangle (used by window mode).

// @vitest-environment jsdom

import { describe, expect, it, afterEach, vi } from "vitest";
import { render, fireEvent, cleanup, act } from "@testing-library/react";
import { useReducer } from "react";
import { DrawingLayer } from "./DrawingLayer";
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

describe("DrawingLayer seeded create gesture", () => {
  // A seeded gesture starts from an external BEGIN_CREATE (e.g. the picker click)
  // rather than from a mousedown in the layer itself. It has `seeded: true` on
  // the active gesture and begins with a zero-size rect.
  const seededState = reducer(initialState, {
    type: "BEGIN_CREATE",
    id: "r1",
    x: 50,
    y: 50,
    seeded: true,
  } as Parameters<typeof reducer>[1]);

  it("rubber-bands the rect to the cursor on mousemove, even without a button held", () => {
    const { container } = render(<Harness initialAnnotations={seededState} />);
    const surface = container.firstChild as HTMLDivElement;
    // Move cursor without button held — rect should track the cursor.
    fireEvent.mouseMove(surface, { clientX: 200, clientY: 200, buttons: 0 });
    const rect = surface.querySelector("[data-rect-id]") as HTMLDivElement;
    expect(parseFloat(rect!.style.width)).toBeGreaterThan(0);
    expect(parseFloat(rect!.style.height)).toBeGreaterThan(0);
  });

  it("commits via a native document mouseup (cross-webview drag release)", () => {
    const { container } = render(<Harness initialAnnotations={seededState} />);
    const surface = container.firstChild as HTMLDivElement;
    // Grow the rect to a committable size
    fireEvent.mouseMove(surface, { clientX: 200, clientY: 200 });
    // Simulate the button release arriving as a native document event rather
    // than through React's onMouseUp on the div (cross-webview scenario).
    act(() => {
      document.dispatchEvent(new MouseEvent("mouseup"));
    });
    // Rect spans 50→200 = 150px on both axes — large enough to commit
    expect(surface.querySelectorAll("[data-rect-id]")).toHaveLength(1);
    // And state.active is null, so a fresh normal mousedown works
    fireEvent.mouseDown(surface, { clientX: 10, clientY: 10, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 60, clientY: 60 });
    fireEvent.mouseUp(surface);
    expect(surface.querySelectorAll("[data-rect-id]")).toHaveLength(2);
  });

  it("tracks the cursor when the button is held and commits on mouseup", () => {
    const { container } = render(<Harness initialAnnotations={seededState} />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseMove(surface, { clientX: 150, clientY: 150, buttons: 1 });
    fireEvent.mouseMove(surface, { clientX: 200, clientY: 200, buttons: 1 });
    fireEvent.mouseUp(surface);
    expect(surface.querySelectorAll("[data-rect-id]")).toHaveLength(1);
  });

  it("cancels the seeded gesture on mousedown and starts a fresh one at the clicked position", () => {
    const { container } = render(<Harness initialAnnotations={seededState} />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 200, clientY: 200 });
    fireEvent.mouseUp(surface);
    expect(surface.querySelectorAll("[data-rect-id]")).toHaveLength(1);
    const rect = surface.querySelector("[data-rect-id]") as HTMLDivElement;
    expect(parseFloat(rect.style.left)).toBe(100);
    expect(parseFloat(rect.style.top)).toBe(100);
  });
});
