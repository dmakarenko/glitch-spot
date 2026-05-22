# Glitch Spot v0.2 — Annotated PNG follow-up

> **For agentic workers:** REQUIRED SUB-SKILL: `superpowers:executing-plans`. Steps use checkbox (`- [ ]`) syntax.
>
> Part of initiative: [`docs/initiatives/glitch-spot/INITIATIVE.md`](../../initiatives/glitch-spot/INITIATIVE.md).
> Follow-up to: [`2026-05-22-glitch-spot-v0.2-drawing.md`](./2026-05-22-glitch-spot-v0.2-drawing.md). M1–M12 of that plan are complete; this slice pulls one piece of v0.3 scope (annotated PNG generation) forward into v0.2 before the merge.

**Goal:** Generate `glitch-spot-<id>-annotated.png` alongside the clean PNG on save, with rectangle borders + each annotation's note text rendered onto a clone of the clean image. Populate `snapshot.images.annotated` with its absolute path so downstream consumers (agents, future relay) can use it directly.

**Architecture:** New Rust module `snapshot::annotate` decodes the clean PNG bytes, draws rectangle borders and text labels using `imageproc` + `ab_glyph`, encodes back to PNG. A bundled `DejaVuSans.ttf` (Bitstream Vera license, public-domain modifications) lives in `src-tauri/assets/` and is embedded at compile time via `include_bytes!`. This is the same font the `imageproc` maintainers ship with their examples, so it is known-compatible. `snapshot::build::build_and_write` invokes the annotator after writing the clean PNG, writes the annotated PNG to `/tmp/glitch-spot-<id>-annotated.png`, and stores the path in `Images.annotated`.

**Coord space:** annotations on the `Snapshot` are already in *physical pixels* (multiplied by `frame.scale_factor` at the top of `build_and_write`). The clean PNG is at physical resolution. Coordinates drop in without further scaling.

---

## Scope

**In scope:**

1. New `snapshot::annotate` module with a pure `render_annotated_png(clean_png, annotations, scale_factor) -> Result<Vec<u8>, _>` function.
2. Rectangle borders (3 px red `#ff3b30`, no fill).
3. Note text labels rendered on a semi-opaque white panel below each rect (auto-flips above when it would clip the bottom of the image).
4. Bundled `DejaVuSans.ttf` at `src-tauri/assets/DejaVuSans.ttf`.
5. New deps: `imageproc = "0.25"`, `ab_glyph = "0.2"`.
6. `build_and_write` writes the annotated PNG to `/tmp/glitch-spot-<id>-annotated.png` and sets `Images.annotated` accordingly.
7. Tests: annotated PNG is valid PNG, contains red border pixels at expected coords, file is written, `Images.annotated` is no longer `None`.

**Explicitly out of scope (still deferred to v0.3 or later):**

- Filesystem persistence to `~/.glitch-spot/snapshots/{id}/` — v0.3.
- 24 h auto-cleanup — v0.3.
- Text wrapping. Multi-line via explicit `\n` only; long single lines may extend past the panel.
- Variable font weights / fancy styling. One static font, one size.
- Per-annotation color picker — single hard-coded red for both UI and annotated PNG.

---

## Visual style (locked here so the impl + tests stay aligned)

