// ABOUTME: Unit tests for the pure constraint helpers used by the reducer when
// ABOUTME: moving or resizing an annotation rectangle.
import { describe, it, expect } from 'vitest';
import { clampMoveDelta, clampResize } from './transform';
import { MIN_RECT_DIMENSION } from './state';

const VW = 1000;
const VH = 800;

function rect(x: number, y: number, w: number, h: number) {
  return { x, y, w, h };
}

describe('clampMoveDelta', () => {
  it('applies the full delta when the result stays inside the viewport', () => {
    const out = clampMoveDelta({ x: 100, y: 100, w: 50, h: 50 }, 20, -30, { w: VW, h: VH });
    expect(out).toEqual({ x: 120, y: 70 });
  });

  it('clamps x at 0 when the rect would cross the left edge', () => {
    const out = clampMoveDelta({ x: 10, y: 100, w: 50, h: 50 }, -50, 0, { w: VW, h: VH });
    expect(out).toEqual({ x: 0, y: 100 });
  });

  it('clamps y at 0 when the rect would cross the top edge', () => {
    const out = clampMoveDelta({ x: 100, y: 10, w: 50, h: 50 }, 0, -50, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 0 });
  });

  it('clamps x so the rect stays inside the right edge', () => {
    const out = clampMoveDelta({ x: 900, y: 100, w: 200, h: 50 }, 500, 0, { w: VW, h: VH });
    expect(out).toEqual({ x: VW - 200, y: 100 });
  });

  it('clamps y so the rect stays inside the bottom edge', () => {
    const out = clampMoveDelta({ x: 100, y: 700, w: 50, h: 200 }, 0, 500, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: VH - 200 });
  });
});

