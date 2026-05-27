# Per-window selection mode — design seed

**Date:** 2026-05-27
**Branch:** `feature-overlay-window-selection`
**Status:** design approved, plan pending

A new global hotkey ⌃⌥⌘J enters a window-picker mode that mirrors macOS's `cmd+shift+4` → space behaviour. The user hovers windows across any display, sees a cyan-tinted highlight follow the cursor, clicks to select, then annotates over the live window. Save captures only the selected window's pixels.

This complements the existing live (⌃⌥⌘G) and frozen (⌃⌥⌘H) modes — they capture the whole display; window mode captures one app window.

---

## Goals

- **Native parity:** cursor crosses display boundaries; the highlight follows; clicks anywhere outside a window cancel — matches cmd+shift+4 muscle memory.
- **Live annotation against a moving target:** annotations are placed against the live window; capture happens at save time, by `CGWindowID`, so we get current pixels even if the window's content updated since selection.
- **Scoped output:** the saved PNG is exactly the window's pixel rect — no surrounding desktop. Annotations are in window-local coords.
- **Single-mode discipline:** picker/live/frozen are mutually exclusive. Re-pressing any hotkey while a mode is active is a no-op.

## Non-goals (v1)

- Selecting widgets/panels/popovers — only "standard" top-level app windows.
- Following window movement while the picker is open (we snapshot the window list on session start; user re-presses ⌃⌥⌘J if their target window appeared after).
- Annotating across multiple windows in one session.
- Capturing the desktop wallpaper as a target.
- Windows or Linux. macOS only (matches v1).

---

## User-facing flow

```
Idle ──⌃⌥⌘J──▶ Picking
                  │ cursor moves between displays + windows
                  │ topmost window under cursor gets cyan-checkered highlight
                  │
        ┌─────────┴─────────┐
        │                   │
   click on a window    Esc / click empty desktop
        │                   │
        ▼                   ▼
   Annotating          Idle (picker windows hide)
        │
   ┌────┴────────┐
   │             │
 Save ⌘↩    Esc with rects → DiscardModal
   │
   ▼
 Snapshot persisted, overlay hides → Idle
```

Press ⌃⌥⌘J while in Picking or Annotating → ignored. Same for ⌃⌥⌘G/H.

---

## Architecture

### Windows in play

| Window label | Purpose | Lifetime |
|---|---|---|
| `picker-0` … `picker-7` | Per-display picker overlays. Pre-declared in `tauri.conf.json`. Transparent, decorations off, alwaysOnTop, click-active. | App lifetime; shown/hidden on hotkey. |
| `overlay` (existing) | Annotation overlay. Already used for live/frozen; extended for window mode. | Unchanged. |

Pool of 8 covers nearly any realistic Mac setup. If the user has more than 8 displays we use the first 8 (sorted by `available_monitors()` order) and warn-log.

### State

A single `RunMode` enum in `AppState` enforces mutual exclusion:

```rust
enum RunMode { Idle, Live, Frozen, Picking(PickerSession), Annotating(AnnotatingSession) }
```

`PickerSession` holds the window list, the display→picker-label mapping, and the currently hovered window id. `AnnotatingSession` holds the selected `window_id`, the source `PickableWindow` snapshot at click time, and the target display.

---

## Backend (Rust)

### New module: `src-tauri/src/window_picker/`

```rust
pub struct PickableWindow {
    pub window_id: u32,           // CGWindowID; used at save time
    pub app_name: String,
    pub title: String,
    pub bounds: WindowBounds,     // physical-pixel rect in *global* screen coords
    pub display_index: usize,
    pub layer: i32,
}

pub struct WindowBounds { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }
```

Platform-agnostic surface in `mod.rs`; macOS impl in `macos.rs`.

### Window enumeration

Source: `screencapturekit::shareable_content::SCShareableContent::get().windows()`. Already in our dependency tree.

Filter for "standard app windows":
- `is_on_screen == true`
- `window_layer == 0` (drops Dock, menu bar, status items, floating panels)
- `owning_application.bundle_identifier != "com.glitchspot.app"`
- `owning_application.application_name` not in `{"Dock", "WindowServer", "SystemUIServer", "Control Center"}`
- width ≥ 80pt **and** height ≥ 80pt

Z-order: SCShareableContent returns windows in **front-to-back** order. Preserve that — hit-test iterates front-to-back, returns the first containing window.

### Coordinate normalisation

- `SCWindow.frame()` → points (logical), origin at global display arrangement top-left.
- Tauri `available_monitors()` and `cursor_position()` → physical pixels.
- Store `PickableWindow.bounds` in **physical pixels** by multiplying SCWindow point coords by the owning monitor's `scale_factor`. All hit-testing happens in one unit.