| Element | Spec |
|---|---|
| Border color | `Rgba([255, 59, 48, 255])` — matches the UI's `#ff3b30` |
| Border width | 3 physical pixels |
| Border fill | none (transparent interior) |
| Text font | DejaVu Sans (Book), bundled from imageproc's example font |
| Text size | `24 * (scale_factor / 2.0).max(1.0)` physical px (so ~12 pt at 2× Retina, scales reasonably on 1× displays) |
| Text color | `Rgba([17, 17, 17, 255])` (`#111`) |
| Panel background | `Rgba([255, 255, 255, 245])` (slightly translucent white) |
| Panel padding | 8 physical px |
| Panel gap from rect | 6 physical px (matches the UI's `NOTE_OFFSET`) |
| Panel placement | below the rect, flipped above if `rect.y + rect.h + panel_height + gap > image_height` |
| Panel max width | min(rect.w, 480) physical px when the rect is at least 240 px wide; otherwise 240 px |
| Empty notes | rect border still drawn; no panel rendered |

---

## Verification mode

| Step | Mode | Why |
|---|---|---|
| Deps + font | `cargo check` | Compilation gate |
| `render_annotated_png` | TDD (cargo test) | Pure function, asserts on output pixels |
| `build_and_write` wire-up | TDD (cargo test) extending existing tests | Schema invariant change |
| End-to-end | Manual visual: open the `/tmp/*-annotated.png` after a save | Font rendering / layout can only be eyeballed |

---

## Milestones and tasks

### M13.1: Add deps + bundled font

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/assets/DejaVuSans.ttf` (binary)
- Create: `src-tauri/assets/LICENSE-DejaVu.txt` (Bitstream Vera + DejaVu license text)

- [ ] **Step 1: Fetch DejaVu Sans TTF.** Download from <https://raw.githubusercontent.com/image-rs/imageproc/master/examples/DejaVuSans.ttf> (the imageproc maintainers' canonical example font, ~757 KB) and place at `src-tauri/assets/DejaVuSans.ttf`. Download the DejaVu license text from <https://raw.githubusercontent.com/dejavu-fonts/dejavu-fonts/master/LICENSE> to `src-tauri/assets/LICENSE-DejaVu.txt`.

- [ ] **Step 2: Verify the TTF is valid:**

```bash
file src-tauri/assets/DejaVuSans.ttf
# expected: TrueType Font data ...
```

- [ ] **Step 3: Add deps to `src-tauri/Cargo.toml`:**

```toml
imageproc = { version = "0.25", default-features = false }
ab_glyph = "0.2"
```

(Disable `imageproc`'s default features to avoid pulling in unused image-format support; we only need drawing primitives.)

- [ ] **Step 4: Run `cargo check` to confirm the dependency tree resolves:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5
```

Expected: exit 0.

- [ ] **Step 5: Commit:**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/assets/
git commit -m "Bundle DejaVu Sans and add imageproc/ab_glyph for annotated PNG"
```

---

### M13.2: TDD — `snapshot::annotate::render_annotated_png`

**Files:**
- Create: `src-tauri/src/snapshot/annotate.rs`
- Modify: `src-tauri/src/snapshot/mod.rs` (register `pub mod annotate;`)
- Modify: `src-tauri/src/snapshot/tests.rs` (add tests)

- [ ] **Step 1: Generate a synthetic test image programmatically inside the test module** (a small all-white RGBA PNG, ~64×64) to avoid needing a real screenshot fixture. Use the `image` crate's encoder.

- [ ] **Step 2: Add failing tests at the bottom of `src-tauri/src/snapshot/tests.rs`:**

```rust
use crate::snapshot::annotate::render_annotated_png;
use image::Rgba;

fn solid_white_png(width: u32, height: u32) -> Vec<u8> {
    let img: image::RgbaImage =
        image::ImageBuffer::from_pixel(width, height, Rgba([255u8, 255, 255, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn annotated_png_round_trips_with_no_annotations() {
    let clean = solid_white_png(32, 32);
    let bytes = render_annotated_png(&clean, &[], 2.0).unwrap();
    assert!(bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "must be a PNG");
    // Round-trips to a 32x32 image, all white still.
    let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(decoded.width(), 32);
    assert_eq!(decoded.height(), 32);
    assert_eq!(decoded.get_pixel(0, 0), &Rgba([255, 255, 255, 255]));
}

#[test]
fn annotated_png_draws_red_pixels_on_rect_border() {
    let clean = solid_white_png(64, 64);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords { x: 10.0, y: 10.0, w: 20.0, h: 20.0 },
        note: String::new(),
    }];
    let bytes = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // Top-left corner of the rect should be red.
    let p = img.get_pixel(10, 10);
    assert_eq!(p, &Rgba([255, 59, 48, 255]), "top-left of rect must be red, got {p:?}");
    // Center inside the rect should NOT be red (no fill).
    let center = img.get_pixel(20, 20);
    assert_ne!(center, &Rgba([255, 59, 48, 255]), "rect interior must not be filled");
}

#[test]
fn annotated_png_renders_note_panel_for_non_empty_notes() {
    let clean = solid_white_png(200, 200);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords { x: 20.0, y: 20.0, w: 60.0, h: 30.0 },
        note: "hello".into(),
    }];
    let bytes = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // Below the rect (panel area), some pixel should be close to the panel background.
    // Panel background is rgba(255,255,255,245); over a white image it composites to white,
    // so instead we look for dark text pixels in the panel region.
    let panel_top_y = 20 + 30 + 6; // rect.y + rect.h + gap
    let mut found_dark = false;
    for y in panel_top_y..(panel_top_y + 40).min(200) {
        for x in 20..(20 + 80).min(200) {
            let pix = img.get_pixel(x as u32, y as u32);
            if pix.0[0] < 80 && pix.0[1] < 80 && pix.0[2] < 80 {
                found_dark = true;
                break;
            }
        }
        if found_dark { break; }
    }
    assert!(found_dark, "expected dark text pixels in the panel region below the rect");
}

#[test]
fn annotated_png_skips_panel_for_empty_notes() {
    let clean = solid_white_png(200, 200);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords { x: 20.0, y: 20.0, w: 60.0, h: 30.0 },
        note: String::new(),
    }];
    let bytes = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // No panel ⇒ region below the rect stays white.
    let panel_top_y = 20 + 30 + 6;
    let pix = img.get_pixel(30, panel_top_y as u32 + 5);
    assert_eq!(pix, &Rgba([255, 255, 255, 255]), "empty note ⇒ no panel pixels");
}
```

- [ ] **Step 3: Run the tests; they should fail because `annotate` doesn't exist yet:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot 2>&1 | tail -15
```

