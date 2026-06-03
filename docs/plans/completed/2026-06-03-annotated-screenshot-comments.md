# Annotated Screenshot Comments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw each annotation's comment text as a caption box on the exported annotated PNG, in both render paths (Tauri/Rust and the Electron canvas), so the saved image carries the notes — not just the region rectangles and number badges.

**Architecture:** The `note` text is already stored end-to-end (`Rect` → `AnnotationPayload` → Rust `Annotation`, and into `.json` metadata). Neither export path draws it. This plan adds a caption renderer to each path: a word-wrapped, fixed-max-width box anchored below each rectangle (flipped above on bottom-overflow, clamped on-canvas), styled to mirror the live Note Textarea. The wrapping logic is extracted as a pure, unit-tested helper in each language; the box drawing reuses each path's existing primitives. The two implementations share zero code — they mirror each other by convention, exactly as the pill-badge geometry already does.

**Tech Stack:** Rust (`image` + `imageproc` + `ab_glyph`) for the Tauri path; TypeScript + HTML Canvas 2D for the Electron path; `cargo test` and Vitest.

---

## Decisions (locked)

| Decision | Choice | Rationale |
|---|---|---|
| Layout | Caption box near each rect | User choice. Below the rect by default; flip above if it overflows the bottom; clamp fully on-canvas. |
| Caption content | Note text only (no `01` prefix) | User choice. Matches the live textarea, which shows no number. |
| Long notes | Box grows to fit the full wrapped note | User choice. A static PNG can't scroll; preserving the full comment is the point of the feature. |
| Border colour | **Opaque** `#ee6633`, 1px | Deviation from the live textarea's `rgba(238,102,51,0.28)`. Opaque colours mean neither path needs per-pixel alpha-blending against arbitrary screenshot pixels (imageproc's fill writes alpha verbatim, it does not blend). Keeps Rust simple and the two paths pixel-consistent. On-brand (same accent as the rect border). |
| Live overlay | **Unchanged** — export-only feature | The live overlay shows the editable textarea while drawing, not a persistent caption. This is a known, accepted WYSIWYG gap; do not "fix" it here. |
| Code sharing | None — mirror by convention | Rust and TS share no files. The pill geometry already lives mirrored in both; captions follow the same pattern. Do not invent a shared module. |

---

## Shared visual constants (mirrored by convention)

All values are **CSS pixels**, multiplied by `scale_factor` / `scaleFactor` at use, exactly like the existing pill constants:

| Constant | CSS value |
|---|---|
| Max caption width (wrap width) | `240` |
| Gap between rect and caption | `6` |
| Horizontal padding | `8` |
| Vertical padding | `6` |
| Corner radius | `6` |
| Border width | `1` |
| Font size | `13` |
| Line-height factor | `1.4` |
| Background fill | `#161616` → `Rgba([22, 22, 22, 255])` |
| Border colour | `#ee6633` → `Rgba([238, 102, 51, 255])` |
| Text colour | `#aaaaaa` → `Rgba([170, 170, 170, 255])` |

**Box drawing technique (both paths):** draw the box twice — first the full box in the border colour, then an inset box (inset by `border_width` on every edge, radius reduced by `border_width`) in the background colour. The visible 1px ring of the first draw is the border. No hollow-rounded-rect primitive needed; no alpha blending.

---

## Stacked-PR structure

Two PRs. PR2 stacks on PR1.

- **PR 1 — Rust/Tauri caption rendering.** Branch: `feat-annotated-screenshot-should-contain-comments` (the current worktree branch). Tasks 1–2. PR targets `main`.
- **PR 2 — Electron canvas caption rendering.** Branch: `feat-annotated-comments-canvas`, created **from PR1's head** once PR1 is pushed. Tasks 3–4. PR targets the PR1 branch (stacked), so its diff shows only the canvas changes.

Create the PR2 branch only when starting Task 3:

```bash
# From the worktree, with PR1 work committed and pushed:
git checkout -b feat-annotated-comments-canvas
```

---

## File structure

