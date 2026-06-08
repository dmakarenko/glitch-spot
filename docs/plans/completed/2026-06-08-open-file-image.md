# Open Image for Annotation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fourth tray-menu item, `Open Image…`, that opens any image file for annotation through the existing overlay/annotate/save pipeline.

**Architecture:** The opened image is letterboxed inside the work-area overlay, modeled on the existing **window mode** (dim surround + clip rect + chromatic glitch resolve + toolbar + redaction). The decoded file becomes a `CapturedFrame` at native resolution (`scale_factor: 1.0`); a separately-carried `coord_scale` (native px per overlay CSS px, downscale-only so always ≥ 1.0) maps annotation coordinates from overlay CSS into image pixels via `build()`'s existing `a.x * scale - crop_x` transform.

**Tech Stack:** Tauri v2 (Rust), `tauri-plugin-dialog`, the `image` crate, React + TypeScript overlay, Vitest, `cargo test`.

> Design seed: `docs/seeds/2026-06-08-open-file-image-design.md`.

---

## File Structure

- **Create** `src-tauri/src/open_image.rs` — pure, unit-tested helpers: `fit_image_rect`, `load_image_frame`, the `ImageSession` struct, `OpenImageError`.
- **Modify** `src-tauri/Cargo.toml` — add `tauri-plugin-dialog`; expand `image` features.
- **Modify** `src-tauri/src/overlay.rs` — add `ImageModePayload` + `show_image_mode` (next to `show_window_mode`).
- **Modify** `src-tauri/src/snapshot/build.rs` — add explicit `coord_scale` parameter to `build()`.
- **Modify** `src-tauri/src/lib.rs` — `mod open_image`; `AppState.pending_image`; `trigger_open_image` + `present_image` + `cursor_monitor_work_css`; `save_snapshot` `"image"` arm; manage-init.
- **Modify** `src-tauri/src/tray.rs` — `Open Image…` menu item + handler.
- **Modify** `src/picker/types.ts` — `ImageModePayload` TS type.
- **Modify** `src/overlay/Overlay.tsx` — `'image'` mode reusing the window-mode framed branch.
- **Modify** `src/overlay/Overlay.test.tsx` — image-mode render test.

---

## Task 1: Dependencies + dialog plugin + LSUIElement focus spike

This app is an `LSUIElement` (menu-bar accessory) with no main window. Native open panels from such apps often open behind everything or never take keyboard focus. **De-risk this first** — before any other task — because if the panel won't focus, the feature feels broken regardless of the rest.

**Files:**
- Modify: `src-tauri/Cargo.toml:20-45`
- Modify: `src-tauri/src/lib.rs:1101` (plugin registration), `src-tauri/src/tray.rs` (temporary spike button)

- [ ] **Step 1: Add the dialog plugin and expand image decode formats**

In `src-tauri/Cargo.toml`, change the `image` line and add the dialog plugin:

```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "webp", "bmp"] }
```

```toml
tauri-plugin-dialog = "2"
```

(Add the `tauri-plugin-dialog` line alongside the other `tauri-plugin-*` entries, e.g. after line 45.)

- [ ] **Step 2: Register the plugin**

In `src-tauri/src/lib.rs`, after `.plugin(tauri_plugin_notification::init())` (line ~1101):

```rust
        .plugin(tauri_plugin_dialog::init())
```

- [ ] **Step 3: Add a temporary spike trigger to the tray**

In `src-tauri/src/tray.rs` `build_menu`, add a throwaway item after `capture_window` (line ~312):

```rust
    let open_image_spike = MenuItemBuilder::new("Open Image (spike)")
        .id("open_image_spike")
        .build(app)?;
```

Add `.item(&open_image_spike)` to the menu builder right after `.item(&capture_window)` (line ~483), and add this handler arm in `on_menu_event` (next to `"capture_window"`):

```rust
                "open_image_spike" => {
                    use tauri_plugin_dialog::DialogExt;
                    #[cfg(target_os = "macos")]
                    unsafe {
                        use objc2::msg_send;
                        use objc2::runtime::AnyObject;
                        let cls = objc2::class!(NSApplication);
                        let ns_app: *mut AnyObject = msg_send![cls, sharedApplication];
                        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
                    }
                    app.dialog().file().pick_file(|path| {
                        eprintln!("[spike] picked: {path:?}");
                    });
                }
```

- [ ] **Step 4: Build and run the spike**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles. Then launch the app (`PATH="$HOME/.cargo/bin:$PATH" npm run tauri:dev`), click the tray, click "Open Image (spike)".

