// ABOUTME: Pure constraint helpers for moving and resizing annotation rectangles.
// ABOUTME: Clamps to viewport bounds and enforces the MIN_RECT_DIMENSION floor.

import { MIN_RECT_DIMENSION } from './state';
import type { Corner } from './types';

export type Viewport = { w: number; h: number };
export type Point = { x: number; y: number };

export function clampMoveDelta(
  rect: { x: number; y: number; w: number; h: number },
  deltaX: number,
  deltaY: number,
  viewport: Viewport,
): Point {
  const x = clamp(rect.x + deltaX, 0, viewport.w - rect.w);
  const y = clamp(rect.y + deltaY, 0, viewport.h - rect.h);
  return { x, y };
}

export function clampResize(
  corner: Corner,
  anchor: Point,
  pointer: Point,
  viewport: Viewport,
): { x: number; y: number; w: number; h: number } {
  const pointerX = clamp(pointer.x, 0, viewport.w);
  const pointerY = clamp(pointer.y, 0, viewport.h);

  let x: number;
  let y: number;
  let w: number;
  let h: number;

  switch (corner) {
    case 'nw':
      // Anchor is SE corner; pointer is NW corner.
      x = Math.min(pointerX, anchor.x - MIN_RECT_DIMENSION);
      y = Math.min(pointerY, anchor.y - MIN_RECT_DIMENSION);
      w = anchor.x - x;
      h = anchor.y - y;
      break;
    case 'ne':
      // Anchor is SW corner; pointer is NE corner.
      x = anchor.x;
      y = Math.min(pointerY, anchor.y - MIN_RECT_DIMENSION);
      w = Math.max(pointerX - anchor.x, MIN_RECT_DIMENSION);
      h = anchor.y - y;
      break;
    case 'sw':
      // Anchor is NE corner; pointer is SW corner.
      x = Math.min(pointerX, anchor.x - MIN_RECT_DIMENSION);
      y = anchor.y;
      w = anchor.x - x;
      h = Math.max(pointerY - anchor.y, MIN_RECT_DIMENSION);
      break;
    case 'se':
      // Anchor is NW corner; pointer is SE corner.
      x = anchor.x;
      y = anchor.y;
      w = Math.max(pointerX - anchor.x, MIN_RECT_DIMENSION);
      h = Math.max(pointerY - anchor.y, MIN_RECT_DIMENSION);
      break;
  }

  return { x, y, w, h };
}

function clamp(v: number, lo: number, hi: number): number {
  if (hi < lo) return lo;
  return Math.min(hi, Math.max(lo, v));
}
