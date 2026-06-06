# Redacting Regions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users cover sensitive regions of a capture so the covered pixels are destroyed in **both** the clean and annotated images, with a persisted three-way style setting (solid fill / pixelated / branded pixelated).

**Architecture:** A `Redact` draw-mode toggle in the overlay produces redaction rectangles (same rect machinery as annotations, distinguished by a `kind` field). `snapshot::build::build()` is the sole producer of both escaping images; it decodes the clean PNG, destructively covers each redaction region, encodes that redacted image as the clean PNG, then renders annotations on top of the redacted base for the annotated PNG. Style is a persisted setting controlled from both an overlay toolbar split-control and a tray submenu.

**Tech Stack:** Rust (Tauri 2, `image`/`imageproc` crates), React + TypeScript (inline styles), Vitest, `cargo test`.

**Spec:** `docs/seeds/2026-06-06-redaction-design.md`

---

## File Structure

**Rust (create):**
- `src-tauri/src/snapshot/redact.rs` — pure cover functions (solid / pixelate / branded) + `apply_redaction` dispatch + `Region`.

**Rust (modify):**
- `src-tauri/src/settings.rs` — `RedactionStyle` enum, persistence, `[snapshot] redaction_style`.
- `src-tauri/src/snapshot/mod.rs` — `RedactionRegion` struct, `Snapshot.redactions` field, declare `redact` module, agent-prompt sentence.
- `src-tauri/src/snapshot/annotate.rs` — extract `render_annotations_onto(RgbaImage, …)` from `render_annotated_png`.
- `src-tauri/src/snapshot/build.rs` — `IncomingAnnotation.kind`, `build()` gains `redaction_style` param, partition + redact + fast path, populate `redactions`.
- `src-tauri/src/lib.rs` — pass style to `build()`, `get_/set_redaction_style` commands, `do_set_redaction_style`, registration.
- `src-tauri/src/tray.rs` — `Redaction style` submenu + handler arm.

**TypeScript (modify):**
- `src/overlay/annotations/types.ts` — `Rect.kind`, `BEGIN_CREATE.kind`.
- `packages/electron/src/types.ts` — `AnnotationPayload.kind`.
- `src/overlay/annotations/label.ts` — `annotationLabels(rects)` helper.
- `src/overlay/annotations/state.ts` — `BEGIN_CREATE` stores `kind`.
- `src/overlay/annotations/DrawingLayer.tsx` — pass `kind` on create; render redaction preview per style; label via `annotationLabels`.
- `src/overlay/bridge.ts` — `RedactionStyle` type, `get/setRedactionStyle`, Electron `saveSnapshot` redacts clean.
- `src/overlay/annotate.ts` — `redactCanvas` + derive annotated from redacted clean.
- `src/overlay/Toolbar.tsx` — `Redact` split control (toggle + style picker).
- `src/overlay/Overlay.tsx` — `redactMode` + `redactionStyle` state, wiring, payload `kind`.

---

## Task 1: `RedactionStyle` setting (Rust)

**Files:**
- Modify: `src-tauri/src/settings.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src-tauri/src/settings.rs`:

```rust
    #[test]
    fn missing_redaction_style_yields_branded_default() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_redaction_style(), RedactionStyle::Branded);
    }

    #[test]
    fn explicit_redaction_style_solid_is_parsed() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[snapshot]\nredaction_style = \"solid\"\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_redaction_style(), RedactionStyle::Solid);
    }

    #[test]
    fn unknown_redaction_style_falls_back_to_branded() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[snapshot]\nredaction_style = \"bogus\"\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_redaction_style(), RedactionStyle::Branded);
    }

    #[test]
    fn save_redaction_style_roundtrip() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_redaction_style(temp.path(), RedactionStyle::Pixelated).unwrap();
        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_redaction_style(), RedactionStyle::Pixelated);
    }

    #[test]
    fn save_redaction_style_preserves_other_sections() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_provider(temp.path(), ProviderKind::Ngrok).unwrap();
        settings.save_clipboard_format(temp.path(), ClipboardFormat::Image).unwrap();
        settings.save_redaction_style(temp.path(), RedactionStyle::Solid).unwrap();
        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_provider(), ProviderKind::Ngrok, "provider preserved");
        assert_eq!(reloaded.current_clipboard_format(), ClipboardFormat::Image, "format preserved");
        assert_eq!(reloaded.current_redaction_style(), RedactionStyle::Solid, "style applied");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib settings::tests::`
Expected: FAIL — `RedactionStyle` and the methods don't exist (compile error).

- [ ] **Step 3: Add the enum**

After the `ClipboardFormat` impl block (around line 89) in `src-tauri/src/settings.rs`:

```rust
/// How a redaction region is covered in the saved images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactionStyle {
    /// Opaque dark box — destroys the underlying content.
    Solid,
    /// Averaged mosaic. Safe at coarse block sizes; not guaranteed against a
    /// determined attacker on known-charset text.
    Pixelated,
    /// Accent glitch mosaic whose pixels are independent of the source content —
    /// destroys the underlying content. Default.
    #[default]
    Branded,
}

impl RedactionStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Pixelated => "pixelated",
            Self::Branded => "branded",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "solid" => Some(Self::Solid),
            "pixelated" => Some(Self::Pixelated),
            "branded" => Some(Self::Branded),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Add config field, struct field, load + save + write**

In `SnapshotSection` (around line 123) add:

```rust
    #[serde(default)]
    redaction_style: Option<String>,
```

In `struct Settings` (around line 141) add after `clipboard_format`:

```rust
    redaction_style: RwLock<RedactionStyle>,
```

In `load_from`, after the `clipboard_format` extraction (around line 206) add:

```rust
        let redaction_style = parsed
            .get("snapshot")
            .and_then(|s| s.get("redaction_style"))
            .and_then(|v| v.as_str())
            .and_then(RedactionStyle::from_name)
            .unwrap_or_default();
```

In the `Ok(Settings { … })` constructor add:

```rust
            redaction_style: RwLock::new(redaction_style),
```

After `save_clipboard_format` (around line 284) add:

```rust
    pub fn current_redaction_style(&self) -> RedactionStyle {
        *self.redaction_style.read().expect("settings lock poisoned")
    }

    /// Update the in-memory redaction style AND persist it to disk.
    pub fn save_redaction_style(&self, dir: &Path, style: RedactionStyle) -> io::Result<()> {
        {
            let mut w = self.redaction_style.write().expect("settings lock poisoned");
            *w = style;
        }
        self.write_config_file(dir)
    }
```

In `write_config_file`'s `SnapshotSection { … }` literal add:

```rust
                redaction_style: Some(self.current_redaction_style().as_str().to_string()),
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib settings::tests::`
Expected: PASS (all settings tests, including the five new ones).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/settings.rs
git commit -m "feat(settings): add persisted RedactionStyle (solid/pixelated/branded)"
```

---

## Task 2: `redact` cover module (Rust)

**Files:**
- Create: `src-tauri/src/snapshot/redact.rs`
- Modify: `src-tauri/src/snapshot/mod.rs` (declare module)

- [ ] **Step 1: Declare the module**

In `src-tauri/src/snapshot/mod.rs`, with the other `pub mod` lines (around line 147):

```rust
pub mod redact;
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/snapshot/redact.rs` with ONLY the test module first (the rest follows in Step 4):