Verify manually: the open panel appears **in front** and accepts keyboard/mouse focus. Note in the task whether `activateIgnoringOtherApps:` was sufficient, or whether a temporary `setActivationPolicy(.regular)` flip around the dialog is also needed. If focus is fine without the `activate` call, keep it anyway (harmless) — real handler (Task 7) will reuse this pattern.

- [ ] **Step 5: Remove the spike item, keep the findings**

Delete the `open_image_spike` menu item, its `.item(...)`, and its handler arm. (The real menu item lands in Task 8.) Keep the dependency + plugin changes.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(deps): add dialog plugin and image decode formats"
```

---

## Task 2: `fit_image_rect` — pure fit math

**Files:**
- Create: `src-tauri/src/open_image.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod open_image;`)

- [ ] **Step 1: Create the module with the failing test**

Create `src-tauri/src/open_image.rs`:

```rust
// ABOUTME: Opening an image file for annotation: fit math, decode-to-frame, session.
// ABOUTME: Pure helpers (fit/decode) are unit-tested; orchestration lives in lib.rs.

/// Result of fitting an image into the overlay work area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitResult {
    /// Where to draw the image, in overlay CSS px: (x, y, w, h), centered.
    pub css_rect: (f32, f32, f32, f32),
    /// Native image px per overlay CSS px. Downscale-only, so always >= 1.0.
    pub coord_scale: f32,
    /// Native-px offset of the image's top-left within its own pixel space,
    /// derived so `a.x * coord_scale - crop_origin.0` lands in image pixels.
    pub crop_origin: (f32, f32),
}

/// Fit an image into the work area, downscale-only: images larger than the work
/// area shrink to fit (preserving aspect); images at or below it show at native
/// pixel size, centered. `coord_scale` is native px per CSS px and is clamped
/// to `>= 1.0` so annotation strokes never thin below native resolution.
pub fn fit_image_rect(work_css: (f32, f32), image: (u32, u32)) -> FitResult {
    let (ww, wh) = work_css;
    let (iw, ih) = (image.0 as f32, image.1 as f32);
    let fit = (ww / iw).min(wh / ih);
    let css_per_img = fit.min(1.0);
    let w = iw * css_per_img;
    let h = ih * css_per_img;
    let x = ((ww - w) / 2.0).max(0.0);
    let y = ((wh - h) / 2.0).max(0.0);
    let coord_scale = 1.0 / css_per_img;
    FitResult {
        css_rect: (x, y, w, h),
        coord_scale,
        crop_origin: (x * coord_scale, y * coord_scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 0.01, "expected {b}, got {a}");
    }

    #[test]
    fn large_image_downscales_to_fit_and_centers() {
        // Work 1000x800, image 4000x2000 (wider aspect) → width-constrained.
        let r = fit_image_rect((1000.0, 800.0), (4000, 2000));
        approx(r.css_rect.2, 1000.0); // w fills width
        approx(r.css_rect.3, 500.0); // h = 2000 * 0.25
        approx(r.css_rect.0, 0.0); // centered x
        approx(r.css_rect.1, 150.0); // centered y = (800-500)/2
        approx(r.coord_scale, 4.0); // 4000/1000
        approx(r.crop_origin.0, 0.0);
        approx(r.crop_origin.1, 600.0); // 150 * 4
    }

    #[test]
    fn small_image_shows_native_size_centered() {
        // Work 1000x800, image 200x100 → no upscale.
        let r = fit_image_rect((1000.0, 800.0), (200, 100));
        approx(r.css_rect.2, 200.0);
        approx(r.css_rect.3, 100.0);
        approx(r.css_rect.0, 400.0); // (1000-200)/2
        approx(r.css_rect.1, 350.0); // (800-100)/2
        approx(r.coord_scale, 1.0); // clamped, no upscale
        approx(r.crop_origin.0, 400.0);
        approx(r.crop_origin.1, 350.0);
    }

    #[test]
    fn tall_image_is_height_constrained() {
        // Work 1000x800, image 1000x4000 → height-constrained.
        let r = fit_image_rect((1000.0, 800.0), (1000, 4000));
        approx(r.css_rect.3, 800.0); // h fills height
        approx(r.css_rect.2, 200.0); // w = 1000 * 0.2
        approx(r.coord_scale, 5.0); // 4000/800
    }

    #[test]
    fn coord_scale_is_never_below_one() {
        let r = fit_image_rect((4000.0, 4000.0), (50, 50));
        assert!(r.coord_scale >= 1.0, "got {}", r.coord_scale);
    }

    #[test]
    fn annotation_at_image_top_left_maps_to_origin() {
        // The invariant build() relies on: a click at the rect's top-left maps to
        // image px (0, 0) via a*coord_scale - crop_origin.
        let r = fit_image_rect((1000.0, 800.0), (4000, 2000));
        let (x, y, _, _) = r.css_rect;
        approx(x * r.coord_scale - r.crop_origin.0, 0.0);
        approx(y * r.coord_scale - r.crop_origin.1, 0.0);
    }
}
```

Add to `src-tauri/src/lib.rs` after the other `mod` lines (line ~10):

```rust
mod open_image;
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml open_image::tests::`
Expected: 5 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/open_image.rs src-tauri/src/lib.rs
git commit -m "feat(open-image): pure fit_image_rect math"
```

