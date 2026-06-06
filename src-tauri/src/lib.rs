// ABOUTME: Entry point for the Glitch Spot Tauri 2 application library.
// ABOUTME: Builds and runs the Tauri app with registered Tauri commands.
mod capture;
mod hotkey;
pub mod mcp;
mod overlay;
mod picker_session;
mod window_picker;
pub mod snapshot;
mod spike_capture;
mod tray;
pub mod settings;
pub mod share;

pub use capture::CapturedFrame;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use snapshot::build::IncomingAnnotation;

/// Process-wide cache of the most recently captured frame.
///
/// Currently only populated by the frozen-overlay path; the live-overlay path
/// doesn't capture at trigger time. When the save/persist slice lands, both
/// paths will capture (live mode at save time, frozen mode at trigger time)
/// and this slot becomes the staging area for the snapshot artifact.
pub struct AppState {
    pub latest_frame: Mutex<Option<capture::CapturedFrame>>,
    pub snapshot_store: Arc<mcp::store::SnapshotStore>,
    pub latest_snapshot_at: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub tunnel: Arc<share::tunnel::TunnelManager>,
    pub settings: Arc<settings::Settings>,
    pub config_dir: std::path::PathBuf,
    /// Set to true by `mcp::server::serve` once the listener is bound, cleared
    /// if/when serve returns. Tray reads this to decide healthy vs unhealthy —
    /// avoids the TCP-connect false positive when another process happens to
    /// own the configured port.
    pub mcp_bound: Arc<AtomicBool>,
    pub picker_state: Arc<picker_session::PickerStateHolder>,
    /// Bumped whenever a frozen overlay session starts or is dismissed. A frozen
    /// capture records the value at trigger time and only publishes its frame if
    /// it's still current — so a capture left in flight by a cancel (Escape) or a
    /// superseding session can't replace a newer overlay with a stale screenshot.
    pub freeze_generation: Arc<AtomicU64>,
    /// Bumped whenever a live overlay session starts. The fallback reveal task
    /// captures this value and skips the reveal if it has since changed, so a
    /// cancel-and-reopen within 300ms doesn't reveal the new alpha-0 session early.
    pub live_generation: Arc<AtomicU64>,
}