```rust
// ABOUTME: Destructive cover rendering for redaction regions on a captured frame.
// ABOUTME: Solid fill, averaged pixelation, and a source-independent branded glitch cover.

use image::{Rgba, RgbaImage};

use crate::settings::RedactionStyle;

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_red(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255]))
    }

    #[test]
    fn solid_fill_covers_region_and_leaves_outside_untouched() {
        let mut img = solid_red(20, 20);
        apply_redaction(&mut img, Region { x: 5, y: 5, w: 8, h: 8 }, RedactionStyle::Solid, 1.0);
        // Inside the region: no longer the original red.
        assert_ne!(img.get_pixel(8, 8), &Rgba([255, 0, 0, 255]), "region must be covered");
        // Outside: untouched.
        assert_eq!(img.get_pixel(0, 0), &Rgba([255, 0, 0, 255]), "outside must be untouched");
        assert_eq!(img.get_pixel(19, 19), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn pixelate_changes_region_and_preserves_outside() {
        // Half red, half blue inside what will be one big block — the average
        // must differ from both source colours, proving real averaging.
        let mut img = RgbaImage::from_fn(20, 20, |x, _| {
            if x < 10 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) }
        });
        apply_redaction(&mut img, Region { x: 0, y: 0, w: 20, h: 20 }, RedactionStyle::Pixelated, 1.0);
        let p = img.get_pixel(5, 5);
        assert_ne!(p, &Rgba([255, 0, 0, 255]));
        assert_ne!(p, &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn branded_is_independent_of_source_pixels() {
        // Same region covered on two different source images must yield identical
        // pixels inside the region — proving the cover carries no source info.
        let mut a = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 255]));
        let mut b = RgbaImage::from_pixel(20, 20, Rgba([0, 255, 0, 255]));
        let region = Region { x: 2, y: 2, w: 16, h: 16 };
        apply_redaction(&mut a, region, RedactionStyle::Branded, 1.0);
        apply_redaction(&mut b, region, RedactionStyle::Branded, 1.0);
        for y in 2..18u32 {
            for x in 2..18u32 {
                assert_eq!(a.get_pixel(x, y), b.get_pixel(x, y), "branded cover must not depend on source");
            }
        }
    }

    #[test]
    fn region_clamped_to_image_bounds_does_not_panic() {
        let mut img = solid_red(10, 10);
        // Region extends past the right/bottom edges and starts off the top-left.
        apply_redaction(&mut img, Region { x: -4, y: -4, w: 40, h: 40 }, RedactionStyle::Solid, 1.0);
        assert_ne!(img.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib snapshot::redact`
Expected: FAIL — `apply_redaction` / `Region` not defined (compile error).

- [ ] **Step 4: Implement the module**

Insert above the `#[cfg(test)]` line in `src-tauri/src/snapshot/redact.rs`:

```rust
const COVER_DARK: Rgba<u8> = Rgba([12, 12, 12, 255]); // #0c0c0c — DESIGN.md background
const ACCENT: Rgba<u8> = Rgba([238, 102, 51, 255]); // #ee6633
const ACCENT_DIM: Rgba<u8> = Rgba([120, 52, 26, 255]); // darkened accent for mosaic contrast
/// Pixelation/branded block size in CSS px. Coarse so a text glyph spans <1 block.
const BLOCK_CSS: f32 = 12.0;

/// A region to cover, in physical pixels on the captured image. May extend
/// outside the image; all writes are clamped.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Clamp the region to the image bounds, returning `(x0, y0, x1, y1)` as a
/// half-open pixel range. Returns `None` if the clamped region is empty.
fn clamp(region: Region, img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let iw = img.width() as i32;
    let ih = img.height() as i32;
    let x0 = region.x.max(0).min(iw);
    let y0 = region.y.max(0).min(ih);
    let x1 = (region.x + region.w).max(0).min(iw);
    let y1 = (region.y + region.h).max(0).min(ih);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// Cover `region` of `img` in place using `style`. Destructive — the original
/// pixels under the region are overwritten and cannot be recovered (except that
/// `Pixelated` averages them; see [`RedactionStyle`] docs).
pub fn apply_redaction(img: &mut RgbaImage, region: Region, style: RedactionStyle, scale_factor: f32) {
    let Some(bounds) = clamp(region, img) else { return };
    match style {
        RedactionStyle::Solid => solid_fill(img, bounds),
        RedactionStyle::Pixelated => pixelate(img, bounds, scale_factor),
        RedactionStyle::Branded => branded(img, bounds, scale_factor),
    }
}

fn block_px(scale_factor: f32) -> u32 {
    (BLOCK_CSS * scale_factor).round().max(2.0) as u32
}

fn solid_fill(img: &mut RgbaImage, (x0, y0, x1, y1): (u32, u32, u32, u32)) {
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, COVER_DARK);
        }
    }
}

fn pixelate(img: &mut RgbaImage, (x0, y0, x1, y1): (u32, u32, u32, u32), scale_factor: f32) {
    let block = block_px(scale_factor);
    let mut by = y0;
    while by < y1 {
        let by_end = (by + block).min(y1);
        let mut bx = x0;
        while bx < x1 {
            let bx_end = (bx + block).min(x1);
            // Average this block.
            let (mut r, mut g, mut b, mut a, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for y in by..by_end {
                for x in bx..bx_end {
                    let p = img.get_pixel(x, y).0;
                    r += p[0] as u64;
                    g += p[1] as u64;
                    b += p[2] as u64;
                    a += p[3] as u64;
                    n += 1;
                }
            }
            let avg = Rgba([
                (r / n) as u8,
                (g / n) as u8,
                (b / n) as u8,
                (a / n) as u8,
            ]);
            for y in by..by_end {
                for x in bx..bx_end {
                    img.put_pixel(x, y, avg);
                }
            }
            bx = bx_end;
        }
        by = by_end;
    }
}

fn branded(img: &mut RgbaImage, (x0, y0, x1, y1): (u32, u32, u32, u32), scale_factor: f32) {
    let block = block_px(scale_factor);
    let mut by = y0;
    let mut row = 0u32;
    while by < y1 {
        let by_end = (by + block).min(y1);
        let mut bx = x0;
        let mut col = 0u32;
        while bx < x1 {
            let bx_end = (bx + block).min(x1);
            // Deterministic palette pick from the block's grid position only —
            // never reads a source pixel, so the cover leaks zero content.
            let color = match (row.wrapping_add(col.wrapping_mul(3))) % 4 {
                0 => ACCENT,
                1 => ACCENT_DIM,
                _ => COVER_DARK,
            };
            for y in by..by_end {
                for x in bx..bx_end {
                    img.put_pixel(x, y, color);
                }
            }
            bx = bx_end;
            col += 1;
        }
        by = by_end;
        row += 1;
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib snapshot::redact`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/snapshot/redact.rs src-tauri/src/snapshot/mod.rs
git commit -m "feat(redact): cover module — solid/pixelate/branded region fills"
```

---

## Task 3: Snapshot data model — `redactions` field (Rust)

**Files:**
- Modify: `src-tauri/src/snapshot/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module referenced at the bottom of `mod.rs` — open `src-tauri/src/snapshot/tests.rs` and add:

