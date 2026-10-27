// ABOUTME: Pure cursor-position → window_id hit test for the picker UI.
// ABOUTME: Iterates the front-to-back ordered window list; first containing window wins.

export type PickableBounds = {
  window_id: number;
  bounds: { x: number; y: number; w: number; h: number };
};

export function hitTest(windows: PickableBounds[], cursorX: number, cursorY: number): number | null {
  for (const w of windows) {
    const { x, y, w: width, h: height } = w.bounds;
    if (cursorX >= x && cursorX < x + width && cursorY >= y && cursorY < y + height) {
      return w.window_id;
    }
  }
  return null;
}
