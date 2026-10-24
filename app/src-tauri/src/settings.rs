use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub hotkey: String,
    pub first_run_complete: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".into(),
            first_run_complete: false,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct ModelSnapshot {
    id: &'static str,
    status: &'static str,
    progress: u8,
}
#[derive(Debug, Clone, Serialize)]
pub struct SettingsSnapshot {
    platform: &'static str,
    hotkey: String,
    first_run_complete: bool,
    model: ModelSnapshot,
    cloud_enabled: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct PermissionSnapshot {
    pub accessibility: Option<bool>,
    pub screen_recording: Option<bool>,
}
impl From<offhand_platform::Permissions> for PermissionSnapshot {
    fn from(value: offhand_platform::Permissions) -> Self {
        Self {
            accessibility: value.accessibility,
            screen_recording: value.screen_recording,
        }
    }
}
impl Preferences {
    fn path(app: &AppHandle) -> Result<PathBuf, AppError> {
        Ok(app
            .path()
            .app_config_dir()
            .map_err(AppError::storage)?
            .join("preferences.json"))
    }
    pub fn load(app: &AppHandle) -> Result<Self, AppError> {
        let path = Self::path(app)?;
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(AppError::storage),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(AppError::storage(e)),
        }
    }
    pub fn save(&self, app: &AppHandle) -> Result<(), AppError> {
        let path = Self::path(app)?;
        let parent = path
            .parent()
            .ok_or_else(|| AppError::storage("missing config directory"))?;
        fs::create_dir_all(parent).map_err(AppError::storage)?;
        let bytes = serde_json::to_vec(self).map_err(AppError::storage)?;
        fs::write(path, bytes).map_err(AppError::storage)
    }
    pub fn snapshot(&self) -> SettingsSnapshot {
        SettingsSnapshot {
            platform: if cfg!(target_os = "macos") {
                "macos"
            } else if cfg!(target_os = "windows") {
                "windows"
            } else {
                "unsupported"
            },
            hotkey: self.hotkey.clone(),
            first_run_complete: self.first_run_complete,
            model: ModelSnapshot {
                id: "qwen3.5-4b",
                status: "unavailable",
                progress: 0,
            },
            cloud_enabled: false,
        }
    }
}