```rust
    #[test]
    fn snapshot_without_redactions_field_deserializes_to_empty() {
        // Older snapshots have no `redactions` key; serde(default) must fill it.
        let json = r#"{
            "id": "x", "created_at": "t", "mode": "frozen",
            "annotations": [],
            "context": {
                "screen": {"width": 1, "height": 1, "scale_factor": 1.0, "display_index": 0},
                "os": {"platform": "macos", "version": "14"}
            },
            "images": {"clean": "c", "annotated": "a"}
        }"#;
        let snap: crate::snapshot::Snapshot = serde_json::from_str(json).unwrap();
        assert!(snap.redactions.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib snapshot::tests::snapshot_without_redactions_field`
Expected: FAIL — `Snapshot` has no `redactions` field (compile error).

- [ ] **Step 3: Add the struct + field + prompt sentence**

In `src-tauri/src/snapshot/mod.rs`, add to `struct Snapshot` after `pub annotations: Vec<Annotation>,`:

```rust
    #[serde(default)]
    pub redactions: Vec<RedactionRegion>,
```

Add a new struct after `Coords` (around line 37):

```rust
/// A user-redacted region, physical pixels relative to the captured image. The
/// pixels are destroyed in the saved images; these coordinates are recorded only
/// so an agent knows a region was intentionally hidden.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedactionRegion {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
```

Update `default_agent_prompt()` — replace the entire function so the single returned string literal ends with the new sentence (still one `.to_string()`):

```rust
pub fn default_agent_prompt() -> String {
    "Glitch Spot annotation snapshot. Each entry in 'annotations' is a labelled rectangle (by 'label': '01', '02', …) drawn over the captured screenshot. The 'note' field is the user's feedback for that region. 'images.annotated' is the screenshot with rectangles and badges rendered; 'images.clean' is the original. When addressing feedback, reference annotations by their label. Any entries in 'redactions' mark regions the user intentionally hid; their pixels are destroyed in both images — do not attempt to infer their contents.".to_string()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test --lib snapshot::tests::snapshot_without_redactions_field`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/snapshot/mod.rs src-tauri/src/snapshot/tests.rs
git commit -m "feat(snapshot): add redactions field + agent-prompt note"
```

---

## Task 4: Extract `render_annotations_onto` (Rust refactor)

**Files:**
- Modify: `src-tauri/src/snapshot/annotate.rs`

This refactor lets the redaction path render annotations onto an already-decoded redacted image without re-decoding. Behaviour-preserving; existing annotate tests must stay green.

- [ ] **Step 1: Refactor `render_annotated_png` to delegate**

In `src-tauri/src/snapshot/annotate.rs`, replace the body of `render_annotated_png` (the function spanning ~line 58-84) so it decodes then delegates, and add the new `render_annotations_onto`:

```rust
pub fn render_annotated_png(
    clean_png: &[u8],
    annotations: &[Annotation],
    scale_factor: f32,
) -> Result<(Vec<u8>, RgbaImage), AnnotateError> {
    let img =
        image::load_from_memory_with_format(clean_png, image::ImageFormat::Png)?.to_rgba8();
    render_annotations_onto(img, annotations, scale_factor)
}

/// Draw rectangle borders, numbered badges, and captions onto an already-decoded
/// image, then encode. Used directly by the redaction path (which decodes once,
/// covers the redacted regions, and renders annotations on the result) so the
/// image is not decoded a second time.
pub fn render_annotations_onto(
    mut img: RgbaImage,
    annotations: &[Annotation],
    scale_factor: f32,
) -> Result<(Vec<u8>, RgbaImage), AnnotateError> {
    let font = FontRef::try_from_slice(FONT_BYTES).map_err(|_| AnnotateError::Font)?;

    let border_width = (BORDER_WIDTH_CSS * scale_factor).round().max(1.0) as i32;
    let pill_w = (PILL_W_CSS * scale_factor).round() as i32;
    let pill_h = (PILL_H_CSS * scale_factor).round() as i32;
    let pill_r = (PILL_R_CSS * scale_factor).round() as i32;
    let badge_scale = PxScale::from(BADGE_FONT_PX_CSS * scale_factor);

    for anno in annotations {
        draw_rect_border(&mut img, anno, border_width);
        draw_pill_badge(&mut img, anno, pill_w, pill_h, pill_r, &font, badge_scale);
    }

    for anno in annotations {
        draw_caption(&mut img, anno, scale_factor, &font);
    }

    let png = crate::capture::encode_png(&img).map_err(|e| AnnotateError::Encode(e.to_string()))?;
    Ok((png, img))
}
```

- [ ] **Step 2: Run the existing annotate tests**

Run: `cd src-tauri && cargo test --lib snapshot::annotate`
Expected: PASS — all existing annotate tests unchanged (`returned_image_matches_the_encoded_png`, caption tests, etc.).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/snapshot/annotate.rs
git commit -m "refactor(annotate): extract render_annotations_onto for the redaction path"
```

---

## Task 5: `build()` redaction + fast path (Rust)