```
src-tauri/src/snapshot/
  annotate.rs          # MODIFY — add caption constants, wrap_text(), draw_caption(),
                       #          draw_filled_rounded_rect(); call captions in a second pass.
                       #          Add #[cfg(test)] mod tests for wrap_text + a render smoke test.

src/overlay/annotations/
  wrapNote.ts          # NEW — pure word-wrap helper (measure-injected, testable)
  wrapNote.test.ts     # NEW — unit tests for wrapNote
src/overlay/
  annotate.ts          # MODIFY — caption constants + second-pass caption drawing using wrapNote
  annotate.test.ts     # MODIFY — assert caption box + note text drawn; empty note draws none
```

No IPC/type changes — `note` already flows through `AnnotationPayload` and the Rust `Annotation`. No changes to `state.ts`, `Overlay.tsx`, or `packages/electron/src/types.ts`.

---

# PR 1 — Rust / Tauri path

## Task 1: Add `wrap_text` and `draw_filled_rounded_rect` helpers (Rust)

**Files:**
- Modify: `src-tauri/src/snapshot/annotate.rs`

These are the two pure-ish building blocks. `wrap_text` is fully pure and is where the TDD value is. `draw_filled_rounded_rect` is a drawing primitive verified by the smoke test in Task 2.

- [ ] **Step 1: Add the caption constants**

Open `src-tauri/src/snapshot/annotate.rs`. After the existing `BADGE_FONT_PX_CSS` constant (line 21), add:

```rust
// CSS-pixel caption geometry; mirrors src/overlay/annotate.ts.
const CAPTION_BG: Rgba<u8> = Rgba([22, 22, 22, 255]); // #161616
const CAPTION_BORDER: Rgba<u8> = Rgba([238, 102, 51, 255]); // accent, opaque
const CAPTION_TEXT: Rgba<u8> = Rgba([170, 170, 170, 255]); // #aaaaaa
const CAPTION_MAX_W_CSS: f32 = 240.0;
const CAPTION_OFFSET_CSS: f32 = 6.0;
const CAPTION_PAD_X_CSS: f32 = 8.0;
const CAPTION_PAD_Y_CSS: f32 = 6.0;
const CAPTION_RADIUS_CSS: f32 = 6.0;
const CAPTION_BORDER_W_CSS: f32 = 1.0;
const CAPTION_FONT_PX_CSS: f32 = 13.0;
const CAPTION_LINE_HEIGHT: f32 = 1.4;
```

- [ ] **Step 2: Write the failing test for `wrap_text`**

At the **end** of `src-tauri/src/snapshot/annotate.rs`, add this test module. It uses the bundled font so it exercises real metrics; assertions check structural invariants (every line fits, newlines split, long words hard-break) rather than exact pixel widths, so they are font-version robust.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> FontRef<'static> {
        FontRef::try_from_slice(FONT_BYTES).unwrap()
    }

    fn line_width(line: &str, font: &FontRef<'_>, scale: PxScale) -> f32 {
        let scaled = font.as_scaled(scale);
        line.chars()
            .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
            .sum()
    }

    #[test]
    fn wraps_long_text_so_every_line_fits_max_width() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        let max_w = 120.0;
        let text = "the quick brown fox jumps over the lazy dog repeatedly";
        let lines = wrap_text(text, &font, scale, max_w);
        assert!(lines.len() > 1, "expected multiple lines, got {lines:?}");
        for line in &lines {
            assert!(
                line_width(line, &font, scale) <= max_w,
                "line {line:?} exceeds max width {max_w}"
            );
        }
    }

    #[test]
    fn preserves_explicit_newlines() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        let lines = wrap_text("first\nsecond", &font, scale, 1000.0);
        assert_eq!(lines, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn hard_breaks_a_word_longer_than_max_width() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        let max_w = 40.0;
        let lines = wrap_text("supercalifragilistic", &font, scale, max_w);
        assert!(lines.len() > 1, "expected the long word to hard-break");
        for line in &lines {
            assert!(line_width(line, &font, scale) <= max_w);
        }
    }

    #[test]
    fn empty_text_yields_a_single_empty_line() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        assert_eq!(wrap_text("", &font, scale, 100.0), vec!["".to_string()]);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd src-tauri && cargo test --lib snapshot::annotate::tests::wraps_long_text_so_every_line_fits_max_width`
Expected: FAIL — `cannot find function wrap_text in this scope` (compile error).

- [ ] **Step 4: Implement `wrap_text` and `draw_filled_rounded_rect`**

Add both functions to `annotate.rs` (place them after `draw_pill_badge`, before the `#[cfg(test)]` module):

