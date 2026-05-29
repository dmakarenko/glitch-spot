# Per-window selection mode — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a ⌃⌥⌘J global hotkey that lets the user pick a window across any display, then annotate over that live window and save a snapshot containing only the window's pixels.

**Architecture:** A pool of 8 transparent `picker-N` overlay windows (pre-declared in `tauri.conf.json`) shows on hotkey, one per active display. Cursor-position hit-test runs client-side against a snapshot of windows from `SCShareableContent`. On click, the existing `overlay` window takes over in a new `'window'` mode: dim veil outside the window rect, gray-checker over the window rect, drawing constrained to that rect. Save captures the live window via `SCContentFilter::with_window(CGWindowID)` and records `context.window` in the snapshot.

**Tech Stack:** Tauri v2 (Rust + WebView), React + TypeScript (Vite), `screencapturekit` v6 for window enumeration and capture, `tauri-plugin-global-shortcut`, `tauri-plugin-notification`. Tests: `cargo test --manifest-path src-tauri/Cargo.toml --lib` (Rust), `npm test` (Vitest).

**Design seed:** `docs/seeds/2026-05-27-per-window-selection-seed.md` — read this before starting any task. It is the source of truth for behaviour.

---

## File structure

### New files (Rust)
- `src-tauri/src/window_picker/mod.rs` — public types + platform-agnostic surface
- `src-tauri/src/window_picker/macos.rs` — SCShareableContent enumeration
- `src-tauri/src/window_picker/filter.rs` — pure filter for "standard app windows"
- `src-tauri/src/window_picker/hit_test.rs` — pure cursor → window_id hit test
- `src-tauri/src/window_picker/coords.rs` — points → physical pixels conversion
- `src-tauri/src/picker_session.rs` — RunMode + session state holder

### New files (frontend)
- `picker.html` — entry HTML at project root
- `src/picker/entry.tsx` — React mount
- `src/picker/Picker.tsx` — picker UI
- `src/picker/hitTest.ts` — pure JS twin of Rust hit_test
- `src/picker/cursor.ts` — inlined SVG data URL
- `src/picker/types.ts` — shared TS types for picker payloads

### New files (fixtures + tests)
- `tests/fixtures/hit-test-cases.json` — shared Rust + JS hit-test cases
- `src/picker/hitTest.test.ts`
- `src/picker/Picker.test.tsx`

### Modified files
- `src-tauri/src/capture/mod.rs` (+ `macos.rs`) — add `capture_window`
- `src-tauri/src/snapshot/mod.rs` — add `WindowInfo` and `Context.window`
- `src-tauri/src/snapshot/build.rs` — add `crop_origin` parameter
- `src-tauri/src/snapshot/tests.rs` — extend tests
- `src-tauri/src/overlay.rs` — add `show_window_mode`
- `src-tauri/src/hotkey.rs` — register ⌃⌥⌘J
- `src-tauri/src/lib.rs` — register commands, wire `RunMode`, extend `save_snapshot`
- `src-tauri/tauri.conf.json` — declare picker-0..picker-7
- `vite.config.ts` — add `picker` Rollup input
- `src/overlay/Overlay.tsx` — `'window'` mode branch
- `src/overlay/annotations/DrawingLayer.tsx` — accept `clipRect` prop
- `src/overlay/annotations/DrawingLayer.test.tsx` (new) — clipRect tests

### Manual smoke doc
- `docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md` (new) — checklist

---

## Conventions

- **Branch:** `feature-overlay-window-selection` (already created in this worktree). Before any commit, run `git rev-parse --abbrev-ref HEAD` and confirm.
- **Test commands:**
  - Rust: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- <filter>` (use `cargo test` without `--lib` if integration tests exist later).
  - Frontend: `npx vitest run <path>` for a single test file, `npm test` for all.
- **Commit cadence:** one commit per task. Use the commit message shown in the task. Always use a HEREDOC for multi-line messages.
- **All Rust code that touches `SCShareableContent` / `SCContentFilter` lives behind `#[cfg(target_os = "macos")]`** and has a `#[cfg(not(target_os = "macos"))]` stub returning an error string — matches the existing capture pattern.

---

## Task 1: Add `WindowInfo` to snapshot schema