#[tauri::command]
async fn capture_active_monitor(app: tauri::AppHandle) -> Result<capture::CapturedFrame, String> {
    capture::capture_active_monitor(&app).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_overlay_live(app: tauri::AppHandle) -> Result<(), String> {
    trigger_overlay_live(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_overlay_frozen(app: tauri::AppHandle) -> Result<(), String> {
    trigger_overlay_frozen(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.picker_state.reset_to_idle();
    // Invalidate any in-flight frozen capture or live fallback reveal so stale
    // tasks from the dismissed session don't affect the next presentation.
    state.freeze_generation.fetch_add(1, Ordering::SeqCst);
    state.live_generation.fetch_add(1, Ordering::SeqCst);
    overlay::hide(&app).map_err(|e| e.to_string())
}

/// Post a synthetic mouse-moved event at the current cursor location so macOS
/// re-evaluates the displayed cursor. The overlay's chromatic crosshair is a
/// CSS cursor, and macOS only redraws the cursor on a mouse event — so after
/// the overlay is shown under a stationary pointer the cursor stays an arrow
/// until the user moves the mouse. The event carries the cursor's existing
/// location, producing no visible movement.
#[cfg(target_os = "macos")]
#[tauri::command]
fn refresh_cursor() {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return;
    };
    // Read the current location in CoreGraphics' global display space by
    // creating a null event — this avoids the physical-vs-logical pixel
    // mismatch that would otherwise make the cursor jump on Retina displays.
    let Ok(here) = CGEvent::new(source.clone()) else {
        return;
    };
    let location = here.location();
    if let Ok(moved) =
        CGEvent::new_mouse_event(source, CGEventType::MouseMoved, location, CGMouseButton::Left)
    {
        moved.post(CGEventTapLocation::HID);
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn refresh_cursor() {}

/// Fade the calling window in to full opacity. The overlay and the picker pool
/// are shown at alpha 0 so each repaints the target display's layout before
/// becoming visible (see `overlay::show_window_mode` and `show_picker`); the
/// frontend invokes this one rAF after its frame paints. The `window` parameter
/// is the calling webview, so a picker reveals itself and the overlay reveals
/// itself through the same command. Idempotent.
#[tauri::command]
fn reveal_window(window: tauri::WebviewWindow) {
    overlay::set_window_alpha(&window, 1.0);
}

#[tauri::command]
fn get_latest_frame(state: tauri::State<'_, AppState>) -> Option<capture::CapturedFrame> {
    state.latest_frame.lock().ok().and_then(|g| g.clone())
}

/// Resolve the frame to save for window mode. Window mode is frozen: the frame
/// captured at pick time (cached in `latest_frame`) is what the user annotated
/// on, so reuse it rather than recapturing — correct (no window drift) and fast.
/// Returns the frame (with `display_index` set to the window's display) and the
/// crop origin (window offset within its display, in physical pixels).
fn resolve_window_save_frame(
    session: &picker_session::AnnotatingSession,
    latest_frame: Option<capture::CapturedFrame>,
) -> Result<(capture::CapturedFrame, (f32, f32)), String> {
    let mut frame = latest_frame.ok_or_else(|| "no cached frame for window mode".to_string())?;
    frame.display_index = session.window.display_index;
    let crop_x = (session.window.bounds.x - session.display.x) as f32;
    let crop_y = (session.window.bounds.y - session.display.y) as f32;
    Ok((frame, (crop_x, crop_y)))
}

#[tauri::command]
async fn save_snapshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
    annotations: Vec<IncomingAnnotation>,
) -> Result<snapshot::Snapshot, String> {
    let (frame, crop_origin) = match mode.as_str() {
        "frozen" => {
            // Dismiss the overlay immediately — the saved image comes from the
            // cached frame, not a re-capture, so hiding first doesn't affect the
            // output. This matches `live` (which hides up front) so the overlay
            // doesn't linger on screen during the annotate/encode/persist work.
            overlay::hide(&app).map_err(|e| e.to_string())?;
            let frame = state
                .latest_frame
                .lock()
                .ok()
                .and_then(|g| g.clone())
                .ok_or_else(|| "no cached frame for frozen mode".to_string())?;
            (frame, None)
        }
        "live" => {
            // Hide the overlay BEFORE capturing so the saved screenshot doesn't
            // include our own annotation rectangles + checker pattern. A brief
            // sleep gives macOS time to repaint without the overlay before the
            // screen-capture sample fires.
            overlay::hide(&app).map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(120));
            let frame = capture::capture_active_monitor(&app).map_err(|e| e.to_string())?;
            (frame, None)
        }
        "window" => {
            // Dismiss the overlay immediately (see the `frozen` arm) — window mode
            // also saves from the cached frame, so hiding first is safe and keeps
            // the overlay from lingering during the annotate/encode/persist work.
            overlay::hide(&app).map_err(|e| e.to_string())?;
            let session = match state.picker_state.current() {
                picker_session::RunMode::Annotating { session } => session,
                other => return Err(format!("save_snapshot('window') while in {:?}", other)),
            };
            let cached = state.latest_frame.lock().ok().and_then(|g| g.clone());
            let (frame, crop) = resolve_window_save_frame(&session, cached)?;
            (frame, Some(crop))
        }
        other => return Err(format!("unknown mode: {other}")),
    };

    let os_platform: &str = tauri_plugin_os::platform();
    let os_version: String = tauri_plugin_os::version().to_string();

    let base = snapshot::persist::snapshots_dir().map_err(|e| e.to_string())?;
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

    // For window mode, attach the captured window metadata to the snapshot
    // *before* persisting, so the on-disk JSON (and share routes that read it)
    // include context.window — not just the clipboard/MCP copy.
    if mode == "window" {
        if let picker_session::RunMode::Annotating { session } = state.picker_state.current() {
            built.snapshot.context.window = Some(snapshot::WindowInfo {
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

    let snap = built.snapshot.clone();

    // Persist to disk and copy to the clipboard concurrently: the disk write
    // needs the encoded PNG bytes (moved into the blocking task with `built`),
    // while the clipboard copy reuses the already-composited RGBA — so neither
    // waits on the other, and the Image path skips re-decoding the annotated PNG.
    let base_for_write = base.clone();
    let write_task =
        tokio::task::spawn_blocking(move || snapshot::persist::write(&base_for_write, &built));

    let clipboard_result: Result<(), String> = match state.settings.current_clipboard_format() {
        settings::ClipboardFormat::Json => {
            serde_json::to_string_pretty(&snap)
                .map_err(|e| e.to_string())
                .and_then(|json| {
                    app.clipboard()
                        .write_text(json)
                        .map_err(|e| format!("clipboard: {e}"))
                })
        }
        settings::ClipboardFormat::Prompt => {
            let prompt = snapshot::render_as_prompt(&snap);
            app.clipboard()
                .write_text(prompt)
                .map_err(|e| format!("clipboard: {e}"))
        }
        settings::ClipboardFormat::Image => {
            let (w, h) = (annotated_image.width(), annotated_image.height());
            let tauri_img = tauri::image::Image::new_owned(annotated_image.into_vec(), w, h);
            app.clipboard()
                .write_image(&tauri_img)
                .map_err(|e| format!("clipboard image: {e}"))
        }
    };

    // The disk write must succeed before we publish — MCP/share consumers read
    // the on-disk files. A clipboard failure is reported only after publishing,
    // so a saved-and-published snapshot isn't lost to a clipboard hiccup.
    write_task
        .await
        .map_err(|e| format!("persist task: {e}"))?
        .map_err(|e| e.to_string())?;

    state.snapshot_store.publish(snap.clone());

    if let Ok(mut ts) = state.latest_snapshot_at.lock() {
        *ts = Some(chrono::Utc::now());
    }
    tray::refresh_menu(&app);

    clipboard_result?;

    overlay::hide(&app).map_err(|e| e.to_string())?;
    state.picker_state.reset_to_idle();
    Ok(snap)
}

/// Shares the latest snapshot: starts the tunnel, generates a token, writes it to disk,
/// copies the URL to the clipboard, and fires a notification. Called by both the Tauri
/// command and the tray menu handler.
pub(crate) async fn do_share_latest(
    app: &tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    let snap = state
        .snapshot_store
        .get_latest()
        .ok_or_else(|| "no snapshot to share".to_string())?;
    let snap_id = snap.id.clone();

    let provider: Arc<dyn share::tunnel::provider::TunnelProvider> =
        match state.settings.current_provider() {
            settings::ProviderKind::Cloudflared => Arc::new(share::tunnel::cloudflared::Cloudflared),
            settings::ProviderKind::Ngrok       => Arc::new(share::tunnel::ngrok::Ngrok),
            settings::ProviderKind::Tailscale   => Arc::new(share::tunnel::tailscale::Tailscale),
        };

    let tunnel_url = Arc::clone(&state.tunnel).start_tunnel(provider, 7821).await
        .map_err(|e| e.to_string())?;

    let token = share::token::generate();
    let snap_dir = snapshot::persist::snapshots_dir().map_err(|e| e.to_string())?.join(&snap_id);
    share::token::write_for(&snap_dir, &token).map_err(|e| e.to_string())?;

    let share_url = format!("{tunnel_url}/share/{snap_id}/{token}/");
    app.clipboard().write_text(share_url.clone()).map_err(|e| e.to_string())?;

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification()
        .builder()
        .title("Glitch Spot")
        .body("Share URL copied to clipboard")
        .show();

    Ok(share_url)
}

/// Sets the tunnel provider: shuts down any running tunnel and persists the new choice.
/// Called by both the Tauri command and the tray menu handler.
pub(crate) async fn do_set_tunnel_provider(
    state: &AppState,
    kind: String,
) -> Result<(), String> {
    let parsed = settings::ProviderKind::from_name(&kind)
        .ok_or_else(|| format!("unknown provider: {kind}"))?;
    state.tunnel.shutdown().await;
    state.settings.save_provider(&state.config_dir, parsed).map_err(|e| e.to_string())
}

/// Persists the tray icon appearance. The animation loop reads the same
/// atomic flags every tick, so the change takes effect on the next ~100ms
/// tick — no need to wake the task explicitly.
pub(crate) fn do_set_tray_appearance(
    state: &AppState,
    animated: bool,
    coloured: bool,
) -> Result<(), String> {
    state
        .settings
        .save_tray_appearance(&state.config_dir, animated, coloured)
        .map_err(|e| e.to_string())
}

/// Persists the snapshot retention setting and immediately sweeps snapshots
/// that now exceed the new threshold. Expires in-memory state for any
/// snapshot whose directory was deleted.
pub(crate) async fn do_set_snapshot_retention(
    state: &AppState,
    key: String,
) -> Result<(), String> {
    let retention = settings::SnapshotRetention::from_name(&key)
        .ok_or_else(|| format!("unknown retention key: {key}"))?;
    state
        .settings
        .save_retention(&state.config_dir, retention)
        .map_err(|e| e.to_string())?;
    if let Some(max_age) = retention.max_age() {
        let base = snapshot::persist::snapshots_dir().map_err(|e| e.to_string())?;
        if let Err(e) = snapshot::persist::cleanup(&base, max_age) {
            eprintln!("[glitch-spot] retention cleanup failed: {e}");
        }
        state.snapshot_store.expire_if_deleted(&base);
        if state.snapshot_store.get_latest().is_none() {
            if let Ok(mut ts) = state.latest_snapshot_at.lock() {
                *ts = None;
            }
        }
    }
    Ok(())
}

/// Runs one cleanup sweep and expires in-memory state for any snapshot whose
/// directory was deleted. Returns true if the latest snapshot was expired
/// (caller should refresh the tray); returns false if the latest is still
/// present even if older snapshots were removed.
pub(crate) fn run_cleanup_sweep(state: &AppState) -> bool {
    let Some(max_age) = state.settings.current_retention().max_age() else {
        return false;
    };
    let base = match snapshot::persist::snapshots_dir() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[glitch-spot] cleanup: snapshots dir error: {e}");
            return false;
        }
    };
    if let Err(e) = snapshot::persist::cleanup(&base, max_age) {
        eprintln!("[glitch-spot] cleanup failed: {e}");
    }
    state.snapshot_store.expire_if_deleted(&base);
    let latest_expired = state.snapshot_store.get_latest().is_none()
        && state.latest_snapshot_at.lock().map_or(false, |ts| ts.is_some());
    if latest_expired {
        if let Ok(mut ts) = state.latest_snapshot_at.lock() {
            *ts = None;
        }
    }
    latest_expired
}

#[tauri::command]
async fn share_latest(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    do_share_latest(&app, &state).await
}

#[tauri::command]
fn current_tunnel_state(state: tauri::State<'_, AppState>) -> String {
    match state.tunnel.current_state() {
        share::tunnel::TunnelState::Idle => "idle".into(),
        share::tunnel::TunnelState::Starting => "starting…".into(),
        share::tunnel::TunnelState::Live { url } => url,
        share::tunnel::TunnelState::Error { reason } => format!("error — {reason}"),
    }
}

#[tauri::command]
async fn set_tunnel_provider(
    state: tauri::State<'_, AppState>,
    kind: String,
) -> Result<(), String> {
    do_set_tunnel_provider(&state, kind).await
}

/// Max picker windows pre-declared in tauri.conf.json. Each display gets one
/// picker; the rest stay hidden. Eight covers any realistic workstation setup.
const PICKER_POOL_SIZE: usize = 8;

pub(crate) async fn trigger_window_picker(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let state = app.state::<AppState>();

    let (windows, displays) = window_picker::macos::enumerate(app)
        .map_err(|e| anyhow::anyhow!("enumerate: {e}"))?;

    if displays.is_empty() {
        return Err(anyhow::anyhow!("no displays detected"));
    }

    // Force-reset state so the hotkey is self-recovering — if a prior picker
    // session never reached a clean cancel/save (e.g. macOS hid the picker via
    // Space switch without our visibilitychange listener firing), pressing
    // ⌃⌥⌘J again starts a fresh picker rather than failing with "already in
    // Picking".
    state.picker_state.reset_to_idle();
    // Hide any prior overlay/picker that might still be visible from the
    // stale session — fresh activation should not stack windows.
    let _ = overlay::hide(app);
    hide_all_pickers(app);

    let session = picker_session::PickerSession {
        windows: windows.clone(),
        displays: displays.clone(),
    };
    state
        .picker_state
        .try_enter_picking(session.clone())
        .map_err(|e| anyhow::anyhow!(e))?;

    let result = show_picker(app, &windows, &displays);
    if result.is_err() {
        // Roll back the state transition so a subsequent press isn't blocked
        // by a stale Picking entry from a half-failed show.
        state.picker_state.reset_to_idle();
        hide_all_pickers(app);
    } else {
        // Drive hover from the Rust side — see `poll_hover` for the why.
        // The task self-terminates when state leaves Picking, so cancel and
        // select paths just need to flip the state machine.
        let app_for_poll = app.clone();
        tokio::spawn(async move { poll_hover(app_for_poll).await });
    }
    result
}

/// Show one picker window per display (capped at PICKER_POOL_SIZE). Each
/// picker is single-display–sized so macOS's `constrainFrameRect` never
/// triggers. After all pickers are visible, activate the *app* (not any
/// individual window) — `set_focus` per-window would make each picker key in
/// turn, and only the last would receive mouseMoved events.
fn show_picker(
    app: &tauri::AppHandle,
    windows: &[window_picker::PickableWindow],
    displays: &[window_picker::DisplayBounds],
) -> anyhow::Result<()> {
    if displays.len() > PICKER_POOL_SIZE {
        eprintln!(
            "[picker] {} displays detected; pool size is {} — extra displays will not show picker",
            displays.len(),
            PICKER_POOL_SIZE,
        );
    }

    for (i, display) in displays.iter().enumerate().take(PICKER_POOL_SIZE) {
        let label = format!("picker-{i}");
        let win = app
            .get_webview_window(&label)
            .ok_or_else(|| anyhow::anyhow!("{label} not configured"))?;
        // Intentionally NOT setting canJoinAllSpaces — we want macOS to hide
        // pickers when the user swipes to a different Space, so the frontend's
        // visibilitychange listener fires picker_cancel and resets state.
        win.set_position(tauri::PhysicalPosition::new(display.x, display.y))?;
        win.set_size(tauri::PhysicalSize::new(display.w, display.h))?;
        // Show invisible: the pool windows are reused across displays and macOS
        // retains each one's last-painted buffer (a prior session's hover
        // checker). Showing at alpha 0 lets the window repaint this session's
        // blank layout while unseen; the picker reveals itself (invoke
        // reveal_window) one rAF after it paints, so show() can't flash the
        // stale checker. The fallback below reveals anyway if that signal is lost.
        overlay::set_window_alpha(&win, 0.0);
        let payload = serde_json::json!({
            "windows": windows,
            "displays": displays,
            "picker_origin": { "x": display.x, "y": display.y },
            "picker_scale": display.scale_factor,
        });
        let js = format!(
            "if (window.__glitchSpotPickerStart) window.__glitchSpotPickerStart({});",
            payload
        );
        let _ = win.eval(&js);
        win.show()?;
        // macOS constrainFrameRect pushes a full-display-sized window below the
        // menu bar on the initial show(s), before the borderless window is
        // realized — so re-assert the frame after it is on-screen. Without this
        // the first picker per launch lands a title-bar height too low.
        win.set_position(tauri::PhysicalPosition::new(display.x, display.y))?;
        win.set_size(tauri::PhysicalSize::new(display.w, display.h))?;
        let actual_pos = win
            .outer_position()
            .map_err(|e| anyhow::anyhow!("{label} outer_position: {e}"))?;
        eprintln!(
            "[picker] {label} requested=({},{}) {}x{} ; actual=({},{}) ; display_index={}",
            display.x, display.y, display.w, display.h,
            actual_pos.x, actual_pos.y, display.display_index,
        );
    }

    // App-level activation, *after* all pickers are visible. This makes the
    // pickers' NSViews receive mouseMoved events without per-window
    // makeKeyAndOrderFront (which would unfocus the others).
    activate_application();

    // Fallback reveal: each picker normally reveals itself one rAF after it
    // paints (invoke reveal_window). If that signal is ever lost, reveal every
    // shown picker after a short delay so an alpha-0 picker can't stay invisible.
    // Idempotent with the per-picker self-reveal.
    {
        let app_fallback = app.clone();
        let shown = displays.len().min(PICKER_POOL_SIZE);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            for i in 0..shown {
                if let Some(w) = app_fallback.get_webview_window(&format!("picker-{i}")) {
                    overlay::set_window_alpha(&w, 1.0);
                }
            }
        });
    }

    Ok(())
}

/// Hide every picker window in the pool. Used by force-reset, cancel, and
/// select paths so a partial show never leaves stale picker frames on screen.
fn hide_all_pickers(app: &tauri::AppHandle) {
    for i in 0..PICKER_POOL_SIZE {
        if let Some(w) = app.get_webview_window(&format!("picker-{i}")) {
            let _ = w.hide();
        }
    }
}

/// Drive picker hover from the Rust side by polling the global cursor
/// position at ~60Hz, hit-testing it against the session's windows, and
/// broadcasting the hovered window_id to every picker via a Tauri event.
///
/// Why polling instead of per-window JS mousemove: macOS delivers mouseMoved
/// events only to the key window. With one key window per app, only the
/// picker on the activation display ever fires mouseMoved, and the other
/// pickers stay silent regardless of `acceptsMouseMovedEvents` or window
/// level. Hit-testing from Rust dodges window-level event routing entirely.
///
/// Termination: the loop exits as soon as the state machine leaves Picking
/// (cancel, select, or force-reset all transition out), so `picker_cancel`
/// /`picker_select` don't need to touch this task explicitly.
async fn poll_hover(app: tauri::AppHandle) {
    let interval = std::time::Duration::from_millis(16);
    // Track which display's picker we last made key, so we only call set_focus
    // when the cursor crosses a display boundary (not every tick).
    let mut last_focused_display: Option<usize> = None;
    loop {
        let mode = app.state::<AppState>().picker_state.current();
        let session = match mode {
            picker_session::RunMode::Picking { session } => session,
            _ => break,
        };
        if let Ok(cursor) = app.cursor_position() {
            let cx = cursor.x as i32;
            let cy = cursor.y as i32;

            let id = window_picker::hit_test::hit_test(&session.windows, cx, cy);
            let _ = app.emit(
                "picker://hover",
                serde_json::json!({ "window_id": id }),
            );

            // The CSS picker cursor only activates on the key window. Move
            // key-ness to whichever picker the cursor is currently over so
            // the chromatic crosshair follows the cursor across displays.
            // Each picker is single-display-sized, so makeKeyAndOrderFront
            // can't trigger constrainFrameRect-driven relocation.
            let current_display = session.displays.iter().position(|d| {
                cx >= d.x && cx < d.x + d.w as i32 && cy >= d.y && cy < d.y + d.h as i32
            });
            if current_display.is_some() && current_display != last_focused_display {
                if let Some(i) = current_display {
                    if let Some(w) = app.get_webview_window(&format!("picker-{i}")) {
                        let _ = w.set_focus();
                    }
                }
                last_focused_display = current_display;
            }
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(target_os = "macos")]
fn activate_application() {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if !app.is_null() {
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_application() {}

#[tauri::command]
async fn picker_select(
    app: tauri::AppHandle,
    window_id: u32,
    initial_rect_x: Option<i32>,
    initial_rect_y: Option<i32>,
    initial_rect_w: Option<i32>,
    initial_rect_h: Option<i32>,
) -> Result<(), String> {
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

    hide_all_pickers(&app);

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
        initial_rect_css: match (initial_rect_x, initial_rect_y, initial_rect_w, initial_rect_h) {
            (Some(rx), Some(ry), Some(rw), Some(rh)) => Some(overlay::CssRect {
                x: (rx - display.x) as f32 / scale,
                y: (ry - display.y) as f32 / scale,
                w: rw as f32 / scale,
                h: rh as f32 / scale,
            }),
            _ => None,
        },
    };

    state
        .picker_state
        .try_select_window(picker_session::AnnotatingSession {
            window_id,
            window: window.clone(),
            display,
        })?;

    // Show the overlay first (dim surround + frozen-frame veil), then capture the
    // isolated window so the user never sees a blank pause while the grab/encode
    // runs. capture_window filters with .with_window(), so the visible overlay
    // does not pollute the grab.
    overlay::show_window_mode(&app, window.display_index, &payload).map_err(|e| e.to_string())?;

    // The overlay is shown at alpha 0 and revealed by the frontend one rAF after
    // its window-mode frame paints (invoke reveal_window). If that signal is
    // ever lost, reveal anyway after a short delay so the alpha-0 overlay can't
    // stay invisible. Idempotent with the frontend signal.
    {
        let app_fallback = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = overlay::reveal(&app_fallback);
        });
    }

    let mut frame = match capture::capture_window(&app, window_id) {
        Ok(f) => f,
        Err(e) => {
            use tauri_plugin_notification::NotificationExt;
            let _ = app
                .notification()
                .builder()
                .title("Glitch Spot")
                .body("Selected window closed — annotations discarded")
                .show();
            eprintln!("[glitch-spot] window capture failed: {e}");
            overlay::hide(&app).map_err(|err| err.to_string())?;
            state.picker_state.reset_to_idle();
            return Err(format!("window capture failed: {e}"));
        }
    };
    frame.display_index = window.display_index;

    // Cache for save reuse (get_latest_frame / save_snapshot read this) and push
    // to the already-visible overlay. push_frame runs after show_window_mode's
    // eval('window'), so the frontend resetFrames() cannot wipe the pushed frame.
    if let Ok(mut guard) = state.latest_frame.lock() {
        *guard = Some(frame.clone());
    }
    overlay::push_frame(&app, &frame).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn picker_cancel(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    // No-op unless we're actually in Picking. The Picker.tsx visibilitychange
    // listener fires picker_cancel whenever its window hides — including the
    // legitimate hide inside picker_select after a window is chosen. Without
    // this gate, that callback would reset state from Annotating back to Idle
    // and break the save flow ("save_snapshot('window') while in Idle").
    if !matches!(
        state.picker_state.current(),
        picker_session::RunMode::Picking { .. }
    ) {
        return Ok(());
    }
    hide_all_pickers(&app);
    state.picker_state.reset_to_idle();
    Ok(())
}

#[tauri::command]
fn set_tray_appearance(
    state: tauri::State<'_, AppState>,
    animated: bool,
    coloured: bool,
) -> Result<(), String> {
    do_set_tray_appearance(&state, animated, coloured)
}

#[tauri::command]
fn get_clipboard_format(state: tauri::State<'_, AppState>) -> String {
    state.settings.current_clipboard_format().as_str().to_string()
}

#[tauri::command]
fn set_clipboard_format(
    state: tauri::State<'_, AppState>,
    format: String,
) -> Result<(), String> {
    let fmt = settings::ClipboardFormat::from_name(&format)
        .ok_or_else(|| format!("unknown clipboard format: {format}"))?;
    state
        .settings
        .save_clipboard_format(&state.config_dir, fmt)
        .map_err(|e| e.to_string())
}

pub(crate) async fn trigger_overlay_live(app: &tauri::AppHandle) -> anyhow::Result<()> {
    if let Some(state) = app.try_state::<AppState>() {
        state.picker_state.try_enter_live().map_err(anyhow::Error::msg)?;
    }
    let my_gen = app
        .try_state::<AppState>()
        .map(|s| s.live_generation.fetch_add(1, Ordering::SeqCst) + 1);
    // Roll back to Idle if showing the overlay fails, so the state machine
    // isn't left stuck in Live (later hotkeys would fail with "cannot enter").
    let result = overlay::show_live(app).map_err(anyhow::Error::from);
    if result.is_err() {
        if let Some(state) = app.try_state::<AppState>() {
            state.picker_state.reset_to_idle();
        }
    } else {
        // Fallback reveal: show_live presents at alpha 0; the frontend normally
        // reveals via revealOverlay() one double-rAF after presentationId bumps.
        // If that signal is ever lost, reveal after a short delay so the overlay
        // can't stay invisible. Gate on live_generation so a cancel-and-reopen
        // within 300ms doesn't reveal the new alpha-0 session before its paint gate.
        let app_fallback = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let still_current = app_fallback
                .try_state::<AppState>()
                .zip(my_gen)
                .map_or(true, |(s, g)| s.live_generation.load(Ordering::SeqCst) == g);
            if still_current {
                let _ = overlay::reveal(&app_fallback);
            }
        });
    }
    result
}

pub(crate) async fn trigger_overlay_frozen(app: &tauri::AppHandle) -> anyhow::Result<()> {
    if let Some(state) = app.try_state::<AppState>() {
        state.picker_state.try_enter_frozen().map_err(anyhow::Error::msg)?;
    }
    // Tag this frozen session. A cancel (hide_overlay) or a superseding session
    // bumps the generation; a capture that finishes afterwards is dropped rather
    // than overwriting a newer overlay with its now-stale screenshot.
    let my_gen = app
        .try_state::<AppState>()
        .map(|s| s.freeze_generation.fetch_add(1, Ordering::SeqCst) + 1);
    // Clear any stale frame BEFORE capturing the new one. Without this, a fast
    // hotkey press right after switching monitors would let the overlay's
    // mount-time pull return the *previous* monitor's screenshot for a moment.
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut slot) = state.latest_frame.lock() {
            *slot = None;
        }
    }
    // Roll back to Idle and hide the overlay on any failure: because we show the
    // overlay before capturing, a capture/encode error would otherwise leave the
    // grey overlay stuck on screen.
    let result = (|| -> anyhow::Result<()> {
        // Show the grey overlay immediately — before the (slower) capture — so
        // pressing the shortcut feels instant. show_frozen_pending presents at
        // alpha 0; the frontend's double-rAF revealOverlay() brings it to alpha 1
        // once the grey veil has painted at the correct position.
        overlay::show_frozen_pending(app)?;

        // Fallback reveal: if the frontend's revealOverlay() signal is ever lost,
        // reveal after a short delay so the overlay can't stay invisible. Gate on
        // freeze_generation so a cancel-and-reopen within 300ms doesn't reveal
        // the new alpha-0 session before its own double-rAF paint gate runs.
        {
            let app_fallback = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                let still_current = app_fallback
                    .try_state::<AppState>()
                    .zip(my_gen)
                    .map_or(true, |(s, g)| s.freeze_generation.load(Ordering::SeqCst) == g);
                if still_current {
                    let _ = overlay::reveal(&app_fallback);
                }
            });
        }

        let frame = capture::capture_active_monitor(app)?;

        // Drop the frame if this session was canceled or superseded while the
        // capture was running.
        let still_current = app
            .try_state::<AppState>()
            .zip(my_gen)
            .map(|(s, g)| s.freeze_generation.load(Ordering::SeqCst) == g)
            .unwrap_or(true);
        if !still_current {
            return Ok(());
        }

        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut slot) = state.latest_frame.lock() {
                *slot = Some(frame.clone());
            }
        }
        overlay::push_frame(app, &frame)?;
        Ok(())
    })();
    if result.is_err() {
        if let Some(state) = app.try_state::<AppState>() {
            state.picker_state.reset_to_idle();
        }
        let _ = overlay::hide(app);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use crate::window_picker::{DisplayBounds, PickableWindow, WindowBounds};

    fn annotating_session() -> picker_session::AnnotatingSession {
        picker_session::AnnotatingSession {
            window_id: 7,
            window: PickableWindow {
                window_id: 7,
                app_name: "Safari".into(),
                title: "Reader".into(),
                bounds: WindowBounds { x: 240, y: 160, w: 800, h: 600 },
                display_index: 1,
                layer: 0,
                visible_regions: vec![],
            },
            display: DisplayBounds {
                display_index: 1, x: 200, y: 100, w: 2560, h: 1414, scale_factor: 2.0,
            },
        }
    }

    fn sample_frame() -> capture::CapturedFrame {
        capture::CapturedFrame {
            width: 800, height: 600, scale_factor: 2.0, png: vec![1, 2, 3], display_index: 0,
        }
    }

    #[test]
    fn window_save_reuses_cached_frame_and_computes_crop() {
        let session = annotating_session();
        let cached = sample_frame();
        let (frame, crop) =
            resolve_window_save_frame(&session, Some(cached.clone())).unwrap();
        // Same pixels — no recapture.
        assert_eq!(frame.png, cached.png);
        // display_index tracks the window's display.
        assert_eq!(frame.display_index, 1);
        // Crop origin is the window's offset within its display, in physical px.
        assert_eq!(crop, (40.0, 60.0));
    }

    #[test]
    fn window_save_errors_when_no_cached_frame() {
        let session = annotating_session();
        let err = resolve_window_save_frame(&session, None).unwrap_err();
        assert!(err.contains("no cached frame"), "got: {err}");
    }

    #[test]
    fn app_state_latest_snapshot_at_starts_none() {
        let temp = tempfile::TempDir::new().unwrap();
        let state = AppState {
            latest_frame: Mutex::new(None),
            snapshot_store: Arc::new(mcp::store::SnapshotStore::new()),
            latest_snapshot_at: Mutex::new(None),
            started_at: chrono::Utc::now(),
            tunnel: Arc::new(share::tunnel::TunnelManager::new()),
            settings: Arc::new(
                settings::Settings::load_from(temp.path()).unwrap(),
            ),
            config_dir: temp.path().to_path_buf(),
            mcp_bound: Arc::new(AtomicBool::new(false)),
            picker_state: Arc::new(picker_session::PickerStateHolder::new()),
            freeze_generation: Arc::new(AtomicU64::new(0)),
            live_generation: Arc::new(AtomicU64::new(0)),
        };
        assert!(state.latest_snapshot_at.lock().unwrap().is_none());
    }

    #[test]
    fn share_router_is_reachable_from_lib() {
        let temp = tempfile::TempDir::new().unwrap();
        let _router = crate::share::router::build_share_router(temp.path().to_path_buf());
        // Smoke test: just confirm construction doesn't panic.
    }

    // Only meaningful in a debug build, where autostart_on_launch() must be false.
    // Under `cargo test --release` debug_assertions is off (release autostarts), so
    // skip rather than assert the wrong branch.
    #[cfg(debug_assertions)]
    #[test]
    fn debug_builds_do_not_autoregister_login_item() {
        assert!(!autostart_on_launch());
    }
}

