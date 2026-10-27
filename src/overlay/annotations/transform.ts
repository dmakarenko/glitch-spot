// ABOUTME: Pure constraint helpers for moving and resizing annotation rectangles.
// ABOUTME: Clamps to viewport bounds and enforces the MIN_RECT_DIMENSION floor.

import { MIN_RECT_DIMENSION } from './state';
import type { Handle } from './types';

export type Viewport = { w: number; h: number };
export type Point = { x: number; y: number };
export type RectGeom = { x: number; y: number; w: number; h: number };
export type Bounds = { x: number; y: number; w: number; h: number };

export function clampMoveDelta(
  rect: RectGeom,
  deltaX: number,
  deltaY: number,
  bounds: Bounds,
): Point {
  const x = clamp(rect.x + deltaX, bounds.x, bounds.x + bounds.w - rect.w);
  const y = clamp(rect.y + deltaY, bounds.y, bounds.y + bounds.h - rect.h);
  return { x, y };
}

export function clampResize(
  handle: Handle,
  rectStart: RectGeom,
  pointer: Point,
  viewport: Viewport,
): RectGeom {
  const pointerX = clamp(pointer.x, 0, viewport.w);
  const pointerY = clamp(pointer.y, 0, viewport.h);

  const left = rectStart.x;
  const top = rectStart.y;
  const right = rectStart.x + rectStart.w;
  const bottom = rectStart.y + rectStart.h;

  let x: number;
  let y: number;
  let w: number;
  let h: number;

  switch (handle) {
    case 'nw':
      // Anchor is SE corner; pointer is NW corner.
      x = Math.min(pointerX, right - MIN_RECT_DIMENSION);
      y = Math.min(pointerY, bottom - MIN_RECT_DIMENSION);
      w = right - x;
      h = bottom - y;
      break;
    case 'ne':
      // Anchor is SW corner; pointer is NE corner.
      x = left;
      y = Math.min(pointerY, bottom - MIN_RECT_DIMENSION);
      w = Math.max(pointerX - left, MIN_RECT_DIMENSION);
      h = bottom - y;
      break;
    case 'sw':
      // Anchor is NE corner; pointer is SW corner.
      x = Math.min(pointerX, right - MIN_RECT_DIMENSION);
      y = top;
      w = right - x;
      h = Math.max(pointerY - top, MIN_RECT_DIMENSION);
      break;
    case 'se':
      // Anchor is NW corner; pointer is SE corner.
      x = left;
      y = top;
      w = Math.max(pointerX - left, MIN_RECT_DIMENSION);
      h = Math.max(pointerY - top, MIN_RECT_DIMENSION);
      break;
    case 'n':
      // Anchor is bottom edge; only y/h change. x and w preserved.
      x = left;
      y = Math.min(pointerY, bottom - MIN_RECT_DIMENSION);
      w = rectStart.w;
      h = bottom - y;
      break;
    case 's':
      // Anchor is top edge; only y/h change. x and w preserved.
      x = left;
      y = top;
      w = rectStart.w;
      h = Math.max(pointerY - top, MIN_RECT_DIMENSION);
      break;
    case 'w':
      // Anchor is right edge; only x/w change. y and h preserved.
      x = Math.min(pointerX, right - MIN_RECT_DIMENSION);
      y = top;
      w = right - x;
      h = rectStart.h;
      break;
    case 'e':
      // Anchor is left edge; only x/w change. y and h preserved.
      x = left;
      y = top;
      w = Math.max(pointerX - left, MIN_RECT_DIMENSION);
      h = rectStart.h;
      break;
  }

  return { x, y, w, h };
}

function clamp(v: number, lo: number, hi: number): number {
  if (hi < lo) return lo;
  return Math.min(hi, Math.max(lo, v));
}
