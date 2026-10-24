//! Desktop shell: the actor owns native objects; webviews send typed intent only.

mod actor;
mod commands;
mod desktop;
mod settings;

use serde::Serialize;
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}
impl AppError {
    fn with(code: &'static str, message: impl std::fmt::Display) -> Self {
        Self {
            code,
            message: message.to_string(),
        }
    }
    pub fn actor(e: impl std::fmt::Display) -> Self {
        Self::with("actor_unavailable", e)
    }
    pub fn core(e: impl std::fmt::Display) -> Self {
        Self::with("agent_error", e)
    }
    pub fn storage(e: impl std::fmt::Display) -> Self {
        Self::with("storage_error", e)
    }
    pub fn shortcut(e: impl std::fmt::Display) -> Self {
        Self::with("shortcut_error", e)
    }
    pub fn input(e: impl std::fmt::Display) -> Self {
        Self::with("invalid_input", e)
    }
}

/// Start the desktop shell and retain its tray-owned lifetime.
///
/// # Panics
/// Panics if the operating system cannot start Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::offhand_submit,
            commands::offhand_stop,
            commands::offhand_confirm,
            commands::offhand_settings,
            commands::offhand_set_hotkey,
            commands::offhand_permissions,
            commands::offhand_request_permission,
            commands::offhand_complete_setup,
        ])
        .setup(|app| {
            let (tx, rx) = std::sync::mpsc::channel();
            let bridge = actor::Bridge(tx);
            app.manage(bridge.clone());
            desktop::create_windows(app.handle())?;
            desktop::tray(app.handle(), &offhand_core::UiState::Idle, &[], "Alt+Space")?;
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            actor::start(app.handle().clone(), rx, bridge);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while starting Offhand");
    app.run(|_, event| {
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
