// ABOUTME: Menu bar tray icon setup and dynamic menu builder for Glitch Spot.
// ABOUTME: Rebuilds the menu on every tray click to reflect current snapshot age and autostart state.

use chrono::{DateTime, Utc};
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;

/// Stable id for the single tray icon, so background tasks can fetch it via
/// `app.tray_by_id` and refresh the menu without holding a clone.
pub const TRAY_ID: &str = "glitch-spot";

// Cyan channel of the chromatic split (`--glitch-cyan` in DESIGN.md). Used for
// both the healthy pulse and the cyan flashes inside the glitch burst.
const CYAN_RGB: (u8, u8, u8) = (0, 245, 255);

// Red for the unhealthy status indicator (`--delete` in DESIGN.md — the
// destructive/error red also used on the × badge). Semantically a *signal*,
// not a glitch flourish.
const RED_RGB: (u8, u8, u8) = (255, 59, 48);

/// Maximum pixel shift applied to each scanline band during a glitch burst.
pub(crate) const GLITCH_SHIFT_MAX: i8 = 3;

/// Replace the RGB channels of every non-transparent pixel with `rgb`, and
/// scale the existing alpha by `alpha_factor`. Pure — used to build animation
/// frames once at startup.
pub(crate) fn tint_frame(source_rgba: &[u8], rgb: (u8, u8, u8), alpha_factor: f32) -> Vec<u8> {
    let mut out = source_rgba.to_vec();
    for chunk in out.chunks_mut(4) {
        if chunk.len() != 4 { continue; }
        if chunk[3] == 0 { continue; }
        chunk[0] = rgb.0;
        chunk[1] = rgb.1;
        chunk[2] = rgb.2;
        let scaled = (chunk[3] as f32 * alpha_factor).clamp(0.0, 255.0);
        chunk[3] = scaled as u8;
    }
    out
}

/// Number of frames in the cyan pulse cycle. Combined with `ANIM_TICK_MS`,
/// determines cycle length: 68 × 35ms ≈ 2.4s gentle pulse.
pub(crate) const CYAN_FRAME_COUNT: usize = 68;

/// 48 frames forming a 2.4s cosine pulse (alpha 30% → 100% → 30%).
pub(crate) fn build_cyan_frames(source: &[u8]) -> Vec<Vec<u8>> {
    const BASE: f32 = 0.65;
    const AMP: f32 = 0.35;
    (0..CYAN_FRAME_COUNT).map(|i| {
        let t = i as f32 / CYAN_FRAME_COUNT as f32;
        let alpha = BASE + AMP * (2.0 * std::f32::consts::PI * t).cos();
        tint_frame(source, CYAN_RGB, alpha)
    }).collect()
}

/// Steady red frame shown between bursts during the unhealthy state.
pub(crate) fn build_red_steady(source: &[u8]) -> Vec<u8> {
    tint_frame(source, RED_RGB, 0.65)
}

/// Brighter red base used as input to scanline shift during unhealthy bursts.
pub(crate) fn build_red_burst_base(source: &[u8]) -> Vec<u8> {
    tint_frame(source, RED_RGB, 0.80)
}

pub(crate) struct AnimationFrames {
    pub cyan: Vec<Vec<u8>>,
    /// Constant-alpha red frame shown between unhealthy scanline bursts.
    pub red_steady: Vec<u8>,
    /// Brighter red base fed into scanline-shift during unhealthy bursts.
    pub red_burst_base: Vec<u8>,
    /// Raw source bytes kept for runtime scanline-shift rendering.
    pub source: Vec<u8>,
    /// Cyan-tinted base frame used as input to scanline-shift during bursts,
    /// so the displaced pixels stay cyan rather than reverting to raw black.
    pub burst_base: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// One horizontal band to shift during a glitch burst step.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScanlineBand {
    pub y0: u8,
    pub height: u8,
    /// Pixels to shift right (positive) or left (negative).
    pub dx: i8,
}

/// One step of a glitch burst: 2-3 bands each shifted independently.
#[derive(Debug, Clone)]
pub(crate) struct ScanlineStep {
    pub bands: Vec<ScanlineBand>,
    pub ticks: u8,
}

