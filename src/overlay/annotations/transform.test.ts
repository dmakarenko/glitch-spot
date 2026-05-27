// ABOUTME: Unit tests for the pure constraint helpers used by the reducer when
// ABOUTME: moving or resizing an annotation rectangle.
import { describe, it, expect } from 'vitest';
import { clampMoveDelta, clampResize } from './transform';
import { MIN_RECT_DIMENSION } from './state';

const VW = 1000;
const VH = 800;

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

describe('clampResize', () => {
  // anchor = the corner that stays pinned; pointer = the corner being dragged.
  it('NW corner drag towards SE shrinks rect with SE anchor fixed', () => {
    const out = clampResize('nw', { x: 500, y: 400 }, { x: 200, y: 100 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 200, y: 100, w: 300, h: 300 });
  });

  it('SE corner drag towards NW shrinks rect with NW anchor fixed', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: 400, y: 300 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 300, h: 200 });
  });

  it('NE corner drag', () => {
    const out = clampResize('ne', { x: 100, y: 400 }, { x: 500, y: 100 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 400, h: 300 });
  });

  it('SW corner drag', () => {
    const out = clampResize('sw', { x: 500, y: 100 }, { x: 100, y: 400 }, { w: VW, h: VH });
    expect(out).toEqual({ x: 100, y: 100, w: 400, h: 300 });
  });

  it('stops at MIN_RECT_DIMENSION when the dragged corner approaches the anchor', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: 105, y: 110 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('refuses to flip past the anchor (SE corner dragged past NW)', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: 50, y: 50 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100,
      y: 100,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('refuses to flip past the anchor (NW corner dragged past SE)', () => {
    const out = clampResize('nw', { x: 100, y: 100 }, { x: 200, y: 200 }, { w: VW, h: VH });
    expect(out).toEqual({
      x: 100 - MIN_RECT_DIMENSION,
      y: 100 - MIN_RECT_DIMENSION,
      w: MIN_RECT_DIMENSION,
      h: MIN_RECT_DIMENSION,
    });
  });

  it('clamps the dragged corner inside the viewport (SE corner beyond right edge)', () => {
    const out = clampResize('se', { x: 100, y: 100 }, { x: VW + 200, y: 300 }, { w: VW, h: VH });
    expect(out.x + out.w).toBe(VW);
  });

  it('clamps the dragged corner inside the viewport (NW corner beyond top edge)', () => {
    const out = clampResize('nw', { x: 500, y: 400 }, { x: 200, y: -50 }, { w: VW, h: VH });
    expect(out.y).toBe(0);
    expect(out.y + out.h).toBe(400);
  });
});
