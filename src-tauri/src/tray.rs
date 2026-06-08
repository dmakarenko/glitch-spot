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

// Mono tint base RGB — same as the bundled monochrome PNG. The Mono modes
// render with macOS's template-image flag, so this RGB never reaches the
// pixel: the system tints the alpha mask to the menu-bar foreground colour
// (white on dark, black on light). The constant just feeds `tint_frame`
// for the alpha modulation we still want during the pulse + bursts.
const MONO_RGB: (u8, u8, u8) = (0, 0, 0);

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

/// Number of frames in the pulse cycle. Combined with `ANIM_TICK_MS`,
/// determines cycle length: 24 × 100ms = 2.4s gentle pulse.
pub(crate) const PULSE_FRAME_COUNT: usize = 24;

/// 24 frames forming a 2.4s cosine pulse (alpha 30% → 100% → 30%) at the
/// given tint colour. Used for both cyan (coloured mode) and mono tints.
pub(crate) fn build_pulse_frames(source: &[u8], rgb: (u8, u8, u8)) -> Vec<Vec<u8>> {
    const BASE: f32 = 0.65;
    const AMP: f32 = 0.35;
    (0..PULSE_FRAME_COUNT).map(|i| {
        let t = i as f32 / PULSE_FRAME_COUNT as f32;
        let alpha = BASE + AMP * (2.0 * std::f32::consts::PI * t).cos();
        tint_frame(source, rgb, alpha)
    }).collect()
}

/// One of the two icon tints the user can pick from the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tint {
    /// Cyan when healthy, red when unhealthy — the at-a-glance status colours.
    Coloured,
    /// Rendered as a macOS template image — the system tints the alpha mask
    /// to match the menu-bar foreground (white on dark, black on light), so
    /// it blends in with system menu-bar icons. Health is not encoded in the
    /// icon when this is selected — it only shows in the MCP menu row.
    Mono,
}

/// All pre-built frames for a single tint, computed once at startup.
pub(crate) struct TintAssets {
    /// Cosine pulse cycle, used during animated-healthy state.
    pub pulse: Vec<Vec<u8>>,
    /// Constant-alpha frame held between scanline bursts during animated-
    /// unhealthy state. Slightly dimmer than the burst peak.
    pub steady_unhealthy: Vec<u8>,
    /// Brighter base fed into scanline-shift during animated-healthy bursts,
    /// so displaced pixels stay tinted rather than reverting to raw black.
    pub burst_base_healthy: Vec<u8>,
    /// Brighter base fed into scanline-shift during animated-unhealthy bursts.
    pub burst_base_unhealthy: Vec<u8>,
    /// Single frame held while static mode + healthy.
    pub static_healthy: Vec<u8>,
    /// Single frame held while static mode + unhealthy. For the Mono tint
    /// this is the same as `static_healthy` since Mono ignores health.
    pub static_unhealthy: Vec<u8>,
}

pub(crate) struct AnimationFrames {
    pub coloured: TintAssets,
    pub mono: TintAssets,
    pub width: u32,
    pub height: u32,
}

