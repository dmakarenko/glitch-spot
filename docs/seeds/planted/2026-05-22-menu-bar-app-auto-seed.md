# Glitch Spot: Menu Bar App — Design

Brainstorming output, 2026-05-22. Captures all decisions made before writing the implementation plan.

---

## Goal

Convert Glitch Spot from a two-window app (main debug window + overlay) into a production-ready menu bar utility. No Dock icon, no main window, no app switcher presence. The app lives entirely in the macOS menu bar.

---

## Architecture

### Activation policy

Call `app.set_activation_policy(ActivationPolicy::Accessory)` in the Tauri builder's `setup` closure (in `lib.rs`). This removes the app from the Dock and ⌘-Tab switcher. The app is invisible except for its menu bar icon.

### Windows

- **Main window**: deleted entirely. `index.html`, `src/main.tsx`, and `src/App.tsx` are removed.
- **Overlay window**: unchanged. Still triggered by hotkeys and menu actions.

### Tray icon

Built with Tauri 2's `TrayIconBuilder`. Icon source: `icons/32x32.png` with `icon_as_template(true)` so macOS renders it correctly in both light and dark menu bars.

The tray registers an `on_tray_icon_event` handler. On each `LeftClick` event the menu is rebuilt from current state (snapshot age, MCP status, login item state) and installed with `tray.set_menu(Some(new_menu))` before the OS renders the dropdown. No polling, no timers.

### Menu structure

```
Capture Screen          ⌃⌥⌘G
Capture + Freeze        ⌃⌥⌘H
─────────────────────────────
MCP Server              ● :7820     (non-clickable status item)
─────────────────────────────
Latest snapshot         3 min ago   (clickable → copy JSON to clipboard)
─────────────────────────────
✓ Launch at Login                   (toggle; enabled by default on first launch)
─────────────────────────────
Quit Glitch Spot
```

**Item behaviours:**

| Item | Behaviour |
|---|---|
| Capture Screen | Triggers live overlay — identical to ⌃⌥⌘G hotkey |
| Capture + Freeze | Screenshot + overlay — identical to ⌃⌥⌘H hotkey |
| MCP Server | Non-clickable. Shows `● :7820` (green) when server is running. |
| Latest snapshot | Rebuilt on each menu open; shows relative time ("3 min ago", "None yet"). Click copies the latest snapshot JSON to clipboard. |
| Launch at Login | Checkmark reflects current state. Click toggles. Enabled by default on first launch. |
| Quit Glitch Spot | Stops MCP server gracefully, exits. |

### Login item

Uses `tauri-plugin-autostart`. On each startup, if `autostart_manager.is_enabled()` returns `false`, `enable()` is called automatically — so the login item is active by default. Menu toggle calls `enable()` / `disable()` and the checkmark reflects `is_enabled()` at the time the menu is built.

---

## State changes

`AppState` gains one new field:

```rust
pub latest_snapshot_at: Mutex<Option<DateTime<Utc>>>,
```

This is set in the existing `save_snapshot` path alongside the MCP store publish. The tray menu builder reads it to compute relative time without touching the snapshot store.

---

## New module

`src-tauri/src/tray.rs` owns:
- `setup_tray(app: &AppHandle) -> Result<TrayIcon>` — creates the icon, registers the click handler
- `build_menu(app: &AppHandle) -> Menu` — called on each click; reads `AppState` (snapshot timestamp) and autostart state to build a fresh menu. MCP status is always shown as `● :7820` — the server is started unconditionally at launch and a startup failure would have panicked before the tray is created.

`lib.rs` calls `tray::setup_tray(&app)` in the builder's `setup` closure, after hotkeys and MCP server are started.

---

## Files changed / removed / added

| File | Change |
|---|---|
| `src-tauri/tauri.conf.json` | Remove `"main"` window entry |
| `src-tauri/src/lib.rs` | Add autostart plugin; call `tray::setup_tray`; add `latest_snapshot_at` to `AppState` |
| `src-tauri/src/tray.rs` | **New** — tray setup and menu builder |
| `src-tauri/Cargo.toml` | Add `tauri-plugin-autostart` |
| `src-tauri/capabilities/default.json` | Add tray + autostart permissions |
| `src/App.tsx` | **Deleted** |
| `src/main.tsx` | **Deleted** |
| `index.html` | **Deleted** |
| `vite.config.ts` | Remove main entry point, keep overlay entry |

---

## What is unchanged

- `src/overlay/` — all overlay frontend code untouched
- `overlay.html` — untouched
- `src-tauri/src/hotkey.rs` — untouched
- `src-tauri/src/mcp/` — untouched
- `src-tauri/src/snapshot/` — untouched
- `src-tauri/src/capture/` — untouched

---

## Out of scope

- Preferences window (YAGNI — add if needed later)
- Icon state changes during capture (YAGNI)
- MCP server port configuration (hardcoded `:7820` for now)
