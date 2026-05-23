// ABOUTME: Global-hotkey registration. Ctrl+Opt+Cmd+G triggers a live overlay,
// ABOUTME: Ctrl+Opt+Cmd+H captures a frozen screenshot and displays it.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const TRIGGER_MODS: Modifiers = Modifiers::CONTROL
    .union(Modifiers::ALT)
    .union(Modifiers::SUPER);

pub fn live_hotkey() -> Shortcut {
    Shortcut::new(Some(TRIGGER_MODS), Code::KeyG)
}

pub fn frozen_hotkey() -> Shortcut {
    Shortcut::new(Some(TRIGGER_MODS), Code::KeyH)
}

pub fn register(app: &AppHandle) -> anyhow::Result<()> {
    let ext = app.global_shortcut();

    ext.on_shortcut(live_hotkey(), |app, _shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::trigger_overlay_live(&handle).await {
                eprintln!("trigger_overlay_live failed: {e}");
            }
        });
    })?;

    ext.on_shortcut(frozen_hotkey(), |app, _shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::trigger_overlay_frozen(&handle).await {
                eprintln!("trigger_overlay_frozen failed: {e}");
            }
        });
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::{Code, Modifiers};

    #[test]
    fn live_hotkey_uses_ctrl_opt_cmd_g() {
        let hk = live_hotkey();
        assert_eq!(hk.key, Code::KeyG);
        assert!(hk.mods.contains(Modifiers::CONTROL));
        assert!(hk.mods.contains(Modifiers::ALT));
        assert!(hk.mods.contains(Modifiers::SUPER));
    }

    #[test]
    fn frozen_hotkey_uses_ctrl_opt_cmd_h() {
        let hk = frozen_hotkey();
        assert_eq!(hk.key, Code::KeyH);
        assert!(hk.mods.contains(Modifiers::CONTROL));
        assert!(hk.mods.contains(Modifiers::ALT));
        assert!(hk.mods.contains(Modifiers::SUPER));
    }

    #[test]
    fn live_and_frozen_hotkeys_are_distinct() {
        assert_ne!(live_hotkey().key, frozen_hotkey().key);
    }
}