---

## Task 3: `load_image_frame` — decode any image to a CapturedFrame

**Files:**
- Modify: `src-tauri/src/open_image.rs`

- [ ] **Step 1: Add the failing test + the error type and function**

In `src-tauri/src/open_image.rs`, add above the `#[cfg(test)]` block:

```rust
use crate::capture::{encode_png, CaptureError, CapturedFrame};

#[derive(Debug, thiserror::Error)]
pub enum OpenImageError {
    #[error("read file: {0}")]
    Read(#[from] std::io::Error),
    #[error("decode image: {0}")]
    Decode(#[from] image::ImageError),
    #[error("encode png: {0}")]
    Encode(#[from] CaptureError),
}

/// Decode image bytes (any format the `image` crate supports) into a
/// `CapturedFrame` at native resolution. `scale_factor` is 1.0 — the saved
/// image's own pixels are the coordinate space; the overlay-to-image scaling is
/// carried separately as `coord_scale` (see `fit_image_rect`).
pub fn load_image_frame(
    bytes: &[u8],
    display_index: usize,
) -> Result<CapturedFrame, OpenImageError> {
    let img = image::load_from_memory(bytes)?.to_rgba8();
    let (width, height) = (img.width(), img.height());
    let png = encode_png(&img)?;
    Ok(CapturedFrame { width, height, scale_factor: 1.0, png, display_index })
}
```

Add inside the `tests` module:

```rust
    #[test]
    fn load_image_frame_decodes_to_native_dims_at_scale_one() {
        use image::{Rgba, RgbaImage};
        let src = RgbaImage::from_pixel(64, 32, Rgba([10, 20, 30, 255]));
        let png = crate::capture::encode_png(&src).unwrap();
        let frame = load_image_frame(&png, 2).unwrap();
        assert_eq!((frame.width, frame.height), (64, 32));
        assert_eq!(frame.scale_factor, 1.0);
        assert_eq!(frame.display_index, 2);
        let decoded = image::load_from_memory(&frame.png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 32));
    }

    #[test]
    fn load_image_frame_rejects_garbage_bytes() {
        let err = load_image_frame(&[1, 2, 3, 4], 0).unwrap_err();
        assert!(matches!(err, OpenImageError::Decode(_)), "got {err:?}");
    }
```

- [ ] **Step 2: Run the tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml open_image::tests::load_image_frame`
Expected: both PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/open_image.rs
git commit -m "feat(open-image): decode image files to a CapturedFrame"
```

---

## Task 4: `ImageSession` + `AppState.pending_image`

**Files:**
- Modify: `src-tauri/src/open_image.rs`
- Modify: `src-tauri/src/lib.rs:30-57` (struct), `src-tauri/src/lib.rs:1102-1115` (manage init)

- [ ] **Step 1: Add the `ImageSession` struct**

In `src-tauri/src/open_image.rs`, add (above the tests):

```rust
/// What `save_snapshot("image")` needs at save time: the decoded frame plus the
/// coordinate mapping computed at open time by `fit_image_rect`.
pub struct ImageSession {
    pub frame: CapturedFrame,
    pub crop_origin: (f32, f32),
    pub coord_scale: f32,
}
```

- [ ] **Step 2: Add the AppState field**

In `src-tauri/src/lib.rs`, add to `pub struct AppState` (after `live_generation`, line ~56):

```rust
    /// Set at image-open time, consumed by `save_snapshot("image")`. Holds the
    /// decoded file frame and the overlay-CSS→image-px mapping.
    pub pending_image: Mutex<Option<open_image::ImageSession>>,
```

In the `.manage(AppState { ... })` block (line ~1102), add:

```rust
            pending_image: Mutex::new(None),
```