**Files:**
- Modify: `src-tauri/src/snapshot/build.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src-tauri/src/snapshot/build.rs` (create the module if absent — `build.rs` currently has no test module, so add one at the end of the file):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturedFrame;
    use crate::settings::RedactionStyle;
    use image::{Rgba, RgbaImage};

    fn red_frame_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255]));
        crate::capture::encode_png(&img).unwrap()
    }

    fn frame(w: u32, h: u32) -> CapturedFrame {
        CapturedFrame { width: w, height: h, scale_factor: 1.0, png: red_frame_png(w, h), display_index: 0 }
    }

    fn incoming(id: &str, kind: &str, x: f32, y: f32, w: f32, h: f32) -> IncomingAnnotation {
        IncomingAnnotation { id: id.into(), x, y, w, h, note: String::new(), kind: kind.into() }
    }

    #[test]
    fn no_redactions_uses_fast_path_clean_equals_frame() {
        let f = frame(40, 40);
        let base = std::path::PathBuf::from("/tmp/gs-test-fast");
        let (built, _img) = build(&f, "frozen", vec![incoming("a", "annotation", 4.0, 4.0, 10.0, 10.0)],
            "macos", "14", &base, None).unwrap();
        assert_eq!(built.clean_bytes, f.png, "fast path must clone the original frame");
        assert!(built.snapshot.redactions.is_empty());
        assert_eq!(built.snapshot.annotations.len(), 1);
    }

    #[test]
    fn redaction_destroys_pixels_in_clean_image() {
        let f = frame(40, 40);
        let base = std::path::PathBuf::from("/tmp/gs-test-redact");
        let (built, _img) = build(&f, "frozen", vec![incoming("r", "redaction", 5.0, 5.0, 20.0, 20.0)],
            "macos", "14", &base, None).unwrap();
        // The on-disk clean PNG must no longer equal the raw frame …
        assert_ne!(built.clean_bytes, f.png, "clean image must be redacted, not the raw frame");
        // … and the covered pixels must actually differ from the original red.
        let clean = image::load_from_memory(&built.clean_bytes).unwrap().to_rgba8();
        assert_ne!(clean.get_pixel(10, 10), &Rgba([255, 0, 0, 255]), "region pixels covered");
        // Outside the region stays the original red.
        assert_eq!(clean.get_pixel(38, 38), &Rgba([255, 0, 0, 255]), "outside untouched");
        // Redaction recorded, not added to annotations.
        assert_eq!(built.snapshot.redactions.len(), 1);
        assert!(built.snapshot.annotations.is_empty());
    }

    #[test]
    fn redactions_do_not_consume_annotation_labels() {
        let f = frame(60, 60);
        let base = std::path::PathBuf::from("/tmp/gs-test-labels");
        // annotation, redaction, annotation → labels must be 01, 02 (redaction skipped).
        let (built, _img) = build(&f, "frozen", vec![
            incoming("a1", "annotation", 2.0, 2.0, 8.0, 8.0),
            incoming("r1", "redaction", 12.0, 2.0, 8.0, 8.0),
            incoming("a2", "annotation", 22.0, 2.0, 8.0, 8.0),
        ], "macos", "14", &base, None).unwrap();
        let labels: Vec<&str> = built.snapshot.annotations.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, vec!["01", "02"]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib snapshot::build`
Expected: FAIL — `IncomingAnnotation` has no `kind`, `build` arity wrong (compile error).

- [ ] **Step 3: Add `kind` to `IncomingAnnotation`**

In `src-tauri/src/snapshot/build.rs`, in `struct IncomingAnnotation` add:

```rust
    #[serde(default = "default_incoming_kind")]
    pub kind: String,
```

And add the default fn above the struct:

```rust
fn default_incoming_kind() -> String {
    "annotation".to_string()
}
```

- [ ] **Step 4: Add `BuildError` variants**

Replace the `BuildError` enum with:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("annotate: {0}")]
    Annotate(#[from] AnnotateError),
    #[error("decode clean png: {0}")]
    Decode(#[from] image::ImageError),
    #[error("encode redacted png: {0}")]
    Encode(#[from] crate::capture::CaptureError),
}
```

- [ ] **Step 5: Rewrite `build()` with partition + fast path**

Update imports at the top of `build.rs`:

```rust
use crate::settings::RedactionStyle;
use crate::snapshot::annotate::{render_annotated_png, render_annotations_onto, AnnotateError};
use crate::snapshot::redact::{self, Region};
use crate::snapshot::{
    annotated_png_filename, clean_png_filename, default_agent_prompt, new_id, Annotation, Context,
    Coords, Images, OsInfo, RedactionRegion, ScreenInfo, Snapshot,
};
```

Change the `build` signature to add the style parameter:

```rust
pub fn build(
    frame: &CapturedFrame,
    mode: &str,
    annotations: Vec<IncomingAnnotation>,
    os_platform: &str,
    os_version: &str,
    base_dir: &Path,
    crop_origin: Option<(f32, f32)>,
    redaction_style: RedactionStyle,
) -> Result<(BuiltSnapshot, RgbaImage), BuildError> {
```

Replace the body from `let scale = frame.scale_factor;` through the `render_annotated_png` call with:

```rust
    let scale = frame.scale_factor;
    let (crop_x, crop_y) = crop_origin.unwrap_or((0.0, 0.0));

    // Split redactions out before labelling — redactions are not annotations and
    // must not consume label numbers.
    let (redaction_inc, annotation_inc): (Vec<IncomingAnnotation>, Vec<IncomingAnnotation>) =
        annotations.into_iter().partition(|a| a.kind == "redaction");

    let annotations: Vec<Annotation> = annotation_inc
        .into_iter()
        .enumerate()
        .map(|(idx, a)| Annotation {
            id: a.id,
            shape: "rect".into(),
            coords: Coords {
                x: a.x * scale - crop_x,
                y: a.y * scale - crop_y,
                w: a.w * scale,
                h: a.h * scale,
            },
            note: a.note,
            label: format_label(idx),
        })
        .collect();

    let redaction_regions: Vec<RedactionRegion> = redaction_inc
        .iter()
        .map(|a| RedactionRegion {
            x: a.x * scale - crop_x,
            y: a.y * scale - crop_y,
            w: a.w * scale,
            h: a.h * scale,
        })
        .collect();

    // The security core: both clean_bytes and annotated_image derive from the
    // SAME base. With no redactions, keep the fast path (clean is the raw frame,
    // no second encode). With redactions, decode → cover → that redacted image
    // becomes the clean PNG, and annotations render on a copy of it.
    let (clean_bytes, annotated_bytes, annotated_image) = if redaction_regions.is_empty() {
        let (annotated_bytes, annotated_image) =
            render_annotated_png(&frame.png, &annotations, scale)?;
        (frame.png.clone(), annotated_bytes, annotated_image)
    } else {
        let mut base =
            image::load_from_memory_with_format(&frame.png, image::ImageFormat::Png)?.to_rgba8();
        for r in &redaction_regions {
            redact::apply_redaction(
                &mut base,
                Region { x: r.x.round() as i32, y: r.y.round() as i32, w: r.w.round() as i32, h: r.h.round() as i32 },
                redaction_style,
                scale,
            );
        }
        let clean_bytes = crate::capture::encode_png(&base)?;
        let (annotated_bytes, annotated_image) =
            render_annotations_onto(base, &annotations, scale)?;
        (clean_bytes, annotated_bytes, annotated_image)
    };
```

Then update the `Snapshot { … }` literal to add `redactions: redaction_regions,` after `annotations: annotations,` (note: `annotations` is moved into the struct; if the borrow checker complains because `annotations` was used by reference above, it isn't — `render_*` borrow it, then we move it here, which is fine since the borrows ended). And update the returned `BuiltSnapshot { snapshot, clean_bytes, annotated_bytes }` to use the new `clean_bytes`/`annotated_bytes` bindings (remove the old `clean_bytes: frame.png.clone()` line).

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib snapshot::build`
Expected: PASS (3 new tests).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/snapshot/build.rs
git commit -m "feat(build): redact clean+annotated from one base; fast path when none"
```

---

## Task 6: Wire `save_snapshot`, commands, and tray (Rust)

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/tray.rs`

- [ ] **Step 1: Pass the style into `build()`**

In `src-tauri/src/lib.rs` `save_snapshot`, change the `build(...)` call (around line 200) to pass the style as the final argument:

```rust
    let (mut built, annotated_image) = snapshot::build::build(
        &frame,
        &mode,
        annotations,
        os_platform,
        &os_version,
        &base,
        crop_origin,
        state.settings.current_redaction_style(),
    )
    .map_err(|e| e.to_string())?;
```

- [ ] **Step 2: Add the `do_set_redaction_style` helper**

In `src-tauri/src/lib.rs`, after `do_set_tray_appearance` (around line 305), add:

```rust
/// Persists the redaction style. Shared by the Tauri command and the tray menu
/// handler.
pub(crate) fn do_set_redaction_style(state: &AppState, key: String) -> Result<(), String> {
    let style = settings::RedactionStyle::from_name(&key)
        .ok_or_else(|| format!("unknown redaction style: {key}"))?;
    state
        .settings
        .save_redaction_style(&state.config_dir, style)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Add the two commands**

In `src-tauri/src/lib.rs`, after `set_clipboard_format` (around line 810), add:

```rust
#[tauri::command]
fn get_redaction_style(state: tauri::State<'_, AppState>) -> String {
    state.settings.current_redaction_style().as_str().to_string()
}

#[tauri::command]
fn set_redaction_style(
    state: tauri::State<'_, AppState>,
    style: String,
) -> Result<(), String> {
    do_set_redaction_style(&state, style)
}
```

- [ ] **Step 4: Register the commands**

In the `tauri::generate_handler![ … ]` list (around line 1148) add after `set_clipboard_format,`:

```rust
            get_redaction_style,
            set_redaction_style,
```

- [ ] **Step 5: Add the tray submenu**

In `src-tauri/src/tray.rs` `build_menu`, after the `retention_submenu` block (around line 366) add:

```rust
    let current_redaction = state.settings.current_redaction_style();
    let redact_solid = CheckMenuItemBuilder::new("Solid fill")
        .id("redaction:solid")
        .checked(matches!(current_redaction, crate::settings::RedactionStyle::Solid))
        .build(app)?;
    let redact_pixelated = CheckMenuItemBuilder::new("Pixelated")
        .id("redaction:pixelated")
        .checked(matches!(current_redaction, crate::settings::RedactionStyle::Pixelated))
        .build(app)?;
    let redact_branded = CheckMenuItemBuilder::new("Branded pixelated")
        .id("redaction:branded")
        .checked(matches!(current_redaction, crate::settings::RedactionStyle::Branded))
        .build(app)?;
    let redaction_submenu = SubmenuBuilder::new(app, "Redaction style")
        .item(&redact_solid)
        .item(&redact_pixelated)
        .item(&redact_branded)
        .build()?;
```

Then add it to the menu by inserting after `.item(&retention_submenu)` (around line 417):

```rust
        .item(&redaction_submenu)
```

- [ ] **Step 6: Add the tray handler arm**

In the `on_menu_event` match in `src-tauri/src/tray.rs`, after the `"retention:24h" | … => { … }` arm (around line 606) add:

```rust
                "redaction:solid"
                | "redaction:pixelated"
                | "redaction:branded" => {
                    let key = event.id.as_ref().split(':').nth(1).unwrap_or("branded").to_string();
                    let state = app.state::<crate::AppState>();
                    if let Err(e) = crate::do_set_redaction_style(&state, key) {
                        eprintln!("tray: set redaction style failed: {e}");
                    }
                    refresh_menu(&app);
                }
```

- [ ] **Step 7: Build and run the full Rust suite**

Run: `cd src-tauri && cargo test --lib`
Expected: PASS — all library tests, no warnings about unused items.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/tray.rs
git commit -m "feat: wire redaction style into save, commands, and tray submenu"
```

---

## Task 7: Frontend types + reducer + label helper (TS)

**Files:**
- Modify: `src/overlay/annotations/types.ts`
- Modify: `packages/electron/src/types.ts`
- Modify: `src/overlay/annotations/label.ts`
- Modify: `src/overlay/annotations/state.ts`
- Test: `src/overlay/annotations/label.test.ts` (create), `src/overlay/annotations/state.test.ts` (existing or create)

- [ ] **Step 1: Write the failing label test**

Create `src/overlay/annotations/label.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { annotationLabels } from './label'

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
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/overlay/annotations/label.test.ts`
Expected: FAIL — `annotationLabels` is not exported.

- [ ] **Step 3: Add the types**

In `src/overlay/annotations/types.ts`, add `kind` to `Rect`:

```ts
export type Rect = {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
  kind: 'annotation' | 'redaction';
};
```

And add `kind` to the `BEGIN_CREATE` action:

```ts
  | { type: 'BEGIN_CREATE'; id: string; x: number; y: number; kind: 'annotation' | 'redaction' }
```

In `packages/electron/src/types.ts`, add `kind` to `AnnotationPayload`:

```ts
export interface AnnotationPayload {
  id: string
  x: number
  y: number
  w: number
  h: number
  note: string
  kind: 'annotation' | 'redaction'
}
```

- [ ] **Step 4: Add the label helper**

Append to `src/overlay/annotations/label.ts`:

```ts
// Returns, for each rect, its annotation label ('01', '02', …) or null when the
// rect is a redaction. Numbering counts only annotation-kind rects, so a
// redaction between two annotations does not renumber them.
export function annotationLabels(rects: { kind: 'annotation' | 'redaction' }[]): (string | null)[] {
  let n = 0;
  return rects.map((r) => (r.kind === 'redaction' ? null : formatLabel(n++)));
}
```

- [ ] **Step 5: Store `kind` in the reducer**

In `src/overlay/annotations/state.ts`, in the `BEGIN_CREATE` case, set the rect's kind from the action:

```ts
      const rect: Rect = {
        id: action.id,
        x: action.x,
        y: action.y,
        w: 0,
        h: 0,
        note: "",
        kind: action.kind,
      };
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npx vitest run src/overlay/annotations/label.test.ts src/overlay/annotations/state.test.ts`
Expected: PASS. (If `state.test.ts` constructs `BEGIN_CREATE` actions, add `kind: 'annotation'` to each so they compile — update those call sites.)

- [ ] **Step 7: Commit**

```bash
git add src/overlay/annotations/types.ts packages/electron/src/types.ts src/overlay/annotations/label.ts src/overlay/annotations/label.test.ts src/overlay/annotations/state.ts src/overlay/annotations/state.test.ts
git commit -m "feat(overlay): rect kind discriminator + annotation-only labels"
```

---

## Task 8: Bridge — redaction style + Electron clean redaction (TS)

**Files:**
- Modify: `src/overlay/bridge.ts`

- [ ] **Step 1: Add the type and interface methods**

In `src/overlay/bridge.ts`, after the `ClipboardFormat` type (line 7) add:

```ts
export type RedactionStyle = 'solid' | 'pixelated' | 'branded'
```

Add to the `OverlayBridge` interface:

```ts
  getRedactionStyle(): Promise<RedactionStyle>
  setRedactionStyle(style: RedactionStyle): Promise<void>
```

- [ ] **Step 2: Implement on `TauriBridge`**

Add to `class TauriBridge`:

```ts
  async getRedactionStyle(): Promise<RedactionStyle> {
    return invoke<string>('get_redaction_style') as Promise<RedactionStyle>
  }
  async setRedactionStyle(style: RedactionStyle): Promise<void> {
    await invoke('set_redaction_style', { style })
  }
```

- [ ] **Step 3: Implement on `ElectronBridge` + redact the clean image**

Add the getter/setter to `class ElectronBridge`:

```ts
  async getRedactionStyle(): Promise<RedactionStyle> {
    return 'branded'
  }
  async setRedactionStyle(_style: RedactionStyle): Promise<void> {}
```

Replace `ElectronBridge.saveSnapshot` so the clean image is redacted too (it currently ships the raw `cleanBase64` to the host):

```ts
  async saveSnapshot(_mode: string, annotations: AnnotationPayload[]): Promise<void> {
    if (!this.cleanBase64 || !this.cleanBlobUrl) {
      throw new Error('[glitch-spot] saveSnapshot called before first frame')
    }
    const { renderAnnotated, redactClean } = await import('./annotate')
    const style = await this.getRedactionStyle()
    // Redact the clean image FIRST, then derive the annotated image from the
    // redacted clean — both bytes that reach the host must be covered.
    const redactedCleanBase64 = await redactClean(
      this.cleanBlobUrl,
      annotations,
      window.devicePixelRatio,
      style,
    )
    const redactedCleanBlobUrl = base64PngToBlobUrl(redactedCleanBase64)
    try {
      const annotatedBase64 = await renderAnnotated(
        redactedCleanBlobUrl,
        annotations,
        window.devicePixelRatio,
      )
      window.glitchSpotBridge!.saveSnapshot(annotations, redactedCleanBase64, annotatedBase64)
    } finally {
      URL.revokeObjectURL(redactedCleanBlobUrl)
    }
  }
```

Add this helper near `frameToBlob` (top of file):

```ts
function base64PngToBlobUrl(b64: string): string {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  return URL.createObjectURL(new Blob([bytes], { type: 'image/png' }))
}
```

- [ ] **Step 4: Typecheck**

Run: `npx tsc --noEmit`
Expected: FAIL only on `redactClean` not yet existing in `annotate.ts` — that is implemented in Task 9. (If you are running tasks in order, defer the final typecheck to Task 9 Step 4.)

- [ ] **Step 5: Commit**

```bash
git add src/overlay/bridge.ts
git commit -m "feat(bridge): redaction-style methods; Electron redacts clean image"
```

---

## Task 9: Electron canvas redaction (TS)

**Files:**
- Modify: `src/overlay/annotate.ts`
- Test: `src/overlay/annotate.redact.test.ts` (create)

- [ ] **Step 1: Write the failing test**

Create `src/overlay/annotate.redact.test.ts`:

```ts
import { describe, it, expect, beforeAll } from 'vitest'
import { redactClean } from './annotate'

// jsdom lacks a real 2D canvas; back it with a tiny stub that records fills.
// We assert behaviour at the API level: redactClean returns base64 PNG-ish data
// and invokes fillRect over the region for a solid style.
beforeAll(() => {
  // Provide a deterministic devicePixelRatio.
  ;(globalThis as unknown as { devicePixelRatio: number }).devicePixelRatio = 1
})

describe('redactClean', () => {
  it('returns a base64 string and covers a solid region', async () => {
    const fills: Array<[number, number, number, number]> = []
    const fakeCtx = {
      drawImage: () => {},
      set fillStyle(_v: string) {},
      get fillStyle() { return '#000' },
      fillRect: (x: number, y: number, w: number, h: number) => fills.push([x, y, w, h]),
      beginPath: () => {}, roundRect: () => {}, fill: () => {},
      getImageData: () => ({ data: new Uint8ClampedArray(4) }),
      putImageData: () => {},
    }
    const fakeCanvas = {
      width: 0, height: 0,
      getContext: () => fakeCtx,
      toDataURL: () => 'data:image/png;base64,AAAA',
    }
    const origCreate = document.createElement.bind(document)
    ;(document as unknown as { createElement: (t: string) => unknown }).createElement = (t: string) =>
      t === 'canvas' ? fakeCanvas : origCreate(t)
    // Stub Image so loadImage resolves immediately.
    class FakeImage { naturalWidth = 40; naturalHeight = 40; onload: (() => void) | null = null; set src(_s: string) { this.onload?.() } }
    ;(globalThis as unknown as { Image: unknown }).Image = FakeImage as unknown

    const out = await redactClean('blob:x', [
      { id: 'r', x: 5, y: 5, w: 10, h: 10, note: '', kind: 'redaction' },
    ], 1, 'solid')

    expect(typeof out).toBe('string')
    expect(out.length).toBeGreaterThan(0)
    expect(fills.length).toBeGreaterThan(0)
  })
})
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/overlay/annotate.redact.test.ts`
Expected: FAIL — `redactClean` is not exported.

- [ ] **Step 3: Implement `redactClean` and the cover helper**

Add to `src/overlay/annotate.ts` (after the imports):

```ts
import type { RedactionStyle } from './bridge'

const COVER_DARK = '#0c0c0c'
const ACCENT = '#ee6633'
const ACCENT_DIM = '#78341a'
const BLOCK_CSS = 12

// Cover redaction regions on `ctx`. Mirrors src-tauri/src/snapshot/redact.rs so
// the Tauri and Electron outputs match. Operates in physical pixels.
function coverRegions(
  ctx: CanvasRenderingContext2D,
  regions: AnnotationPayload[],
  scaleFactor: number,
  style: RedactionStyle,
) {
  const block = Math.max(2, Math.round(BLOCK_CSS * scaleFactor))
  for (const r of regions) {
    const x = Math.round(r.x * scaleFactor)
    const y = Math.round(r.y * scaleFactor)
    const w = Math.round(r.w * scaleFactor)
    const h = Math.round(r.h * scaleFactor)
    if (w <= 0 || h <= 0) continue

    if (style === 'solid') {
      ctx.fillStyle = COVER_DARK
      ctx.fillRect(x, y, w, h)
      continue
    }

    if (style === 'branded') {
      let row = 0
      for (let by = y; by < y + h; by += block, row++) {
        let col = 0
        for (let bx = x; bx < x + w; bx += block, col++) {
          const bw = Math.min(block, x + w - bx)
          const bh = Math.min(block, y + h - by)
          const pick = (row + col * 3) % 4
          ctx.fillStyle = pick === 0 ? ACCENT : pick === 1 ? ACCENT_DIM : COVER_DARK
          ctx.fillRect(bx, by, bw, bh)
        }
      }
      continue
    }

    // pixelated: average each block from the source and refill it.
    for (let by = y; by < y + h; by += block) {
      for (let bx = x; bx < x + w; bx += block) {
        const bw = Math.min(block, x + w - bx)
        const bh = Math.min(block, y + h - by)
        const data = ctx.getImageData(bx, by, bw, bh).data
        let rr = 0, gg = 0, bb = 0, aa = 0
        const n = data.length / 4
        for (let i = 0; i < data.length; i += 4) {
          rr += data[i]; gg += data[i + 1]; bb += data[i + 2]; aa += data[i + 3]
        }
        ctx.fillStyle = `rgba(${Math.round(rr / n)}, ${Math.round(gg / n)}, ${Math.round(bb / n)}, ${(aa / n / 255).toFixed(3)})`
        ctx.fillRect(bx, by, bw, bh)
      }
    }
  }
}

/**
 * Draws the clean image and covers its redaction regions, returning a raw base64
 * PNG (no data: prefix). The annotated image must be rendered FROM this output,
 * so the covered pixels never survive in either image.
 */
export async function redactClean(
  cleanBlobUrl: string,
  annotations: AnnotationPayload[],
  scaleFactor: number,
  style: RedactionStyle,
): Promise<string> {
  const img = await loadImage(cleanBlobUrl)
  const canvas = document.createElement('canvas')
  canvas.width = img.naturalWidth
  canvas.height = img.naturalHeight
  const ctx = canvas.getContext('2d')!
  ctx.drawImage(img, 0, 0)
  const regions = annotations.filter((a) => a.kind === 'redaction')
  coverRegions(ctx, regions, scaleFactor, style)
  return canvas.toDataURL('image/png').split(',')[1]
}
```

Also update `renderAnnotated` so it ignores redaction-kind rects when drawing borders/badges/captions (redactions are already covered in the clean base). In its `annotations.forEach` passes, skip redactions — change the first line of each `forEach` body to early-return on redactions, e.g. at the top of both `annotations.forEach(...)` callbacks add:

```ts
    if (rect.kind === 'redaction') return
```

- [ ] **Step 4: Run the test + typecheck**

Run: `npx vitest run src/overlay/annotate.redact.test.ts`
Expected: PASS.

Run: `npx tsc --noEmit`
Expected: PASS (Task 8's `redactClean` reference now resolves).

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotate.ts src/overlay/annotate.redact.test.ts
git commit -m "feat(annotate): canvas redaction covers (solid/pixelate/branded)"
```

---

## Task 10: DrawingLayer redaction preview (TS)

**Files:**
- Modify: `src/overlay/annotations/DrawingLayer.tsx`

- [ ] **Step 1: Accept `redactionStyle` + a `redactMode` create kind**

In `src/overlay/annotations/DrawingLayer.tsx`, extend `Props`:

```ts
type Props = {
  state: AnnotationsState;
  dispatch: (action: AnnotationsAction) => void;
  clipRect?: ClipRect;
  onOutsideClick?: () => void;
  redactMode?: boolean;
  redactionStyle?: 'solid' | 'pixelated' | 'branded';
};
```

Destructure them: `export function DrawingLayer({ state, dispatch, clipRect, onOutsideClick, redactMode = false, redactionStyle = 'branded' }: Props)`.

In `onMouseDown`, set the kind on create:

```ts
      dispatch({ type: "BEGIN_CREATE", id: nextRectId(), x: startInside.x, y: startInside.y, kind: redactMode ? 'redaction' : 'annotation' });
```

- [ ] **Step 2: Render redaction rects distinctly**

Add a helper above the component for the preview background:

```ts
function redactionPreviewBg(style: 'solid' | 'pixelated' | 'branded'): string {
  if (style === 'solid') return '#0c0c0c'
  // A small mosaic/checker pattern stands in for the baked cover (we have no
  // live-screen pixels in the DOM). Branded uses the accent; pixelated greys.
  const a = style === 'branded' ? '%23ee6633' : '%23777777'
  const b = style === 'branded' ? '%2378341a' : '%23444444'
  return (
    "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12'>" +
    `<rect width='12' height='12' fill='${a}'/>` +
    `<rect width='6' height='6' fill='${b}'/>` +
    `<rect x='6' y='6' width='6' height='6' fill='${b}'/>` +
    "</svg>\")"
  )
}
```

In the `state.rects.map((r, idx) => …)` render, replace the single rect `<div style={{…}}>` so a redaction rect uses the cover background, no chromatic split, and renders no badge/note. Concretely, compute label and a redaction flag at the top of the map body:

```ts
      {state.rects.map((r, idx) => {
        const isRedaction = r.kind === 'redaction'
        const labels = annotationLabels(state.rects)
        const label = labels[idx]
```

(Import `annotationLabels` from `./label` alongside `formatLabel`.)

Then set the rect div style conditionally:

```ts
          style={{
            position: "absolute",
            left: r.x,
            top: r.y,
            width: r.w,
            height: r.h,
            border: isRedaction ? "1.5px solid rgba(238,102,51,0.5)" : RECT_BORDER,
            background: isRedaction ? redactionPreviewBg(redactionStyle) : RECT_FILL,
            backgroundSize: isRedaction && redactionStyle !== 'solid' ? '12px 12px' : undefined,
            boxSizing: "border-box",
            pointerEvents: "auto",
            cursor: "move",
            boxShadow: isRedaction ? "none" : "-2px 0 0 rgba(255, 0, 60, 0.4), 2px 0 0 rgba(0, 245, 255, 0.4)",
          }}
```

Inside the `r.w >= MIN_RECT_DIMENSION && r.h >= MIN_RECT_DIMENSION` block, gate the number badge on `!isRedaction` and use `label`:

```ts
              {!isRedaction && (
                <div aria-hidden style={{ /* …existing badge styles… */ }}>
                  {label}
                </div>
              )}
```

(Keep the delete-X button and resize handles for both kinds. Close the map callback with `)` → `})` and the surrounding `)}`.)

In the notes map below, skip redactions:

```ts
      {state.rects.map((r) =>
        r.kind === 'redaction' ? null : (
          <RectangleNote
            key={`${r.id}-note`}
            rect={r}
            containerHeight={containerHeight}
            autoFocus={!state.active && r.id === state.lastCreatedId}
            onChange={(note) => dispatch({ type: "SET_NOTE", id: r.id, note })}
          />
        ),
      )}
```

- [ ] **Step 3: Typecheck + existing DrawingLayer tests**

Run: `npx tsc --noEmit && npx vitest run src/overlay`
Expected: PASS. (Existing tests that don't pass `redactMode`/`redactionStyle` still work — both are optional with defaults.)

- [ ] **Step 4: Commit**

```bash
git add src/overlay/annotations/DrawingLayer.tsx
git commit -m "feat(overlay): redaction preview boxes + label numbering skips them"
```

---

## Task 11: Toolbar `Redact` split control (TS)

**Files:**
- Modify: `src/overlay/Toolbar.tsx`

- [ ] **Step 1: Extend props**

In `src/overlay/Toolbar.tsx`, add to `Props` and `import type { ClipboardFormat, RedactionStyle } from './bridge'`:

```ts
type Props = {
  rectCount: number
  clipboardFormat: ClipboardFormat
  onSave: () => void
  onChangeFormat: (format: ClipboardFormat) => void
  redactMode: boolean
  redactionStyle: RedactionStyle
  onToggleRedact: () => void
  onChangeRedactionStyle: (style: RedactionStyle) => void
}
```

Destructure the four new props in the function signature.

- [ ] **Step 2: Render the redact split control**

The toolbar currently returns `null` when `rectCount === 0`. The Redact toggle must be available before any rect exists, so change the early return to keep the toolbar visible whenever the overlay is interactive. Replace `if (rectCount === 0) return null` with nothing (always render), and instead gate only the rect-count label + Save split button on `rectCount > 0`.

Add redaction-style picker state near the existing dropdown state:

```ts
  const [redactMenuOpen, setRedactMenuOpen] = useState(false)
  const redactRef = useRef<HTMLDivElement>(null)
```

Add an outside-click effect mirroring the existing one (close `redactMenuOpen` on outside click).

Inside the toolbar's flex row, before the rect-count span, add the redact split control:

```ts
        <div ref={redactRef} style={{ position: 'relative', display: 'flex', borderRadius: 4 }}>
          <button
            data-testid="toolbar-redact-toggle"
            type="button"
            onClick={onToggleRedact}
            aria-pressed={redactMode}
            style={{
              padding: '6px 12px',
              background: redactMode ? '#ee6633' : 'transparent',
              color: redactMode ? '#fff' : '#e0e0e0',
              border: '1px solid rgba(255,255,255,0.20)',
              borderRight: 'none',
              borderRadius: '4px 0 0 4px',
              fontWeight: 600,
              cursor: 'pointer',
              whiteSpace: 'nowrap',
            }}
          >
            ▣ Redact
          </button>
          <button
            data-testid="toolbar-redact-chevron"
            type="button"
            onClick={() => setRedactMenuOpen((o) => !o)}
            style={{
              padding: '6px 8px',
              background: redactMode ? '#ee6633' : 'transparent',
              color: redactMode ? '#fff' : '#e0e0e0',
              border: '1px solid rgba(255,255,255,0.20)',
              borderRadius: '0 4px 4px 0',
              cursor: 'pointer',
              fontSize: 10,
            }}
          >
            {redactMenuOpen ? '▴' : '▾'}
          </button>
          {redactMenuOpen && (
            <div
              data-testid="toolbar-redact-dropdown"
              style={{
                position: 'absolute',
                top: 'calc(100% + 6px)',
                left: 0,
                background: '#1e1e1e',
                border: '1px solid rgba(255,255,255,0.10)',
                borderRadius: 6,
                boxShadow: '0 8px 24px rgba(0,0,0,0.7)',
                minWidth: 180,
                overflow: 'hidden',
                zIndex: 100,
              }}
            >
              {(['solid', 'pixelated', 'branded'] as RedactionStyle[]).map((style) => {
                const active = style === redactionStyle
                const labelText = style === 'solid' ? 'Solid fill' : style === 'pixelated' ? 'Pixelated' : 'Branded pixelated'
                return (
                  <div
                    key={style}
                    data-testid={`toolbar-redact-option-${style}`}
                    data-active={String(active)}
                    onClick={() => { setRedactMenuOpen(false); onChangeRedactionStyle(style) }}
                    style={{
                      padding: '9px 12px',
                      font: '13px/1 system-ui',
                      color: active ? '#fff' : '#e0e0e0',
                      background: active ? '#ee6633' : 'transparent',
                      cursor: 'pointer',
                      display: 'flex', alignItems: 'center', gap: 8,
                    }}
                  >
                    <span style={{ fontSize: 11, opacity: active ? 0.8 : 0 }}>✓</span>
                    {labelText}
                  </div>
                )
              })}
            </div>
          )}
        </div>
```

Wrap the existing rect-count span + Save split button in `{rectCount > 0 && ( … )}` so they only appear once a rect exists.

- [ ] **Step 3: Typecheck + existing toolbar tests**

Run: `npx tsc --noEmit && npx vitest run src/overlay`
Expected: type errors in `Overlay.tsx` (missing new props) — those are fixed in Task 12. Toolbar-only render tests for the redact control pass. If a Toolbar test file exists, add a render test asserting `toolbar-redact-toggle` is present at `rectCount === 0`.

- [ ] **Step 4: Commit**

```bash
git add src/overlay/Toolbar.tsx
git commit -m "feat(toolbar): Redact split control with style picker"
```

---

## Task 12: Overlay wiring (TS)

**Files:**
- Modify: `src/overlay/Overlay.tsx`

- [ ] **Step 1: Add state and fetch the style**

In `src/overlay/Overlay.tsx`, import the type: `import type { ClipboardFormat, RedactionStyle } from './bridge'`.

Add state near `clipboardFormat`:

```ts
  const [redactMode, setRedactMode] = useState(false)
  const [redactionStyle, setRedactionStyle] = useState<RedactionStyle>('branded')
```

In the mount `useEffect`, alongside `bridge.getClipboardFormat().then(setClipboardFormat)`, add:

```ts
    bridge.getRedactionStyle().then(setRedactionStyle).catch(console.error)
```

In `__glitchSpotPresent`, reset redact mode on each present (after `setShowDiscardModal(false)`):

```ts
      setRedactMode(false);
```

- [ ] **Step 2: Map `kind` into the save payload**

Update `handleSave`'s payload map to include `kind`:

```ts
    const payload: AnnotationPayload[] = annotations.rects.map((r) => ({
      id: r.id,
      x: r.x,
      y: r.y,
      w: r.w,
      h: r.h,
      note: r.note,
      kind: r.kind,
    }))
```

- [ ] **Step 3: Add the style-change handler**

After `handleFormatChange`, add:

```ts
  const handleRedactionStyleChange = async (style: RedactionStyle) => {
    try {
      await bridge.setRedactionStyle(style)
      setRedactionStyle(style)
    } catch (err) {
      console.error('setRedactionStyle failed', err)
    }
  }
```

- [ ] **Step 4: Pass props to both `DrawingLayer` and `Toolbar`**

In BOTH the window-mode branch and the default branch, update the `DrawingLayer` usage to pass `redactMode={redactMode}` and `redactionStyle={redactionStyle}`, and the `Toolbar` usage to pass the four new props:

```tsx
        <DrawingLayer
          state={annotations}
          dispatch={dispatch}
          /* window branch also keeps clipRect + onOutsideClick */
          redactMode={redactMode}
          redactionStyle={redactionStyle}
        />
        <Toolbar
          rectCount={annotations.rects.length}
          clipboardFormat={clipboardFormat}
          onSave={handleSave}
          onChangeFormat={handleFormatChange}
          redactMode={redactMode}
          redactionStyle={redactionStyle}
          onToggleRedact={() => setRedactMode((m) => !m)}
          onChangeRedactionStyle={handleRedactionStyleChange}
        />
```

- [ ] **Step 5: Typecheck + full overlay suite**

Run: `npx tsc --noEmit && npx vitest run`
Expected: PASS — whole TS suite green.

- [ ] **Step 6: Commit**

```bash
git add src/overlay/Overlay.tsx
git commit -m "feat(overlay): wire redact mode + style through to drawing and toolbar"
```

---

## Task 13: Full suite + security done-gate

**Files:** none (verification only)

- [ ] **Step 1: Run both full suites**

Run: `cd src-tauri && cargo test` then `cd .. && npx vitest run`
Expected: ALL PASS. Record exact pass counts.

- [ ] **Step 2: Clean-image exit-point grep (the security done-gate)**

Run:

```bash
grep -rn "frame.png\|latest_frame\|clean_bytes\|write_image\|annotated_image" src-tauri/src/lib.rs src-tauri/src/snapshot/
```

Confirm by reading the hits:
- `build()` is the **only** producer of `clean_bytes` and `annotated_image`, and both come from the same (redacted, when present) base.
- The cached raw `latest_frame` is read only to display the frame in the overlay and to feed `build()` — it is never written to disk or shared directly.
- The clipboard `Image` path uses `annotated_image` (redacted base), not the raw frame.

- [ ] **Step 3: Manual smoke test (all three modes × all three styles)**

Run the app: `PATH="$HOME/.cargo/bin:$PATH" npm run tauri:dev`

For each style (set via tray submenu AND via the toolbar chevron — confirm they stay in sync after reopening the overlay):
- Frozen capture (⌃⌥⌘H): draw one annotation + one redaction over visible text; save; open `~/.glitch-spot/snapshots/<id>/<id>_clean.png` and confirm the redacted region is covered in the **clean** image, and `<id>_annotated.png` shows the annotation with the same region covered.
- Live (⌃⌥⌘G) and Window (⌃⌥⌘J): repeat; confirm the covered region in both images.
- Confirm a save with **no** redactions still produces a clean image identical to the unannotated screenshot (fast path).

- [ ] **Step 4: Commit any doc/status updates**

```bash
git add -A && git commit -m "test: full-suite + security done-gate verification for redaction" || echo "nothing to commit"
```

---

## Notes for the implementer

- **TDD discipline:** every code task starts with a failing test (Dmitry's CLAUDE.md mandates it). The pixel-level Rust tests in Tasks 2 and 5 are the security regression guards — never weaken them.
- **The one rule that matters most:** `clean_bytes` must derive from the redacted base whenever a redaction exists. Task 5's `redaction_destroys_pixels_in_clean_image` test (`assert_ne!(built.clean_bytes, f.png)`) fails loudly if anyone reverts to `frame.png.clone()`.
- **Rust ↔ TS parity:** `redact.rs` and `annotate.ts`'s `coverRegions` must stay visually equivalent (same `#0c0c0c`/`#ee6633`/`#78341a`, same 12-CSS-px block, same branded palette pick `(row + col*3) % 4`). If you change one, change both.
- **Pixelated caveat:** documented in `RedactionStyle` doc-comment and the seed — do not "improve" the pixelate path into something finer-grained; coarse blocks are the safety property.