impl AnimationFrames {
    pub(crate) fn assets(&self, tint: Tint) -> &TintAssets {
        match tint {
            Tint::Coloured => &self.coloured,
            Tint::Mono => &self.mono,
        }
    }
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

pub(crate) fn share_status_label(healthy: bool, port: u16) -> String {
    let indicator = if healthy { "🟢" } else { "🔴" };
    format!("{indicator} Share Server :{port}")
}

pub(crate) fn release_version_label() -> String {
    format!("Version: {} ({})", env!("CARGO_PKG_VERSION"), env!("GIT_SHA"))
}

/// Read a server's shared `bound` flag, toggled around its bind+serve loop.
/// True ⇒ *this* process owns the listener (not just "some process is on the
/// port"), which is what we want to communicate to the user. Used for both the
/// MCP and share servers.
fn is_healthy(bound: &std::sync::atomic::AtomicBool) -> bool {
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

/// Max characters of an error reason shown inline in the tray menu. The full
/// text stays available by clicking the item, which copies it to the clipboard.
const MAX_ERROR_REASON_CHARS: usize = 30;

/// Builds the "Tunnel: …" status-line label. Long error reasons are collapsed
/// to a single line and truncated; the full reason is recovered by clicking the
/// item (see the `tunnel_status` menu-event handler).
pub(crate) fn tunnel_status_label(state: &crate::share::tunnel::TunnelState) -> String {
    use crate::share::tunnel::TunnelState;
    match state {
        TunnelState::Idle => "Tunnel: idle".to_string(),
        TunnelState::Starting => "Tunnel: starting…".to_string(),
        TunnelState::Live { url } => {
            let host = url.trim_start_matches("https://").trim_start_matches("http://");
            format!("Tunnel: {host}")
        }
        TunnelState::Error { reason } => {
            let one_line = reason.split_whitespace().collect::<Vec<_>>().join(" ");
            let short = truncate_chars(&one_line, MAX_ERROR_REASON_CHARS);
            format!("Tunnel: error — {short} (click to copy)")
        }
    }
}

/// Submenu title that surfaces the configured provider in the main menu without
/// unfolding, e.g. "Tunnel: ngrok". The macOS submenu disclosure arrow is added
/// by the system.
pub(crate) fn tunnel_provider_label(kind: crate::settings::ProviderKind) -> String {
    format!("Tunnel: {}", kind.display_name())
}

/// Truncates to at most `max` characters on a char boundary, appending an
/// ellipsis when truncation occurred.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}…")
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
    let mcp_status = MenuItemBuilder::new(mcp_status_label(is_healthy(&state.mcp_bound), port))
        .id("mcp_status")
        .enabled(false)
        .build(app)?;

    let share_status = MenuItemBuilder::new(share_status_label(
        is_healthy(&state.share_bound),
        crate::share::router::SHARE_PORT,
    ))
        .id("share_status")
        .enabled(false)
        .build(app)?;

    let latest_snapshot = MenuItemBuilder::new(&snapshot_label)
        .id("latest_snapshot")
        .build(app)?;

    let has_snapshot = state.snapshot_store.get_latest().is_some();

    let show_in_finder = MenuItemBuilder::new("Show in Finder")
        .id("show_in_finder")
        .build(app)?;
    let tunnel_state = state.tunnel.current_state();
    let tunnel_state_label = tunnel_status_label(&tunnel_state);
    let tunnel_is_error =
        matches!(tunnel_state, crate::share::tunnel::TunnelState::Error { .. });
    // The status line only appears when the tunnel is active; when idle, the
    // provider submenu ("Tunnel: <provider>") already conveys everything.
    let show_status_line =
        !matches!(tunnel_state, crate::share::tunnel::TunnelState::Idle);
    let share_disabled = !has_snapshot
        || matches!(tunnel_state, crate::share::tunnel::TunnelState::Starting);

    let share_latest_item = MenuItemBuilder::new("Share latest snapshot (copy link)")
        .id("share_latest")
        .enabled(!share_disabled)
        .build(app)?;

    // The status line is a passive label except in Error state, where it
    // becomes clickable to copy the full (untruncated) error to the clipboard.
    let tunnel_status_item = MenuItemBuilder::new(&tunnel_state_label)
        .id("tunnel_status")
        .enabled(tunnel_is_error)
        .build(app)?;

    let current = state.settings.current_provider();
    let provider_cf = CheckMenuItemBuilder::new(crate::settings::ProviderKind::Cloudflared.display_name())
        .id("provider:cloudflared")
        .checked(matches!(current, crate::settings::ProviderKind::Cloudflared))
        .build(app)?;
    let provider_ng = CheckMenuItemBuilder::new(crate::settings::ProviderKind::Ngrok.display_name())
        .id("provider:ngrok")
        .checked(matches!(current, crate::settings::ProviderKind::Ngrok))
        .build(app)?;
    let provider_ts = CheckMenuItemBuilder::new(crate::settings::ProviderKind::Tailscale.display_name())
        .id("provider:tailscale")
        .checked(matches!(current, crate::settings::ProviderKind::Tailscale))
        .build(app)?;
    let provider_submenu = SubmenuBuilder::new(app, tunnel_provider_label(current))
        .item(&provider_cf)
        .item(&provider_ng)
        .item(&provider_ts)
        .build()?;

    let (current_animated, current_coloured) = state.settings.current_tray();
    let tray_ca = CheckMenuItemBuilder::new("Coloured + Animated")
        .id("tray:coloured-animated")
        .checked(current_coloured && current_animated)
        .build(app)?;
    let tray_cs = CheckMenuItemBuilder::new("Coloured + Static")
        .id("tray:coloured-static")
        .checked(current_coloured && !current_animated)
        .build(app)?;
    let tray_ba = CheckMenuItemBuilder::new("Mono + Animated")
        .id("tray:mono-animated")
        .checked(!current_coloured && current_animated)
        .build(app)?;
    let tray_bs = CheckMenuItemBuilder::new("Mono + Static")
        .id("tray:mono-static")
        .checked(!current_coloured && !current_animated)
        .build(app)?;
    let tray_submenu = SubmenuBuilder::new(app, "Tray icon")
        .item(&tray_ca)
        .item(&tray_cs)
        .item(&tray_ba)
        .item(&tray_bs)
        .build()?;

    let current_retention = state.settings.current_retention();
    let ret_24h = CheckMenuItemBuilder::new("Keep 24 hours")
        .id("retention:24h")
        .checked(matches!(current_retention, crate::settings::SnapshotRetention::Hours24))
        .build(app)?;
    let ret_7d = CheckMenuItemBuilder::new("Keep 7 days")
        .id("retention:7d")
        .checked(matches!(current_retention, crate::settings::SnapshotRetention::Days7))
        .build(app)?;
    let ret_30d = CheckMenuItemBuilder::new("Keep 30 days")
        .id("retention:30d")
        .checked(matches!(current_retention, crate::settings::SnapshotRetention::Days30))
        .build(app)?;
    let ret_never = CheckMenuItemBuilder::new("Never auto-delete")
        .id("retention:never")
        .checked(matches!(current_retention, crate::settings::SnapshotRetention::Never))
        .build(app)?;
    let retention_submenu = SubmenuBuilder::new(app, "Auto-delete snapshots")
        .item(&ret_24h)
        .item(&ret_7d)
        .item(&ret_30d)
        .item(&ret_never)
        .build()?;

    let current_redaction = state.settings.current_redaction_style();
    let redact_solid = CheckMenuItemBuilder::new("Solid fill")
        .id("redaction:solid")
        .checked(matches!(current_redaction, crate::settings::RedactionStyle::Solid))
        .build(app)?;
    let redact_pixelated = CheckMenuItemBuilder::new("Pixelated")
        .id("redaction:pixelated")
        .checked(matches!(current_redaction, crate::settings::RedactionStyle::Pixelated))
        .build(app)?;
    let redact_branded = CheckMenuItemBuilder::new("Mosaic")
        .id("redaction:branded")
        .checked(matches!(current_redaction, crate::settings::RedactionStyle::Branded))
        .build(app)?;
    let redaction_submenu = SubmenuBuilder::new(app, "Redaction style")
        .item(&redact_solid)
        .item(&redact_pixelated)
        .item(&redact_branded)
        .build()?;

    let launch_at_login = CheckMenuItemBuilder::new("Launch at Login")
        .id("launch_at_login")
        .checked(autostart_on)
        .build(app)?;

    let quit = MenuItemBuilder::new("Quit Glitch Spot")
        .id("quit")
        .build(app)?;

    #[cfg(debug_assertions)]
    let build_branch = MenuItemBuilder::new(format!("branch: {}", env!("GIT_BRANCH")))
        .id("build_branch")
        .enabled(false)
        .build(app)?;

    #[cfg(debug_assertions)]
    let build_commit = MenuItemBuilder::new(format!("commit: {}", env!("GIT_SHA")))
        .id("build_commit")
        .enabled(false)
        .build(app)?;

    #[cfg(debug_assertions)]
    let build_built_at = MenuItemBuilder::new(
        format!("started: {}", state.started_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S %Z"))
    )
    .id("build_built_at")
    .enabled(false)
    .build(app)?;

    #[cfg(not(debug_assertions))]
    let build_version = MenuItemBuilder::new(release_version_label())
        .id("build_version")
        .enabled(false)
        .build(app)?;

    let menu_builder = MenuBuilder::new(app)
        .item(&capture_live)
        .item(&capture_frozen)
        .item(&capture_window)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&mcp_status)
        .item(&share_status)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&latest_snapshot);