```rust
/// Greedy word-wrap `text` into lines no wider than `max_w` px under `font`/`scale`.
/// Explicit newlines split paragraphs; a single word wider than `max_w` is
/// hard-broken character-by-character so no line ever overflows.
fn wrap_text(text: &str, font: &FontRef<'_>, scale: PxScale, max_w: f32) -> Vec<String> {
    let scaled = font.as_scaled(scale);
    let char_w = |c: char| scaled.h_advance(scaled.scaled_glyph(c).id);
    let str_w = |s: &str| s.chars().map(char_w).sum::<f32>();

    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ').filter(|w| !w.is_empty()) {
            if str_w(word) > max_w {
                // Word alone overflows: flush the current line, then hard-break it.
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let mut chunk = String::new();
                for ch in word.chars() {
                    if !chunk.is_empty() && str_w(&chunk) + char_w(ch) > max_w {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(ch);
                }
                line = chunk;
                continue;
            }
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if str_w(&candidate) > max_w && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Fill a rounded rectangle by compositing axis-aligned bands and four corner
/// circles. `r` is clamped to half the smaller side. All pixels written opaque.
fn draw_filled_rounded_rect(
    img: &mut RgbaImage,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: i32,
    color: Rgba<u8>,
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let r = r.min(w / 2).min(h / 2).max(0);
    if r == 0 {
        draw_filled_rect_mut(img, IpRect::at(x, y).of_size(w as u32, h as u32), color);
        return;
    }
    // Center band (full width, inset top/bottom by r).
    draw_filled_rect_mut(
        img,
        IpRect::at(x, y + r).of_size(w as u32, (h - 2 * r) as u32),
        color,
    );
    // Top and bottom bands (inset left/right by r).
    draw_filled_rect_mut(
        img,
        IpRect::at(x + r, y).of_size((w - 2 * r) as u32, r as u32),
        color,
    );
    draw_filled_rect_mut(
        img,
        IpRect::at(x + r, y + h - r).of_size((w - 2 * r) as u32, r as u32),
        color,
    );
    // Four corner circles (right/bottom centers nudged in by 1px to stay in bounds).
    draw_filled_circle_mut(img, (x + r, y + r), r, color);
    draw_filled_circle_mut(img, (x + w - r - 1, y + r), r, color);
    draw_filled_circle_mut(img, (x + r, y + h - r - 1), r, color);
    draw_filled_circle_mut(img, (x + w - r - 1, y + h - r - 1), r, color);
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test --lib snapshot::annotate::tests`
Expected: PASS — 4 tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/snapshot/annotate.rs
git commit -m "feat(snapshot): add wrap_text + rounded-rect helpers for captions"
```

---

## Task 2: Draw captions in the annotated PNG (Rust)

**Files:**
- Modify: `src-tauri/src/snapshot/annotate.rs`

Add `draw_caption`, call it in a second pass (after all borders + badges, so captions sit on top), and add a render smoke test.

- [ ] **Step 1: Write the failing smoke test**

Add these two tests **inside** the existing `#[cfg(test)] mod tests` block from Task 1 (alongside the `wrap_text` tests):

```rust
    use crate::snapshot::{Annotation, Coords};

    fn white_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn anno_with_note(note: &str) -> Annotation {
        Annotation {
            id: "a1".into(),
            shape: "rect".into(),
            coords: Coords { x: 40.0, y: 40.0, w: 120.0, h: 80.0 },
            note: note.into(),
            label: "01".into(),
        }
    }

    #[test]
    fn caption_draws_dark_pixels_below_the_rect_when_note_present() {
        let clean = white_png(400, 400);
        let out =
            render_annotated_png(&clean, &[anno_with_note("hello world")], 1.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (400, 400), "dimensions must not change");
        // The caption box sits just below the rect (rect bottom = 40+80 = 120,
        // gap 6 → box top ~126). Sample a pixel inside the box; it must be dark
        // (#161616 fill or accent border), not the white background.
        let px = img.get_pixel(50, 135);
        assert!(px[0] < 250, "expected a non-white caption pixel, got {px:?}");
    }

    #[test]
    fn no_caption_when_note_is_blank() {
        let clean = white_png(400, 400);
        let out = render_annotated_png(&clean, &[anno_with_note("   ")], 1.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        // Same sample point as above must stay white — no caption box drawn.
        assert_eq!(img.get_pixel(50, 135), &Rgba([255, 255, 255, 255]));
    }
```