### Hit testing

Pure function (testable without macOS):

```rust
pub fn hit_test(windows: &[PickableWindow], cursor_x: i32, cursor_y: i32) -> Option<u32> {
    windows.iter()
        .find(|w| {
            cursor_x >= w.bounds.x
            && cursor_x < w.bounds.x + w.bounds.w as i32
            && cursor_y >= w.bounds.y
            && cursor_y < w.bounds.y + w.bounds.h as i32
        })
        .map(|w| w.window_id)
}
```

A pure JS twin (`src/picker/hitTest.ts`) runs client-side so cursor highlight updates without IPC round-trip. Both share fixture cases in `tests/fixtures/hit-test-cases.json`.

### Capture by window id

New fn in `src-tauri/src/capture/macos.rs`:

```rust
pub(super) fn capture_window(
    app: &tauri::AppHandle,
    window_id: u32,
) -> Result<CapturedFrame, CaptureError>
```

Implementation:
1. `SCShareableContent::get().windows()` — find by `window_id`.
2. `SCContentFilter::with_window(window).build()` — single-window filter; captures the window's pixels only.
3. `SCStreamConfiguration::new().with_width(...).with_height(...)` at the window's physical-pixel size.
4. `SCScreenshotManager::capture_image(filter, config)`.
5. PNG-encode like the existing `capture_active_monitor`.

Returns `CapturedFrame { width, height, scale_factor, png, display_index }` where `display_index` is the window's owning display from the picker session.

### Tauri commands

```rust
#[tauri::command] async fn picker_start(app: AppHandle) -> Result<PickerSession, String>;
#[tauri::command] async fn picker_select(app: AppHandle, window_id: u32) -> Result<(), String>;
#[tauri::command] async fn picker_cancel(app: AppHandle) -> Result<(), String>;
```

`picker_start`:
1. Reject if `RunMode != Idle`.
2. Enumerate displays and windows; build `PickerSession`.
3. For each display, resize picker-N to the display's full physical rect, show, focus.
4. Emit `picker://session-start` to each picker with its `my_display_index`.
5. Transition `RunMode` → `Picking(session)`.

`picker_select`:
1. Look up `PickableWindow` by `window_id`.
2. Hide picker-0…picker-7.
3. Resize `overlay` to fully cover the selected window's display.
4. Emit `glitch-spot://window-mode-start` with payload (see § Frontend).
5. `window.eval("__glitchSpotPresent('window')")` to reset React state pre-paint.
6. `overlay.show()` + `set_focus()`.
7. Transition `RunMode` → `Annotating(session)`.

`picker_cancel`:
1. Hide all pickers, transition `RunMode` → `Idle`.

### Hotkey registration

Extend `src-tauri/src/hotkey.rs`:

```rust
pub fn window_picker_hotkey() -> Shortcut {
    Shortcut::new(Some(TRIGGER_MODS), Code::KeyJ)
}
```

Register alongside live/frozen in `register()`. On press, spawn `picker_start` on the async runtime.

---

## Frontend

### New picker UI

- `picker.html` (project root, alongside `overlay.html`) — entry HTML
- `src/picker/entry.tsx` — mounts the React tree (mirrors `src/overlay/entry.tsx`)
- `src/picker/Picker.tsx` — root component
- `src/picker/hitTest.ts` — pure JS hit-test (shares fixtures with Rust)
- `src/picker/cursor.ts` — inlined SVG data URL for the cursor

Vite config: add `picker: resolve(__dirname, "picker.html")` to `rollupOptions.input` alongside the existing `overlay` entry.

### Picker window config (tauri.conf.json)

```jsonc
{
  "label": "picker-0",
  "url": "picker.html",
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
```

Repeat for `picker-1` … `picker-7`.

### Picker UI behaviour