**Files:**
- Modify: `src-tauri/src/snapshot/mod.rs`
- Modify: `src-tauri/src/snapshot/tests.rs`

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/snapshot/tests.rs` at the bottom of the existing `mod tests`:

```rust
#[test]
fn context_window_round_trips() {
    let s = Snapshot {
        id: "snap_test_w".into(),
        created_at: "2026-05-27T10:00:00Z".into(),
        mode: "window".into(),
        annotations: vec![],
        context: Context {
            screen: ScreenInfo {
                width: 1440,
                height: 900,
                scale_factor: 2.0,
                display_index: 0,
            },
            os: OsInfo { platform: "macos".into(), version: "26.0".into() },
            window: Some(WindowInfo {
                app_name: "Safari".into(),
                title: "Reader — example.com".into(),
                bounds: WindowBounds { x: 120, y: 88, w: 1440, h: 900 },
                window_id: 12345,
            }),
        },
        images: Images { clean: "x".into(), annotated: None },
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn context_window_omitted_when_none() {
    let mut s = sample_snapshot();
    s.context.window = None;
    let json = serde_json::to_string(&s).unwrap();
    assert!(!json.contains("\"window\""), "expected `window` key omitted, got: {json}");
}
```

Also extend `sample_snapshot()` in the same file to set `window: None` in the Context literal so existing tests still compile.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot::tests::context_window`
Expected: FAIL — `WindowInfo` / `WindowBounds` not in scope, or `Context.window` field missing.

- [ ] **Step 3: Add the new types and field**

Edit `src-tauri/src/snapshot/mod.rs`:

Add two new structs just before the existing `pub mod annotate;` line:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowInfo {
    pub app_name: String,
    pub title: String,
    pub bounds: WindowBounds,
    pub window_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}
```

Extend `Context` to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    pub screen: ScreenInfo,
    pub os: OsInfo,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub window: Option<WindowInfo>,
}
```

In `src-tauri/src/snapshot/build.rs`, update the `Context { ... }` literal in `build()` to set `window: None,` (the build function does not produce window-mode snapshots yet — that's wired in later tasks).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot`
Expected: PASS — all snapshot tests green, including the two new ones.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/snapshot/mod.rs src-tauri/src/snapshot/build.rs src-tauri/src/snapshot/tests.rs
git commit -m "feat(snapshot): add optional context.window for window-mode snapshots"
```

---

## Task 2: Add `crop_origin` parameter to `snapshot::build`

**Files:**
- Modify: `src-tauri/src/snapshot/build.rs`
- Modify: `src-tauri/src/snapshot/tests.rs`

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/src/snapshot/tests.rs`:

```rust
#[test]
fn build_crop_origin_translates_annotation_coords_into_window_local() {
    use crate::capture::CapturedFrame;
    use crate::snapshot::build::{build, IncomingAnnotation};
    use tempfile::TempDir;

    let frame = CapturedFrame {
        width: 800,
        height: 600,
        scale_factor: 2.0,
        png: include_bytes!("../../assets/DejaVuSans.ttf").to_vec(), // any non-empty bytes; not decoded for this test
        display_index: 0,
    };
    let annotations = vec![IncomingAnnotation {
        id: "r1".into(),
        x: 100.0,   // CSS px on the overlay
        y: 50.0,
        w: 80.0,
        h: 40.0,
        note: "n".into(),
    }];

    let temp = TempDir::new().unwrap();
    // crop_origin in physical pixels: (60, 30)
    let built = build(
        &frame,
        "window",
        annotations,
        "macos",
        "26.0",
        temp.path(),
        Some((60.0, 30.0)),
    );
    // The image bytes aren't a valid PNG so annotate will fail — we expect that.
    // For the coord assertion we need the function to skip annotated rendering when the
    // annotated PNG isn't required for this test. Use a separate code path:
    let built = built.expect("build with crop_origin should succeed even if annotated render is stub");
    let a = &built.snapshot.annotations[0];
    // x*scale - crop_x = 100*2 - 60 = 140
    // y*scale - crop_y = 50*2  - 30 = 70
    assert_eq!(a.coords.x, 140.0);
    assert_eq!(a.coords.y, 70.0);
    assert_eq!(a.coords.w, 160.0);
    assert_eq!(a.coords.h, 80.0);
}
```

NOTE: `render_annotated_png` will error on the fake PNG bytes. We need to refactor the test to avoid that, OR we need a real PNG. The simplest fix: use a real 1×1 PNG. Replace the `png` field with these bytes:

```rust
png: vec![
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
    0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
    0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
],
```

That's a valid 1×1 transparent PNG that `render_annotated_png` can ingest.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot::tests::build_crop_origin`
Expected: FAIL — `build` takes 6 args, test passes 7.

- [ ] **Step 3: Add the `crop_origin` parameter to `build`**

Edit `src-tauri/src/snapshot/build.rs`:

Change the `build` signature to:

```rust
pub fn build(
    frame: &CapturedFrame,
    mode: &str,
    annotations: Vec<IncomingAnnotation>,
    os_platform: &str,
    os_version: &str,
    base_dir: &Path,
    crop_origin: Option<(f32, f32)>,
) -> Result<BuiltSnapshot, BuildError> {
```

Replace the annotation mapping block with:

```rust
let scale = frame.scale_factor;
let (crop_x, crop_y) = crop_origin.unwrap_or((0.0, 0.0));
let annotations: Vec<Annotation> = annotations
    .into_iter()
    .map(|a| Annotation {
        id: a.id,
        shape: "rect".into(),
        coords: Coords {
            x: a.x * scale - crop_x,
            y: a.y * scale - crop_y,
            w: a.w * scale,
            h: a.h * scale,
        },
        note: a.note,
    })
    .collect();
```

Update the call site in `src-tauri/src/lib.rs`'s `save_snapshot` to pass `None` for now (we'll fill it in Task 14):

```rust
let built = snapshot::build::build(
    &frame,
    &mode,
    annotations,
    os_platform,
    &os_version,
    &base,
    None,
)
.map_err(|e| e.to_string())?;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib snapshot`
Expected: PASS — including the new `build_crop_origin_translates_annotation_coords_into_window_local`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/snapshot/build.rs src-tauri/src/snapshot/tests.rs src-tauri/src/lib.rs
git commit -m "feat(snapshot): add crop_origin to build() for window-local coords"
```

---

## Task 3: Define `window_picker::types`

**Files:**
- Create: `src-tauri/src/window_picker/mod.rs`
- Modify: `src-tauri/src/lib.rs` — register the module

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/window_picker/mod.rs` with:

```rust
// ABOUTME: Public surface for the per-window picker mode. Cross-platform types;
// ABOUTME: macOS implementation lives in window_picker::macos behind a cfg gate.

use serde::{Deserialize, Serialize};

pub mod filter;
pub mod hit_test;
pub mod coords;

#[cfg(target_os = "macos")]
pub mod macos;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PickableWindow {
    pub window_id: u32,
    pub app_name: String,
    pub title: String,
    pub bounds: WindowBounds,
    pub display_index: usize,
    pub layer: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DisplayBounds {
    pub display_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub scale_factor: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pickable_window_round_trips_through_json() {
        let w = PickableWindow {
            window_id: 42,
            app_name: "Safari".into(),
            title: "Reader".into(),
            bounds: WindowBounds { x: 10, y: 20, w: 800, h: 600 },
            display_index: 1,
            layer: 0,
        };
        let json = serde_json::to_string(&w).unwrap();
        let back: PickableWindow = serde_json::from_str(&json).unwrap();
        assert_eq!(w, back);
    }
}
```

Add to `src-tauri/src/lib.rs` near the other `mod` declarations:

```rust
mod window_picker;
```

Create empty stubs for the submodules referenced above so the test file compiles. Add:

`src-tauri/src/window_picker/filter.rs`:
```rust
// ABOUTME: Pure filter that selects "standard app windows" for the picker.
// ABOUTME: No platform dependencies; takes the raw list and returns the pickable subset.
```

`src-tauri/src/window_picker/hit_test.rs`:
```rust
// ABOUTME: Pure cursor-position → window_id hit test for the picker mode.
// ABOUTME: Iterates the front-to-back ordered window list and returns the first contains-match.
```

`src-tauri/src/window_picker/coords.rs`:
```rust
// ABOUTME: Coordinate conversions for window picker — logical points to physical pixels.
// ABOUTME: All hit-testing operates on physical pixels to match Tauri's cursor_position units.
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib window_picker::tests::pickable_window_round_trips`
Expected: FAIL with `unresolved module` or similar — module not registered yet.

Then after creating the files:

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib window_picker`
Expected: PASS

- [ ] **Step 3: No additional implementation needed for this task.** The types compile and serialise.

- [ ] **Step 4: Run tests to confirm green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All previously green tests still green; `window_picker::tests::pickable_window_round_trips` green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/window_picker/ src-tauri/src/lib.rs
git commit -m "feat(window_picker): scaffold module + PickableWindow / WindowBounds / DisplayBounds types"
```

---

## Task 4: Pure filter for "standard app windows"

**Files:**
- Modify: `src-tauri/src/window_picker/filter.rs`

- [ ] **Step 1: Write the failing tests**

Replace the contents of `src-tauri/src/window_picker/filter.rs` with:

```rust
// ABOUTME: Pure filter that selects "standard app windows" for the picker.
// ABOUTME: No platform dependencies; takes the raw list and returns the pickable subset.

use super::{PickableWindow, WindowBounds};

/// Minimum width and height (in physical px) for a window to be considered
/// "standard". Below this we treat the entry as chrome/shadow/transient.
pub const MIN_DIMENSION_PX: u32 = 80;

/// Owning-application names that are always excluded regardless of size/layer.
/// Bundle ID is the primary check (`com.glitchspot.app`); these are belt-and-braces
/// for system processes that don't surface a stable bundle id.
const EXCLUDED_APP_NAMES: &[&str] = &[
    "Dock",
    "WindowServer",
    "SystemUIServer",
    "Control Center",
];

/// Source-of-truth bundle id for *our own* app — excluded from the pickable set.
pub const SELF_BUNDLE_ID: &str = "com.glitchspot.app";

/// Inputs to the filter. Decoupled from the macOS SCWindow type so this is
/// trivially testable on any platform.
#[derive(Debug, Clone, PartialEq)]
pub struct RawWindow {
    pub window_id: u32,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub title: String,
    pub bounds: WindowBounds,
    pub display_index: usize,
    pub layer: i32,
    pub is_on_screen: bool,
}

pub fn filter_standard_windows(raws: &[RawWindow]) -> Vec<PickableWindow> {
    raws.iter()
        .filter(|r| r.is_on_screen)
        .filter(|r| r.layer == 0)
        .filter(|r| r.bundle_id.as_deref() != Some(SELF_BUNDLE_ID))
        .filter(|r| !EXCLUDED_APP_NAMES.contains(&r.app_name.as_str()))
        .filter(|r| r.bounds.w >= MIN_DIMENSION_PX && r.bounds.h >= MIN_DIMENSION_PX)
        .map(|r| PickableWindow {
            window_id: r.window_id,
            app_name: r.app_name.clone(),
            title: r.title.clone(),
            bounds: r.bounds,
            display_index: r.display_index,
            layer: r.layer,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(window_id: u32) -> RawWindow {
        RawWindow {
            window_id,
            app_name: "Safari".into(),
            bundle_id: Some("com.apple.Safari".into()),
            title: "Reader".into(),
            bounds: WindowBounds { x: 0, y: 0, w: 800, h: 600 },
            display_index: 0,
            layer: 0,
            is_on_screen: true,
        }
    }

    #[test]
    fn standard_safari_window_is_included() {
        let out = filter_standard_windows(&[raw(1)]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].window_id, 1);
    }

    #[test]
    fn off_screen_windows_are_excluded() {
        let mut r = raw(1);
        r.is_on_screen = false;
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty());
    }

    #[test]
    fn non_zero_layer_windows_are_excluded() {
        let mut r = raw(1);
        r.layer = 1000;
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty(), "menu/panel layers must be excluded");
    }

    #[test]
    fn own_bundle_id_is_excluded() {
        let mut r = raw(1);
        r.bundle_id = Some(SELF_BUNDLE_ID.into());
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty());
    }

    #[test]
    fn system_app_names_are_excluded() {
        for name in &["Dock", "WindowServer", "SystemUIServer", "Control Center"] {
            let mut r = raw(1);
            r.app_name = (*name).into();
            r.bundle_id = None; // some don't have a stable bundle id
            let out = filter_standard_windows(&[r]);
            assert!(out.is_empty(), "{name} should be excluded");
        }
    }

    #[test]
    fn windows_smaller_than_min_are_excluded() {
        let mut r = raw(1);
        r.bounds = WindowBounds { x: 0, y: 0, w: 50, h: 50 };
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty());
    }

    #[test]
    fn preserves_input_order() {
        let mut a = raw(1);
        a.title = "A".into();
        let mut b = raw(2);
        b.title = "B".into();
        let mut c = raw(3);
        c.title = "C".into();
        let out = filter_standard_windows(&[a, b, c]);
        let titles: Vec<&str> = out.iter().map(|w| w.title.as_str()).collect();
        assert_eq!(titles, vec!["A", "B", "C"]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib window_picker::filter`
Expected: tests compile and run; all pass. (The filter is implemented inline with the tests, so this step verifies the implementation works.)

- [ ] **Step 3: If any test fails, fix the filter implementation until all green.**

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/window_picker/filter.rs
git commit -m "feat(window_picker): pure standard-window filter + tests"
```

---

## Task 5: Pure hit-test

**Files:**
- Modify: `src-tauri/src/window_picker/hit_test.rs`

- [ ] **Step 1: Write the failing tests**

Replace `src-tauri/src/window_picker/hit_test.rs` with:

```rust
// ABOUTME: Pure cursor-position → window_id hit test for the picker mode.
// ABOUTME: Iterates the front-to-back ordered window list and returns the first contains-match.

use super::{PickableWindow, WindowBounds};

/// Returns the `window_id` of the topmost window containing the cursor, or
/// `None` if the cursor is over no pickable window. Input list is assumed to
/// be in **front-to-back** order; the first match wins.
///
/// Coordinates are physical pixels in global screen space.
pub fn hit_test(windows: &[PickableWindow], cursor_x: i32, cursor_y: i32) -> Option<u32> {
    windows.iter().find(|w| contains(&w.bounds, cursor_x, cursor_y)).map(|w| w.window_id)
}

fn contains(b: &WindowBounds, x: i32, y: i32) -> bool {
    x >= b.x && x < b.x + b.w as i32 && y >= b.y && y < b.y + b.h as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pw(id: u32, x: i32, y: i32, w: u32, h: u32) -> PickableWindow {
        PickableWindow {
            window_id: id,
            app_name: format!("App{id}"),
            title: format!("Window {id}"),
            bounds: WindowBounds { x, y, w, h },
            display_index: 0,
            layer: 0,
        }
    }

    #[test]
    fn cursor_inside_single_window_returns_its_id() {
        let ws = vec![pw(1, 100, 100, 400, 300)];
        assert_eq!(hit_test(&ws, 200, 200), Some(1));
    }

    #[test]
    fn cursor_outside_all_windows_returns_none() {
        let ws = vec![pw(1, 100, 100, 400, 300)];
        assert_eq!(hit_test(&ws, 50, 50), None);
        assert_eq!(hit_test(&ws, 600, 600), None);
    }

    #[test]
    fn cursor_in_overlap_returns_front_window() {
        // Input order is front-to-back: id 2 is in front of id 1.
        let ws = vec![pw(2, 150, 150, 300, 300), pw(1, 100, 100, 400, 400)];
        assert_eq!(hit_test(&ws, 200, 200), Some(2));
    }

    #[test]
    fn cursor_on_left_edge_is_inside() {
        let ws = vec![pw(1, 100, 100, 50, 50)];
        assert_eq!(hit_test(&ws, 100, 120), Some(1));
    }

    #[test]
    fn cursor_on_right_edge_is_outside_half_open_interval() {
        let ws = vec![pw(1, 100, 100, 50, 50)];
        // bounds.x + w = 150 → 150 is NOT inside (half-open).
        assert_eq!(hit_test(&ws, 150, 120), None);
    }

    #[test]
    fn empty_window_list_returns_none() {
        assert_eq!(hit_test(&[], 0, 0), None);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib window_picker::hit_test`
Expected: PASS — all six tests.

- [ ] **Step 3: No additional implementation; the test file and implementation are written together.**

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/window_picker/hit_test.rs
git commit -m "feat(window_picker): pure hit_test + tests"
```

---

## Task 6: Shared hit-test fixtures (Rust + JS parity)

**Files:**
- Create: `tests/fixtures/hit-test-cases.json`
- Modify: `src-tauri/src/window_picker/hit_test.rs` (add parity test that loads the fixture)

- [ ] **Step 1: Create the fixture file**

Create `tests/fixtures/hit-test-cases.json`:

```json
{
  "cases": [
    {
      "name": "inside-single",
      "windows": [
        { "window_id": 1, "bounds": { "x": 100, "y": 100, "w": 400, "h": 300 } }
      ],
      "cursor": { "x": 200, "y": 200 },
      "expected": 1
    },
    {
      "name": "outside-all",
      "windows": [
        { "window_id": 1, "bounds": { "x": 100, "y": 100, "w": 400, "h": 300 } }
      ],
      "cursor": { "x": 50, "y": 50 },
      "expected": null
    },
    {
      "name": "front-wins-on-overlap",
      "windows": [
        { "window_id": 2, "bounds": { "x": 150, "y": 150, "w": 300, "h": 300 } },
        { "window_id": 1, "bounds": { "x": 100, "y": 100, "w": 400, "h": 400 } }
      ],
      "cursor": { "x": 200, "y": 200 },
      "expected": 2
    },
    {
      "name": "right-edge-is-half-open",
      "windows": [
        { "window_id": 1, "bounds": { "x": 100, "y": 100, "w": 50, "h": 50 } }
      ],
      "cursor": { "x": 150, "y": 120 },
      "expected": null
    },
    {
      "name": "left-edge-is-inside",
      "windows": [
        { "window_id": 1, "bounds": { "x": 100, "y": 100, "w": 50, "h": 50 } }
      ],
      "cursor": { "x": 100, "y": 120 },
      "expected": 1
    }
  ]
}
```

- [ ] **Step 2: Write the failing parity test in Rust**

Append to `src-tauri/src/window_picker/hit_test.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn parity_with_shared_fixture() {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture { cases: Vec<Case> }
    #[derive(Deserialize)]
    struct Case {
        name: String,
        windows: Vec<FixtureWindow>,
        cursor: Cursor,
        expected: Option<u32>,
    }
    #[derive(Deserialize)]
    struct FixtureWindow { window_id: u32, bounds: WindowBounds }
    #[derive(Deserialize)]
    struct Cursor { x: i32, y: i32 }

    let raw = include_str!("../../../tests/fixtures/hit-test-cases.json");
    let fixture: Fixture = serde_json::from_str(raw).expect("fixture parses");

    for case in fixture.cases {
        let ws: Vec<PickableWindow> = case.windows.into_iter().map(|w| PickableWindow {
            window_id: w.window_id,
            app_name: String::new(),
            title: String::new(),
            bounds: w.bounds,
            display_index: 0,
            layer: 0,
        }).collect();
        let got = hit_test(&ws, case.cursor.x, case.cursor.y);
        assert_eq!(got, case.expected, "case `{}` mismatch", case.name);
    }
}
```

- [ ] **Step 3: Run the parity test**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib window_picker::hit_test::tests::parity_with_shared_fixture`
Expected: PASS — all five cases match.

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 5: Commit**

```bash
git add tests/fixtures/hit-test-cases.json src-tauri/src/window_picker/hit_test.rs
git commit -m "test(window_picker): shared hit-test fixture for Rust + JS parity"
```

---

## Task 7: Coordinate scaling helper

**Files:**
- Modify: `src-tauri/src/window_picker/coords.rs`

- [ ] **Step 1: Write the failing tests**

Replace `src-tauri/src/window_picker/coords.rs` with:

```rust
// ABOUTME: Coordinate conversions for window picker — logical points to physical pixels.
// ABOUTME: All hit-testing operates on physical pixels to match Tauri's cursor_position units.

use super::WindowBounds;

/// Convert a SCWindow CGRect (logical points, origin top-left of the global
/// display arrangement) into physical-pixel bounds. The display the window
/// primarily lives on supplies the scale_factor. The display's *own* physical
/// origin is supplied so we can offset the rect into global physical space.
///
/// All inputs and outputs are i32/f32 — overflow guards aren't critical given
/// realistic display geometry.
pub fn points_to_physical_bounds(
    rect_points: (f32, f32, f32, f32),       // (x, y, w, h) in logical points
    display_logical_origin: (i32, i32),      // logical-point origin of the display
    display_physical_origin: (i32, i32),     // physical-pixel origin of the same display
    scale_factor: f32,
) -> WindowBounds {
    let (rx, ry, rw, rh) = rect_points;
    let (dlx, dly) = display_logical_origin;
    let (dpx, dpy) = display_physical_origin;
    // 1. translate rect's logical origin into display-local logical coords
    let local_x = rx - dlx as f32;
    let local_y = ry - dly as f32;
    // 2. scale to physical pixels
    let phys_local_x = local_x * scale_factor;
    let phys_local_y = local_y * scale_factor;
    let phys_w = rw * scale_factor;
    let phys_h = rh * scale_factor;
    // 3. translate back into global physical coords using display physical origin
    WindowBounds {
        x: (dpx as f32 + phys_local_x).round() as i32,
        y: (dpy as f32 + phys_local_y).round() as i32,
        w: phys_w.round().max(0.0) as u32,
        h: phys_h.round().max(0.0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_when_scale_is_1_and_origins_match() {
        let out = points_to_physical_bounds(
            (10.0, 20.0, 800.0, 600.0),
            (0, 0),
            (0, 0),
            1.0,
        );
        assert_eq!(out, WindowBounds { x: 10, y: 20, w: 800, h: 600 });
    }

    #[test]
    fn scales_dimensions_by_factor() {
        let out = points_to_physical_bounds(
            (0.0, 0.0, 800.0, 600.0),
            (0, 0),
            (0, 0),
            2.0,
        );
        assert_eq!(out, WindowBounds { x: 0, y: 0, w: 1600, h: 1200 });
    }

    #[test]
    fn offsets_into_secondary_display_physical_origin() {
        // Secondary display sits to the right of primary in logical space at
        // x=1440pt, primary is 1440×900 @ 2x → 2880px wide.
        let out = points_to_physical_bounds(
            (1500.0, 50.0, 200.0, 100.0),   // window 60pt into the secondary display
            (1440, 0),                       // logical origin of secondary
            (2880, 0),                       // physical origin of secondary
            2.0,
        );
        // local_x = 1500 - 1440 = 60pt → 120px; global_x = 2880 + 120 = 3000
        assert_eq!(out, WindowBounds { x: 3000, y: 100, w: 400, h: 200 });
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib window_picker::coords`
Expected: PASS — all three tests.

- [ ] **Step 3: No additional implementation needed.**

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/window_picker/coords.rs
git commit -m "feat(window_picker): coord conversion helper + tests"
```

---

## Task 8: RunMode + picker_session state holder

**Files:**
- Create: `src-tauri/src/picker_session.rs`
- Modify: `src-tauri/src/lib.rs` — register module, attach to AppState

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/picker_session.rs`:

```rust
// ABOUTME: Picker-mode runtime state holder. Enforces mutual exclusion across
// ABOUTME: live/frozen/picking/annotating modes via a single RunMode enum.

use std::sync::Mutex;

use crate::window_picker::{DisplayBounds, PickableWindow};

#[derive(Debug, Clone, PartialEq)]
pub enum RunMode {
    Idle,
    Live,
    Frozen,
    Picking { session: PickerSession },
    Annotating { session: AnnotatingSession },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PickerSession {
    pub windows: Vec<PickableWindow>,
    pub displays: Vec<DisplayBounds>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotatingSession {
    pub window_id: u32,
    pub window: PickableWindow,
    pub display: DisplayBounds,
}

pub struct PickerStateHolder {
    inner: Mutex<RunMode>,
}

impl PickerStateHolder {
    pub fn new() -> Self {
        Self { inner: Mutex::new(RunMode::Idle) }
    }

    pub fn current(&self) -> RunMode {
        self.inner.lock().unwrap().clone()
    }

    pub fn try_enter_live(&self) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Idle) {
            return Err(format!("cannot enter live from {:?}", *g));
        }
        *g = RunMode::Live;
        Ok(())
    }

    pub fn try_enter_frozen(&self) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Idle) {
            return Err(format!("cannot enter frozen from {:?}", *g));
        }
        *g = RunMode::Frozen;
        Ok(())
    }

    pub fn try_enter_picking(&self, session: PickerSession) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Idle) {
            return Err(format!("cannot enter picking from {:?}", *g));
        }
        *g = RunMode::Picking { session };
        Ok(())
    }

    pub fn try_select_window(&self, session: AnnotatingSession) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Picking { .. }) {
            return Err(format!("cannot select window from {:?}", *g));
        }
        *g = RunMode::Annotating { session };
        Ok(())
    }

    pub fn reset_to_idle(&self) {
        let mut g = self.inner.lock().unwrap();
        *g = RunMode::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_picker::WindowBounds;

    fn session() -> PickerSession {
        PickerSession {
            windows: vec![],
            displays: vec![DisplayBounds {
                display_index: 0, x: 0, y: 0, w: 2560, h: 1414, scale_factor: 2.0,
            }],
        }
    }

    fn pw() -> PickableWindow {
        PickableWindow {
            window_id: 7, app_name: "Safari".into(), title: "x".into(),
            bounds: WindowBounds { x: 0, y: 0, w: 1, h: 1 },
            display_index: 0, layer: 0,
        }
    }

    fn ann() -> AnnotatingSession {
        AnnotatingSession {
            window_id: 7,
            window: pw(),
            display: DisplayBounds { display_index: 0, x: 0, y: 0, w: 1, h: 1, scale_factor: 1.0 },
        }
    }

    #[test]
    fn starts_idle() {
        let h = PickerStateHolder::new();
        assert!(matches!(h.current(), RunMode::Idle));
    }

    #[test]
    fn enters_picking_from_idle() {
        let h = PickerStateHolder::new();
        h.try_enter_picking(session()).unwrap();
        assert!(matches!(h.current(), RunMode::Picking { .. }));
    }

    #[test]
    fn rejects_picking_from_live() {
        let h = PickerStateHolder::new();
        h.try_enter_live().unwrap();
        let err = h.try_enter_picking(session()).unwrap_err();
        assert!(err.contains("Live"), "got: {err}");
    }

    #[test]
    fn picker_to_annotating_succeeds() {
        let h = PickerStateHolder::new();
        h.try_enter_picking(session()).unwrap();
        h.try_select_window(ann()).unwrap();
        assert!(matches!(h.current(), RunMode::Annotating { .. }));
    }

    #[test]
    fn select_window_rejected_from_idle() {
        let h = PickerStateHolder::new();
        let err = h.try_select_window(ann()).unwrap_err();
        assert!(err.contains("Idle"));
    }

    #[test]
    fn reset_to_idle_from_annotating() {
        let h = PickerStateHolder::new();
        h.try_enter_picking(session()).unwrap();
        h.try_select_window(ann()).unwrap();
        h.reset_to_idle();
        assert!(matches!(h.current(), RunMode::Idle));
    }
}
```

Add to `src-tauri/src/lib.rs` near the other `mod` decls:

```rust
mod picker_session;
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib picker_session`
Expected: PASS — all six tests.

- [ ] **Step 3: Attach `PickerStateHolder` to `AppState`**

Edit `src-tauri/src/lib.rs`:

In `AppState`, add a new field:

```rust
pub picker_state: Arc<picker_session::PickerStateHolder>,
```

In `run()`'s `tauri::Builder::default()...manage(AppState { ... })` block, initialise it:

```rust
picker_state: Arc::new(picker_session::PickerStateHolder::new()),
```

In the existing `app_state_latest_snapshot_at_starts_none` unit test in `lib.rs`, also add `picker_state: Arc::new(picker_session::PickerStateHolder::new()),` to the AppState literal so it still compiles.

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/picker_session.rs src-tauri/src/lib.rs
git commit -m "feat(picker_session): RunMode state holder enforcing mutual exclusion"
```

---

## Task 9: macOS window enumeration (real platform code)

**Files:**
- Modify: `src-tauri/src/window_picker/macos.rs`

> This task touches real macOS SCShareableContent. Unit tests only smoke-check the integration; full verification happens in the manual smoke checklist (Task 26).

- [ ] **Step 1: Write the macOS-only smoke test**

Create `src-tauri/src/window_picker/macos.rs`:

```rust
// ABOUTME: macOS-specific window enumeration via SCShareableContent + filter pipeline.
// ABOUTME: Translates SCWindow logical-point CGRects into PickableWindow physical-px bounds.

use screencapturekit::shareable_content::SCShareableContent;

use super::{
    coords::points_to_physical_bounds,
    filter::{filter_standard_windows, RawWindow},
    DisplayBounds, PickableWindow, WindowBounds,
};

#[derive(Debug, thiserror::Error)]
pub enum EnumerateError {
    #[error("SCShareableContent: {0}")]
    ShareableContent(String),
}

/// Enumerate pickable windows and the displays they live on.
///
/// Returns the windows in front-to-back order (preserving SCShareableContent's
/// own ordering), filtered to "standard app windows", with bounds normalised
/// to physical pixels in the global screen coordinate space.
pub fn enumerate(
    app: &tauri::AppHandle,
) -> Result<(Vec<PickableWindow>, Vec<DisplayBounds>), EnumerateError> {
    use tauri::Manager;
    let monitors = app
        .available_monitors()
        .map_err(|e| EnumerateError::ShareableContent(format!("monitors: {e}")))?;

    let displays: Vec<DisplayBounds> = monitors
        .iter()
        .enumerate()
        .map(|(i, m)| DisplayBounds {
            display_index: i,
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
            scale_factor: m.scale_factor() as f32,
        })
        .collect();

    let content = SCShareableContent::get()
        .map_err(|e| EnumerateError::ShareableContent(format!("{e:?}")))?;

    // SCDisplay::frame() is in logical points (origin in the global logical
    // arrangement). Pair these with Tauri monitors by index — the existing
    // capture path already assumes same-index ordering (see capture/macos.rs
    // for the TODO note documenting this).
    let display_logical_frames: Vec<(f64, f64, f64, f64)> = content
        .displays()
        .iter()
        .map(|d| {
            let f = d.frame();
            (f.origin.x, f.origin.y, f.size.width, f.size.height)
        })
        .collect();

    let raws: Vec<RawWindow> = content
        .windows()
        .iter()
        .filter_map(|w| {
            let frame = w.frame();
            let app_obj = w.owning_application();
            let app_name = app_obj
                .as_ref()
                .and_then(|a| a.application_name().ok())
                .unwrap_or_default();
            let bundle_id = app_obj
                .as_ref()
                .and_then(|a| a.bundle_identifier().ok());

            // Find the display this window primarily lives on by checking its
            // logical-point origin against each SCDisplay's logical frame.
            let display_index = display_logical_frames
                .iter()
                .position(|(lx, ly, lw, lh)| {
                    frame.origin.x >= *lx
                        && frame.origin.x < lx + lw
                        && frame.origin.y >= *ly
                        && frame.origin.y < ly + lh
                })
                .unwrap_or(0);

            let display = displays.get(display_index)?;
            let (lx, ly, _, _) = display_logical_frames.get(display_index).copied().unwrap_or((0.0, 0.0, 0.0, 0.0));
            let bounds = points_to_physical_bounds(
                (frame.origin.x as f32, frame.origin.y as f32, frame.size.width as f32, frame.size.height as f32),
                (lx.round() as i32, ly.round() as i32),     // logical origin (points)
                (display.x, display.y),                       // physical origin (px) from Tauri
                display.scale_factor,
            );

            Some(RawWindow {
                window_id: w.window_id(),
                app_name,
                bundle_id,
                title: w.title().unwrap_or_default(),
                bounds,
                display_index,
                layer: w.window_layer() as i32,
                is_on_screen: w.is_on_screen(),
            })
        })
        .collect();

    Ok((filter_standard_windows(&raws), displays))
}

// The macOS enumerator has no unit-testable surface beyond what's already
// covered by window_picker::filter and window_picker::coords. End-to-end
// verification is the manual smoke checklist (Task 25).
```

NOTE: the `screencapturekit` crate API may have subtly different method names (`window_id()` vs `id()`, `is_on_screen()` vs `on_screen()`, etc.). Run the build, fix names against the actual crate API errors, keep iterating until it compiles. The pure-Rust unit test above does not call into SCShareableContent and runs on any platform.

- [ ] **Step 2: Verify the full crate builds**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: clean build with no warnings.

If method names don't match the actual `screencapturekit` v6 API, fix them per the compiler error. Reference: `src-tauri/src/capture/macos.rs` already uses `SCShareableContent::get()`, `content.displays()`; windows enumeration is `content.windows()`.

The enumerator has no unit-testable surface beyond what's covered by `window_picker::filter` and `window_picker::coords` — end-to-end verification happens in the manual smoke checklist (Task 25).

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/window_picker/macos.rs
git commit -m "feat(window_picker): macOS enumeration via SCShareableContent"
```

---

## Task 10: `capture::capture_window` for macOS

**Files:**
- Modify: `src-tauri/src/capture/mod.rs`
- Modify: `src-tauri/src/capture/macos.rs`

- [ ] **Step 1: Add the cross-platform surface**

In `src-tauri/src/capture/mod.rs`, add (after the existing `capture_active_monitor` definitions):

```rust
#[cfg(target_os = "macos")]
pub fn capture_window(app: &tauri::AppHandle, window_id: u32) -> Result<CapturedFrame, CaptureError> {
    macos::capture_window(app, window_id)
}

#[cfg(not(target_os = "macos"))]
pub fn capture_window(_app: &tauri::AppHandle, _window_id: u32) -> Result<CapturedFrame, CaptureError> {
    Err(CaptureError::Platform("only macOS is supported".into()))
}
```

- [ ] **Step 2: Add the macOS impl**

In `src-tauri/src/capture/macos.rs`, add a new fn at the bottom (mirrors the existing `capture_active_monitor`):

```rust
use screencapturekit::shareable_content::SCShareableContent;
// (SCShareableContent::get + SCContentFilter + SCStreamConfiguration + SCScreenshotManager
// already imported at top of file)

pub(super) fn capture_window(
    app: &tauri::AppHandle,
    window_id: u32,
) -> Result<CapturedFrame, CaptureError> {
    use tauri::Manager;

    let content = SCShareableContent::get()
        .map_err(|e| CaptureError::Platform(format!("shareable content: {e:?}")))?;

    let window = content
        .windows()
        .iter()
        .find(|w| w.window_id() == window_id)
        .cloned()
        .ok_or_else(|| CaptureError::Platform(format!("window {window_id} no longer available")))?;

    let frame = window.frame();
    let logical_w = frame.size.width as f32;
    let logical_h = frame.size.height as f32;

    // Determine the window's owning display so we know the scale factor.
    let monitors = app
        .available_monitors()
        .map_err(|e| CaptureError::Platform(format!("monitors: {e}")))?;
    let scale_factor = monitors
        .iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            let mx_min = pos.x as f64;
            let my_min = pos.y as f64;
            let mx_max = mx_min + size.width as f64;
            let my_max = my_min + size.height as f64;
            frame.origin.x >= mx_min
                && frame.origin.x < mx_max
                && frame.origin.y >= my_min
                && frame.origin.y < my_max
        })
        .map(|m| m.scale_factor() as f32)
        .unwrap_or(1.0);

    let physical_w = (logical_w * scale_factor).round() as u32;
    let physical_h = (logical_h * scale_factor).round() as u32;

    let config = SCStreamConfiguration::new()
        .with_width(physical_w as usize)
        .with_height(physical_h as usize);

    let filter = SCContentFilter::create().with_desktop_independent_window(&window).build();

    let image = SCScreenshotManager::capture_image(&filter, &config)
        .map_err(|e| CaptureError::Platform(format!("capture: {e:?}")))?;

    let pixel_width = image.width() as u32;
    let pixel_height = image.height() as u32;
    let rgba = image
        .rgba_data()
        .map_err(|e| CaptureError::Encoding(format!("rgba_data: {e:?}")))?;

    let img = RgbaImage::from_raw(pixel_width, pixel_height, rgba)
        .ok_or_else(|| CaptureError::Encoding("rgba buffer size mismatch".into()))?;

    let mut png_bytes: Vec<u8> = Vec::new();
    PngEncoder::new(&mut png_bytes)
        .write_image(img.as_raw(), pixel_width, pixel_height, image::ColorType::Rgba8.into())
        .map_err(|e| CaptureError::Encoding(format!("png encode: {e}")))?;

    Ok(CapturedFrame {
        width: pixel_width,
        height: pixel_height,
        scale_factor,
        png: png_bytes,
        display_index: 0, // set by the caller from picker_session
    })
}
```

NOTE: The exact `SCContentFilter` builder method for a single-window filter in `screencapturekit` v6 may be one of `with_desktop_independent_window`, `with_window`, or similar. Check by running `cargo build` and fixing per the compiler error.

- [ ] **Step 3: Verify the build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: clean build.

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

Note: `capture_window` is not covered by an automated test in this task; manual smoke is in Task 26.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/capture/mod.rs src-tauri/src/capture/macos.rs
git commit -m "feat(capture): capture_window via SCContentFilter::with_window"
```

---

## Task 11: Register ⌃⌥⌘J hotkey

**Files:**
- Modify: `src-tauri/src/hotkey.rs`

- [ ] **Step 1: Write the failing tests**

Append to `src-tauri/src/hotkey.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn window_picker_hotkey_uses_ctrl_opt_cmd_j() {
    let hk = window_picker_hotkey();
    assert_eq!(hk.key, Code::KeyJ);
    assert!(hk.mods.contains(Modifiers::CONTROL));
    assert!(hk.mods.contains(Modifiers::ALT));
    assert!(hk.mods.contains(Modifiers::SUPER));
}

#[test]
fn window_picker_hotkey_is_distinct_from_live_and_frozen() {
    assert_ne!(window_picker_hotkey().key, live_hotkey().key);
    assert_ne!(window_picker_hotkey().key, frozen_hotkey().key);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib hotkey::tests::window_picker_hotkey`
Expected: FAIL — `window_picker_hotkey` is not defined.

- [ ] **Step 3: Add the new hotkey and registration**

Add to `src-tauri/src/hotkey.rs` after `frozen_hotkey`:

```rust
pub fn window_picker_hotkey() -> Shortcut {
    Shortcut::new(Some(TRIGGER_MODS), Code::KeyJ)
}
```

In `register()`, add another `on_shortcut` call after the frozen one (the handler dispatches to a `trigger_window_picker` function we'll declare in lib.rs in Task 13 — for now wire it up as a no-op so the test passes):

```rust
ext.on_shortcut(window_picker_hotkey(), |app, _shortcut, event| {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::trigger_window_picker(&handle).await {
            eprintln!("trigger_window_picker failed: {e}");
        }
    });
})?;
```

In `src-tauri/src/lib.rs`, add a stub at the top-level so this compiles:

```rust
pub(crate) async fn trigger_window_picker(_app: &tauri::AppHandle) -> anyhow::Result<()> {
    Ok(())   // wired up in Task 13
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib hotkey`
Expected: PASS — all three hotkey tests (live, frozen, window_picker).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/hotkey.rs src-tauri/src/lib.rs
git commit -m "feat(hotkey): register Ctrl+Opt+Cmd+J for window picker"
```

---

## Task 12: Declare picker-0..picker-7 windows in tauri.conf.json

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Update the config**

In `src-tauri/tauri.conf.json`, replace the `windows` array with:

```json
"windows": [
  {
    "label": "overlay",
    "url": "overlay.html",
    "title": "Glitch Spot Overlay",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-0",
    "url": "picker.html",
    "title": "Glitch Spot Picker 0",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-1",
    "url": "picker.html",
    "title": "Glitch Spot Picker 1",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-2",
    "url": "picker.html",
    "title": "Glitch Spot Picker 2",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-3",
    "url": "picker.html",
    "title": "Glitch Spot Picker 3",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-4",
    "url": "picker.html",
    "title": "Glitch Spot Picker 4",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-5",
    "url": "picker.html",
    "title": "Glitch Spot Picker 5",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-6",
    "url": "picker.html",
    "title": "Glitch Spot Picker 6",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  },
  {
    "label": "picker-7",
    "url": "picker.html",
    "title": "Glitch Spot Picker 7",
    "transparent": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false,
    "shadow": false,
    "focus": true,
    "acceptFirstMouse": true
  }
]
```

The corresponding `picker.html` file is created in Task 17. For now the build will fail at the bundling step because `picker.html` doesn't exist yet. Don't run the full build — instead skip ahead to verify the config is valid JSON:

- [ ] **Step 2: Verify the config parses**

Run: `python3 -c "import json; json.load(open('src-tauri/tauri.conf.json'))"`
Expected: no output (parses successfully).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat(config): declare 8 picker overlay windows in tauri.conf.json"
```

---

## Task 13: Picker Tauri commands + overlay::show_window_mode

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/overlay.rs`

- [ ] **Step 1: Add `show_window_mode` to overlay**

Append to `src-tauri/src/overlay.rs` after `show_frozen`:

```rust
/// Payload sent to the overlay React tree when transitioning into window mode.
/// Coordinates are mixed-unit by design: backend keeps physical-px for capture,
/// frontend renders in CSS-px.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowModePayload {
    pub app_name: String,
    pub title: String,
    pub window_id: u32,
    /// Window rect in CSS pixels relative to the overlay's top-left.
    pub window_rect_css: CssRect,
    /// Same window rect in physical pixels; used by the backend at save time.
    pub window_rect_physical: WindowRectPhysical,
    pub display_scale_factor: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CssRect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowRectPhysical { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }

pub fn show_window_mode(
    app: &tauri::AppHandle,
    display_index: usize,
    payload: &WindowModePayload,
) -> Result<(), OverlayError> {
    let window: WebviewWindow = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;
    let monitors = app.available_monitors()?;
    let monitor = monitors.get(display_index).ok_or(OverlayError::NoMonitor)?;

    // Cover the *full* physical display (not just work area) so a window that
    // overlaps the menu bar can be annotated without cropping.
    let pos = monitor.position();
    let size = monitor.size();
    window.set_position(PhysicalPosition::new(pos.x, pos.y))?;
    window.set_size(PhysicalSize::new(size.width, size.height))?;

    let json = serde_json::to_string(payload).map_err(|e| OverlayError::Tauri(tauri::Error::Json(e)))?;
    let js = format!(
        "if (window.__glitchSpotPresent) window.__glitchSpotPresent('window', {});",
        json
    );
    let _ = window.eval(&js);
    window.show()?;
    window.set_focus()?;
    Ok(())
}
```

- [ ] **Step 2: Add the picker commands and `trigger_window_picker`**

Replace the stub `trigger_window_picker` in `src-tauri/src/lib.rs` with the real implementation:

```rust
pub(crate) async fn trigger_window_picker(app: &tauri::AppHandle) -> anyhow::Result<()> {
    use tauri::Manager;
    let state = app.state::<AppState>();

    let (windows, displays) = window_picker::macos::enumerate(app)
        .map_err(|e| anyhow::anyhow!("enumerate: {e}"))?;

    let session = picker_session::PickerSession {
        windows: windows.clone(),
        displays: displays.clone(),
    };
    state
        .picker_state
        .try_enter_picking(session.clone())
        .map_err(|e| anyhow::anyhow!(e))?;

    // Show one picker window per active display, capped at 8.
    const POOL_SIZE: usize = 8;
    let count = displays.len().min(POOL_SIZE);
    if displays.len() > POOL_SIZE {
        eprintln!("[glitch-spot] {} displays detected, picker covers first {}", displays.len(), POOL_SIZE);
    }
    for i in 0..count {
        let label = format!("picker-{i}");
        let Some(win) = app.get_webview_window(&label) else {
            eprintln!("[glitch-spot] picker window `{label}` not configured");
            continue;
        };
        let d = &displays[i];
        win.set_position(tauri::PhysicalPosition::new(d.x, d.y))?;
        win.set_size(tauri::PhysicalSize::new(d.w, d.h))?;
        let payload = serde_json::json!({
            "windows": windows,
            "displays": displays,
            "my_display_index": i,
        });
        let js = format!(
            "if (window.__glitchSpotPickerStart) window.__glitchSpotPickerStart({});",
            payload
        );
        let _ = win.eval(&js);
        win.show()?;
        win.set_focus()?;
    }
    Ok(())
}

#[tauri::command]
async fn picker_select(app: tauri::AppHandle, window_id: u32) -> Result<(), String> {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let session = match state.picker_state.current() {
        picker_session::RunMode::Picking { session } => session,
        other => return Err(format!("picker_select called while in {:?}", other)),
    };
    let window = session
        .windows
        .iter()
        .find(|w| w.window_id == window_id)
        .cloned()
        .ok_or_else(|| format!("window {window_id} not in session"))?;
    let display = session
        .displays
        .iter()
        .find(|d| d.display_index == window.display_index)
        .copied()
        .ok_or_else(|| format!("display {} not in session", window.display_index))?;

    // Hide all picker windows.
    for i in 0..8 {
        let label = format!("picker-{i}");
        if let Some(w) = app.get_webview_window(&label) { let _ = w.hide(); }
    }

    let scale = display.scale_factor;
    let payload = overlay::WindowModePayload {
        app_name: window.app_name.clone(),
        title: window.title.clone(),
        window_id: window.window_id,
        window_rect_css: overlay::CssRect {
            x: (window.bounds.x - display.x) as f32 / scale,
            y: (window.bounds.y - display.y) as f32 / scale,
            w: window.bounds.w as f32 / scale,
            h: window.bounds.h as f32 / scale,
        },
        window_rect_physical: overlay::WindowRectPhysical {
            x: window.bounds.x - display.x,
            y: window.bounds.y - display.y,
            w: window.bounds.w,
            h: window.bounds.h,
        },
        display_scale_factor: scale,
    };

    state
        .picker_state
        .try_select_window(picker_session::AnnotatingSession {
            window_id,
            window: window.clone(),
            display,
        })
        .map_err(|e| e)?;

    overlay::show_window_mode(&app, window.display_index, &payload).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn picker_cancel(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    for i in 0..8 {
        let label = format!("picker-{i}");
        if let Some(w) = app.get_webview_window(&label) { let _ = w.hide(); }
    }
    let state = app.state::<AppState>();
    state.picker_state.reset_to_idle();
    Ok(())
}
```

Wire the existing live/frozen paths into the RunMode state holder so all three modes share mutual exclusion. In `src-tauri/src/lib.rs`:

- At the top of `trigger_overlay_live`, before `overlay::show_live`, call:

  ```rust
  if let Some(state) = app.try_state::<AppState>() {
      state.picker_state.try_enter_live().map_err(anyhow::Error::msg)?;
  }
  ```

- At the top of `trigger_overlay_frozen`, before the existing body:

  ```rust
  if let Some(state) = app.try_state::<AppState>() {
      state.picker_state.try_enter_frozen().map_err(anyhow::Error::msg)?;
  }
  ```

The `hide_overlay` command (modified later in Task 15) resets state to Idle, which closes the loop for both modes.

Register the new commands in `tauri::Builder::default()...invoke_handler(...)`:

```rust
.invoke_handler(tauri::generate_handler![
    capture_active_monitor,
    open_overlay_live,
    open_overlay_frozen,
    hide_overlay,
    get_latest_frame,
    save_snapshot,
    spike_capture::spike_capture,
    share_latest,
    current_tunnel_state,
    set_tunnel_provider,
    picker_select,
    picker_cancel,
])
```

`picker_start` is invoked indirectly via the hotkey calling `trigger_window_picker`, so it doesn't need a `#[tauri::command]` decorator. (If the frontend ever needs to trigger it from JS, add `open_window_picker` like the existing `open_overlay_live`.)

- [ ] **Step 3: Verify the build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: clean build.

- [ ] **Step 4: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green (no new tests, but existing ones stay green).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/overlay.rs
git commit -m "feat(picker): Tauri commands + overlay::show_window_mode"
```

---

## Task 14: Wire `save_snapshot` "window" mode arm

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Update the `save_snapshot` match**

In `src-tauri/src/lib.rs`, replace the existing `let frame = match mode.as_str() { ... }` block in `save_snapshot` with:

```rust
let (frame, crop_origin) = match mode.as_str() {
    "frozen" => {
        let frame = state
            .latest_frame
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .ok_or_else(|| "no cached frame for frozen mode".to_string())?;
        (frame, None)
    }
    "live" => {
        overlay::hide(&app).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(120));
        let frame = capture::capture_active_monitor(&app).map_err(|e| e.to_string())?;
        (frame, None)
    }
    "window" => {
        let session = match state.picker_state.current() {
            picker_session::RunMode::Annotating { session } => session,
            other => return Err(format!("save_snapshot('window') while in {:?}", other)),
        };
        // Hide overlay first so the recapture doesn't include our checker/annotations.
        overlay::hide(&app).map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(120));
        let mut frame = capture::capture_window(&app, session.window_id)
            .map_err(|e| e.to_string())?;
        // capture_window doesn't know the display_index — fill it in from the session.
        frame.display_index = session.window.display_index;
        // crop_origin is the window's display-local position in physical pixels:
        // already exactly the WindowRectPhysical we sent to the frontend.
        let crop_x = (session.window.bounds.x - session.display.x) as f32;
        let crop_y = (session.window.bounds.y - session.display.y) as f32;
        (frame, Some((crop_x, crop_y)))
    }
    other => return Err(format!("unknown mode: {other}")),
};
```

Replace the `snapshot::build::build(...)` call below to pass `crop_origin`:

```rust
let built = snapshot::build::build(
    &frame,
    &mode,
    annotations,
    os_platform,
    &os_version,
    &base,
    crop_origin,
)
.map_err(|e| e.to_string())?;
```

After `snapshot::persist::write`, just before `snap.clone()` is published, attach the window context into the snapshot. Modify the block to:

```rust
snapshot::persist::write(&base, &built).map_err(|e| e.to_string())?;
let mut snap = built.snapshot;

// If we just saved a window-mode snapshot, attach the captured window metadata.
if mode == "window" {
    if let picker_session::RunMode::Annotating { session } = state.picker_state.current() {
        snap.context.window = Some(snapshot::WindowInfo {
            app_name: session.window.app_name.clone(),
            title: session.window.title.clone(),
            bounds: snapshot::WindowBounds {
                x: session.window.bounds.x,
                y: session.window.bounds.y,
                w: session.window.bounds.w,
                h: session.window.bounds.h,
            },
            window_id: session.window_id,
        });
    }
}

state.snapshot_store.publish(snap.clone());
```

At the very end of `save_snapshot`, before returning Ok(snap), reset the picker state to idle so the next hotkey works:

```rust
overlay::hide(&app).map_err(|e| e.to_string())?;
state.picker_state.reset_to_idle();
Ok(snap)
```

- [ ] **Step 2: Verify the build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: clean build. Fix any type mismatches between `window_picker::WindowBounds` and `snapshot::WindowBounds` — they have identical shapes; either reuse the snapshot one in both places or convert explicitly as shown above.

- [ ] **Step 3: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(save_snapshot): handle 'window' mode (recapture by id, crop_origin, context.window)"
```

---

## Task 15: Closed-window error path with notification

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Handle the capture_window error in save_snapshot**

In `src-tauri/src/lib.rs`, modify the `"window"` arm of `save_snapshot` so that if `capture_window` fails with "window no longer available", we fire a notification and short-circuit cleanly:

```rust
"window" => {
    let session = match state.picker_state.current() {
        picker_session::RunMode::Annotating { session } => session,
        other => return Err(format!("save_snapshot('window') while in {:?}", other)),
    };
    overlay::hide(&app).map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(120));
    let frame_result = capture::capture_window(&app, session.window_id);
    let mut frame = match frame_result {
        Ok(f) => f,
        Err(e) => {
            use tauri_plugin_notification::NotificationExt;
            let _ = app.notification()
                .builder()
                .title("Glitch Spot")
                .body("Selected window closed — annotations discarded")
                .show();
            eprintln!("[glitch-spot] window capture failed: {e}");
            state.picker_state.reset_to_idle();
            return Err(format!("window capture failed: {e}"));
        }
    };
    frame.display_index = session.window.display_index;
    let crop_x = (session.window.bounds.x - session.display.x) as f32;
    let crop_y = (session.window.bounds.y - session.display.y) as f32;
    (frame, Some((crop_x, crop_y)))
}
```

Same handling in `picker_cancel` already resets state; ensure the overlay's Esc/cancel path also resets state. Confirm the existing `hide_overlay` command resets the picker state when called during annotation:

```rust
#[tauri::command]
async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.picker_state.reset_to_idle();
    overlay::hide(&app).map_err(|e| e.to_string())
}
```

(Use `tauri::Manager;` import already present in this file.)

- [ ] **Step 2: Verify the build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: clean build.

- [ ] **Step 3: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
Expected: All green.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(save_snapshot): notify and reset state when window is gone at save time"
```

---

## Task 16: Cursor SVG data URL

**Files:**
- Create: `src/picker/cursor.ts`
- Create: `src/picker/cursor.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/picker/cursor.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { PICKER_CURSOR_CSS, PICKER_CURSOR_SVG } from "./cursor";

describe("picker cursor", () => {
  it("SVG has a 48×48 viewBox", () => {
    expect(PICKER_CURSOR_SVG).toContain('viewBox="0 0 48 48"');
  });

  it("includes accent orange #ee6633 strokes", () => {
    expect(PICKER_CURSOR_SVG.toLowerCase()).toContain("#ee6633");
  });

  it("includes the chromatic red and cyan offset rings", () => {
    expect(PICKER_CURSOR_SVG).toContain("rgba(255,0,60,");
    expect(PICKER_CURSOR_SVG).toContain("rgba(0,245,255,");
  });

  it("CSS uses a data URL with hotspot 16 16 and a crosshair fallback", () => {
    expect(PICKER_CURSOR_CSS).toMatch(/^url\("data:image\/svg\+xml/);
    expect(PICKER_CURSOR_CSS).toContain("16 16, crosshair");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/picker/cursor.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Create the cursor module**

Create `src/picker/cursor.ts`:

```ts
// ABOUTME: Inline-SVG cursor for window picker mode. Chromatic crosshair
// ABOUTME: with accent-orange ticks; rendered at 32px with hotspot at centre.

export const PICKER_CURSOR_SVG = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48" width="32" height="32">
  <circle cx="24" cy="24" r="10" fill="#0c0c0c" stroke="#e0e0e0" stroke-width="1.5"/>
  <circle cx="22" cy="24" r="10" fill="none" stroke="rgba(255,0,60,0.85)" stroke-width="1.5"/>
  <circle cx="26" cy="24" r="10" fill="none" stroke="rgba(0,245,255,0.85)" stroke-width="1.5"/>
  <line x1="24" y1="2"  x2="24" y2="10" stroke="#ee6633" stroke-width="2"/>
  <line x1="24" y1="38" x2="24" y2="46" stroke="#ee6633" stroke-width="2"/>
  <line x1="2"  y1="24" x2="10" y2="24" stroke="#ee6633" stroke-width="2"/>
  <line x1="38" y1="24" x2="46" y2="24" stroke="#ee6633" stroke-width="2"/>
</svg>
`.trim();

function encodeSvgForCssUrl(svg: string): string {
  // The smallest reliable encoding for CSS cursor URLs across WebKit: percent-encode '<', '>', '#', and '"'.
  return svg
    .replace(/\n/g, "")
    .replace(/"/g, '\'')
    .replace(/</g, "%3C")
    .replace(/>/g, "%3E")
    .replace(/#/g, "%23");
}

export const PICKER_CURSOR_CSS = `url("data:image/svg+xml;utf8,${encodeSvgForCssUrl(PICKER_CURSOR_SVG)}") 16 16, crosshair`;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/picker/cursor.test.ts`
Expected: PASS — all four assertions.

- [ ] **Step 5: Commit**

```bash
git add src/picker/cursor.ts src/picker/cursor.test.ts
git commit -m "feat(picker): inline SVG picker cursor with chromatic crosshair"
```

---

## Task 17: JS hit-test twin (parity with Rust)

**Files:**
- Create: `src/picker/hitTest.ts`
- Create: `src/picker/hitTest.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/picker/hitTest.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import fixture from "../../tests/fixtures/hit-test-cases.json";
import { hitTest, type PickableBounds } from "./hitTest";

type Case = {
  name: string;
  windows: { window_id: number; bounds: { x: number; y: number; w: number; h: number } }[];
  cursor: { x: number; y: number };
  expected: number | null;
};

describe("hitTest (JS twin)", () => {
  for (const c of (fixture as { cases: Case[] }).cases) {
    it(`case: ${c.name}`, () => {
      const ws: PickableBounds[] = c.windows.map((w) => ({ window_id: w.window_id, bounds: w.bounds }));
      expect(hitTest(ws, c.cursor.x, c.cursor.y)).toBe(c.expected);
    });
  }
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `npx vitest run src/picker/hitTest.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Create the JS hit-test**

Create `src/picker/hitTest.ts`:

```ts
// ABOUTME: Pure cursor-position → window_id hit test for the picker UI.
// ABOUTME: Iterates the front-to-back ordered window list; first containing window wins.

export type PickableBounds = {
  window_id: number;
  bounds: { x: number; y: number; w: number; h: number };
};

export function hitTest(windows: PickableBounds[], cursorX: number, cursorY: number): number | null {
  for (const w of windows) {
    const { x, y, w: width, h: height } = w.bounds;
    if (cursorX >= x && cursorX < x + width && cursorY >= y && cursorY < y + height) {
      return w.window_id;
    }
  }
  return null;
}
```

The project already has `"resolveJsonModule": true` in `tsconfig.json`. However the `include` only covers `src` — extend it so TS can find the fixture file:

```json
"include": ["src", "tests/fixtures"]
```

Apply this edit in `tsconfig.json`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `npx vitest run src/picker/hitTest.test.ts`
Expected: PASS — all five parity cases.

- [ ] **Step 5: Commit**

```bash
git add src/picker/hitTest.ts src/picker/hitTest.test.ts tsconfig.json
git commit -m "feat(picker): JS hit-test twin (parity with Rust via shared fixtures)"
```

---

## Task 18: Picker types shared between TS and Tauri payload

**Files:**
- Create: `src/picker/types.ts`

- [ ] **Step 1: Create the types file**

Create `src/picker/types.ts`:

```ts
// ABOUTME: TS mirrors of the Rust picker payloads sent over Tauri eval / events.
// ABOUTME: Field names match Rust serde (snake_case) — keep in sync with window_picker/mod.rs.

export type WindowBounds = { x: number; y: number; w: number; h: number };

export type PickableWindow = {
  window_id: number;
  app_name: string;
  title: string;
  bounds: WindowBounds;
  display_index: number;
  layer: number;
};

export type DisplayBounds = {
  display_index: number;
  x: number;
  y: number;
  w: number;
  h: number;
  scale_factor: number;
};

export type PickerStartPayload = {
  windows: PickableWindow[];
  displays: DisplayBounds[];
  my_display_index: number;
};

export type WindowModePayload = {
  app_name: string;
  title: string;
  window_id: number;
  window_rect_css: { x: number; y: number; w: number; h: number };
  window_rect_physical: { x: number; y: number; w: number; h: number };
  display_scale_factor: number;
};
```

- [ ] **Step 2: Verify the project type-checks**

Run: `npx tsc --noEmit`
Expected: no new errors.

- [ ] **Step 3: Commit**

```bash
git add src/picker/types.ts
git commit -m "chore(picker): shared TS types mirroring Rust payloads"
```

---

## Task 19: Picker.tsx + entry + picker.html + Vite config

**Files:**
- Create: `picker.html`
- Create: `src/picker/entry.tsx`
- Create: `src/picker/Picker.tsx`
- Modify: `vite.config.ts`

- [ ] **Step 1: Create picker.html**

Create `picker.html` at the project root:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>Glitch Spot Picker</title>
    <style>
      html, body, #root { margin: 0; padding: 0; height: 100%; background: transparent; }
    </style>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/picker/entry.tsx"></script>
  </body>
</html>
```

- [ ] **Step 2: Create entry.tsx**

Create `src/picker/entry.tsx`:

```tsx
// ABOUTME: Entry for picker overlay windows. Mounts <Picker/> into #root.
import React from "react";
import ReactDOM from "react-dom/client";
import { Picker } from "./Picker";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Picker />
  </React.StrictMode>
);
```

- [ ] **Step 3: Create Picker.tsx (minimal mount; behaviour comes in Task 20)**

Create `src/picker/Picker.tsx`:

```tsx
// ABOUTME: Per-display picker UI. Renders a transparent overlay with a custom
// ABOUTME: cursor; on mousemove hit-tests against the session's window list.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PICKER_CURSOR_CSS } from "./cursor";
import { hitTest } from "./hitTest";
import type { PickerStartPayload, PickableWindow, DisplayBounds } from "./types";

export function Picker() {
  const [session, setSession] = useState<PickerStartPayload | null>(null);
  const [hoveredId, setHoveredId] = useState<number | null>(null);

  // Receive the session payload from Rust via window.eval-injected setter.
  useEffect(() => {
    (window as unknown as { __glitchSpotPickerStart?: (p: PickerStartPayload) => void })
      .__glitchSpotPickerStart = (p) => {
        setSession(p);
        setHoveredId(null);
      };
    return () => {
      delete (window as unknown as { __glitchSpotPickerStart?: unknown }).__glitchSpotPickerStart;
    };
  }, []);

  // Esc cancels.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        invoke("picker_cancel");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!session) return null;

  const myDisplay: DisplayBounds | undefined = session.displays[session.my_display_index];
  const scale = myDisplay?.scale_factor ?? 1;

  const onMouseMove = (e: React.MouseEvent) => {
    if (!myDisplay) return;
    // Convert client (CSS px in *this* picker window) → global physical px.
    const globalPhysX = myDisplay.x + e.clientX * scale;
    const globalPhysY = myDisplay.y + e.clientY * scale;
    const id = hitTest(session.windows, globalPhysX, globalPhysY);
    setHoveredId(id);
  };

  const onMouseLeave = () => setHoveredId(null);

  const onClick = () => {
    if (hoveredId == null) {
      invoke("picker_cancel");
      return;
    }
    invoke("picker_select", { windowId: hoveredId });
  };

  const highlight = session.windows.find((w) => w.window_id === hoveredId);

  return (
    <div
      onMouseMove={onMouseMove}
      onMouseLeave={onMouseLeave}
      onClick={onClick}
      style={{
        position: "fixed",
        inset: 0,
        cursor: PICKER_CURSOR_CSS,
        background: "transparent",
      }}
      data-testid="picker-root"
    >
      {highlight && myDisplay && (
        <HighlightRect window={highlight} display={myDisplay} />
      )}
    </div>
  );
}

const CYAN_CHECKER =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'>" +
  "<rect width='10' height='10' fill='%2300f5ff' fill-opacity='0.28'/>" +
  "<rect x='10' y='10' width='10' height='10' fill='%2300f5ff' fill-opacity='0.28'/>" +
  "<rect x='10' y='0' width='10' height='10' fill='%23ffffff' fill-opacity='0.12'/>" +
  "<rect x='0' y='10' width='10' height='10' fill='%23ffffff' fill-opacity='0.12'/>" +
  "</svg>\")";

function HighlightRect({ window: w, display }: { window: PickableWindow; display: DisplayBounds }) {
  const scale = display.scale_factor;
  // Convert window's global physical-px bounds → this picker's display-local CSS px.
  const left = (w.bounds.x - display.x) / scale;
  const top = (w.bounds.y - display.y) / scale;
  const width = w.bounds.w / scale;
  const height = w.bounds.h / scale;
  return (
    <div
      data-testid="picker-highlight"
      style={{
        position: "absolute",
        left,
        top,
        width,
        height,
        backgroundImage: CYAN_CHECKER,
        backgroundRepeat: "repeat",
        pointerEvents: "none",
      }}
    />
  );
}
```

- [ ] **Step 4: Update vite.config.ts**

Replace `src-tauri`/`vite.config.ts` (project root one):

```ts
// ABOUTME: Vite build configuration for Glitch Spot's overlay + picker frontends.
// ABOUTME: Two HTML entrypoints — overlay.html and picker.html — each bundled separately.
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        overlay: resolve(__dirname, "overlay.html"),
        picker: resolve(__dirname, "picker.html"),
      },
    },
  },
});
```

- [ ] **Step 5: Verify the project builds**

Run: `npm run build`
Expected: build completes successfully and produces both `dist/overlay.html` and `dist/picker.html`.

- [ ] **Step 6: Commit**

```bash
git add picker.html src/picker/entry.tsx src/picker/Picker.tsx vite.config.ts
git commit -m "feat(picker): mount Picker.tsx + add picker.html entry"
```

---

## Task 20: Picker.tsx tests (mousemove, click, cancel)

**Files:**
- Create: `src/picker/Picker.test.tsx`

- [ ] **Step 1: Write the tests**

Create `src/picker/Picker.test.tsx`:

```tsx
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import { Picker } from "./Picker";
import type { PickerStartPayload } from "./types";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args: unknown) => invokeMock(cmd, args),
}));

function makeSession(): PickerStartPayload {
  return {
    my_display_index: 0,
    displays: [
      { display_index: 0, x: 0, y: 0, w: 1440, h: 900, scale_factor: 1 },
    ],
    windows: [
      {
        window_id: 7,
        app_name: "Safari",
        title: "Reader",
        bounds: { x: 100, y: 100, w: 400, h: 300 },
        display_index: 0,
        layer: 0,
      },
    ],
  };
}

describe("<Picker/>", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders nothing before the session payload arrives", () => {
    const { queryByTestId } = render(<Picker />);
    expect(queryByTestId("picker-root")).toBeNull();
  });

  it("renders the picker root once __glitchSpotPickerStart is invoked", () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    expect(getByTestId("picker-root")).toBeTruthy();
  });

  it("highlights the window when cursor moves inside its bounds", () => {
    const { getByTestId, queryByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 200, clientY: 200 });
    expect(queryByTestId("picker-highlight")).toBeTruthy();
  });

  it("clears the highlight on mouseLeave", () => {
    const { getByTestId, queryByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 200, clientY: 200 });
    fireEvent.mouseLeave(getByTestId("picker-root"));
    expect(queryByTestId("picker-highlight")).toBeNull();
  });

  it("calls picker_select when clicking on a window", () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 200, clientY: 200 });
    fireEvent.click(getByTestId("picker-root"));
    expect(invokeMock).toHaveBeenCalledWith("picker_select", { windowId: 7 });
  });

  it("calls picker_cancel when clicking on empty desktop", () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 1000, clientY: 800 });
    fireEvent.click(getByTestId("picker-root"));
    expect(invokeMock).toHaveBeenCalledWith("picker_cancel");
  });

  it("calls picker_cancel on Escape", () => {
    render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(invokeMock).toHaveBeenCalledWith("picker_cancel");
  });
});
```

- [ ] **Step 2: Run the tests**

Run: `npx vitest run src/picker/Picker.test.tsx`
Expected: PASS — all seven cases.

- [ ] **Step 3: Commit**

```bash
git add src/picker/Picker.test.tsx
git commit -m "test(picker): Picker.tsx interaction tests"
```

---

## Task 21: DrawingLayer `clipRect` prop

**Files:**
- Modify: `src/overlay/annotations/DrawingLayer.tsx`
- Create: `src/overlay/annotations/DrawingLayer.test.tsx`

- [ ] **Step 1: Write the failing tests**

Create `src/overlay/annotations/DrawingLayer.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { useReducer } from "react";
import { DrawingLayer } from "./DrawingLayer";
import { initialState, reducer } from "./state";

function Harness({ clipRect }: { clipRect?: { x: number; y: number; w: number; h: number } }) {
  const [state, dispatch] = useReducer(reducer, initialState);
  return <DrawingLayer state={state} dispatch={dispatch} clipRect={clipRect} />;
}

describe("DrawingLayer with clipRect", () => {
  it("ignores mouse-down outside clipRect", () => {
    const { container } = render(<Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} />);
    const surface = container.firstChild as HTMLDivElement;
    // 50,50 is outside (100..300, 100..300)
    fireEvent.mouseDown(surface, { clientX: 50, clientY: 50, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 60, clientY: 60 });
    fireEvent.mouseUp(surface);
    expect(surface.querySelectorAll('[data-rect-id]')).toHaveLength(0);
  });

  it("starts a rect when mouse-down inside clipRect", () => {
    const { container } = render(<Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 150, clientY: 150, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 200, clientY: 200 });
    fireEvent.mouseUp(surface);
    const rects = surface.querySelectorAll('[data-rect-id]');
    expect(rects.length).toBe(1);
  });

  it("clamps rect drag to clipRect bounds", () => {
    const { container } = render(<Harness clipRect={{ x: 100, y: 100, w: 200, h: 200 }} />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 150, clientY: 150, button: 0 });
    // Try to drag past clipRect's right edge (300)
    fireEvent.mouseMove(surface, { clientX: 500, clientY: 500 });
    fireEvent.mouseUp(surface);
    const rect = surface.querySelector('[data-rect-id]') as HTMLDivElement | null;
    expect(rect).toBeTruthy();
    const left = parseFloat(rect!.style.left);
    const width = parseFloat(rect!.style.width);
    // Right edge must not exceed clipRect's right edge (300).
    expect(left + width).toBeLessThanOrEqual(300);
  });

  it("when clipRect is undefined, behaves like the existing layer (no clamp)", () => {
    const { container } = render(<Harness />);
    const surface = container.firstChild as HTMLDivElement;
    fireEvent.mouseDown(surface, { clientX: 10, clientY: 10, button: 0 });
    fireEvent.mouseMove(surface, { clientX: 100, clientY: 100 });
    fireEvent.mouseUp(surface);
    expect(surface.querySelectorAll('[data-rect-id]').length).toBe(1);
  });
});
```

NOTE: `getBoundingClientRect` returns 0×0 in jsdom by default; the existing DrawingLayer reads it via `ref.current?.getBoundingClientRect()`. We're testing the clipRect logic, not the ref. We need the test container to have a known offset of (0,0) so clientX/Y maps directly to drawing coords — jsdom's default 0×0 offset is exactly what we want.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/overlay/annotations/DrawingLayer.test.tsx`
Expected: FAIL — `clipRect` prop not supported.

- [ ] **Step 3: Add the clipRect prop to DrawingLayer**

Modify `src/overlay/annotations/DrawingLayer.tsx`:

Change the `Props` type:

```tsx
type ClipRect = { x: number; y: number; w: number; h: number };

type Props = {
  state: AnnotationsState;
  dispatch: (action: AnnotationsAction) => void;
  clipRect?: ClipRect;
};
```

Change the component signature:

```tsx
export function DrawingLayer({ state, dispatch, clipRect }: Props) {
```

Add a clamping helper at the top of the component body (after the existing `useState`):

```tsx
const clamp = (x: number, y: number): { x: number; y: number } | null => {
  if (!clipRect) return { x, y };
  const inside =
    x >= clipRect.x &&
    x < clipRect.x + clipRect.w &&
    y >= clipRect.y &&
    y < clipRect.y + clipRect.h;
  if (!inside) return null;
  const clampedX = Math.max(clipRect.x, Math.min(x, clipRect.x + clipRect.w));
  const clampedY = Math.max(clipRect.y, Math.min(y, clipRect.y + clipRect.h));
  return { x: clampedX, y: clampedY };
};
```

In `onMouseDown`, replace the existing `dispatch({ type: "BEGIN_DRAG", id: nextRectId(), x, y })` with:

```tsx
const startInside = clamp(x, y);
if (!startInside) return;   // mouse-down outside clipRect: no-op
dispatch({ type: "BEGIN_DRAG", id: nextRectId(), x: startInside.x, y: startInside.y });
```

In `onMouseMove`, replace `dispatch({ type: "DRAG_TO", x, y })` with:

```tsx
let nx = x;
let ny = y;
if (clipRect) {
  nx = Math.max(clipRect.x, Math.min(x, clipRect.x + clipRect.w));
  ny = Math.max(clipRect.y, Math.min(y, clipRect.y + clipRect.h));
}
dispatch({ type: "DRAG_TO", x: nx, y: ny });
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run src/overlay/annotations/DrawingLayer.test.tsx`
Expected: PASS — all four cases.

Run: `npx vitest run` (full suite)
Expected: All green — no regression in existing tests.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/annotations/DrawingLayer.tsx src/overlay/annotations/DrawingLayer.test.tsx
git commit -m "feat(drawing): clipRect prop constrains rect creation + drag"
```

---

## Task 22: Overlay window-mode render branch

**Files:**
- Modify: `src/overlay/Overlay.tsx`

- [ ] **Step 1: Extend Mode and accept window payload**

Modify `src/overlay/Overlay.tsx`:

Change the `Mode` type:

```tsx
type Mode = 'live' | 'frozen' | 'window'
```

Add a state hook for the window payload (just below the `mode` useState):

```tsx
import type { WindowModePayload } from '../picker/types'

// ...
const [windowPayload, setWindowPayload] = useState<WindowModePayload | null>(null)
```

Replace the `__glitchSpotPresent` installation in the existing `useEffect`:

```tsx
;(window as unknown as { __glitchSpotPresent?: (mode: Mode, payload?: WindowModePayload) => void }).__glitchSpotPresent = (
  newMode,
  payload,
) => {
  if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current);
  currentUrlRef.current = null;
  setImageUrl(null);
  setMode(newMode);
  dispatch({ type: "CLEAR" });
  setShowDiscardModal(false);
  setWindowPayload(newMode === 'window' && payload ? payload : null);
};
```

Add a new render branch. Replace the existing `return ( <div ...> ... </div> )` block with:

```tsx
const backdrop =
  mode === 'frozen' && imageUrl ? `center / 100% 100% no-repeat url(${imageUrl})` : 'transparent'

if (mode === 'window' && windowPayload) {
  const { window_rect_css } = windowPayload
  return (
    <div style={{ position: 'fixed', inset: 0, background: 'transparent' }}>
      {/* dim veil over everything */}
      <div
        style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.40)' }}
        onClick={() => {
          if (annotations.rects.length === 0) {
            bridge.cancel()
          } else {
            setShowDiscardModal(true)
          }
        }}
      />
      {/* gray-white checker over the window rect */}
      <div
        style={{
          position: 'absolute',
          left: window_rect_css.x,
          top: window_rect_css.y,
          width: window_rect_css.w,
          height: window_rect_css.h,
          backgroundImage: CHECKER_BG,
          backgroundRepeat: 'repeat',
          opacity: 0.12,
          pointerEvents: 'none',
        }}
      />
      {/* drawing layer constrained to window rect */}
      <DrawingLayer state={annotations} dispatch={dispatch} clipRect={window_rect_css} />
      <Toolbar rectCount={annotations.rects.length} onSave={handleSave} />
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

return (
  // ... unchanged live/frozen markup
)
```

- [ ] **Step 2: Verify the project type-checks**

Run: `npx tsc --noEmit`
Expected: clean.

- [ ] **Step 3: Run all tests**

Run: `npx vitest run`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add src/overlay/Overlay.tsx
git commit -m "feat(overlay): window-mode render branch with dim veil + checker cutout"
```

---

## Task 23: handleSave passes 'window' mode through

**Files:**
- Modify: `src/overlay/Overlay.tsx`

- [ ] **Step 1: Verify the existing handleSave works**

The existing `handleSave` already calls `bridge.saveSnapshot(mode, payload)` with the current `mode` state. Since we set `mode = 'window'` when the window payload arrives, no change needed here.

But — the existing `getFrame()` / `onFrame()` paths force `setMode('frozen')` on first frame. We must make sure those don't fire while in window mode:

In the `useEffect` that subscribes to `bridge.onFrame`, gate the `setMode('frozen')` call:

```tsx
bridge
  .onFrame((blobUrl) => {
    // Don't override window mode — frames only arrive on the frozen path.
    if (modeRef.current === 'window') {
      URL.revokeObjectURL(blobUrl)
      return
    }
    setMode("frozen")
    // ... rest unchanged
```

Add a `modeRef` to track the latest mode synchronously (above the useEffect):

```tsx
const modeRef = useRef<Mode>('live')
useEffect(() => { modeRef.current = mode }, [mode])
```

Apply the same gate to the `bridge.getFrame()` callback below.

- [ ] **Step 2: Verify type-check + tests**

Run: `npx tsc --noEmit && npx vitest run`
Expected: clean type-check; all tests green.

- [ ] **Step 3: Commit**

```bash
git add src/overlay/Overlay.tsx
git commit -m "fix(overlay): don't override window mode when a frozen frame arrives"
```

---

## Task 24: End-to-end manual smoke verification

**Files:**
- (no code changes)

- [ ] **Step 1: Build the app**

Run: `npm run build && cargo build --manifest-path src-tauri/Cargo.toml --release`
Expected: clean builds.

- [ ] **Step 2: Run `npm run tauri dev`** and confirm in the running app:

1. Press ⌃⌥⌘J. The cursor changes to the chromatic crosshair. Hovering different windows highlights each with a cyan checker tile. Esc dismisses the picker.
2. Hover a window, click. The picker disappears; the existing overlay appears over that display with a dim veil outside the window and a gray-white checker over the window itself.
3. Draw a rectangle inside the window. The rect appears with the existing orange chromatic split. Try to draw outside the window — nothing happens.
4. Press ⌘↩ (Save). The overlay hides briefly, the window is recaptured, and the JSON is copied to the clipboard. Paste it — confirm `"mode": "window"` and `"context": { ..., "window": { "app_name": "...", "title": "...", "bounds": {...}, "window_id": ... } }`.
5. Verify the annotated PNG at `~/.glitch-spot/snapshots/<id>/annotated.png` is exactly the window-rect size and contains the rectangle in the right place.
6. Drag the selected window to a new position between click and save (with the overlay temporarily hidden — Cmd+H this Tauri overlay won't work because Esc cancels; verify with two displays so you can drag from another). The save should still produce the correct pixels.
7. Close the selected window between click and save (Cmd+Q the app). A notification fires: "Selected window closed — annotations discarded".
8. Press ⌃⌥⌘J on a multi-display Mac, move cursor across displays, confirm highlight follows.

- [ ] **Step 3: Commit the verification log (if you keep one)**

Optional: write a brief journal entry summarising the smoke. No git change required.

---

## Task 25: Smoke checklist doc

**Files:**
- Create: `docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md`

- [ ] **Step 1: Write the checklist**

Create `docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md`:

```markdown
# Window-mode smoke checklist

Captured from Task 24 of `docs/plans/2026-05-27-per-window-selection.md`.

## Single display

- [ ] ⌃⌥⌘J shows the chromatic crosshair cursor
- [ ] Hovering different windows updates the cyan highlight
- [ ] Esc dismisses the picker
- [ ] Click empty desktop dismisses the picker
- [ ] Click on a window enters annotation mode with dim veil + checker over the window
- [ ] Cannot draw rectangle outside the window
- [ ] Cmd+Enter saves; JSON in clipboard has `mode: "window"` + `context.window`
- [ ] Annotated PNG is window-sized and rectangle is in the correct position

## Multi display

- [ ] Highlight follows cursor across display boundaries
- [ ] Can pick a window on the non-primary display
- [ ] Annotation overlay positions correctly on the secondary display

## Error paths

- [ ] Close selected window mid-annotation → notification fires, no snapshot written
- [ ] Minimise selected window → same path
- [ ] Resize selected window before save → save still produces a coherent PNG; rect clamped if needed

## Permission / startup

- [ ] First launch shows the macOS Screen Recording prompt (if not already granted)
- [ ] After granting + relaunch, ⌃⌥⌘J works
```

- [ ] **Step 2: Commit**

```bash
git add docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md
git commit -m "docs(smoke): window-mode manual smoke checklist"
```

---

## Self-review checklist (run after Task 25)

Run this checklist with fresh eyes against the spec at `docs/seeds/2026-05-27-per-window-selection-seed.md`. Confirm each requirement landed:

- [ ] ⌃⌥⌘J hotkey registered and routes to picker mode (Tasks 11, 13)
- [ ] Picker pool of 8 windows declared (Task 12)
- [ ] SCShareableContent enumeration filtered to "standard app windows" (Tasks 4, 9)
- [ ] Hit-test pure + shared fixtures (Tasks 5, 6, 17)
- [ ] Coordinate scaling helper (Task 7)
- [ ] RunMode mutual exclusion (Task 8)
- [ ] `capture_window` via SCContentFilter (Task 10)
- [ ] `picker_select` shows overlay with window-mode payload (Task 13)
- [ ] Save path captures the live window, builds annotations with crop_origin (Tasks 2, 14)
- [ ] `context.window` populated on window-mode snapshots (Tasks 1, 14)
- [ ] Window-closed at save time → notification + clean reset (Task 15)
- [ ] Custom chromatic crosshair cursor (Task 16)
- [ ] Picker.tsx: hover, click, cancel, Esc (Tasks 19, 20)
- [ ] DrawingLayer clipRect prevents drawing outside window (Task 21)
- [ ] Overlay.tsx renders window-mode layout (Task 22)
- [ ] Frozen-frame race in window mode handled (Task 23)
- [ ] Manual smoke executed (Task 24) + checklist (Task 25)

---

## Once shipped: planted seed

Per project convention, after this plan is merged:

```bash
mkdir -p docs/seeds/planted/
git mv docs/seeds/2026-05-27-per-window-selection-seed.md docs/seeds/planted/
mkdir -p docs/plans/completed/
git mv docs/plans/2026-05-27-per-window-selection.md docs/plans/completed/
```

These moves are part of the merge commit, not a separate task.