describe('clampResize corner handles', () => {
  // rectStart is the rect at the start of the gesture; pointer is the new position
  // of the dragged corner.
  it('NW corner drag towards SE shrinks rect with SE anchor fixed', () => {
    const out = clampResize('nw', rect(0, 0, 500, 400), { x: 200, y: 100 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 200, y: 100, w: 300, h: 300 });
  });

  it('SE corner drag towards NW shrinks rect with NW anchor fixed', () => {
    const out = clampResize('se', rect(100, 100, 50, 50), { x: 400, y: 300 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 300, h: 200 });
  });

  it('NE corner drag', () => {
    const out = clampResize('ne', rect(100, 300, 100, 100), { x: 500, y: 100 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 400, h: 300 });
  });

  it('SW corner drag', () => {
    const out = clampResize('sw', rect(400, 100, 100, 100), { x: 100, y: 400 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 400, h: 300 });
  });

  it('stops at MIN_RECT_DIMENSION when the dragged corner approaches the anchor', () => {
    const out = clampResize('se', rect(100, 100, 50, 50), { x: 105, y: 110 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('refuses to flip past the anchor (SE corner dragged past NW)', () => {
    const out = clampResize('se', rect(100, 100, 50, 50), { x: 50, y: 50 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('refuses to flip past the anchor (NW corner dragged past SE)', () => {
    const out = clampResize('nw', rect(50, 50, 50, 50), { x: 200, y: 200 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100 - MIN_RECT_DIMENSION,
      y: 100 - MIN_RECT_DIMENSION,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('clamps the dragged corner inside the viewport (SE corner beyond right edge)', () => {
    const out = clampResize('se', rect(100, 100, 50, 50), { x: VW + 200, y: 300 }, { w: VW, h: VH });
    expect(out.x + out.w).toBe(VW);
  });

  it('clamps the dragged corner inside the viewport (NW corner beyond top edge)', () => {
    const out = clampResize('nw', rect(0, 0, 500, 400), { x: 200, y: -50 }, { w: VW, h: VH });
    expect(out.y).toBe(0);
    expect(out.y + out.h).toBe(400);
  });
});

describe('clampResize edge handles', () => {
  // Edge handles change a single dimension and preserve the orthogonal extent.
  // Anchor is the opposite edge: dragging N anchors the bottom, dragging E anchors the left, etc.

  describe('N edge', () => {
    it('moves the top edge while keeping x, w, and bottom fixed', () => {
      const out = clampResize('n', rect(100, 200, 300, 200), { x: 250, y: 120 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 120, w: 300, h: 280 });
    });

    it('ignores pointer.x — only pointer.y matters', () => {
      const out = clampResize('n', rect(100, 200, 300, 200), { x: 999, y: 120 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 120, w: 300, h: 280 });
    });

    it('stops at MIN_RECT_DIMENSION when dragged toward the bottom anchor', () => {
      const out = clampResize('n', rect(100, 200, 300, 200), { x: 250, y: 395 }, { w: VW, h: VH });
      expect(out).toEqual({
        x: 100,
        y: 400 - MIN_RECT_DIMENSION,
        w: 300,
        h: MIN_RECT_DIMENSION,
      });
    });

    it('refuses to flip past the bottom anchor', () => {
      const out = clampResize('n', rect(100, 200, 300, 200), { x: 250, y: 500 }, { w: VW, h: VH });
      expect(out.y + out.h).toBe(400);
      expect(out.h).toBe(MIN_RECT_DIMENSION);
    });

    it('clamps pointer.y to viewport top', () => {
      const out = clampResize('n', rect(100, 200, 300, 200), { x: 250, y: -50 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 0, w: 300, h: 400 });
    });
  });

  describe('S edge', () => {
    it('moves the bottom edge while keeping x, w, and top fixed', () => {
      const out = clampResize('s', rect(100, 200, 300, 200), { x: 250, y: 500 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: 300, h: 300 });
    });

    it('ignores pointer.x', () => {
      const out = clampResize('s', rect(100, 200, 300, 200), { x: -999, y: 500 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: 300, h: 300 });
    });

    it('stops at MIN_RECT_DIMENSION when dragged toward the top anchor', () => {
      const out = clampResize('s', rect(100, 200, 300, 200), { x: 250, y: 210 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: 300, h: MIN_RECT_DIMENSION });
    });

    it('refuses to flip past the top anchor', () => {
      const out = clampResize('s', rect(100, 200, 300, 200), { x: 250, y: 0 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: 300, h: MIN_RECT_DIMENSION });
    });

    it('clamps pointer.y to viewport bottom', () => {
      const out = clampResize('s', rect(100, 200, 300, 200), { x: 250, y: VH + 99 }, { w: VW, h: VH });
      expect(out.y + out.h).toBe(VH);
    });
  });

  describe('W edge', () => {
    it('moves the left edge while keeping y, h, and right fixed', () => {
      const out = clampResize('w', rect(100, 200, 300, 200), { x: 50, y: 250 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 50, y: 200, w: 350, h: 200 });
    });

    it('ignores pointer.y', () => {
      const out = clampResize('w', rect(100, 200, 300, 200), { x: 50, y: 999 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 50, y: 200, w: 350, h: 200 });
    });

    it('stops at MIN_RECT_DIMENSION when dragged toward the right anchor', () => {
      const out = clampResize('w', rect(100, 200, 300, 200), { x: 395, y: 250 }, { w: VW, h: VH });
      expect(out).toEqual({
        x: 400 - MIN_RECT_DIMENSION,
        y: 200,
        w: MIN_RECT_DIMENSION,
        h: 200,
      });
    });

    it('refuses to flip past the right anchor', () => {
      const out = clampResize('w', rect(100, 200, 300, 200), { x: 600, y: 250 }, { w: VW, h: VH });
      expect(out.x + out.w).toBe(400);
      expect(out.w).toBe(MIN_RECT_DIMENSION);
    });

    it('clamps pointer.x to viewport left', () => {
      const out = clampResize('w', rect(100, 200, 300, 200), { x: -50, y: 250 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 0, y: 200, w: 400, h: 200 });
    });
  });

  describe('E edge', () => {
    it('moves the right edge while keeping y, h, and left fixed', () => {
      const out = clampResize('e', rect(100, 200, 300, 200), { x: 600, y: 250 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: 500, h: 200 });
    });

    it('ignores pointer.y', () => {
      const out = clampResize('e', rect(100, 200, 300, 200), { x: 600, y: -999 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: 500, h: 200 });
    });

    it('stops at MIN_RECT_DIMENSION when dragged toward the left anchor', () => {
      const out = clampResize('e', rect(100, 200, 300, 200), { x: 110, y: 250 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: MIN_RECT_DIMENSION, h: 200 });
    });

    it('refuses to flip past the left anchor', () => {
      const out = clampResize('e', rect(100, 200, 300, 200), { x: 0, y: 250 }, { w: VW, h: VH });
      expect(out).toEqual({ x: 100, y: 200, w: MIN_RECT_DIMENSION, h: 200 });
    });

    it('clamps pointer.x to viewport right', () => {
      const out = clampResize('e', rect(100, 200, 300, 200), { x: VW + 99, y: 250 }, { w: VW, h: VH });
      expect(out.x + out.w).toBe(VW);
    });
  });
});