/// Shift horizontal bands of the source RGBA image. Each band's rows are
/// displaced by `dx` pixels; pixels shifted outside the image width become
/// transparent. Rows outside all bands are copied unchanged.
pub(crate) fn apply_scanline_shift(
    source: &[u8],
    width: u32,
    height: u32,
    bands: &[ScanlineBand],
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut out = source.to_vec();
    for band in bands {
        let y0 = band.y0 as usize;
        let y1 = (y0 + band.height as usize).min(h);
        let dx = band.dx as i32;
        for y in y0..y1 {
            let row_base = y * w * 4;
            let mut row = vec![0u8; w * 4];
            for x in 0..w {
                let src_x = x as i32 - dx;
                if src_x < 0 || src_x >= w as i32 {
                    continue;
                }
                let si = row_base + (src_x as usize) * 4;
                let di = x * 4;
                row[di..di + 4].copy_from_slice(&source[si..si + 4]);
            }
            out[row_base..row_base + w * 4].copy_from_slice(&row);
        }
    }
    out
}

/// Generate a randomised scanline burst: 6-10 steps, each with 2-3 bands
/// shifted ±1–GLITCH_SHIFT_MAX px. Non-uniform shifts across bands give the
/// VHS tape-tear feel rather than a uniform nudge.
pub(crate) fn random_scanline_sequence<R: rand::Rng>(
    rng: &mut R,
    height: u32,
) -> Vec<ScanlineStep> {
    let n = rng.gen_range(6..=10usize);
    let h = height as u8;
    (0..n).map(|_| {
        let num_bands = rng.gen_range(2..=3usize);
        let bands = (0..num_bands).map(|_| {
            let band_h = rng.gen_range((h / 7).max(2)..=(h / 4).max(3));
            let max_y = h.saturating_sub(band_h);
            let y0 = if max_y > 0 { rng.gen_range(0..max_y) } else { 0 };
            let dx_mag = rng.gen_range(1i8..=GLITCH_SHIFT_MAX);
            let dx = if rng.gen_bool(0.5) { dx_mag } else { -dx_mag };
            ScanlineBand { y0, height: band_h, dx }
        }).collect();
        ScanlineStep { bands, ticks: rng.gen_range(1..=2) }
    }).collect()
}

pub(crate) fn mcp_status_label(healthy: bool, port: u16) -> String {
    let indicator = if healthy { "🟢" } else { "🔴" };
    format!("{indicator} MCP Server :{port}")
}

/// Read the shared `bound` flag that the MCP server toggles around its
/// bind+serve loop. True ⇒ *this* process owns the listener (not just "some
/// process is on the port"), which is what we want to communicate to the user.
fn mcp_is_healthy(bound: &std::sync::atomic::AtomicBool) -> bool {
    bound.load(std::sync::atomic::Ordering::SeqCst)
}

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