- **Background:** fully transparent. No veil during picking — the user must see windows beneath.
- **Cursor:** inline-SVG data URL via `cursor: url('data:image/svg+xml;utf8,...') 16 16, crosshair`. The SVG is the geometric chromatic crosshair: dark circle (#0c0c0c), 1.5px #e0e0e0 stroke, two offset rgba(255,0,60,0.85) and rgba(0,245,255,0.85) chromatic rings, four 2px #ee6633 ticks at NSEW outside the circle. Drawn in 48×48 viewBox, rendered at 32×32 px, hotspot at the centre.
- **Highlight:** absolutely-positioned div over the hovered window's rect (display-local coords), with a cyan-tinted variant of the existing checker pattern (cyan #00f5ff tiles at ~14% alpha over a 12% white background). Instant snap on rect change.
- **`mousemove` handler:** convert client → display-global, hit-test client-side, set `hoveredWindowId` locally, then emit `picker://hovered` so every picker in the session updates its highlight. This matters for windows that straddle two displays — the user's cursor is on one display but the highlight must render on both pickers' display-local portions of the window rect.
- **`mouseleave` on this picker:** emit `picker://hovered` with `null` *only if* this picker still believes it's the source of the current highlight (debounced by the round-trip event). Avoids flicker when cursor crosses display boundaries.
- **`click` on a window:** `invoke('picker_select', { window_id })`.
- **`click` outside any window (empty desktop) or Esc:** `invoke('picker_cancel')`.

### Picker IPC

| Event | Direction | Payload |
|---|---|---|
| `picker://session-start` | Rust → each picker | `{ windows: PickableWindow[], displays: DisplayBounds[], my_display_index: number }` |
| `picker://hovered` | Picker → Rust → broadcast to all pickers | `{ window_id: number \| null }` |
| `picker://end` | Rust → all pickers | `{}` |

### Annotation extension (`Overlay.tsx`)

Mode union widens: `'live' | 'frozen' | 'window'`.

Window-mode layout:

```
┌── overlay window covers full display ────────┐
│ dim veil rgba(0,0,0,0.40)                    │
│                                               │
│ ░░░░ ┌─ window_rect ────────────┐ ░░░░░░░░░░ │
│ ░░░░ │ 12% gray-white checker   │ ░░░░░░░░░░ │
│ ░░░░ │ (existing CHECKER_BG)    │ ░░░░░░░░░░ │
│ ░░░░ │ + annotation layer       │ ░░░░░░░░░░ │
│ ░░░░ └──────────────────────────┘ ░░░░░░░░░░ │
│                                               │
│       ┌─[Toolbar pill]─┐                      │
└───────────────────────────────────────────────┘
```

- **Dim veil:** `<div style={{position:'fixed', inset:0, background:'rgba(0,0,0,0.40)'}}/>`. Click on the veil = same path as `picker_cancel` (with the existing DiscardModal flow if rects exist).
- **Window cutout:** the existing `CHECKER_BG` renders **only inside** `window_rect_display_local`.
- **DrawingLayer:** accept optional `clipRect` prop. Mouse-down outside `clipRect` → no-op. New rect creation clamped to `clipRect`. Existing rect drag/resize clamped at `clipRect` boundary.
- **Toolbar:** unchanged. Top-centre of display; backdrop blur keeps it legible even over a top-of-display window rect.

### Saving

`save_snapshot('window', annotations)` flow:

1. Hide overlay first (existing live-mode pattern — 120ms sleep, then recapture).
2. `capture::capture_window(&app, session.window_id)`.
3. `snapshot::build` with a new optional `crop_origin: (i32, i32)` = `window_rect_display_local.{x,y}`. Each annotation's `(x*scale, y*scale)` becomes `(x*scale - crop_x, y*scale - crop_y)` — already in window-local physical pixels.
4. Persist + publish + clipboard, as today.

### Snapshot JSON

```jsonc
{
  "id": "01HXY...",
  "created_at": "...",
  "mode": "window",
  "annotations": [ ... ],   // x,y,w,h in window-local physical px
  "context": {
    "os": { ... },
    "app": { ... },
    "window": {
      "app_name": "Safari",
      "title": "Reader — example.com",
      "bounds": { "x": 120, "y": 88, "w": 1440, "h": 900 },
      "window_id": 12345
    }
  },
  "images": { ... }
}
```

`context.window` is absent (key omitted) for live/frozen snapshots. Old consumers reading the mode field as a string remain compatible.

---

## Error handling & edge cases

| Scenario | Behaviour |
|---|---|
| No displays detected | `picker_start` returns `Err`; hotkey logs and no-ops. |
| > 8 displays | Use first 8 by `available_monitors()` order; warn-log. |
| Screen Recording permission revoked | `SCShareableContent::get()` errors; bubble up like existing live-capture failure. |
| Cursor over no window | `hoveredWindowId == null`. No highlight. Click → `picker_cancel`. |
| Selected window closes before save | `SCShareableContent::get()` no longer contains `window_id`; `capture_window` returns `Err`. Save handler dismisses the overlay and fires a Tauri notification (`tauri-plugin-notification`, already in deps and used for tunnel errors) titled `"Glitch Spot"` with body `"Selected window closed — annotations discarded"`. |
| Selected window moved between click and save | OK by design: `SCContentFilter::with_window` captures wherever it is. Pixels match annotations because annotations are window-local. |
| Selected window resized between click and save | Capture is the new size. Annotation rects may overflow. At save time, clamp rect coords to capture `(width, height)`. If clamped area < 4×4 px, drop the rect with a log. |
| Selected window minimised before save | Same path as closed. |
| Hotkey re-pressed during Picking or Annotating | Ignored (`RunMode != Idle` guard). |
| Display unplugged mid-session | If selected window was on that display, save fails (window no longer available); user re-presses ⌃⌥⌘J. |
| Window straddles two displays | Hit-tests on either display. `SCContentFilter::with_window` captures the full window regardless. Overlay positions on the display with the larger overlap (tie-break: lower `display_index`). |
| `__glitchSpotPresent` not installed before `show()` | Same race as existing overlay: payload also emitted via `glitch-spot://window-mode-start` after show, React effect listens. |

---

## Testing

### Rust unit tests

- `window_picker::filter_standard_windows` — fake `Vec<MockWindow>`, asserts filtering. Pure; no SC dependency.
- `window_picker::hit_test` — fixtures with overlap, occlusion, miss cases. Same fixtures as JS.
- `window_picker::coords::scale_point_to_physical` — multi-display scale factor conversion.
- `snapshot::build` — new `mode: "window"` case with `crop_origin`; verifies annotation coords are window-local.
- `picker_session` — state-machine transitions; invalid-transition rejections.
- `capture::capture_window` — smoke-test on macOS that the function compiles and returns the expected `CapturedFrame` shape; full integration verified manually.

### Frontend unit tests (vitest)

- `picker/hitTest.test.ts` — parity with Rust fixtures.
- `picker/Picker.test.tsx` — mousemove triggers highlight; click invokes `picker_select`; Esc invokes `picker_cancel`.
- `overlay/DrawingLayer.test.tsx` — extend with `clipRect`: mouse-down outside ignored, drag clamps at boundary.
- `overlay/Overlay.test.tsx` — `mode: 'window'` renders dim veil + checker cutout + toolbar.

### Manual smoke checklist

- ⌃⌥⌘J on a single-display Mac, hover three different windows, click one, annotate, save → JSON contains `mode: "window"`, `context.window.{app_name,title}`, annotations in window-local coords, PNG is window-sized.
- Same on a dual-display setup; pick a window on the secondary display.
- Cursor crosses display boundary mid-hover; highlight transfers correctly.
- Esc during picker cancels.
- Click empty desktop during picker cancels.
- Close the selected window between click and save → toolbar shows error, no snapshot written.
- Drag the selected window's content/state between click and save (e.g. start a video, type into a textbox) → save captures the current pixels.

---

## File touch list

### Rust
- `src-tauri/src/window_picker/mod.rs` (new)
- `src-tauri/src/window_picker/macos.rs` (new)
- `src-tauri/src/picker_session.rs` (new)
- `src-tauri/src/capture/macos.rs` (extend with `capture_window`)
- `src-tauri/src/overlay.rs` (add `show_window_mode`)
- `src-tauri/src/hotkey.rs` (register ⌃⌥⌘J)
- `src-tauri/src/lib.rs` (register commands, wire `setup`, extend `save_snapshot` mode arm, add `RunMode`)
- `src-tauri/src/snapshot/build.rs` (`crop_origin` param)
- `src-tauri/tauri.conf.json` (declare `picker-0…picker-7`)

### Frontend
- `picker.html` (new, at project root)
- `src/picker/entry.tsx` (new)
- `src/picker/Picker.tsx` (new)
- `src/picker/hitTest.ts` (new)
- `src/picker/cursor.ts` (new)
- `src/overlay/Overlay.tsx` (extend `Mode`, add window-mode render branch)
- `src/overlay/annotations/DrawingLayer.tsx` (accept `clipRect`, clamp creation/drag)
- `src/overlay/bridge.ts` (extend `saveSnapshot` to forward window context)
- `vite.config.ts` (add `picker.html` as a Rollup input)
- `tests/fixtures/hit-test-cases.json` (new; shared Rust/JS fixtures)

### Docs
- `docs/seeds/2026-05-27-per-window-selection-seed.md` (this file)
- A plan in `docs/plans/2026-05-27-per-window-selection.md` will follow.

---

## Open questions

None at design time — all material decisions are settled. Implementation may surface edge cases (e.g. exact cursor SVG dimensions for crisp rendering on different DPIs) that will be handled inline.
