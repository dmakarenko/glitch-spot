# Menu Bar App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert Glitch Spot from a debug-window app into a production menu bar utility with no Dock icon, no main window, and a dynamic tray menu.

**Architecture:** A new `tray.rs` module owns the `TrayIconBuilder` setup and a `build_menu` function that is called on every tray-icon click to rebuild the menu fresh from current state (snapshot age, autostart toggle). `AppState` gains a `latest_snapshot_at` timestamp field set by `save_snapshot`. `lib.rs` wires autostart plugin + tray setup and sets `ActivationPolicy::Accessory`. The main window and all its frontend files are deleted.

**Tech Stack:** Tauri 2 (`tauri::tray`, `tauri::menu`), `tauri-plugin-autostart 2`, `chrono 0.4`

---

## File Map

| File | Action |
|---|---|
| `src-tauri/Cargo.toml` | Add `tauri-plugin-autostart = "2"` |
| `src-tauri/src/lib.rs` | Add `latest_snapshot_at` to `AppState`; set it in `save_snapshot`; add `mod tray`; register autostart plugin; call `tray::setup_tray` + `set_activation_policy` in setup closure |
| `src-tauri/src/tray.rs` | **New** — `format_snapshot_age`, `build_menu`, `setup_tray` |
| `src-tauri/tauri.conf.json` | Remove the `"main"` window entry |
| `src-tauri/capabilities/default.json` | Remove `"main"` from the `windows` list |
| `vite.config.ts` | Remove `main` rollup entry, keep `overlay` |
| `index.html` | **Deleted** |
| `src/App.tsx` | **Deleted** |
| `src/main.tsx` | **Deleted** |

---

## Task 1: Add `tauri-plugin-autostart` dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add the dependency**

Open `src-tauri/Cargo.toml`. Add this line to `[dependencies]` after the last `tauri-plugin-*` line:

```toml
tauri-plugin-autostart = "2"
```

The `[dependencies]` block should now include:

```toml
tauri-plugin-global-shortcut = "2"
tauri-plugin-clipboard-manager = "2"
tauri-plugin-os = "2"
tauri-plugin-autostart = "2"
```

- [ ] **Step 2: Verify it resolves**

```bash
cd src-tauri && cargo fetch
```

Expected: downloads `tauri-plugin-autostart` and its transitive deps, exits 0.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "Add tauri-plugin-autostart dependency"
```

---

## Task 2: Add `latest_snapshot_at` to `AppState`

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add this test module at the bottom of `src-tauri/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn app_state_latest_snapshot_at_starts_none() {
        let state = AppState {
            latest_frame: Mutex::new(None),
            snapshot_store: Arc::new(mcp::store::SnapshotStore::new()),
            latest_snapshot_at: Mutex::new(None),
        };
        assert!(state.latest_snapshot_at.lock().unwrap().is_none());
    }
}
```

- [ ] **Step 2: Run the test — confirm it fails**

```bash
cd src-tauri && cargo test app_state_latest_snapshot_at_starts_none 2>&1
```

Expected: compile error — field `latest_snapshot_at` does not exist on `AppState`.

- [ ] **Step 3: Add the field to `AppState`**

In `src-tauri/src/lib.rs`, update the `AppState` struct:

```rust
pub struct AppState {
    pub latest_frame: Mutex<Option<capture::CapturedFrame>>,
    pub snapshot_store: Arc<mcp::store::SnapshotStore>,
    pub latest_snapshot_at: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
}
```

Update the `AppState` initialisation in `run()` to include the new field:

```rust
.manage(AppState {
    latest_frame: Mutex::new(None),
    snapshot_store,
    latest_snapshot_at: Mutex::new(None),
})
```

- [ ] **Step 4: Set the timestamp in `save_snapshot`**

In `save_snapshot`, immediately after the line `state.snapshot_store.publish(snap.clone());`, add:

```rust
if let Ok(mut ts) = state.latest_snapshot_at.lock() {
    *ts = Some(chrono::Utc::now());
}
```

- [ ] **Step 5: Run the test — confirm it passes**

```bash
cd src-tauri && cargo test app_state_latest_snapshot_at_starts_none 2>&1
```

Expected: `test tests::app_state_latest_snapshot_at_starts_none ... ok`

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "Add latest_snapshot_at to AppState; set it on save"
```

---

## Task 3: Implement `format_snapshot_age` in `tray.rs`