/// Rebuilds the tray menu from current AppState and installs it.
///
/// On macOS, `on_tray_icon_event` does not fire for left-clicks when the tray
/// has an attached menu — the click opens the menu directly. So state changes
/// (snapshot saved, MCP port changed, etc.) must trigger this eagerly rather
/// than relying on a click-time rebuild.
pub fn refresh_menu(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub(crate) fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let state = app.state::<crate::AppState>();

    let snapshot_label = {
        let ts = *state.latest_snapshot_at.lock().unwrap();
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

    let capture_window = MenuItemBuilder::new("Capture Window")
        .id("capture_window")
        .accelerator("Ctrl+Alt+Super+J")
        .build(app)?;

    let port = state.settings.mcp_port();
    let mcp_status = MenuItemBuilder::new(mcp_status_label(mcp_is_healthy(&state.mcp_bound), port))
        .id("mcp_status")
        .enabled(false)
        .build(app)?;

    let latest_snapshot = MenuItemBuilder::new(&snapshot_label)
        .id("latest_snapshot")
        .build(app)?;

    let has_snapshot = state.snapshot_store.get_latest().is_some();
    let tunnel_state_label = match state.tunnel.current_state() {
        crate::share::tunnel::TunnelState::Idle => "Tunnel: idle".to_string(),
        crate::share::tunnel::TunnelState::Starting => "Tunnel: starting…".to_string(),
        crate::share::tunnel::TunnelState::Live { url } => {
            let host = url.trim_start_matches("https://").trim_start_matches("http://");
            format!("Tunnel: {host}")
        }
        crate::share::tunnel::TunnelState::Error { reason } => format!("Tunnel: error — {reason}"),
    };
    let share_disabled = !has_snapshot
        || matches!(state.tunnel.current_state(), crate::share::tunnel::TunnelState::Starting);

    let share_latest_item = MenuItemBuilder::new("Share latest snapshot…")
        .id("share_latest")
        .enabled(!share_disabled)
        .build(app)?;

    let tunnel_status_item = MenuItemBuilder::new(&tunnel_state_label)
        .id("tunnel_status")
        .enabled(false)
        .build(app)?;

    let current = state.settings.current_provider();
    let provider_cf = CheckMenuItemBuilder::new("cloudflared")
        .id("provider:cloudflared")
        .checked(matches!(current, crate::settings::ProviderKind::Cloudflared))
        .build(app)?;
    let provider_ng = CheckMenuItemBuilder::new("ngrok")
        .id("provider:ngrok")
        .checked(matches!(current, crate::settings::ProviderKind::Ngrok))
        .build(app)?;
    let provider_ts = CheckMenuItemBuilder::new("Tailscale Funnel")
        .id("provider:tailscale")
        .checked(matches!(current, crate::settings::ProviderKind::Tailscale))
        .build(app)?;
    let provider_submenu = SubmenuBuilder::new(app, "Tunnel provider")
        .item(&provider_cf)
        .item(&provider_ng)
        .item(&provider_ts)
        .build()?;

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
        .item(&capture_window)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&mcp_status)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&latest_snapshot)
        .item(&share_latest_item)
        .item(&tunnel_status_item)
        .item(&provider_submenu)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&launch_at_login)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&quit)
        .build()
}

/// Decode the bundled monochrome tray PNG into raw RGBA + dimensions, then
/// pre-compute the cyan pulse and red blink frames. Done once at startup; the
/// animation task just cycles through these.
fn build_animation_frames() -> Option<AnimationFrames> {
    let bytes = include_bytes!("../icons/tray.png");
    let img = match image::load_from_memory(bytes) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            eprintln!("[glitch-spot tray] could not decode tray.png: {e}");
            return None;
        }
    };
    let (width, height) = img.dimensions();
    let source = img.into_raw();
    let cyan_frames = build_cyan_frames(&source);
    let burst_base = tint_frame(&source, CYAN_RGB, 0.80);
    Some(AnimationFrames {
        cyan: cyan_frames,
        red_steady: build_red_steady(&source),
        red_burst_base: build_red_burst_base(&source),
        source,
        burst_base,
        width,
        height,
    })
}

pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    use tauri::image::Image;

    let frames = build_animation_frames();
    // Read health state before build_menu so we know what the menu was built
    // with; passed to the animation task to avoid a spurious refresh at the
    // first health_tick when nothing actually changed.
    let mcp_bound = {
        let state = app.state::<crate::AppState>();
        state.mcp_bound.clone()
    };
    let menu_built_healthy = mcp_is_healthy(&mcp_bound);
    let menu = build_menu(app)?;

    // Initial icon: cyan frame 0 if frames decoded, otherwise the raw black PNG
    // (which would have been rendered via icon_as_template before this change —
    // here it just shows up as black, only used in the failure fallback).
    let initial_icon: Image<'_> = match &frames {
        Some(f) => Image::new(&f.cyan[0], f.width, f.height),
        None => Image::from_bytes(include_bytes!("../icons/tray.png"))?,
    };

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(initial_icon)
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
                "capture_window" => {
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = crate::trigger_window_picker(&app).await {
                            eprintln!("tray: capture window failed: {e}");
                        }
                    });
                }
                "latest_snapshot" => {
                    let state = app.state::<crate::AppState>();
                    let Some(snap) = state.snapshot_store.get_latest() else {
                        let _ = app
                            .notification()
                            .builder()
                            .title("Glitch Spot")
                            .body("No snapshot yet — capture one first.")
                            .show();
                        return;
                    };
                    let body = match serde_json::to_string_pretty(&snap) {
                        Ok(json) => match app.clipboard().write_text(json) {
                            Ok(()) => format!("Copied JSON for {} to clipboard.", snap.id),
                            Err(e) => format!("Couldn't copy to clipboard: {e}"),
                        },
                        Err(e) => format!("Couldn't serialize snapshot: {e}"),
                    };
                    let _ = app
                        .notification()
                        .builder()
                        .title("Glitch Spot")
                        .body(body)
                        .show();
                }
                "share_latest" => {
                    let app2 = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app2.state::<crate::AppState>();
                        if let Err(e) = crate::do_share_latest(&app2, &state).await {
                            eprintln!("tray: share_latest failed: {e}");
                        }
                    });
                }
                "provider:cloudflared" | "provider:ngrok" | "provider:tailscale" => {
                    let kind = event.id.as_ref().split(':').nth(1).unwrap_or("cloudflared").to_string();
                    let app2 = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app2.state::<crate::AppState>();
                        if let Err(e) = crate::do_set_tunnel_provider(&state, kind).await {
                            eprintln!("tray: set provider failed: {e}");
                        }
                        refresh_menu(&app2);
                    });
                }
                "launch_at_login" => {
                    let manager = app.autolaunch();
                    let is_on = manager.is_enabled().unwrap_or(false);
                    if is_on {
                        let _ = manager.disable();
                    } else {
                        let _ = manager.enable();
                    }
                    refresh_menu(&app);
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
        .build(app)?;

    let app_handle = app.clone();
    let tray_for_watch = tray.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<crate::AppState>();
        let mut rx = state.tunnel.subscribe();
        let mut last_state: crate::share::tunnel::TunnelState = rx.borrow().clone();
        while rx.changed().await.is_ok() {
            let new_state = rx.borrow().clone();
            // Fire notification on transition INTO Error from any non-Error state.
            if let crate::share::tunnel::TunnelState::Error { reason } = &new_state {
                if !matches!(last_state, crate::share::tunnel::TunnelState::Error { .. }) {
                    let _ = app_handle.notification()
                        .builder()
                        .title("Glitch Spot — tunnel error")
                        .body(reason.as_str())
                        .show();
                }
            }
            last_state = new_state;
            if let Ok(menu) = build_menu(&app_handle) {
                let _ = tray_for_watch.set_menu(Some(menu));
            }
        }
    });

    // Animate the tray icon based on MCP health: gentle cyan pulse when up,
    // aggressive red blink when down. Polls health every 3s.
    if let Some(frames) = frames {
        let tray_for_anim = tray.clone();
        tauri::async_runtime::spawn(run_tray_animation(tray_for_anim, frames, mcp_bound, app.clone(), menu_built_healthy));
    }

    Ok(tray)
}

const ANIM_TICK_MS: u64 = 35;
/// Min/max ticks between glitch bursts during healthy state.
/// At 35ms/tick: 90-165 ticks ≈ 3.2-5.8s, centred around 4.5s.
const GLITCH_GAP_MIN: u64 = 90;
const GLITCH_GAP_MAX: u64 = 165;
/// Min/max ticks between scanline bursts during unhealthy state.
/// At 35ms/tick: 60-120 ticks ≈ 2.1-4.2s — more agitated than healthy.
const RED_GAP_MIN: u64 = 60;
const RED_GAP_MAX: u64 = 120;

enum AnimState {
    Pulsing,
    Glitching {
        sequence: Vec<ScanlineStep>,
        step_idx: usize,
        ticks_in_step: u8,
    },
}