Expected: compilation failure pointing at `crate::snapshot::annotate` not found.

- [ ] **Step 4: Create `src-tauri/src/snapshot/annotate.rs`:**

```rust
// ABOUTME: Renders rectangle borders + note text labels onto a clone of the captured
// ABOUTME: clean PNG to produce the v0.2 annotated PNG for snapshot.images.annotated.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{ImageError, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect as IpRect;

use crate::snapshot::Annotation;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
const BORDER_COLOR: Rgba<u8> = Rgba([255, 59, 48, 255]);
const TEXT_COLOR: Rgba<u8> = Rgba([17, 17, 17, 255]);
const PANEL_BG: Rgba<u8> = Rgba([255, 255, 255, 245]);
const BORDER_WIDTH: i32 = 3;
const PANEL_PADDING: i32 = 8;
const PANEL_GAP: i32 = 6;
const BASE_FONT_PX: f32 = 24.0;

#[derive(Debug, thiserror::Error)]
pub enum AnnotateError {
    #[error("image decode/encode: {0}")]
    Image(#[from] ImageError),
    #[error("bundled font failed to parse")]
    Font,
}

pub fn render_annotated_png(
    clean_png: &[u8],
    annotations: &[Annotation],
    scale_factor: f32,
) -> Result<Vec<u8>, AnnotateError> {
    let mut img = image::load_from_memory_with_format(clean_png, image::ImageFormat::Png)?
        .to_rgba8();
    let font = FontRef::try_from_slice(FONT_BYTES).map_err(|_| AnnotateError::Font)?;
    let font_px = BASE_FONT_PX * (scale_factor / 2.0).max(1.0);
    let scale = PxScale::from(font_px);

    for anno in annotations {
        draw_rect_border(&mut img, anno);
        if !anno.note.is_empty() {
            draw_note_panel(&mut img, anno, &font, scale);
        }
    }

    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

fn draw_rect_border(img: &mut RgbaImage, anno: &Annotation) {
    let x = anno.coords.x.round() as i32;
    let y = anno.coords.y.round() as i32;
    let w = anno.coords.w.round() as i32;
    let h = anno.coords.h.round() as i32;
    if w <= 0 || h <= 0 { return; }
    // Stack BORDER_WIDTH hollow rects to thicken the line without drawing a fill.
    for i in 0..BORDER_WIDTH {
        let rx = x + i;
        let ry = y + i;
        let rw = (w - 2 * i).max(1) as u32;
        let rh = (h - 2 * i).max(1) as u32;
        if let Some(rect) = checked_rect(rx, ry, rw, rh) {
            draw_hollow_rect_mut(img, rect, BORDER_COLOR);
        }
    }
}

fn checked_rect(x: i32, y: i32, w: u32, h: u32) -> Option<IpRect> {
    if w == 0 || h == 0 { return None; }
    Some(IpRect::at(x, y).of_size(w, h))
}

fn draw_note_panel(
    img: &mut RgbaImage,
    anno: &Annotation,
    font: &FontRef<'_>,
    scale: PxScale,
) {
    let lines: Vec<&str> = anno.note.lines().collect();
    if lines.is_empty() { return; }

    let scaled = font.as_scaled(scale);
    let line_h = (scaled.height() + scaled.line_gap()).round() as i32;
    let max_line_w = lines
        .iter()
        .map(|l| text_width(font, scale, l))
        .fold(0_i32, |acc, w| acc.max(w));

    let rect_x = anno.coords.x.round() as i32;
    let rect_y = anno.coords.y.round() as i32;
    let rect_h = anno.coords.h.round() as i32;
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;

    let panel_w = (max_line_w + 2 * PANEL_PADDING).max(120);
    let panel_h = (line_h * lines.len() as i32) + 2 * PANEL_PADDING;

    // Default: panel sits below the rect. Flip above if it would overflow.
    let mut panel_x = rect_x;
    let mut panel_y = rect_y + rect_h + PANEL_GAP;
    if panel_y + panel_h > img_h {
        panel_y = (rect_y - panel_h - PANEL_GAP).max(0);
    }
    if panel_x + panel_w > img_w {
        panel_x = (img_w - panel_w).max(0);
    }
    if panel_x < 0 { panel_x = 0; }

    if let Some(rect) = checked_rect(panel_x, panel_y, panel_w as u32, panel_h as u32) {
        draw_filled_rect_mut(img, rect, PANEL_BG);
    }

    let text_x = panel_x + PANEL_PADDING;
    let mut text_y = panel_y + PANEL_PADDING;
    for line in lines {
        draw_text_mut(img, TEXT_COLOR, text_x, text_y, scale, font, line);
        text_y += line_h;
    }
}

fn text_width(font: &FontRef<'_>, scale: PxScale, s: &str) -> i32 {
    let scaled = font.as_scaled(scale);
    let mut w = 0.0_f32;
    for c in s.chars() {
        let g = scaled.scaled_glyph(c);
        w += scaled.h_advance(g.id);
    }
    w.ceil() as i32
}
```