    // "Show in Finder" only appears once a snapshot exists to reveal.
    let menu_builder = if has_snapshot {
        menu_builder.item(&show_in_finder)
    } else {
        menu_builder
    };

    let menu_builder = menu_builder.item(&share_latest_item);

    let menu_builder = if show_status_line {
        menu_builder.item(&tunnel_status_item)
    } else {
        menu_builder
    };

    let menu_builder = menu_builder
        .item(&provider_submenu)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&tray_submenu)
        .item(&retention_submenu)
        .item(&redaction_submenu)
        .item(&launch_at_login)
        .item(&PredefinedMenuItem::separator(app)?);

    #[cfg(debug_assertions)]
    let menu_builder = menu_builder
        .item(&build_branch)
        .item(&build_commit)
        .item(&build_built_at);

    #[cfg(not(debug_assertions))]
    let menu_builder = menu_builder.item(&build_version);

    menu_builder
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&quit)
        .build()
}

/// Decode the bundled monochrome tray PNG into raw RGBA + dimensions, then
/// pre-compute every animated/static frame for both tints. Done once at
/// startup; the animation task just looks frames up by `(tint, phase)`.
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

    let coloured = TintAssets {
        pulse: build_pulse_frames(&source, CYAN_RGB),
        steady_unhealthy: tint_frame(&source, RED_RGB, 0.65),
        burst_base_healthy: tint_frame(&source, CYAN_RGB, 0.80),
        burst_base_unhealthy: tint_frame(&source, RED_RGB, 0.80),
        static_healthy: tint_frame(&source, CYAN_RGB, 1.0),
        // 0.85 not 1.0 — full-alpha red on a 22px icon reads as alarmist
        // even when the rest of the system is fine.
        static_unhealthy: tint_frame(&source, RED_RGB, 0.85),
    };

    // Mono tint: the source PNG is already RGB=(0,0,0), so `tint_frame` with
    // MONO_RGB only modulates alpha. macOS treats these frames as template
    // images (see set_icon_as_template wiring) and tints them to the menu
    // bar's foreground colour, so the RGB doesn't actually reach the pixel.
    // Static frames just use the raw source — Mono is health-agnostic.
    let mono = TintAssets {
        pulse: build_pulse_frames(&source, MONO_RGB),
        steady_unhealthy: tint_frame(&source, MONO_RGB, 0.65),
        burst_base_healthy: tint_frame(&source, MONO_RGB, 0.80),
        burst_base_unhealthy: tint_frame(&source, MONO_RGB, 0.80),
        static_healthy: source.clone(),
        static_unhealthy: source.clone(),
    };

    Some(AnimationFrames { coloured, mono, width, height })
}

pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    use tauri::image::Image;

    let frames = build_animation_frames();
    // Read health state before build_menu so we know what the menu was built
    // with; passed to the animation task to avoid a spurious refresh at the
    // first health_tick when nothing actually changed.
    let (mcp_bound, tray_animated_flag, tray_coloured_flag) = {
        let state = app.state::<crate::AppState>();
        (
            state.mcp_bound.clone(),
            state.settings.tray_animated_flag(),
            state.settings.tray_coloured_flag(),
        )
    };
    let menu_built_healthy = is_healthy(&mcp_bound);
    let menu = build_menu(app)?;

    // Initial icon matches the first frame the animation loop will draw at
    // counter=0 — read the user's saved tray appearance so we don't flash a
    // colour they switched off. Falls back to the raw PNG on decode failure.
    let (initial_animated, initial_coloured) = {
        let state = app.state::<crate::AppState>();
        state.settings.current_tray()
    };
    let initial_tint = if initial_coloured { Tint::Coloured } else { Tint::Mono };
    let initial_icon: Image<'_> = match &frames {
        Some(f) => {
            let assets = f.assets(initial_tint);
            let bytes = if initial_animated { &assets.pulse[0] } else { &assets.static_healthy };
            Image::new(bytes, f.width, f.height)
        }
        None => Image::from_bytes(include_bytes!("../icons/tray.png"))?,
    };

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(initial_icon)
        // Mono modes render as macOS template images so the system tints
        // them to the menu bar foreground. Coloured modes ship full-RGBA
        // frames and must NOT be templates, or the colour gets stripped.
        .icon_as_template(!initial_coloured)
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
                "show_in_finder" => {
                    let state = app.state::<crate::AppState>();
                    if let Some(snap) = state.snapshot_store.get_latest() {
                        // `open -R` selects the file in its containing folder.
                        if let Err(e) = std::process::Command::new("open")
                            .arg("-R")
                            .arg(snap.reveal_target())
                            .spawn()
                        {
                            eprintln!("tray: show_in_finder failed: {e}");
                        }
                    }
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
                "tunnel_status" => {
                    // Only clickable in Error state — copy the full (untruncated)
                    // reason so the user can paste the whole thing for debugging.
                    let state = app.state::<crate::AppState>();
                    if let crate::share::tunnel::TunnelState::Error { reason } =
                        state.tunnel.current_state()
                    {
                        let body = match app.clipboard().write_text(reason) {
                            Ok(()) => "Tunnel error copied to clipboard.".to_string(),
                            Err(e) => format!("Couldn't copy to clipboard: {e}"),
                        };
                        let _ = app
                            .notification()
                            .builder()
                            .title("Glitch Spot")
                            .body(body)
                            .show();
                    }
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
                "tray:coloured-animated"
                | "tray:coloured-static"
                | "tray:mono-animated"
                | "tray:mono-static" => {
                    let id = event.id.as_ref();
                    let coloured = id.starts_with("tray:coloured");
                    let animated = id.ends_with("-animated");
                    let state = app.state::<crate::AppState>();
                    if let Err(e) = crate::do_set_tray_appearance(&state, animated, coloured) {
                        eprintln!("tray: set tray appearance failed: {e}");
                    }
                    refresh_menu(&app);
                }
                "retention:24h"
                | "retention:7d"
                | "retention:30d"
                | "retention:never" => {
                    let key = event.id.as_ref().split(':').nth(1).unwrap_or("24h").to_string();
                    let app2 = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app2.state::<crate::AppState>();
                        if let Err(e) = crate::do_set_snapshot_retention(&state, key).await {
                            eprintln!("tray: set retention failed: {e}");
                        }
                        refresh_menu(&app2);
                    });
                }
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
        tauri::async_runtime::spawn(run_tray_animation(
            tray_for_anim,
            frames,
            mcp_bound,
            tray_animated_flag,
            tray_coloured_flag,
            app.clone(),
            menu_built_healthy,
        ));
    }

    Ok(tray)
}

