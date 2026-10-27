// ABOUTME: Unit tests for wrapNote — the pure word-wrap helper used by the canvas
// ABOUTME: exporter to lay out caption text. Measurement is injected for determinism.
import { describe, it, expect } from 'vitest';
import { wrapNote } from './wrapNote';

// Deterministic measure: each character is 10 units wide.
const measure = (s: string) => s.length * 10;

describe('wrapNote', () => {
  it('keeps text that fits on one line as a single line', () => {
    expect(wrapNote('hi there', 1000, measure)).toEqual(['hi there']);
  });

  it('wraps so that no line exceeds maxWidth', () => {
    // maxWidth 60 = 6 chars. "the quick brown" → "the" (30), "quick" (50), "brown" (50).
    const lines = wrapNote('the quick brown', 60, measure);
    expect(lines.length).toBeGreaterThan(1);
    for (const line of lines) expect(measure(line)).toBeLessThanOrEqual(60);
  });

  it('preserves explicit newlines', () => {
    expect(wrapNote('first\nsecond', 1000, measure)).toEqual(['first', 'second']);
  });

  it('hard-breaks a single word longer than maxWidth', () => {
    const lines = wrapNote('supercalifragilistic', 40, measure);
    expect(lines.length).toBeGreaterThan(1);
    for (const line of lines) expect(measure(line)).toBeLessThanOrEqual(40);
  });

  it('returns a single empty line for empty input', () => {
    expect(wrapNote('', 100, measure)).toEqual(['']);
  });

  it('preserves blank lines between paragraphs', () => {
    expect(wrapNote('a\n\nb', 1000, measure)).toEqual(['a', '', 'b']);
    expect(wrapNote('a\n', 1000, measure)).toEqual(['a', '']);
  });
});