- [ ] **Step 5: Register the module in `src-tauri/src/snapshot/mod.rs`:**

```rust
pub mod annotate;
pub mod build;
```

- [ ] **Step 6: Run tests, confirm all green:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot 2>&1 | tail -20
```

Expected: previous 7 tests + 4 new tests = 11 green.

- [ ] **Step 7: Commit:**

```bash
git add src-tauri/src/snapshot/annotate.rs src-tauri/src/snapshot/mod.rs src-tauri/src/snapshot/tests.rs
git commit -m "Add snapshot::annotate to render rect borders and note labels"
```

---

### M13.3: Wire annotate into `build_and_write`

**Files:**
- Modify: `src-tauri/src/snapshot/build.rs`
- Modify: `src-tauri/src/snapshot/tests.rs` (update `annotated_serializes_as_null_when_none` semantics + add a new assertion)

- [ ] **Step 1: Extend `build_and_write` to call `annotate::render_annotated_png` after the clean PNG is written, write the result, and populate `Images.annotated`:**

```rust
// inside build_and_write, after the existing annotations Vec is built:
let annotated_path = PathBuf::from(format!("/tmp/glitch-spot-{}-annotated.png", id));
let annotated_bytes = crate::snapshot::annotate::render_annotated_png(
    &frame.png,
    &annotations,
    frame.scale_factor,
)
.map_err(|e| std::io::Error::other(e.to_string()))?;
std::fs::write(&annotated_path, &annotated_bytes)?;

