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

// Cyan from the glitch chromatic palette — see DESIGN.md.
const CYAN_RGB: (u8, u8, u8) = (0, 245, 255);
// Red from the glitch chromatic palette.
const RED_RGB: (u8, u8, u8) = (255, 0, 60);

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

/// 12 frames forming a 2.4s cosine pulse (alpha 30% → 100% → 30%).
pub(crate) fn build_cyan_frames(source: &[u8]) -> Vec<Vec<u8>> {
    const N: usize = 12;
    const BASE: f32 = 0.65;
    const AMP: f32 = 0.35;
    (0..N).map(|i| {
        let t = i as f32 / N as f32;
        let alpha = BASE + AMP * (2.0 * std::f32::consts::PI * t).cos();
        tint_frame(source, CYAN_RGB, alpha)
    }).collect()
}

/// 2 frames — full opacity and 20% opacity — for a snappy on/off blink.
pub(crate) fn build_red_frames(source: &[u8]) -> Vec<Vec<u8>> {
    vec![
        tint_frame(source, RED_RGB, 1.0),
        tint_frame(source, RED_RGB, 0.20),
    ]
}

pub(crate) struct AnimationFrames {
    pub cyan: Vec<Vec<u8>>,
    pub red: Vec<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

pub(crate) fn mcp_status_label(healthy: bool, port: u16) -> String {
    let indicator = if healthy { "🟢" } else { "🔴" };
    format!("{indicator} MCP Server :{port}")
}

fn mcp_is_healthy(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;
    let addr: SocketAddr = match format!("127.0.0.1:{port}").parse() {
        Ok(a) => a,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok()
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

    let port = state.settings.mcp_port();
    let mcp_status = MenuItemBuilder::new(mcp_status_label(mcp_is_healthy(port), port))
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
    Some(AnimationFrames {
        cyan: build_cyan_frames(&source),
        red: build_red_frames(&source),
        width,
        height,
    })
}

pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    use tauri::image::Image;

    let frames = build_animation_frames();
    let menu = build_menu(app)?;

    // Initial icon: cyan frame 0 if frames decoded, otherwise the raw black PNG
    // (which would have been rendered via icon_as_template before this change —
    // here it just shows up as black, only used in the failure fallback).
    let initial_icon: Image<'_> = match &frames {
        Some(f) => Image::new(&f.cyan[0], f.width, f.height),
        None => Image::from_bytes(include_bytes!("../icons/tray.png"))?,
    };

    let tray = TrayIconBuilder::new()
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
                "latest_snapshot" => {
                    let state = app.state::<crate::AppState>();
                    if let Some(snap) = state.snapshot_store.get_latest() {
                        if let Ok(json) = serde_json::to_string_pretty(&snap) {
                            let _ = app.clipboard().write_text(json);
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
                "provider:cloudflared" | "provider:ngrok" | "provider:tailscale" => {
                    let kind = event.id.as_ref().split(':').nth(1).unwrap_or("cloudflared").to_string();
                    let app2 = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app2.state::<crate::AppState>();
                        if let Err(e) = crate::do_set_tunnel_provider(&state, kind).await {
                            eprintln!("tray: set provider failed: {e}");
                        }
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
                    use tauri_plugin_notification::NotificationExt;
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
        let port = {
            let state = app.state::<crate::AppState>();
            state.settings.mcp_port()
        };
        tauri::async_runtime::spawn(run_tray_animation(tray_for_anim, frames, port));
    }

    Ok(tray)
}

async fn run_tray_animation(
    tray: tauri::tray::TrayIcon,
    frames: AnimationFrames,
    port: u16,
) {
    use tauri::image::Image;
    use tokio::time::{interval, Duration};

    let mut anim_tick = interval(Duration::from_millis(100));
    let mut health_tick = interval(Duration::from_secs(3));
    let mut healthy = mcp_is_healthy(port);
    let mut counter: u64 = 0;

    loop {
        tokio::select! {
            _ = anim_tick.tick() => {
                let bytes: &[u8] = if healthy {
                    // 12 cyan frames, advance every 2 ticks → 200ms/frame, 2.4s cycle.
                    let idx = ((counter / 2) % frames.cyan.len() as u64) as usize;
                    &frames.cyan[idx]
                } else {
                    // 2 red frames, advance every tick → 200ms on/off cycle.
                    let idx = (counter % frames.red.len() as u64) as usize;
                    &frames.red[idx]
                };
                let img = Image::new(bytes, frames.width, frames.height);
                let _ = tray.set_icon(Some(img));
                counter = counter.wrapping_add(1);
            }
            _ = health_tick.tick() => {
                healthy = mcp_is_healthy(port);
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
    fn cyan_frames_produce_twelve_with_full_alpha_peak() {
        let source = vec![0, 0, 0, 255];
        let frames = build_cyan_frames(&source);
        assert_eq!(frames.len(), 12);
        // Frame 0 starts at the cosine peak → alpha factor = BASE + AMP * 1.0 = 1.0.
        assert_eq!(frames[0][3], 255, "frame 0 is fully opaque");
        // Frame 6 (half cycle) → cos(π) = -1.0 → alpha factor = BASE - AMP = 0.30.
        let expected_min = (255.0 * 0.30) as u8;
        assert_eq!(frames[6][3], expected_min, "frame 6 is the dimmest");
    }

    #[test]
    fn red_frames_produce_two_distinct_alphas() {
        let source = vec![0, 0, 0, 255];
        let frames = build_red_frames(&source);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0][3], 255, "on frame is fully opaque");
        assert_eq!(frames[1][3], (255.0 * 0.20) as u8, "off frame is dim");
        assert_eq!(&frames[0][0..3], &[255, 0, 60]);
    }
}
