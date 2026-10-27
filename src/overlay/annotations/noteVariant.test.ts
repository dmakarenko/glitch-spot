// ABOUTME: Unit tests for noteVariant, which maps a rectangle id to one of the
// ABOUTME: three rotating translucent note-textarea styles.
import { describe, it, expect } from 'vitest';
import { noteVariant } from './noteVariant';

describe('noteVariant', () => {
  it('alternates between 0 and 1 as the id number increases', () => {
    expect(noteVariant('r0')).toBe(0);
    expect(noteVariant('r1')).toBe(1);
    expect(noteVariant('r2')).toBe(0);
    expect(noteVariant('r3')).toBe(1);
    expect(noteVariant('r4')).toBe(0);
  });

  it('is stable for a given id regardless of other rects', () => {
    expect(noteVariant('r7')).toBe(noteVariant('r7'));
    expect(noteVariant('r7')).toBe(1);
  });

  it('falls back to variant 0 when the id has no trailing number', () => {
    expect(noteVariant('rect')).toBe(0);
    expect(noteVariant('')).toBe(0);
  });
});