- [ ] **Step 3: Build to verify it compiles**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles (a dead-code warning on `ImageSession` fields is fine until Task 7 wires it).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/open_image.rs src-tauri/src/lib.rs
git commit -m "feat(open-image): ImageSession state slot in AppState"
```

---

## Task 5: `build()` gains an explicit `coord_scale` parameter

Today `build()` derives `let scale = frame.scale_factor;`. Image mode needs a coordinate scale (`iw/dw`) distinct from the frame's metadata scale (which stays `1.0`). Add an explicit parameter; screen modes pass `frame.scale_factor`.

**Files:**
- Modify: `src-tauri/src/snapshot/build.rs:83-99` (signature + `scale`), and the 4 test call sites
- Modify: `src-tauri/src/lib.rs:204-214` (the one production call)

- [ ] **Step 1: Add the failing test**

In `src-tauri/src/snapshot/build.rs` `mod tests`, add:

```rust
    #[test]
    fn coord_scale_overrides_frame_scale_for_mapping() {
        // Frame metadata scale is 1.0 (an opened image), but coord_scale=2.0
        // maps an annotation at CSS (10,10,5,5) to image px (20,20,10,10).
        let f = frame(100, 100);
        let base = std::path::PathBuf::from("/tmp/gs-test-coordscale");
        let (built, _img) = build(&f, "image",
            vec![incoming("a", "annotation", 10.0, 10.0, 5.0, 5.0)],
            "macos", "14", &base, None, 2.0, RedactionStyle::Branded).unwrap();
        let c = &built.snapshot.annotations[0].coords;
        assert_eq!((c.x, c.y, c.w, c.h), (20.0, 20.0, 10.0, 10.0));
    }
```

- [ ] **Step 2: Run it to confirm it fails to compile**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml build::tests::coord_scale_overrides`
Expected: FAIL — `build` takes 8 args, not 9.

- [ ] **Step 3: Add the parameter**

In `src-tauri/src/snapshot/build.rs`, change the signature (line ~83) to insert `coord_scale` before `redaction_style`:

```rust
pub fn build(
    frame: &CapturedFrame,
    mode: &str,
    annotations: Vec<IncomingAnnotation>,
    os_platform: &str,
    os_version: &str,
    base_dir: &Path,
    crop_origin: Option<(f32, f32)>,
    coord_scale: f32,
    redaction_style: RedactionStyle,
) -> Result<(BuiltSnapshot, RgbaImage), BuildError> {
```

Change line ~98 from `let scale = frame.scale_factor;` to:

```rust
    let scale = coord_scale;
```

- [ ] **Step 4: Update the existing test call sites**

In `src-tauri/src/snapshot/build.rs`, the existing 3 `build(...)` calls in tests pass a frame with `scale_factor: 1.0`. Add `1.0,` before each `RedactionStyle::...` argument:

- `no_redactions_uses_fast_path_clean_equals_frame`: `..., &base, None, 1.0, RedactionStyle::Branded`
- `redaction_destroys_pixels_in_clean_image`: `..., &base, None, 1.0, RedactionStyle::Solid`
- `redactions_do_not_consume_annotation_labels`: `..., &base, None, 1.0, RedactionStyle::Branded`
- `unknown_kind_fails_closed`: `..., &base, None, 1.0, RedactionStyle::Solid`

- [ ] **Step 5: Update the production call site**

In `src-tauri/src/lib.rs` `save_snapshot` (the `build::build(...)` call, line ~204), the coordinate scale comes from the match (Task 7 wires it as `coord_scale`). For now, the match still yields `(frame, crop_origin)`; pass `frame.scale_factor` so existing modes are unchanged:

```rust
    let (mut built, annotated_image) = snapshot::build::build(
        &frame,
        &mode,
        annotations,
        os_platform,
        &os_version,
        &base,
        crop_origin,
        frame.scale_factor,
        state.settings.current_redaction_style(),
    )
    .map_err(|e| e.to_string())?;
```

- [ ] **Step 6: Run the build tests**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml build::tests`
Expected: all PASS, including the new `coord_scale_overrides_frame_scale_for_mapping`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/snapshot/build.rs src-tauri/src/lib.rs
git commit -m "feat(snapshot): explicit coord_scale param on build()"
```

---

## Task 6: `overlay::show_image_mode` + `ImageModePayload`

Mirrors `show_window_mode`, but positions over the **work area** (`ScreenRect::Visible`) since the image is letterboxed within it.

**Files:**
- Modify: `src-tauri/src/overlay.rs` (after `show_window_mode`, line ~312)

- [ ] **Step 1: Add the payload type and the present function**

In `src-tauri/src/overlay.rs`, after `show_window_mode` (line ~312), add:

