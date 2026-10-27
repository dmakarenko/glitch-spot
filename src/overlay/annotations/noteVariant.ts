// ABOUTME: Maps a rectangle id to one of two alternating note-textarea styles so
// ABOUTME: consecutive annotations get visually distinct translucent backgrounds.

// Derived from the trailing integer of the id (e.g. "r7" -> 7) so a note's
// variant stays fixed even when earlier rectangles are deleted. Ids without a
// trailing number fall back to the first variant.
export function noteVariant(id: string): 0 | 1 {
  const n = parseInt(id.replace(/^\D+/, ''), 10);
  if (Number.isNaN(n)) return 0;
  return (n % 2) as 0 | 1;
}
