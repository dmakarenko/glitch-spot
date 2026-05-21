// ABOUTME: Global-hotkey registration. Default Cmd+Shift+G triggers overlay capture.
// ABOUTME: Configurable per-user in a later slice.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn default_hotkey() -> Shortcut {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyG)
}

pub fn register(app: &AppHandle) -> anyhow::Result<()> {
    let hotkey = default_hotkey();
    app.global_shortcut()
        .on_shortcut(hotkey, |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::trigger_overlay(&handle).await {
                    eprintln!("trigger_overlay failed: {e}");
                }
            });
        })?;
    Ok(())
}
