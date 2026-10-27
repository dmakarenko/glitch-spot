// ABOUTME: Unit tests for the formatLabel helper used by the live overlay and the
// ABOUTME: canvas exporter to derive a region's visible label from its array index.
import { describe, it, expect } from 'vitest';
import { formatLabel, annotationLabels } from './label';

describe('formatLabel', () => {
  it('zero-pads single-digit positions to two characters', () => {
    expect(formatLabel(0)).toBe('01');
    expect(formatLabel(8)).toBe('09');
  });

  it('returns two characters for the tenth and ninety-ninth positions', () => {
    expect(formatLabel(9)).toBe('10');
    expect(formatLabel(98)).toBe('99');
  });

  it('expands to three characters at one hundred and beyond, never truncating', () => {
    expect(formatLabel(99)).toBe('100');
    expect(formatLabel(998)).toBe('999');
  });
});

describe('annotationLabels', () => {
  it('numbers only annotation-kind rects, skipping redactions', () => {
    const rects = [
      { kind: 'annotation' as const },
      { kind: 'redaction' as const },
      { kind: 'annotation' as const },
    ]
    expect(annotationLabels(rects)).toEqual(['01', null, '02'])
  })

  it('returns all-null for an all-redaction list', () => {
    expect(annotationLabels([{ kind: 'redaction' as const }])).toEqual([null])
  })
})