const ANIM_TICK_MS: u64 = 100;
/// Min/max ticks between glitch bursts during healthy state.
/// At 100ms/tick: 32-58 ticks ≈ 3.2-5.8s, centred around 4.5s.
const GLITCH_GAP_MIN: u64 = 32;
const GLITCH_GAP_MAX: u64 = 58;
/// Min/max ticks between scanline bursts during unhealthy state.
/// At 100ms/tick: 21-42 ticks ≈ 2.1-4.2s — more agitated than healthy.
const RED_GAP_MIN: u64 = 21;
const RED_GAP_MAX: u64 = 42;

enum AnimState {
    Pulsing,
    Glitching {
        sequence: Vec<ScanlineStep>,
        step_idx: usize,
        ticks_in_step: u8,
    },
}

/// Which burst sequence a step belongs to — needed in the key because a
/// healthy and an unhealthy burst at the same `step_idx` paint different
/// frames and must dedup as distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BurstSide {
    Healthy,
    Unhealthy,
}

/// The animation phase the tick wants to draw. Independent of tint so the
/// same phase can be combined with any tint to produce the final image.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    /// Static mode, single frame. The bool distinguishes the healthy and
    /// unhealthy static frames in the Coloured tint; in the Mono tint the
    /// caller collapses both to `Static(true)` because Mono ignores health
    /// — that way the dedup doesn't redraw on health flips.
    Static(bool),
    /// Animated healthy: cosine pulse, current frame index.
    Pulse(usize),
    /// Animated unhealthy: constant frame held between scanline bursts.
    SteadyUnhealthy,
    /// Animated burst step — the actual band-shift pattern is drawn from
    /// the current `AnimState::Glitching` payload, but only the side +
    /// step_idx are in the dedup key (the shift pattern is deterministic
    /// for that step).
    Burst { side: BurstSide, step_idx: usize },
}

/// Stable key identifying which icon image a tick wants to draw. Used to skip
/// `set_icon` (and the `apply_scanline_shift` allocation that feeds it) when
/// the previous tick already drew the same content — biggest win is during
/// the unhealthy steady-red interval and during Static modes where the icon
/// is constant for as long as the user leaves the mode set.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DrawKey {
    tint: Tint,
    phase: Phase,
}

/// Pick the icon to draw this tick. Pure function — does no allocation and
/// does not touch the tray. The animation loop calls this first and only
/// performs the expensive `apply_scanline_shift` + `set_icon` when the key
/// differs from the previous tick.
fn select_draw_key(
    animated: bool,
    coloured: bool,
    healthy: bool,
    state: &AnimState,
    red_state: &AnimState,
    counter: u64,
    pulse_len: usize,
) -> DrawKey {
    let tint = if coloured { Tint::Coloured } else { Tint::Mono };
    let phase = if !animated {
        // Mono tint paints the same image for healthy/unhealthy, so collapse
        // the key — otherwise a health flip would force a no-op set_icon.
        let static_healthy = if coloured { healthy } else { true };
        Phase::Static(static_healthy)
    } else if !healthy {
        match red_state {
            AnimState::Pulsing => Phase::SteadyUnhealthy,
            AnimState::Glitching { step_idx, .. } => Phase::Burst {
                side: BurstSide::Unhealthy,
                step_idx: *step_idx,
            },
        }
    } else {
        match state {
            AnimState::Pulsing => Phase::Pulse((counter % pulse_len as u64) as usize),
            AnimState::Glitching { step_idx, .. } => Phase::Burst {
                side: BurstSide::Healthy,
                step_idx: *step_idx,
            },
        }
    };
    DrawKey { tint, phase }
}