> Note: the struct shape is confirmed against `src-tauri/src/snapshot/mod.rs:16-31` — `Annotation { id, shape, coords: Coords { x, y, w, h }, note, label }`, all `Coords` fields `f32`.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test --lib snapshot::annotate::tests::caption_draws_dark_pixels_below_the_rect_when_note_present`
Expected: FAIL — the sampled pixel is still white `[255,255,255,255]` because captions are not drawn yet.

- [ ] **Step 3: Add `draw_caption`**

Add this function to `annotate.rs` (after `draw_filled_rounded_rect`):

```rust
/// Draw the annotation's note as a word-wrapped caption box anchored below the
/// rectangle (flipped above on bottom-overflow, clamped fully on-canvas).
/// No-op when the note is blank.
fn draw_caption(img: &mut RgbaImage, anno: &Annotation, scale_factor: f32, font: &FontRef<'_>) {
    let note = anno.note.trim();
    if note.is_empty() {
        return;
    }

    let pad_x = (CAPTION_PAD_X_CSS * scale_factor).round() as i32;
    let pad_y = (CAPTION_PAD_Y_CSS * scale_factor).round() as i32;
    let radius = (CAPTION_RADIUS_CSS * scale_factor).round() as i32;
    let offset = (CAPTION_OFFSET_CSS * scale_factor).round() as i32;
    let border_w = (CAPTION_BORDER_W_CSS * scale_factor).round().max(1.0) as i32;
    let font_px = CAPTION_FONT_PX_CSS * scale_factor;
    let scale = PxScale::from(font_px);
    let line_h = (font_px * CAPTION_LINE_HEIGHT).round() as i32;
    let max_box_w = (CAPTION_MAX_W_CSS * scale_factor).round() as i32;
    let text_max_w = (max_box_w - 2 * pad_x).max(1) as f32;

    let lines = wrap_text(note, font, scale, text_max_w);

    let scaled = font.as_scaled(scale);
    let widest = lines
        .iter()
        .map(|l| {
            l.chars()
                .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
                .sum::<f32>()
        })
        .fold(0.0_f32, f32::max);

    let box_w = ((widest.ceil() as i32) + 2 * pad_x).min(max_box_w);
    let box_h = lines.len() as i32 * line_h + 2 * pad_y;

    let rect_x = anno.coords.x.round() as i32;
    let rect_y = anno.coords.y.round() as i32;
    let rect_h = anno.coords.h.round() as i32;
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;

    // Below the rect; flip above if placing below overflows the bottom edge.
    let below_y = rect_y + rect_h + offset;
    let mut box_y = if below_y + box_h > img_h {
        rect_y - offset - box_h
    } else {
        below_y
    };
    box_y = box_y.clamp(0, (img_h - box_h).max(0));
    let box_x = rect_x.clamp(0, (img_w - box_w).max(0));

    // Border layer, then inset background fill → clean 1px rounded border.
    draw_filled_rounded_rect(img, box_x, box_y, box_w, box_h, radius, CAPTION_BORDER);
    draw_filled_rounded_rect(
        img,
        box_x + border_w,
        box_y + border_w,
        box_w - 2 * border_w,
        box_h - 2 * border_w,
        (radius - border_w).max(0),
        CAPTION_BG,
    );

    for (i, line) in lines.iter().enumerate() {
        let tx = box_x + pad_x;
        let ty = box_y + pad_y + i as i32 * line_h;
        draw_text_mut(img, CAPTION_TEXT, tx, ty, scale, font, line);
    }
}
```

- [ ] **Step 4: Call captions in a second pass**

In `render_annotated_png`, the existing loop (lines 51-54) draws borders + badges. Add a second loop right after it (and before the encode at line 56) so captions render on top of all rectangles:

```rust
    for anno in annotations {
        draw_rect_border(&mut img, anno, border_width);
        draw_pill_badge(&mut img, anno, pill_w, pill_h, pill_r, &font, badge_scale);
    }

    for anno in annotations {
        draw_caption(&mut img, anno, scale_factor, &font);
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test --lib snapshot::annotate`
Expected: PASS — all 6 tests (4 from Task 1 + 2 smoke tests).

- [ ] **Step 6: Run the full Rust test suite + clippy**

Run: `cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS, no clippy warnings. (If clippy flags the closures or casts, fix them — do not `#[allow]` away warnings without flagging it.)

- [ ] **Step 7: Manual visual verification (MANDATORY — unit-green is not done)**

This is the only check that catches overlap, off-canvas, wrong wrap, and flip-above misfires. Use the `run` skill (or `PATH="$HOME/.cargo/bin:$PATH" npm run tauri -- dev`).

1. Trigger the overlay, draw a rect near the **top** with a short note ("Fix the alignment"). Save.
2. Draw a rect near the **bottom edge** with a note — verify the caption flips **above** the rect and stays on-canvas.
3. Draw a rect with a **long** note (paste ~300 chars) — verify it wraps at ~240px and the box grows to show the whole note.
4. Open the saved PNGs in `~/.glitch-spot/snapshots/<id>/<id>_annotated.png` and confirm: dark `#161616` box, thin orange border, grey text, anchored to the right rect, no clipping off the image.

Stop and inspect if anything looks wrong; do not tweak constants without comparing live vs exported side by side.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/snapshot/annotate.rs
git commit -m "feat(snapshot): render note text as a caption on the annotated PNG"
```

- [ ] **Step 9: Push and open PR 1**

```bash
git push -u origin feat-annotated-screenshot-should-contain-comments
gh pr create --base main --title "feat: comment captions on annotated PNG (Rust path)" \
  --body "Draws each annotation's note as a caption box on the exported PNG in the Tauri/Rust render path. Canvas path follows in a stacked PR."
```

---

# PR 2 — Electron canvas path (stacked on PR 1)

> Start by branching from PR1's head: `git checkout -b feat-annotated-comments-canvas`

## Task 3: Add `wrapNote` pure helper (TypeScript)

**Files:**
- Create: `src/overlay/annotations/wrapNote.ts`
- Create: `src/overlay/annotations/wrapNote.test.ts`

Mirror of the Rust `wrap_text`. Measurement is injected so the helper is pure and testable without a real canvas.

- [ ] **Step 1: Write the failing test**

Create `src/overlay/annotations/wrapNote.test.ts`:

```ts
// ABOUTME: Unit tests for wrapNote — the pure word-wrap helper used by the canvas
// ABOUTME: exporter to lay out caption text. Measurement is injected for determinism.
import { describe, it, expect } from 'vitest'
import { wrapNote } from './wrapNote'

// Deterministic measure: each character is 10 units wide.
const measure = (s: string) => s.length * 10

describe('wrapNote', () => {
  it('keeps text that fits on one line as a single line', () => {
    expect(wrapNote('hi there', 1000, measure)).toEqual(['hi there'])
  })

  it('wraps so that no line exceeds maxWidth', () => {
    // maxWidth 60 = 6 chars. "the quick brown" → "the" (30), "quick" (50), "brown" (50).
    const lines = wrapNote('the quick brown', 60, measure)
    expect(lines.length).toBeGreaterThan(1)
    for (const line of lines) expect(measure(line)).toBeLessThanOrEqual(60)
  })

  it('preserves explicit newlines', () => {
    expect(wrapNote('first\nsecond', 1000, measure)).toEqual(['first', 'second'])
  })

  it('hard-breaks a single word longer than maxWidth', () => {
    const lines = wrapNote('supercalifragilistic', 40, measure)
    expect(lines.length).toBeGreaterThan(1)
    for (const line of lines) expect(measure(line)).toBeLessThanOrEqual(40)
  })

  it('returns a single empty line for empty input', () => {
    expect(wrapNote('', 100, measure)).toEqual([''])
  })
})
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- src/overlay/annotations/wrapNote.test.ts`
Expected: FAIL — cannot resolve `./wrapNote`.

- [ ] **Step 3: Write the implementation**

Create `src/overlay/annotations/wrapNote.ts`:

```ts
// ABOUTME: Pure greedy word-wrap for caption text. Explicit newlines split lines;
// ABOUTME: a single word wider than maxWidth is hard-broken so no line overflows.

export function wrapNote(
  text: string,
  maxWidth: number,
  measure: (s: string) => number,
): string[] {
  const lines: string[] = []
  for (const paragraph of text.split('\n')) {
    let line = ''
    for (const word of paragraph.split(' ').filter((w) => w !== '')) {
      if (measure(word) > maxWidth) {
        if (line) {
          lines.push(line)
          line = ''
        }
        let chunk = ''
        for (const ch of word) {
          if (chunk && measure(chunk + ch) > maxWidth) {
            lines.push(chunk)
            chunk = ''
          }
          chunk += ch
        }
        line = chunk
        continue
      }
      const candidate = line ? `${line} ${word}` : word
      if (measure(candidate) > maxWidth && line) {
        lines.push(line)
        line = word
      } else {
        line = candidate
      }
    }
    lines.push(line)
  }
  return lines
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- src/overlay/annotations/wrapNote.test.ts`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotations/wrapNote.ts src/overlay/annotations/wrapNote.test.ts
git commit -m "feat(overlay): add wrapNote helper for caption wrapping"
```

---

## Task 4: Draw captions in the canvas exporter (TypeScript)

**Files:**
- Modify: `src/overlay/annotate.ts`
- Modify: `src/overlay/annotate.test.ts`

- [ ] **Step 1: Update the test mock and add caption assertions**

Open `src/overlay/annotate.test.ts`. Two changes:

(a) Make `measureText` length-proportional so wrapping can actually be exercised, and add `textBaseline`/`textAlign` setters if missing. Replace the `createMockCtx` function with:

```ts
function createMockCtx() {
  return {
    drawImage: vi.fn(),
    strokeRect: vi.fn(),
    fillRect: vi.fn(),
    fillText: vi.fn(),
    measureText: vi.fn((s: string) => ({ width: s.length * 8 })),
    beginPath: vi.fn(),
    arc: vi.fn(),
    roundRect: vi.fn(),
    fill: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    set strokeStyle(_v: string) {},
    set fillStyle(_v: string) {},
    set lineWidth(_v: number) {},
    set font(_v: string) {},
    set textAlign(_v: string) {},
    set textBaseline(_v: string) {},
  }
}
```

(b) Add two tests inside the existing `describe('renderAnnotated', ...)` block:

```ts
  it('draws a caption box and the note text when a note is present', async () => {
    mockCtx.fillText.mockClear()
    mockCtx.roundRect.mockClear()
    const { renderAnnotated } = await import('./annotate')

    installImageMock()

    const annotations = [{ id: 'a1', x: 10, y: 20, w: 100, h: 50, note: 'fix this' }]
    await renderAnnotated('blob:fake', annotations, 1)

    // Caption text is drawn (a short note stays one line: "fix this").
    const texts = mockCtx.fillText.mock.calls.map((c) => c[0])
    expect(texts).toContain('fix this')
    // Pill badge (1 roundRect) + caption border + caption fill (2 roundRect) = 3.
    expect(mockCtx.roundRect).toHaveBeenCalledTimes(3)
  })

  it('draws no caption when the note is blank', async () => {
    mockCtx.fillText.mockClear()
    mockCtx.roundRect.mockClear()
    const { renderAnnotated } = await import('./annotate')

    installImageMock()

    const annotations = [{ id: 'a1', x: 10, y: 20, w: 100, h: 50, note: '   ' }]
    await renderAnnotated('blob:fake', annotations, 1)

    const texts = mockCtx.fillText.mock.calls.map((c) => c[0])
    expect(texts).toEqual(['01']) // only the badge label, no caption text
    expect(mockCtx.roundRect).toHaveBeenCalledTimes(1) // only the pill badge
  })
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm test -- src/overlay/annotate.test.ts`
Expected: FAIL — the two new tests fail (no caption drawn yet); existing badge tests still pass.

- [ ] **Step 3: Add caption drawing to `renderAnnotated`**

Open `src/overlay/annotate.ts`. Replace the entire file with:

```ts
// ABOUTME: Canvas-based annotation renderer used by the Electron overlay path.
// ABOUTME: Draws annotation rectangles, pill badges, and note captions onto the clean image.
import type { AnnotationPayload } from '../../packages/electron/src/types'
import { formatLabel } from './annotations/label'
import { wrapNote } from './annotations/wrapNote'

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = url
  })
}

/**
 * Renders annotation rectangles, pill-shaped labels, and note captions onto cleanBlobUrl.
 * Returns the result as a raw base64 PNG string (no data: prefix).
 * scaleFactor converts CSS coords to physical pixels (window.devicePixelRatio).
 */
export async function renderAnnotated(
  cleanBlobUrl: string,
  annotations: AnnotationPayload[],
  scaleFactor: number,
): Promise<string> {
  const img = await loadImage(cleanBlobUrl)
  const canvas = document.createElement('canvas')
  canvas.width = img.naturalWidth
  canvas.height = img.naturalHeight
  const ctx = canvas.getContext('2d')!

  ctx.drawImage(img, 0, 0)

  const PILL_W = 26 * scaleFactor
  const PILL_H = 18 * scaleFactor
  const PILL_R = 9 * scaleFactor

  // Pass 1: rectangles + pill badges.
  annotations.forEach((rect, idx) => {
    const x = rect.x * scaleFactor
    const y = rect.y * scaleFactor
    const w = rect.w * scaleFactor
    const h = rect.h * scaleFactor

    ctx.strokeStyle = '#EE6633'
    ctx.lineWidth = 2 * scaleFactor
    ctx.strokeRect(x, y, w, h)

    // Pill badge straddling the rect's top-left corner (half outside).
    const pillX = x - PILL_W / 2
    const pillY = y - PILL_H / 2
    ctx.fillStyle = '#EE6633'
    ctx.beginPath()
    ctx.roundRect(pillX, pillY, PILL_W, PILL_H, PILL_R)
    ctx.fill()

    ctx.fillStyle = '#ffffff'
    ctx.font = `bold ${11 * scaleFactor}px sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(formatLabel(idx), pillX + PILL_W / 2, pillY + PILL_H / 2)
    ctx.textAlign = 'left'
    ctx.textBaseline = 'alphabetic'
  })

  // Pass 2: note captions, drawn on top of all rectangles.
  const CAPTION_MAX_W = 240 * scaleFactor
  const CAPTION_OFFSET = 6 * scaleFactor
  const CAPTION_PAD_X = 8 * scaleFactor
  const CAPTION_PAD_Y = 6 * scaleFactor
  const CAPTION_RADIUS = 6 * scaleFactor
  const CAPTION_BORDER_W = Math.max(1, Math.round(1 * scaleFactor))
  const CAPTION_FONT_PX = 13 * scaleFactor
  const CAPTION_LINE_HEIGHT = 1.4
  const lineH = Math.round(CAPTION_FONT_PX * CAPTION_LINE_HEIGHT)

  annotations.forEach((rect) => {
    const note = rect.note.trim()
    if (!note) return

    const x = rect.x * scaleFactor
    const y = rect.y * scaleFactor
    const h = rect.h * scaleFactor

    ctx.font = `${CAPTION_FONT_PX}px system-ui`
    const measure = (s: string) => ctx.measureText(s).width
    const textMaxW = Math.max(1, CAPTION_MAX_W - 2 * CAPTION_PAD_X)
    const lines = wrapNote(note, textMaxW, measure)

    const widest = lines.reduce((m, l) => Math.max(m, measure(l)), 0)
    const boxW = Math.min(CAPTION_MAX_W, Math.ceil(widest) + 2 * CAPTION_PAD_X)
    const boxH = lines.length * lineH + 2 * CAPTION_PAD_Y

    // Below the rect; flip above on bottom-overflow; clamp fully on-canvas.
    const belowY = y + h + CAPTION_OFFSET
    let boxY = belowY + boxH > canvas.height ? y - CAPTION_OFFSET - boxH : belowY
    boxY = Math.max(0, Math.min(boxY, canvas.height - boxH))
    const boxX = Math.max(0, Math.min(x, canvas.width - boxW))

    // Border layer, then inset background fill → clean 1px rounded border.
    ctx.fillStyle = '#ee6633'
    ctx.beginPath()
    ctx.roundRect(boxX, boxY, boxW, boxH, CAPTION_RADIUS)
    ctx.fill()
    ctx.fillStyle = '#161616'
    ctx.beginPath()
    ctx.roundRect(
      boxX + CAPTION_BORDER_W,
      boxY + CAPTION_BORDER_W,
      boxW - 2 * CAPTION_BORDER_W,
      boxH - 2 * CAPTION_BORDER_W,
      Math.max(0, CAPTION_RADIUS - CAPTION_BORDER_W),
    )
    ctx.fill()

    ctx.fillStyle = '#aaaaaa'
    ctx.textAlign = 'left'
    ctx.textBaseline = 'top'
    lines.forEach((line, i) => {
      ctx.fillText(line, boxX + CAPTION_PAD_X, boxY + CAPTION_PAD_Y + i * lineH)
    })
    ctx.textBaseline = 'alphabetic'
  })

  return canvas.toDataURL('image/png').split(',')[1]
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm test -- src/overlay/annotate.test.ts`
Expected: PASS — all 4 tests (2 existing badge tests + 2 new caption tests).

- [ ] **Step 5: Run the full JS/TS test suite**

Run: `npm test`
Expected: PASS — all suites green (including `wrapNote.test.ts`).

- [ ] **Step 6: Manual visual verification (MANDATORY)**

The Electron SDK has a smoke harness. Build and run it:

```bash
cd packages/electron
npm run build:all
npx electron smoke-test/index.js
```

Press the smoke-test hotkey (per `packages/electron/smoke-test/index.js`), then:
1. Draw a rect with a short note near the top — confirm the caption box renders below it.
2. Draw a rect near the bottom — confirm the caption flips above and stays on-canvas.
3. Draw a rect with a long (~300 char) note — confirm wrap + box growth.
4. Confirm the exported PNG's captions look **identical** to the Tauri output from PR1: same dark fill, orange border, grey text, position. The base64 is logged / written by the smoke harness — load it in an image viewer.

If the two paths differ, reconcile the mismatching constant before proceeding.

- [ ] **Step 7: Commit**

```bash
git add src/overlay/annotate.ts src/overlay/annotate.test.ts
git commit -m "feat(overlay): render note captions in the canvas exporter"
```

- [ ] **Step 8: Push and open PR 2 (stacked on PR 1)**

```bash
git push -u origin feat-annotated-comments-canvas
gh pr create --base feat-annotated-screenshot-should-contain-comments \
  --title "feat: comment captions on annotated PNG (canvas path)" \
  --body "Stacked on the Rust caption PR. Mirrors the same caption layout in the Electron canvas exporter so both paths produce identical annotated PNGs."