```rust
/// Payload sent to the overlay React tree when presenting an opened image.
/// `image_rect_css` is where the letterboxed image is drawn, in overlay CSS px.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageModePayload {
    pub image_rect_css: CssRect,
}

/// Present the overlay over the cursor monitor's work area in `image` mode. The
/// decoded image frame is pushed separately via `push_frame` after this returns
/// (see `present_image`), arriving through the same `glitch-spot://frame` event
/// the frozen/window paths use.
pub fn show_image_mode(
    app: &tauri::AppHandle,
    payload: &ImageModePayload,
) -> Result<(), OverlayError> {
    let window = position_over_active_monitor(app)?;

    // Show invisible, then reveal once the frame has painted — same alpha-0
    // dance as the live/frozen/window presents (avoids compositing a stale
    // backing buffer when the overlay was last shown on another display).
    set_window_alpha(&window, 0.0);

    let json = serde_json::to_string(payload)?;
    let js = format!(
        "if (window.__glitchSpotPresent) window.__glitchSpotPresent('image', {});",
        json
    );
    let _ = window.eval(&js);
    window.show()?;
    window.set_focus()?;
    reassert_screen_frame(&window, ScreenRect::Visible);
    Ok(())
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles (unused-function warnings until Task 7 calls it).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/overlay.rs
git commit -m "feat(overlay): show_image_mode present path"
```

---

## Task 7: `trigger_open_image` + `present_image` + `save_snapshot("image")`

**Files:**
- Modify: `src-tauri/src/lib.rs` — add helpers near the other `trigger_*` fns (after `trigger_overlay_frozen`, line ~950); extend `save_snapshot` (lines ~159-198)

- [ ] **Step 1: Add the cursor-monitor work-area helper**

In `src-tauri/src/lib.rs`, add near the other free functions (e.g. after `trigger_overlay_frozen`, line ~950):

```rust
/// The work area (CSS px) and display index of the monitor under the cursor.
/// CSS px = physical work-area size / the monitor's scale factor — the same
/// space the overlay renders annotations in.
fn cursor_monitor_work_css(app: &tauri::AppHandle) -> anyhow::Result<(usize, (f32, f32))> {
    let cursor = app.cursor_position()?;
    let monitors = app.available_monitors()?;
    let (idx, monitor) = monitors
        .iter()
        .enumerate()
        .find(|(_, m)| {
            let p = m.position();
            let s = m.size();
            let (x0, y0) = (p.x as f64, p.y as f64);
            cursor.x >= x0
                && cursor.x < x0 + s.width as f64
                && cursor.y >= y0
                && cursor.y < y0 + s.height as f64
        })
        .ok_or_else(|| anyhow::anyhow!("no monitor at cursor"))?;
    let wa = monitor.work_area();
    let scale = monitor.scale_factor() as f32;
    Ok((idx, (wa.size.width as f32 / scale, wa.size.height as f32 / scale)))
}
```

- [ ] **Step 2: Add `present_image`**

Add below the helper:

```rust
/// Decode the chosen file, compute its letterbox fit, cache the session, and
/// present the overlay in image mode. Errors bubble to the caller, which
/// notifies the user.
async fn present_image(app: &tauri::AppHandle, path: std::path::PathBuf) -> anyhow::Result<()> {
    let bytes = std::fs::read(&path)?;
    let (display_index, work_css) = cursor_monitor_work_css(app)?;
    let frame = open_image::load_image_frame(&bytes, display_index)?;
    let fit = open_image::fit_image_rect(work_css, (frame.width, frame.height));

    if let Some(state) = app.try_state::<AppState>() {
        // Defensive reset: opening an image supersedes any in-flight session.
        state.picker_state.reset_to_idle();
        if let Ok(mut slot) = state.pending_image.lock() {
            *slot = Some(open_image::ImageSession {
                frame: frame.clone(),
                crop_origin: fit.crop_origin,
                coord_scale: fit.coord_scale,
            });
        }
        if let Ok(mut f) = state.latest_frame.lock() {
            *f = Some(frame.clone());
        }
    }

    let (x, y, w, h) = fit.css_rect;
    let payload = overlay::ImageModePayload {
        image_rect_css: overlay::CssRect { x, y, w, h },
    };
    overlay::show_image_mode(app, &payload)?;

    // Fallback reveal in case the frontend's double-rAF revealOverlay() signal
    // is lost — reveal is idempotent, so a redundant call is harmless.
    {
        let app_fb = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = overlay::reveal(&app_fb);
        });
    }

    overlay::push_frame(app, &frame)?;
    Ok(())
}
```

- [ ] **Step 3: Add `trigger_open_image`**

Add below `present_image`:

```rust
/// Open a native file picker for an image, then present it for annotation.
/// Called from the tray. The picker is non-blocking (callback form); on a pick
/// we spawn `present_image` on the async runtime.
pub(crate) fn trigger_open_image(app: &tauri::AppHandle) {
    use tauri_plugin_dialog::DialogExt;
    use tauri_plugin_notification::NotificationExt;

    // Bring the accessory (LSUIElement) app forward so the open panel takes
    // focus instead of opening behind other windows. See Task 1 spike findings.
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        let cls = objc2::class!(NSApplication);
        let ns_app: *mut AnyObject = msg_send![cls, sharedApplication];
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
    }

    let app = app.clone();
    app.dialog()
        .file()
        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
        .pick_file(move |maybe_path| {
            let Some(file_path) = maybe_path else { return };
            let Ok(path) = file_path.into_path() else { return };
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = present_image(&app2, path).await {
                    eprintln!("open image failed: {e}");
                    let _ = app2
                        .notification()
                        .builder()
                        .title("Glitch Spot")
                        .body(format!("Couldn't open image: {e}"))
                        .show();
                }
            });
        });
}
```

> If the Task 1 spike found `activateIgnoringOtherApps:` insufficient, also apply the activation-policy flip the spike identified, here, around the `pick_file` call.

- [ ] **Step 4: Wire the `"image"` arm into `save_snapshot`**

In `src-tauri/src/lib.rs` `save_snapshot`, change the match to yield a 3-tuple `(frame, crop_origin, coord_scale)`. Replace the match header/arms (lines ~159-198) so each existing arm appends `frame.scale_factor` and add an `"image"` arm:

```rust
    let (frame, crop_origin, coord_scale) = match mode.as_str() {
        "frozen" => {
            overlay::hide(&app).map_err(|e| e.to_string())?;
            let frame = state
                .latest_frame
                .lock()
                .ok()
                .and_then(|g| g.clone())
                .ok_or_else(|| "no cached frame for frozen mode".to_string())?;
            let sf = frame.scale_factor;
            (frame, None, sf)
        }
        "live" => {
            overlay::hide(&app).map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(120));
            let frame = capture::capture_active_monitor(&app).map_err(|e| e.to_string())?;
            let sf = frame.scale_factor;
            (frame, None, sf)
        }
        "window" => {
            overlay::hide(&app).map_err(|e| e.to_string())?;
            let session = match state.picker_state.current() {
                picker_session::RunMode::Annotating { session } => session,
                other => return Err(format!("save_snapshot('window') while in {:?}", other)),
            };
            let cached = state.latest_frame.lock().ok().and_then(|g| g.clone());
            let (frame, crop) = resolve_window_save_frame(&session, cached)?;
            let sf = frame.scale_factor;
            (frame, Some(crop), sf)
        }
        "image" => {
            overlay::hide(&app).map_err(|e| e.to_string())?;
            let session = state
                .pending_image
                .lock()
                .ok()
                .and_then(|mut g| g.take())
                .ok_or_else(|| "no pending image for image mode".to_string())?;
            (session.frame, Some(session.crop_origin), session.coord_scale)
        }
        other => return Err(format!("unknown mode: {other}")),
    };