async fn run_tray_animation(
    tray: tauri::tray::TrayIcon,
    frames: AnimationFrames,
    mcp_bound: std::sync::Arc<std::sync::atomic::AtomicBool>,
    tray_animated: std::sync::Arc<std::sync::atomic::AtomicBool>,
    tray_coloured: std::sync::Arc<std::sync::atomic::AtomicBool>,
    app: AppHandle,
    menu_built_healthy: bool,
) {
    use rand::{Rng, SeedableRng};
    use std::sync::atomic::Ordering;
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
    let mut healthy = is_healthy(&mcp_bound);
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
    // Last icon we asked the OS to render. Skipping `set_icon` when the key
    // matches eliminates ~3-4s of redundant set_icon calls every gap between
    // red bursts, every Static-mode tick once the first frame lands, plus
    // one redundant call inside any burst step held for 2 ticks.
    let mut last_drawn: Option<DrawKey> = None;
    // Detect when the user flips Animated or Coloured so we can reset the
    // burst state machines — otherwise turning animation back on could
    // resume a stale burst mid-step with garbled output.
    let mut last_mode: Option<(bool, bool)> = None;

    loop {
        tokio::select! {
            _ = anim_tick.tick() => {
                let animated = tray_animated.load(Ordering::Relaxed);
                let coloured = tray_coloured.load(Ordering::Relaxed);
                if last_mode != Some((animated, coloured)) {
                    state = AnimState::Pulsing;
                    red_state = AnimState::Pulsing;
                    next_glitch_at = counter + rng.gen_range(GLITCH_GAP_MIN..=GLITCH_GAP_MAX);
                    next_red_glitch_at = counter + rng.gen_range(RED_GAP_MIN..=RED_GAP_MAX);
                    last_mode = Some((animated, coloured));
                }

                let pulse_len = frames.assets(Tint::Coloured).pulse.len();
                let want = select_draw_key(
                    animated, coloured, healthy, &state, &red_state, counter, pulse_len,
                );
                if last_drawn.as_ref() != Some(&want) {
                    let assets = frames.assets(want.tint);
                    // Mono frames are painted as macOS template images so the
                    // system tints the alpha mask to the menu-bar foreground.
                    // Use set_icon_with_as_template — calling set_icon then
                    // set_icon_as_template separately produces the flicker
                    // Tauri's own docs warn about, because set_icon creates a
                    // fresh NSImage whose template flag defaults to false.
                    let is_template = matches!(want.tint, Tint::Mono);
                    match &want.phase {
                        Phase::Static(h) => {
                            let bytes = if *h { &assets.static_healthy } else { &assets.static_unhealthy };
                            let img = Image::new(bytes, frames.width, frames.height);
                            let _ = tray.set_icon_with_as_template(Some(img), is_template);
                        }
                        Phase::Pulse(idx) => {
                            let img = Image::new(&assets.pulse[*idx], frames.width, frames.height);
                            let _ = tray.set_icon_with_as_template(Some(img), is_template);
                        }
                        Phase::SteadyUnhealthy => {
                            let img = Image::new(&assets.steady_unhealthy, frames.width, frames.height);
                            let _ = tray.set_icon_with_as_template(Some(img), is_template);
                        }
                        Phase::Burst { side, .. } => {
                            let (base, glitch_state) = match side {
                                BurstSide::Healthy => (&assets.burst_base_healthy, &state),
                                BurstSide::Unhealthy => (&assets.burst_base_unhealthy, &red_state),
                            };
                            if let AnimState::Glitching { sequence, step_idx, .. } = glitch_state {
                                let shifted = apply_scanline_shift(
                                    base, frames.width, frames.height,
                                    &sequence[*step_idx].bands,
                                );
                                let img = Image::new(&shifted, frames.width, frames.height);
                                let _ = tray.set_icon_with_as_template(Some(img), is_template);
                            }
                        }
                    }
                    last_drawn = Some(want);
                }

                // Burst state machines only advance in Animated mode — Static
                // mode never enters Glitching so the timers would just churn
                // counters that select_draw_key ignores anyway.
                if animated {
                    if !healthy {
                        match &mut red_state {
                            AnimState::Pulsing => {
                                if counter >= next_red_glitch_at {
                                    red_state = AnimState::Glitching {
                                        sequence: random_scanline_sequence(&mut rng, frames.height),
                                        step_idx: 0,
                                        ticks_in_step: 0,
                                    };
                                }
                            }
                            AnimState::Glitching { sequence, step_idx, ticks_in_step } => {
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
                                if counter >= next_glitch_at {
                                    state = AnimState::Glitching {
                                        sequence: random_scanline_sequence(&mut rng, frames.height),
                                        step_idx: 0,
                                        ticks_in_step: 0,
                                    };
                                }
                            }
                            AnimState::Glitching { sequence, step_idx, ticks_in_step } => {
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
                }
                counter = counter.wrapping_add(1);
            }
            _ = health_tick.tick() => {
                let new_healthy = is_healthy(&mcp_bound);
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
    fn release_version_label_has_expected_format() {
        let label = release_version_label();
        assert!(label.starts_with("Version: "), "expected 'Version: ' prefix, got: {label}");
        assert!(label.contains('('), "expected '(' in label, got: {label}");
        assert!(label.ends_with(')'), "expected ')' suffix, got: {label}");
    }

    #[test]
    fn tunnel_provider_label_shows_human_name() {
        use crate::settings::ProviderKind;
        assert_eq!(tunnel_provider_label(ProviderKind::Cloudflared), "Tunnel: Cloudflare");
        assert_eq!(tunnel_provider_label(ProviderKind::Ngrok), "Tunnel: ngrok");
        assert_eq!(tunnel_provider_label(ProviderKind::Tailscale), "Tunnel: Tailscale");
    }

    #[test]
    fn tunnel_label_idle_starting_live() {
        use crate::share::tunnel::TunnelState;
        assert_eq!(tunnel_status_label(&TunnelState::Idle), "Tunnel: idle");
        assert_eq!(tunnel_status_label(&TunnelState::Starting), "Tunnel: starting…");
        assert_eq!(
            tunnel_status_label(&TunnelState::Live {
                url: "https://abc.trycloudflare.com".into()
            }),
            "Tunnel: abc.trycloudflare.com"
        );
    }

    #[test]
    fn tunnel_label_short_error_keeps_full_reason_and_adds_hint() {
        use crate::share::tunnel::TunnelState;
        let label = tunnel_status_label(&TunnelState::Error { reason: "boom".into() });
        assert_eq!(label, "Tunnel: error — boom (click to copy)");
    }

    #[test]
    fn tunnel_label_long_error_is_truncated_with_ellipsis_and_hint() {
        use crate::share::tunnel::TunnelState;
        let reason = "x".repeat(100);
        let label = tunnel_status_label(&TunnelState::Error { reason });
        assert!(label.ends_with(" (click to copy)"), "missing hint: {label}");
        assert!(label.contains('…'), "missing ellipsis: {label}");
        let xs = label.chars().filter(|&c| c == 'x').count();
        assert_eq!(
            xs, MAX_ERROR_REASON_CHARS,
            "expected reason truncated to {MAX_ERROR_REASON_CHARS} chars, got {xs}"
        );
    }

    #[test]
    fn tunnel_label_collapses_multiline_error_to_one_line() {
        use crate::share::tunnel::TunnelState;
        let label = tunnel_status_label(&TunnelState::Error {
            reason: "line one\n\tline two".into(),
        });
        assert!(!label.contains('\n'), "label should be single-line: {label}");
        assert!(label.contains("line one line two"), "whitespace not collapsed: {label}");
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
    fn share_status_healthy_renders_green() {
        assert_eq!(share_status_label(true, 7821), "🟢 Share Server :7821");
    }

    #[test]
    fn share_status_unhealthy_renders_red() {
        assert_eq!(share_status_label(false, 7821), "🔴 Share Server :7821");
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
    fn pulse_frames_produce_full_set_with_full_alpha_peak() {
        let source = vec![0, 0, 0, 255];
        let frames = build_pulse_frames(&source, CYAN_RGB);
        assert_eq!(frames.len(), PULSE_FRAME_COUNT);
        // Frame 0 starts at the cosine peak → alpha factor = BASE + AMP * 1.0 = 1.0.
        assert_eq!(frames[0][3], 255, "frame 0 is fully opaque");
        // Frame N/2 (half cycle) → cos(π) = -1.0 → alpha factor = BASE - AMP = 0.30.
        let expected_min = (255.0 * 0.30) as u8;
        assert_eq!(
            frames[PULSE_FRAME_COUNT / 2][3],
            expected_min,
            "halfway frame is the dimmest"
        );
    }

    #[test]
    fn pulse_frames_for_mono_tint_keep_rgb_at_zero() {
        // Sanity-check the mono-tint variant: alpha modulates exactly the
        // same as cyan, but RGB stays at (0,0,0) — the source colour. The
        // macOS template flag (set elsewhere) tints the alpha mask to the
        // menu-bar foreground, so the icon ends up white-on-dark, even
        // though these bytes are technically black-with-modulated-alpha.
        let source = vec![0, 0, 0, 255];
        let frames = build_pulse_frames(&source, MONO_RGB);
        assert_eq!(frames.len(), PULSE_FRAME_COUNT);
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(&frame[0..3], &[0, 0, 0], "frame {i} stays at source RGB");
        }
        // Same dimmest-frame alpha as the cyan pulse — only RGB differs.
        let expected_min = (255.0 * 0.30) as u8;
        assert_eq!(frames[PULSE_FRAME_COUNT / 2][3], expected_min);
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

    fn pulsing() -> AnimState { AnimState::Pulsing }
    fn glitching_at(step_idx: usize) -> AnimState {
        AnimState::Glitching { sequence: vec![], step_idx, ticks_in_step: 0 }
    }
    fn key(tint: Tint, phase: Phase) -> DrawKey { DrawKey { tint, phase } }

    #[test]
    fn select_draw_key_coloured_animated_healthy_advances_each_tick() {
        // During the dominant healthy-pulse state, every tick produces a fresh
        // frame index — the dedup in the loop won't skip these, by design.
        let s = pulsing();
        let r = pulsing();
        assert_eq!(
            select_draw_key(true, true, true, &s, &r, 0, 24),
            key(Tint::Coloured, Phase::Pulse(0)),
        );
        assert_eq!(
            select_draw_key(true, true, true, &s, &r, 5, 24),
            key(Tint::Coloured, Phase::Pulse(5)),
        );
        assert_eq!(
            select_draw_key(true, true, true, &s, &r, 25, 24),
            key(Tint::Coloured, Phase::Pulse(1)),
            "wraps at cycle end",
        );
    }

    #[test]
    fn select_draw_key_coloured_animated_unhealthy_is_steady_regardless_of_counter() {
        // This is where dedup pays off most in the original (pre-static) modes:
        // the key is constant for the entire 2-4s gap between red bursts.
        let s = pulsing();
        let r = pulsing();
        for c in [0u64, 1, 17, 999, u64::MAX] {
            assert_eq!(
                select_draw_key(true, true, false, &s, &r, c, 24),
                key(Tint::Coloured, Phase::SteadyUnhealthy),
            );
        }
    }

    #[test]
    fn select_draw_key_burst_keys_carry_side_and_step_index() {
        // Healthy burst — only `state` matters.
        let r = pulsing();
        assert_eq!(
            select_draw_key(true, true, true, &glitching_at(3), &r, 0, 24),
            key(Tint::Coloured, Phase::Burst { side: BurstSide::Healthy, step_idx: 3 }),
        );
        // Unhealthy burst — only `red_state` matters, and the side flips.
        let s = pulsing();
        assert_eq!(
            select_draw_key(true, true, false, &s, &glitching_at(2), 0, 24),
            key(Tint::Coloured, Phase::Burst { side: BurstSide::Unhealthy, step_idx: 2 }),
        );
    }

    #[test]
    fn select_draw_key_stable_across_a_multi_tick_burst_step() {
        // A burst step held for 2 ticks produces the same key both times —
        // proving the dedup will skip the second set_icon.
        let r = pulsing();
        let k0 = select_draw_key(true, true, true, &glitching_at(1), &r, 10, 24);
        let k1 = select_draw_key(true, true, true, &glitching_at(1), &r, 11, 24);
        assert_eq!(k0, k1);
    }

    #[test]
    fn select_draw_key_static_coloured_distinguishes_health() {
        // In static-coloured mode we still want at-a-glance status, so healthy
        // and unhealthy must map to distinct keys (cyan vs red static frame).
        let s = pulsing();
        let r = pulsing();
        let healthy = select_draw_key(false, true, true,  &s, &r, 0, 24);
        let unhealthy = select_draw_key(false, true, false, &s, &r, 0, 24);
        assert_eq!(healthy,   key(Tint::Coloured, Phase::Static(true)));
        assert_eq!(unhealthy, key(Tint::Coloured, Phase::Static(false)));
        assert_ne!(healthy, unhealthy);
    }

    #[test]
    fn select_draw_key_static_mono_collapses_health_to_one_key() {
        // The Mono + Static mode renders the source PNG regardless of
        // health, so the key must collapse — otherwise the dedup would
        // redraw the same image on every health flip.
        let s = pulsing();
        let r = pulsing();
        let healthy = select_draw_key(false, false, true,  &s, &r, 0, 24);
        let unhealthy = select_draw_key(false, false, false, &s, &r, 0, 24);
        assert_eq!(healthy, unhealthy);
        assert_eq!(healthy, key(Tint::Mono, Phase::Static(true)));
    }

    #[test]
    fn select_draw_key_mono_animated_carries_mono_tint_through_phases() {
        // Same phase progression as coloured-animated, but the tint flips
        // every key so the render path picks mono assets.
        let s = pulsing();
        let r = pulsing();
        assert_eq!(
            select_draw_key(true, false, true, &s, &r, 7, 24),
            key(Tint::Mono, Phase::Pulse(7)),
        );
        assert_eq!(
            select_draw_key(true, false, false, &s, &r, 0, 24),
            key(Tint::Mono, Phase::SteadyUnhealthy),
        );
        assert_eq!(
            select_draw_key(true, false, true, &glitching_at(4), &r, 0, 24),
            key(Tint::Mono, Phase::Burst { side: BurstSide::Healthy, step_idx: 4 }),
        );
    }
}