async fn run_tray_animation(
    tray: tauri::tray::TrayIcon,
    frames: AnimationFrames,
    mcp_bound: std::sync::Arc<std::sync::atomic::AtomicBool>,
    app: AppHandle,
    menu_built_healthy: bool,
) {
    use rand::{Rng, SeedableRng};
    use tauri::image::Image;
    use tokio::time::{interval, interval_at, Duration, Instant};

    let mut anim_tick = interval(Duration::from_millis(ANIM_TICK_MS));
    // interval_at so the first tick fires after 3s, not immediately.
    // The initial menu sync deferred to this tick must not race with
    // macOS menu system initialisation at task startup.
    let mut health_tick = interval_at(
        Instant::now() + Duration::from_secs(3),
        Duration::from_secs(3),
    );
    let mut healthy = mcp_is_healthy(&mcp_bound);
    // Tracks whether we've done the one-time startup menu sync. Deferred to
    // the first health_tick (≥3s) so set_menu is never called at task-start
    // while the OS menu system is still initialising (which caused a first-
    // hover glitch on the Tunnel Provider submenu).
    let mut initial_sync_done = false;
    let mut counter: u64 = 0;
    // StdRng is Send, so the future is Send and can be scheduled across threads
    // by tokio. thread_rng() isn't Send because it holds thread-local state.
    let mut rng = rand::rngs::StdRng::from_entropy();
    let mut state = AnimState::Pulsing;
    let mut next_glitch_at: u64 = rng.gen_range(GLITCH_GAP_MIN..=GLITCH_GAP_MAX);
    let mut red_state = AnimState::Pulsing;
    let mut next_red_glitch_at: u64 = rng.gen_range(RED_GAP_MIN..=RED_GAP_MAX);

    loop {
        tokio::select! {
            _ = anim_tick.tick() => {
                if !healthy {
                    match &mut red_state {
                        AnimState::Pulsing => {
                            let img = Image::new(&frames.red_steady, frames.width, frames.height);
                            let _ = tray.set_icon(Some(img));
                            if counter >= next_red_glitch_at {
                                red_state = AnimState::Glitching {
                                    sequence: random_scanline_sequence(&mut rng, frames.height),
                                    step_idx: 0,
                                    ticks_in_step: 0,
                                };
                            }
                        }
                        AnimState::Glitching { sequence, step_idx, ticks_in_step } => {
                            let shifted = apply_scanline_shift(
                                &frames.red_burst_base, frames.width, frames.height,
                                &sequence[*step_idx].bands,
                            );
                            let img = Image::new(&shifted, frames.width, frames.height);
                            let _ = tray.set_icon(Some(img));
                            *ticks_in_step += 1;
                            if *ticks_in_step >= sequence[*step_idx].ticks {
                                *step_idx += 1;
                                *ticks_in_step = 0;
                                if *step_idx >= sequence.len() {
                                    red_state = AnimState::Pulsing;
                                    next_red_glitch_at = counter
                                        + rng.gen_range(RED_GAP_MIN..=RED_GAP_MAX);
                                }
                            }
                        }
                    }
                } else {
                    match &mut state {
                        AnimState::Pulsing => {
                            // 68 cyan frames, advance every tick → 35ms/frame, ~2.4s cycle.
                            let idx = (counter % frames.cyan.len() as u64) as usize;
                            let img = Image::new(&frames.cyan[idx], frames.width, frames.height);
                            let _ = tray.set_icon(Some(img));
                            if counter >= next_glitch_at {
                                state = AnimState::Glitching {
                                    sequence: random_scanline_sequence(&mut rng, frames.height),
                                    step_idx: 0,
                                    ticks_in_step: 0,
                                };
                            }
                        }
                        AnimState::Glitching { sequence, step_idx, ticks_in_step } => {
                            let shifted = apply_scanline_shift(
                                &frames.burst_base, frames.width, frames.height,
                                &sequence[*step_idx].bands,
                            );
                            let img = Image::new(&shifted, frames.width, frames.height);
                            let _ = tray.set_icon(Some(img));
                            *ticks_in_step += 1;
                            if *ticks_in_step >= sequence[*step_idx].ticks {
                                *step_idx += 1;
                                *ticks_in_step = 0;
                                if *step_idx >= sequence.len() {
                                    state = AnimState::Pulsing;
                                    next_glitch_at = counter
                                        + rng.gen_range(GLITCH_GAP_MIN..=GLITCH_GAP_MAX);
                                }
                            }
                        }
                    }
                }
                counter = counter.wrapping_add(1);
            }
            _ = health_tick.tick() => {
                let new_healthy = mcp_is_healthy(&mcp_bound);
                if !initial_sync_done {
                    initial_sync_done = true;
                    // Only refresh if health changed since the menu was built.
                    // An unconditional refresh here calls set_menu on macOS, which
                    // closes any open menu — causing the Tunnel Provider submenu to
                    // make the whole menu vanish on first hover after launch.
                    if new_healthy != menu_built_healthy {
                        refresh_menu(&app);
                    }
                } else if !new_healthy {
                    // Always reset the healthy state machine so the cyan pulse
                    // resumes cleanly when health recovers.
                    state = AnimState::Pulsing;
                    if healthy {
                        // Just transitioned healthy→unhealthy: reset red burst
                        // state so the gap timer starts fresh from now.
                        red_state = AnimState::Pulsing;
                        next_red_glitch_at = counter + rng.gen_range(RED_GAP_MIN..=RED_GAP_MAX);
                    }
                } else if !healthy {
                    // Transitioned to healthy after a delayed bind — refresh the
                    // tray menu so the MCP status item shows 🟢 immediately.
                    refresh_menu(&app);
                }
                healthy = new_healthy;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

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

    #[test]
    fn mcp_status_healthy_renders_green_with_default_port() {
        assert_eq!(mcp_status_label(true, 7820), "🟢 MCP Server :7820");
    }

    #[test]
    fn mcp_status_unhealthy_renders_red_with_default_port() {
        assert_eq!(mcp_status_label(false, 7820), "🔴 MCP Server :7820");
    }

    #[test]
    fn mcp_status_renders_custom_port() {
        assert_eq!(mcp_status_label(true, 9090), "🟢 MCP Server :9090");
    }

    #[test]
    fn tint_frame_replaces_rgb_on_opaque_pixels_only() {
        let source = vec![
            0, 0, 0, 255,  // opaque black → should be tinted
            0, 0, 0, 128,  // half-alpha black → tinted, alpha scaled
            0, 0, 0, 0,    // fully transparent → untouched
        ];
        let out = tint_frame(&source, (255, 0, 60), 1.0);
        assert_eq!(&out[0..4], &[255, 0, 60, 255], "opaque pixel tinted");
        assert_eq!(&out[4..8], &[255, 0, 60, 128], "half-alpha pixel tinted, alpha preserved at factor 1.0");
        assert_eq!(&out[8..12], &[0, 0, 0, 0], "transparent pixel untouched");
    }

    #[test]
    fn tint_frame_scales_alpha_by_factor() {
        let source = vec![0, 0, 0, 200];
        let out = tint_frame(&source, (0, 245, 255), 0.5);
        assert_eq!(out, vec![0, 245, 255, 100]);
    }

    #[test]
    fn cyan_frames_produce_full_set_with_full_alpha_peak() {
        let source = vec![0, 0, 0, 255];
        let frames = build_cyan_frames(&source);
        assert_eq!(frames.len(), CYAN_FRAME_COUNT);
        // Frame 0 starts at the cosine peak → alpha factor = BASE + AMP * 1.0 = 1.0.
        assert_eq!(frames[0][3], 255, "frame 0 is fully opaque");
        // Frame N/2 (half cycle) → cos(π) = -1.0 → alpha factor = BASE - AMP = 0.30.
        let expected_min = (255.0 * 0.30) as u8;
        assert_eq!(
            frames[CYAN_FRAME_COUNT / 2][3],
            expected_min,
            "halfway frame is the dimmest"
        );
    }

    #[test]
    fn red_steady_uses_delete_red_at_constant_alpha() {
        let source = vec![0, 0, 0, 255];
        let frame = build_red_steady(&source);
        // --delete from DESIGN.md, clearly visible but not alarm-bright.
        assert_eq!(&frame[0..3], &[255, 59, 48]);
        assert_eq!(frame[3], (255.0 * 0.65) as u8);
    }

    #[test]
    fn red_burst_base_uses_delete_red_brighter_than_steady() {
        let source = vec![0, 0, 0, 255];
        let steady = build_red_steady(&source);
        let burst = build_red_burst_base(&source);
        assert_eq!(&burst[0..3], &[255, 59, 48]);
        assert!(burst[3] > steady[3], "burst base should be brighter than steady");
        assert_eq!(burst[3], (255.0 * 0.80) as u8);
    }

    #[test]
    fn apply_scanline_shift_leaves_non_band_rows_unchanged() {
        // 4×3 image, all opaque white
        let source: Vec<u8> = vec![255u8; 4 * 3 * 4];
        let band = ScanlineBand { y0: 1, height: 1, dx: 2 };
        let out = apply_scanline_shift(&source, 4, 3, &[band]);
        assert_eq!(&out[0..16],  &source[0..16],  "row 0 before band unchanged");
        assert_eq!(&out[32..48], &source[32..48], "row 2 after band unchanged");
    }

    #[test]
    fn apply_scanline_shift_moves_band_pixels_right() {
        // 4×2, all white opaque. Shift row 0 right by 2.
        let source: Vec<u8> = vec![255u8; 4 * 2 * 4];
        let band = ScanlineBand { y0: 0, height: 1, dx: 2 };
        let out = apply_scanline_shift(&source, 4, 2, &[band]);
        assert_eq!(out[0 * 4 + 3], 0, "x=0 clipped → transparent");
        assert_eq!(out[1 * 4 + 3], 0, "x=1 clipped → transparent");
        assert_eq!(&out[2 * 4..3 * 4], &source[0 * 4..1 * 4], "x=2 gets source x=0");
        assert_eq!(&out[3 * 4..4 * 4], &source[1 * 4..2 * 4], "x=3 gets source x=1");
        assert_eq!(&out[16..32], &source[16..32], "row 1 unchanged");
    }

    #[test]
    fn apply_scanline_shift_moves_band_pixels_left() {
        // 4×1, all white opaque. Shift row 0 left by 2 (dx = -2).
        let source: Vec<u8> = vec![255u8; 4 * 4];
        let band = ScanlineBand { y0: 0, height: 1, dx: -2 };
        let out = apply_scanline_shift(&source, 4, 1, &[band]);
        assert_eq!(&out[0 * 4..1 * 4], &source[2 * 4..3 * 4], "x=0 gets source x=2");
        assert_eq!(&out[1 * 4..2 * 4], &source[3 * 4..4 * 4], "x=1 gets source x=3");
        assert_eq!(out[2 * 4 + 3], 0, "x=2 clipped → transparent");
        assert_eq!(out[3 * 4 + 3], 0, "x=3 clipped → transparent");
    }

    #[test]
    fn scanline_sequence_has_6_to_10_steps() {
        use rand::SeedableRng;
        for seed in 0..20u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let seq = random_scanline_sequence(&mut rng, 44);
            assert!(
                seq.len() >= 6 && seq.len() <= 10,
                "seed {seed} produced {} steps", seq.len()
            );
            for (i, step) in seq.iter().enumerate() {
                assert!(step.ticks >= 1 && step.ticks <= 2,
                    "seed {seed} step {i}: ticks={}", step.ticks);
            }
        }
    }

    #[test]
    fn scanline_sequence_bands_within_bounds_and_shift_within_max() {
        use rand::SeedableRng;
        for seed in 0..20u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let seq = random_scanline_sequence(&mut rng, 44);
            for (si, step) in seq.iter().enumerate() {
                assert!(step.bands.len() >= 2 && step.bands.len() <= 3,
                    "seed {seed} step {si}: {} bands", step.bands.len());
                for band in &step.bands {
                    let end = band.y0 as u32 + band.height as u32;
                    assert!(end <= 44,
                        "seed {seed}: band y0={} h={} extends past height", band.y0, band.height);
                    let abs_dx = band.dx.unsigned_abs();
                    assert!(abs_dx >= 1 && abs_dx <= GLITCH_SHIFT_MAX as u8,
                        "seed {seed}: dx={} outside ±{GLITCH_SHIFT_MAX}", band.dx);
                }
            }
        }
    }
}
