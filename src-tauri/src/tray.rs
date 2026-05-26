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

    let mcp_status = MenuItemBuilder::new("MCP Server  ● :7820")
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

pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    use tauri::image::Image;
    let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
    let menu = build_menu(app)?;

    let tray = TrayIconBuilder::new()
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

    Ok(tray)
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
}