```

Then change the `build::build(...)` call's scale argument (from Task 5 Step 5) from `frame.scale_factor` to `coord_scale`:

```rust
        crop_origin,
        coord_scale,
        state.settings.current_redaction_style(),
```

- [ ] **Step 5: Build**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles. `ImageSession` dead-code warnings are gone.

- [ ] **Step 6: Run the full Rust test suite**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS (no regressions in build/save tests).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(open-image): trigger + present + save_snapshot image arm"
```

---

## Task 8: Tray menu item

**Files:**
- Modify: `src-tauri/src/tray.rs` (build_menu ~312/483, on_menu_event ~623)

- [ ] **Step 1: Build the menu item**

In `src-tauri/src/tray.rs` `build_menu`, after the `capture_window` item (line ~312):

```rust
    let open_image = MenuItemBuilder::new("Open Image…")
        .id("open_image")
        .build(app)?;
```

- [ ] **Step 2: Add it to the menu**

In the `MenuBuilder` chain (line ~480), add after `.item(&capture_window)`:

```rust
        .item(&open_image)
```

- [ ] **Step 3: Add the handler**

In `on_menu_event`, after the `"capture_window"` arm (line ~629):

```rust
                "open_image" => {
                    crate::trigger_open_image(&app);
                }
```

- [ ] **Step 4: Build**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "feat(tray): Open Image menu item"
```

---

## Task 9: Frontend — `'image'` mode in the overlay

The window-mode render branch (dim surround + clipped frozen frame + clipped `DrawingLayer`) is exactly what image mode needs. Generalize it to render for either mode keyed off a single `framedRect`.

**Files:**
- Modify: `src/picker/types.ts` (add type)
- Modify: `src/overlay/Overlay.tsx` (mode union, present, showBlob guard, render branch)
- Modify: `src/overlay/Overlay.test.tsx` (image-mode test)

- [ ] **Step 1: Add the TS payload type**

In `src/picker/types.ts`, after `WindowModePayload` (line ~46):

```ts
export type ImageModePayload = {
  image_rect_css: { x: number; y: number; w: number; h: number };
};
```

- [ ] **Step 2: Write the failing frontend test**

In `src/overlay/Overlay.test.tsx`, add a test that presents image mode and asserts the dimmed framed branch renders with the image rect. Match the existing window-mode test's setup style (lines ~40-80). Add:

```tsx
  it("image mode renders the dimmed framed branch at the image rect", async () => {
    const { getByTestId } = render(<Overlay />);
    await act(async () => {
      (
        window as unknown as {
          __glitchSpotPresent: (mode: string, payload: unknown) => void;
        }
      ).__glitchSpotPresent("image", {
        image_rect_css: { x: 100, y: 50, w: 400, h: 300 },
      });
    });
    // The window-mode dim panels are reused for image mode; their presence
    // proves the framed branch (not the live/frozen branch) rendered.
    const top = getByTestId("window-dim-top");
    expect(top).toBeTruthy();
    const frame = getByTestId("window-frozen-frame");
    expect(frame.style.left).toBe("100px");
    expect(frame.style.top).toBe("50px");
    expect(frame.style.width).toBe("400px");
    expect(frame.style.height).toBe("300px");
  });