```

---

## Self-review checklist (reviewer applies before declaring done)

- [ ] `wrap_text` (Rust) and `wrapNote` (TS) implement the same algorithm: newline split, greedy fit, hard-break of over-long words.
- [ ] Caption constants match between `annotate.rs` and `annotate.ts` (240 / 6 / 8 / 6 / 6 / 1 / 13 / 1.4; `#161616` / `#ee6633` / `#aaaaaa`).
- [ ] Both paths skip drawing when the trimmed note is empty.
- [ ] Both paths anchor below, flip above on bottom-overflow, and clamp the box fully on-canvas (x and y).
- [ ] Both paths draw the box as border-layer + inset-fill (1px opaque accent ring), no alpha blending.
- [ ] Captions render in a **second pass** (Rust) / second `forEach` (TS), after all rects + badges.
- [ ] No IPC/type changes; no changes to `state.ts`, `Overlay.tsx`, `DrawingLayer.tsx`, or `packages/electron/src/types.ts`.
- [ ] `cd src-tauri && cargo test` green; `cargo clippy` clean. `npm test` green.
- [ ] Manual visual verification done for **both** PRs — captions present, correctly positioned, on-canvas, wrap working; the two paths look identical.
- [ ] PR2 is opened with `--base` pointing at PR1's branch (true stacked PR).
- [ ] No chromatic-aberration split on the caption box (it's a text element, per DESIGN.md).
```
