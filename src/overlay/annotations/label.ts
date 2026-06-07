// ABOUTME: Derives a region's visible label from its array index. Zero-pads to two
// ABOUTME: characters minimum, expands further for 100+ regions, never truncates.

export function formatLabel(idx: number): string {
  return String(idx + 1).padStart(2, '0');
}

// Returns, for each rect, its annotation label ('01', '02', …) or null when the
// rect is a redaction. Numbering counts only annotation-kind rects, so a
// redaction between two annotations does not renumber them.
export function annotationLabels(rects: { kind: 'annotation' | 'redaction' }[]): (string | null)[] {
  let n = 0;
  return rects.map((r) => (r.kind === 'redaction' ? null : formatLabel(n++)));
}
