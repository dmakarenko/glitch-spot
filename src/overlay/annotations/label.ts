// ABOUTME: Derives a region's visible label from its array index. Zero-pads to two
// ABOUTME: characters minimum, expands further for 100+ regions, never truncates.

export function formatLabel(idx: number): string {
  return String(idx + 1).padStart(2, '0');
}