// ...inside Snapshot { images: Images { ... } }:
annotated: Some(annotated_path.to_string_lossy().into_owned()),
```

(Define a new `BuildError::Annotate(AnnotateError)` variant if you don't want to flatten through `io::Error::other`.)

- [ ] **Step 2: Update the existing schema test.**

The current `annotated_serializes_as_null_when_none` test still passes — it constructs a `Snapshot` with `annotated: None`, which is still legal in the type. Keep it. **Add** a new test verifying that `build_and_write` populates `annotated` with a path that exists and is a PNG:

```rust
#[test]
fn build_writes_annotated_png_alongside_clean() {
    let snap = build_and_write(&fake_frame(), "frozen", vec![], "macos", "26.0").unwrap();
    let annotated = snap.images.annotated.expect("annotated path must be set");
    let path = std::path::Path::new(&annotated);
    assert!(path.exists(), "annotated PNG must be written");
    assert!(
        path.to_string_lossy().ends_with("-annotated.png"),
        "annotated path must end with -annotated.png, got {}",
        annotated
    );
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "must be a PNG");
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(&snap.images.clean);
}
```

- [ ] **Step 3: Run tests:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot 2>&1 | tail -15
```

Expected: 12 green (11 + 1).

- [ ] **Step 4: Commit:**

```bash
git add src-tauri/src/snapshot/build.rs src-tauri/src/snapshot/tests.rs
git commit -m "Populate snapshot.images.annotated in build_and_write"
```

---

### M13.4: Run all gates

- [ ] **Step 1: Run the full automated gate set:**

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml --lib 2>&1 | tail -5
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --manifest-path src-tauri/Cargo.toml --no-deps -- -D warnings 2>&1 | tail -5
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check --manifest-path src-tauri/Cargo.toml
npm test 2>&1 | tail -5
npx tsc --noEmit
PATH="$HOME/.cargo/bin:$PATH" npm run tauri build 2>&1 | tail -5
```

All six must exit 0.

- [ ] **Step 2: Commit if any auto-formatting happened:**

```bash
git status
# if anything modified, commit it.
```

---

### M13.5: Visual acceptance

- [ ] **Step 1: Launch the built app:**

```bash
pkill -KILL -f glitch_spot 2>/dev/null
DYLD_FALLBACK_LIBRARY_PATH=/usr/lib/swift src-tauri/target/release/glitch_spot &
```

- [ ] **Step 2: Live mode** — `Ctrl+Opt+Cmd+G`, draw 2 rectangles, add a note to each, `Cmd+Return`. Paste the JSON somewhere; confirm `images.annotated` is a path like `/tmp/glitch-spot-snap_*-annotated.png`. `open` that file. Rectangles must have red 3 px borders; each note must appear in a white panel below its rect with readable DejaVu Sans text.

- [ ] **Step 3: Frozen mode** — `Ctrl+Opt+Cmd+H`, repeat. Annotated PNG should show the captured screenshot with overlaid rect borders + notes (no toolbar / textarea chrome).

- [ ] **Step 4: Edge case** — draw a rectangle very close to the bottom of the screen. Save. Open the annotated PNG. The note panel must appear *above* the rect (auto-flip).

- [ ] **Step 5: Edge case** — draw a rectangle with an empty note. Save. Annotated PNG shows the rect border but no panel.

- [ ] **Step 6: Mark slice done** (Dmitry's confirmation, then):

```bash
git commit --allow-empty -m "v0.2 annotated-PNG follow-up acceptance pass"
```

---

## After this plan

- Push `v0.2-drawing` (the branch carries M1–M13 commits) and open the PR against `main`.
- Also open the v0.1-foundation PR against `main`. Merge order: v0.1 first, then v0.2.
- v0.3-persistence's responsibilities shrink: it now owns only filesystem persistence (`~/.glitch-spot/snapshots/{id}/`) + 24 h auto-cleanup. Annotated-PNG generation has already shipped.