```

> Match the existing imports/`act`/`render` usage already in the file — copy the harness from the adjacent window-mode test rather than introducing new helpers.

- [ ] **Step 3: Run it to confirm it fails**

Run: `npm test -- src/overlay/Overlay.test.tsx -t "image mode renders"`
Expected: FAIL — image mode falls through to the live/frozen branch, so `window-dim-top` is absent.

- [ ] **Step 4: Extend the Mode union and present payload typing**

In `src/overlay/Overlay.tsx`:

Change line ~15:

```tsx
type Mode = 'live' | 'frozen' | 'window' | 'image'
```

Add the import (line ~13):

```tsx
import type { WindowModePayload, ImageModePayload } from '../picker/types'
```

Add state next to `windowPayload` (line ~193):

```tsx
  const [imagePayload, setImagePayload] = useState<ImageModePayload | null>(null)
```

- [ ] **Step 5: Handle `'image'` in `__glitchSpotPresent`**

In the `__glitchSpotPresent` assignment (line ~260), broaden the payload type and set/clear `imagePayload`. Change the function signature and add the image line alongside `setWindowPayload`:

```tsx
    ;(window as unknown as { __glitchSpotPresent?: (mode: Mode, payload?: WindowModePayload | ImageModePayload) => void }).__glitchSpotPresent = (
      newMode,
      payload,
    ) => {
      resetFrames();
      setMode(newMode);
      dispatch({ type: "CLEAR" });
      setShowDiscardModal(false);
      setRedactMode(false);
      setWindowPayload(newMode === 'window' && payload ? (payload as WindowModePayload) : null);
      setImagePayload(newMode === 'image' && payload ? (payload as ImageModePayload) : null);
      setPresentationId((n) => n + 1);
      if (newMode === 'window' && payload && 'window_rect_css' in payload && payload.initial_rect_css) {
        const r = payload.initial_rect_css;
        const clip = payload.window_rect_css;
```

(The remaining body of the `initial_rect_css` block is unchanged — only its guard now also checks `'window_rect_css' in payload` so the union narrows correctly. Update the closing `delete` cleanup's cast on line ~338 to the same union type.)

Also update the cleanup cast at line ~338:

```tsx
      delete (window as unknown as { __glitchSpotPresent?: (mode: Mode, payload?: WindowModePayload | ImageModePayload) => void })
        .__glitchSpotPresent;
```

- [ ] **Step 6: Keep image mode from flipping to frozen on frame arrival**

In `showBlob` (line ~304), change the guard so image mode (like window mode) keeps its mode when a frame arrives:

```tsx
      if (modeRef.current !== 'window' && modeRef.current !== 'image') setMode("frozen");
```

- [ ] **Step 7: Render the framed branch for both window and image modes**

Replace the `if (mode === 'window' && windowPayload) {` line (line ~400) and its use of `window_rect_css` with a shared `framedRect`. Change the branch opening to:

```tsx
  const framedRect =
    mode === 'window'
      ? windowPayload?.window_rect_css ?? null
      : mode === 'image'
        ? imagePayload?.image_rect_css ?? null
        : null

  if (framedRect) {
    return (
      <div style={{ position: 'fixed', inset: 0, background: 'transparent' }}>
        <WindowDim rect={framedRect} />
        <div
          data-testid="window-frozen-frame"
          style={{
            position: 'absolute',
            left: framedRect.x,
            top: framedRect.y,
            width: framedRect.w,
            height: framedRect.h,
            overflow: 'hidden',
            pointerEvents: 'none',
          }}
        >
          {!imageUrl && <GlitchShimmer />}
          {imageUrl && <GlitchImage key={imageUrl} url={imageUrl} />}
          <div
            style={{
              position: 'absolute',
              inset: 0,
              backgroundImage: CHECKER_BG,
              backgroundRepeat: 'repeat',
              opacity: 0.12,
              pointerEvents: 'none',
            }}
          />
        </div>
        <DrawingLayer
          state={annotations}
          dispatch={dispatch}
          clipRect={framedRect}
          onOutsideClick={() => {
            if (annotations.rects.length === 0) {
              bridge.cancel()
            } else {
              setShowDiscardModal(true)
            }
          }}
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
          onSetRedactMode={setRedactMode}
          onChangeRedactionStyle={handleRedactionStyleChange}
        />
        {showDiscardModal && (
          <DiscardModal
            onDiscard={() => {
              setShowDiscardModal(false)
              dispatch({ type: 'CLEAR' })
              bridge.cancel()
            }}
            onSave={() => {
              setShowDiscardModal(false)
              handleSave()
            }}
            onCancel={() => setShowDiscardModal(false)}
          />
        )}
      </div>
    )
  }
```

This replaces the old window-only block verbatim except: it is gated on `framedRect` (covering both modes), and `WindowDim`/frozen-frame/`DrawingLayer` use `framedRect` instead of the destructured `window_rect_css`. The `handleSave` path already sends `mode` (`'image'`) to the bridge, so no save-path change is needed.

- [ ] **Step 8: Run the frontend test**

Run: `npm test -- src/overlay/Overlay.test.tsx`
Expected: the new image test PASSES and all existing window/frozen tests still PASS.

- [ ] **Step 9: Typecheck and run the full frontend suite**

Run: `npm run build` (or the project's typecheck), then `npm test`
Expected: no TS errors; all tests green.

- [ ] **Step 10: Commit**

```bash
git add src/picker/types.ts src/overlay/Overlay.tsx src/overlay/Overlay.test.tsx
git commit -m "feat(overlay): image annotation mode"
```

---

## Task 10: Manual end-to-end verification

**Files:** none (manual).

- [ ] **Step 1: Launch the app**

Run: `PATH="$HOME/.cargo/bin:$PATH" npm run tauri:dev`
Expected: app appears in the menu bar, tray menu shows `Open Image…` as the fourth item under `Capture Window`.

- [ ] **Step 2: Large image**

Click `Open Image…`, pick a photo larger than your screen. Verify: the open panel took focus; the image appears letterboxed and centered with the dimmed surround and chromatic resolve; draw an annotation; press ⌘⇧↵ (or click save). Verify a snapshot is produced (tray "Latest snapshot" updates) and the clipboard holds the configured format.

- [ ] **Step 3: Small image**

Pick a small image (e.g. ~200px). Verify it shows at native size centered (not upscaled/blurry), annotate, save, and confirm the saved `annotated.png` strokes are crisp and positioned correctly over the image pixels.

- [ ] **Step 4: Wide and tall images**

Pick a very wide and a very tall image; confirm correct letterbox orientation and that annotations map to the right image pixels (open the saved `clean.png`/`annotated.png` from the snapshots dir and eyeball alignment).

- [ ] **Step 5: Redaction**

Open an image, toggle redact mode, cover a region, save. Open the saved `clean.png` and confirm the region is destroyed (not just visually hidden) and the annotated copy is also covered.

- [ ] **Step 6: Error + cancel paths**

Cancel the open panel → nothing happens, no overlay. (Optional) point at a non-image file via a renamed extension → a "Couldn't open image" notification, no overlay.

- [ ] **Step 7: Final commit (if any manual-fix tweaks were needed)**

```bash
git add -A
git commit -m "chore(open-image): manual verification fixes"
```

---

## Self-Review Notes

- **Spec coverage:** menu item (Task 8), file dialog + focus (Tasks 1, 7), decode→frame (Task 3), fit math downscale-only (Task 2), present mode (Tasks 6, 9), save + coordinate model (Tasks 5, 7), error handling (Task 7 notification + Task 10 manual), testing (unit Tasks 2/3/5/9, manual Task 10). All design sections map to a task.
- **Type consistency:** Rust `coord_scale: f32` and `ImageSession { frame, crop_origin, coord_scale }` used identically in Tasks 4/5/7. `overlay::CssRect`/`ImageModePayload` (Task 6) consumed in Task 7. TS `ImageModePayload.image_rect_css` (Task 9 Step 1) consumed in Steps 5/7. Mode string `"image"` consistent across backend match (Task 7), `show_image_mode` eval (Task 6), and frontend (Task 9).
- **Backward-compat:** `build()`'s new `coord_scale` param defaults conceptually to `frame.scale_factor` at every existing call site, so live/frozen/window behavior is byte-for-byte unchanged.