**Files:**
- Create: `src-tauri/src/tray.rs`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/tray.rs` with the following content (implementation stubs + tests):

```rust
// ABOUTME: Menu bar tray icon setup and dynamic menu builder for Glitch Spot.
// ABOUTME: Rebuilds the menu on every tray click to reflect current snapshot age and autostart state.

use chrono::{DateTime, Duration, Utc};

pub(crate) fn format_snapshot_age(snapshot_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - snapshot_at).num_seconds().max(0);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs_ago: i64) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        (now - Duration::seconds(secs_ago), now)
    }

    #[test]
    fn format_just_now() {
        let (ts, now) = at(30);
        assert_eq!(format_snapshot_age(ts, now), "just now");
    }

    #[test]
    fn format_minutes() {
        let (ts, now) = at(90);
        assert_eq!(format_snapshot_age(ts, now), "1 min ago");
        let (ts, now) = at(180);
        assert_eq!(format_snapshot_age(ts, now), "3 min ago");
    }

    #[test]
    fn format_hours() {
        let (ts, now) = at(3600);
        assert_eq!(format_snapshot_age(ts, now), "1 hr ago");
        let (ts, now) = at(7200);
        assert_eq!(format_snapshot_age(ts, now), "2 hr ago");
    }

    #[test]
    fn format_days() {
        let (ts, now) = at(86400);
        assert_eq!(format_snapshot_age(ts, now), "1 day ago");
        let (ts, now) = at(172800);
        assert_eq!(format_snapshot_age(ts, now), "2 days ago");
    }
}
```

Add `mod tray;` to `src-tauri/src/lib.rs` (after the other `mod` declarations at the top):

```rust
mod tray;
```

- [ ] **Step 2: Run the tests — confirm they fail**

```bash
cd src-tauri && cargo test tray::tests 2>&1
```

Expected: compile error `todo!()` / tests fail with `panicked at 'not yet implemented'`.

- [ ] **Step 3: Implement `format_snapshot_age`**

Replace the `todo!()` body:

```rust
pub(crate) fn format_snapshot_age(snapshot_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - snapshot_at).num_seconds().max(0);
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{mins} min ago")
    } else if secs < 86400 {
        let hrs = secs / 3600;
        format!("{hrs} hr ago")
    } else {
        let days = secs / 86400;
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    }
}
```

- [ ] **Step 4: Run the tests — confirm they pass**

```bash
cd src-tauri && cargo test tray::tests 2>&1
```

Expected: all 4 tests pass (`format_just_now`, `format_minutes`, `format_hours`, `format_days`).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/tray.rs src-tauri/src/lib.rs
git commit -m "Add tray module with format_snapshot_age"
```

---

## Task 4: Implement `build_menu` and `setup_tray`

**Files:**
- Modify: `src-tauri/src/tray.rs`

- [ ] **Step 1: Add the full implementation**

Replace the entire contents of `src-tauri/src/tray.rs` with:

```rust
// ABOUTME: Menu bar tray icon setup and dynamic menu builder for Glitch Spot.
// ABOUTME: Rebuilds the menu on every tray click to reflect current snapshot age and autostart state.

use chrono::{DateTime, Duration, Utc};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;

pub(crate) fn format_snapshot_age(snapshot_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - snapshot_at).num_seconds().max(0);
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{mins} min ago")
    } else if secs < 86400 {
        let hrs = secs / 3600;
        format!("{hrs} hr ago")
    } else {
        let days = secs / 86400;
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    }
}

pub(crate) fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let state = app.state::<crate::AppState>();

    let snapshot_label = {
        let ts = state.latest_snapshot_at.lock().unwrap().clone();
        match ts {
            Some(t) => format!("Latest snapshot: {}", format_snapshot_age(t, Utc::now())),
            None => "Latest snapshot: none yet".to_string(),
        }
    };

    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);

    let capture_live = MenuItemBuilder::new("Capture Screen")
        .id("capture_live")
        .accelerator("Ctrl+Alt+Super+G")
        .build(app)?;

    let capture_frozen = MenuItemBuilder::new("Capture + Freeze")
        .id("capture_frozen")
        .accelerator("Ctrl+Alt+Super+H")
        .build(app)?;

    let mcp_status = MenuItemBuilder::new("MCP Server  ● :7820")
        .id("mcp_status")
        .enabled(false)
        .build(app)?;

    let latest_snapshot = MenuItemBuilder::new(&snapshot_label)
        .id("latest_snapshot")
        .build(app)?;

    let launch_at_login = CheckMenuItemBuilder::new("Launch at Login")
        .id("launch_at_login")
        .checked(autostart_on)
        .build(app)?;

    let quit = MenuItemBuilder::new("Quit Glitch Spot")
        .id("quit")
        .build(app)?;

    MenuBuilder::new(app)
        .item(&capture_live)
        .item(&capture_frozen)
        .item(&PredefinedMenuItem::separator(app, None)?)
        .item(&mcp_status)
        .item(&PredefinedMenuItem::separator(app, None)?)
        .item(&latest_snapshot)
        .item(&PredefinedMenuItem::separator(app, None)?)
        .item(&launch_at_login)
        .item(&PredefinedMenuItem::separator(app, None)?)
        .item(&quit)
        .build()
}

pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    use tauri::image::Image;
    let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
    let menu = build_menu(app)?;

    TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .on_menu_event(|app, event| {
            let app = app.clone();
            match event.id.as_ref() {
                "capture_live" => {
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = crate::trigger_overlay_live(&app).await {
                            eprintln!("tray: capture live failed: {e}");
                        }
                    });
                }
                "capture_frozen" => {
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = crate::trigger_overlay_frozen(&app).await {
                            eprintln!("tray: capture frozen failed: {e}");
                        }
                    });
                }
                "latest_snapshot" => {
                    let state = app.state::<crate::AppState>();
                    if let Some(snap) = state.snapshot_store.get_latest() {
                        if let Ok(json) = serde_json::to_string_pretty(&snap) {
                            let _ = app.clipboard().write_text(json);
                        }
                    }
                }
                "launch_at_login" => {
                    let manager = app.autolaunch();
                    let is_on = manager.is_enabled().unwrap_or(false);
                    if is_on {
                        let _ = manager.disable();
                    } else {
                        let _ = manager.enable();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Ok(menu) = build_menu(tray.app_handle()) {
                    let _ = tray.set_menu(Some(menu));
                }
            }
        })
        .build(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs_ago: i64) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        (now - Duration::seconds(secs_ago), now)
    }

    #[test]
    fn format_just_now() {
        let (ts, now) = at(30);
        assert_eq!(format_snapshot_age(ts, now), "just now");
    }

    #[test]
    fn format_minutes() {
        let (ts, now) = at(90);
        assert_eq!(format_snapshot_age(ts, now), "1 min ago");
        let (ts, now) = at(180);
        assert_eq!(format_snapshot_age(ts, now), "3 min ago");
    }

    #[test]
    fn format_hours() {
        let (ts, now) = at(3600);
        assert_eq!(format_snapshot_age(ts, now), "1 hr ago");
        let (ts, now) = at(7200);
        assert_eq!(format_snapshot_age(ts, now), "2 hr ago");
    }

    #[test]
    fn format_days() {
        let (ts, now) = at(86400);
        assert_eq!(format_snapshot_age(ts, now), "1 day ago");
        let (ts, now) = at(172800);
        assert_eq!(format_snapshot_age(ts, now), "2 days ago");
    }
}
```

- [ ] **Step 2: Verify it compiles and tests pass**

```bash
cd src-tauri && cargo test tray::tests 2>&1
```

Expected: 4 tests pass. If there are compile errors on the Tauri menu/tray API (method names differ between Tauri 2 minor releases), check `cargo doc --open` for the exact signatures on `MenuItemBuilder`, `TrayIconBuilder`, and `PredefinedMenuItem`. The logic stays the same; only method names may need adjustment.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "Implement build_menu and setup_tray in tray module"
```

---

## Task 5: Wire tray and autostart into `lib.rs`

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register the autostart plugin**

In `run()`, in the `tauri::Builder::default()` chain, add the autostart plugin after the existing plugins:

```rust
.plugin(tauri_plugin_autostart::init(
    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
    None::<Vec<&str>>,
))
```

The full plugin chain should be:

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_clipboard_manager::init())
    .plugin(tauri_plugin_os::init())
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        None::<Vec<&str>>,
    ))
```

- [ ] **Step 2: Update the setup closure**

Replace the existing `.setup(|app| { ... })` closure with:

```rust
.setup(|app| {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    // Auto-enable login item on first launch (if not already registered).
    {
        use tauri_plugin_autostart::ManagerExt;
        let manager = app.autolaunch();
        if !manager.is_enabled().unwrap_or(false) {
            let _ = manager.enable();
        }
    }

    hotkey::register(app.handle())?;
    let _tray = tray::setup_tray(app.handle())?;
    Ok(())
})
```

- [ ] **Step 3: Verify it compiles**

```bash
cd src-tauri && cargo check 2>&1
```

Expected: no errors. If `tauri_plugin_autostart::MacosLauncher` is not found, try `tauri_plugin_autostart::MacosLauncher` or check the crate's re-exported types with `cargo doc -p tauri-plugin-autostart --open`.

- [ ] **Step 4: Run all tests**

```bash
cd src-tauri && cargo test 2>&1
```

Expected: all existing tests + new `tray::tests` + new `tests::app_state_latest_snapshot_at_starts_none` pass. Zero failures.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "Wire tray icon, autostart plugin, and Accessory policy into lib.rs"
```

---

## Task 6: Remove main window from config

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: Remove the main window from `tauri.conf.json`**

In `src-tauri/tauri.conf.json`, the `app.windows` array currently has two entries. Remove the first one (the main window). The result should be:

```json
{
  "$schema": "../node_modules/@tauri-apps/cli/config.schema.json",
  "productName": "Glitch Spot",
  "version": "0.1.0",
  "identifier": "com.glitchspot.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "macOSPrivateApi": true,
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
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "android": {
      "debugApplicationIdSuffix": ".debug"
    }
  }
}
```

- [ ] **Step 2: Remove `"main"` from capabilities**

In `src-tauri/capabilities/default.json`, update `windows` to only contain `"overlay"`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "enables the default permissions",
  "windows": [
    "overlay"
  ],
  "permissions": [
    "core:default",
    "global-shortcut:default",
    "clipboard-manager:allow-write-text",
    "os:default"
  ]
}
```

- [ ] **Step 3: Verify backend still compiles**

```bash
cd src-tauri && cargo check 2>&1
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/capabilities/default.json
git commit -m "Remove main window from Tauri config and capabilities"
```

---

## Task 7: Remove main window frontend files

**Files:**
- Delete: `index.html`
- Delete: `src/App.tsx`
- Delete: `src/main.tsx`
- Modify: `vite.config.ts`

- [ ] **Step 1: Delete the main window files**

```bash
git rm index.html src/App.tsx src/main.tsx
```

Expected: three files removed from the repo.

- [ ] **Step 2: Update `vite.config.ts`**

Replace the file contents with:

```typescript
// ABOUTME: Vite build configuration for Glitch Spot's single-page overlay frontend.
// ABOUTME: Only the overlay HTML entry point is declared; the main window has been removed.
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
      },
    },
  },
});
```

- [ ] **Step 3: Verify the frontend builds**

```bash
npm run build 2>&1
```

Expected: build succeeds, `dist/` contains `overlay.html` and its assets. No errors about missing `index.html`.

- [ ] **Step 4: Commit**

```bash
git add vite.config.ts
git commit -m "Remove main window frontend files; keep overlay entry only"
```

---

## Task 8: Full build and smoke test

- [ ] **Step 1: Run all Rust tests one final time**

```bash
cd src-tauri && cargo test 2>&1
```

Expected: all tests pass, zero failures.

- [ ] **Step 2: Build the full Tauri app in dev mode**

From the repo root:

```bash
npm run tauri dev 2>&1
```

Expected:
- No Dock icon appears.
- A Glitch Spot icon appears in the menu bar.
- No main window opens.
- Clicking the menu bar icon shows the menu with: Capture Screen, Capture + Freeze, MCP Server ● :7820, Latest snapshot: none yet, Launch at Login (checked), Quit Glitch Spot.
- Clicking "Capture Screen" or pressing ⌃⌥⌘G opens the overlay.
- Clicking "Capture + Freeze" or pressing ⌃⌥⌘H captures a screenshot and opens the overlay.
- After saving an annotation, reopening the menu shows "Latest snapshot: just now" (or N min ago).
- Clicking "Latest snapshot" copies snapshot JSON to clipboard.
- Clicking "Launch at Login" toggles the checkmark.
- Clicking "Quit Glitch Spot" exits cleanly.

- [ ] **Step 3: Commit**

Everything is already committed. No additional commit needed unless smoke test surfaces a bug — fix it in a new commit with a descriptive message.