/// Whether this build registers itself as a macOS login item on launch.
/// Only release builds auto-start; dev/debug builds must not add themselves to
/// Login Items, since their productName is per-build and the binary lives at a
/// throwaway path.
fn autostart_on_launch() -> bool {
    !cfg!(debug_assertions)
}

pub fn run() {
    let config_dir = std::path::PathBuf::from(
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()),
    )
    .join(".glitch-spot");

    let settings = match settings::Settings::load_from(&config_dir) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("[glitch-spot] settings load failed: {e}; using defaults");
            let temp = std::env::temp_dir().join("glitch-spot-fallback");
            let _ = std::fs::create_dir_all(&temp);
            Arc::new(
                settings::Settings::load_from(&temp)
                    .expect("default settings construction must succeed"),
            )
        }
    };

    let snapshot_store = Arc::new(mcp::store::SnapshotStore::new());
    let store_for_mcp = snapshot_store.clone();
    let mcp_port = settings.mcp_port();
    let mcp_bound = Arc::new(AtomicBool::new(false));
    let mcp_bound_for_serve = mcp_bound.clone();

    tauri::async_runtime::spawn(async move {
        mcp::server::serve(store_for_mcp, mcp_port, mcp_bound_for_serve).await;
    });

    let share_base = snapshot::persist::snapshots_dir().ok();
    if let Some(base) = share_base.clone() {
        tauri::async_runtime::spawn(async move {
            share::router::serve_share(base).await;
        });
    } else {
        eprintln!("[glitch-spot] share server NOT started — could not resolve snapshots dir");
    }

    let tunnel = Arc::new(share::tunnel::TunnelManager::new());

    let tunnel_for_exit = tunnel.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            latest_frame: Mutex::new(None),
            snapshot_store,
            latest_snapshot_at: Mutex::new(None),
            started_at: chrono::Utc::now(),
            tunnel: tunnel.clone(),
            settings: settings.clone(),
            config_dir: config_dir.clone(),
            mcp_bound: mcp_bound.clone(),
            picker_state: Arc::new(picker_session::PickerStateHolder::new()),
            freeze_generation: Arc::new(AtomicU64::new(0)),
            live_generation: Arc::new(AtomicU64::new(0)),
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Auto-enable login item on first launch (if not already registered).
            // Release builds only: dev/debug builds carry a per-build productName,
            // so auto-registering would litter ~/Library/LaunchAgents with throwaway
            // entries pointing at temporary build paths.
            if autostart_on_launch() {
                use tauri_plugin_autostart::ManagerExt;
                let manager = app.autolaunch();
                if !manager.is_enabled().unwrap_or(false) {
                    let _ = manager.enable();
                }
            }

            hotkey::register(app.handle())?;
            let _tray = tray::setup_tray(app.handle())?;

            // Startup sweep: delete snapshots that already exceed the retention
            // threshold and expire any stale in-memory state.
            {
                let state = app.state::<AppState>();
                run_cleanup_sweep(&state);
                // No tray refresh needed here — the menu hasn't been interacted
                // with yet and setup_tray just built it with current state.
            }

            // Periodic sweep: enforce the retention setting every hour while
            // the app is running. Must be inside setup() to have AppHandle for
            // tray refresh after deleting the latest snapshot.
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval =
                        tokio::time::interval(tokio::time::Duration::from_secs(60 * 60));
                    interval.tick().await; // discard the immediate first tick
                    loop {
                        interval.tick().await;
                        let state = app_handle.state::<AppState>();
                        if run_cleanup_sweep(&state) {
                            tray::refresh_menu(&app_handle);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            capture_active_monitor,
            open_overlay_live,
            open_overlay_frozen,
            hide_overlay,
            refresh_cursor,
            reveal_window,
            get_latest_frame,
            save_snapshot,
            spike_capture::spike_capture,
            share_latest,
            current_tunnel_state,
            set_tunnel_provider,
            picker_select,
            picker_cancel,
            set_tray_appearance,
            get_clipboard_format,
            set_clipboard_format,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |_app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = &event {
            let tunnel = tunnel_for_exit.clone();
            tauri::async_runtime::block_on(async move {
                tunnel.shutdown().await;
            });
        }
    });
}
