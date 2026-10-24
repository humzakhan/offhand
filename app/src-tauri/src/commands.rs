use offhand_core::ConfirmationDecision;
use offhand_platform::Permission;
use serde::{Deserialize, de::DeserializeOwned};
use tauri::{
    State, WebviewWindow,
    ipc::{InvokeBody, Request},
};

use crate::{
    AppError,
    actor::{Bridge, Control},
    settings::{PermissionSnapshot, SettingsSnapshot},
};

// Decode inside the handler so malformed arguments also return the typed error shape.
fn arguments<T: DeserializeOwned>(
    window: &WebviewWindow,
    request: &Request<'_>,
    labels: &[&str],
) -> Result<T, AppError> {
    if !labels.contains(&window.label()) {
        return Err(AppError::with("forbidden", "This window cannot use that command"));
    }
    match request.body() {
        InvokeBody::Json(value) => serde_json::from_value(value.clone()).map_err(AppError::input),
        InvokeBody::Raw(_) => Err(AppError::input("Binary arguments are not accepted")),
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyArgs {}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitArgs { command: String }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmArgs { confirmation_id: u64, decision: ConfirmationDecision }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HotkeyArgs { hotkey: String }
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PermissionRequest { Accessibility, ScreenRecording }
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PermissionArgs { permission: PermissionRequest }

#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_submit(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<(), AppError> {
    let args: SubmitArgs = arguments(&window, &request, &["command"])?;
    bridge.ask(move |reply| Control::Submit(args.command, reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_stop(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<(), AppError> {
    let _: EmptyArgs = arguments(&window, &request, &["command"])?;
    bridge.ask(|reply| Control::Stop(Some(reply))).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_confirm(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<(), AppError> {
    let args: ConfirmArgs = arguments(&window, &request, &["confirm"])?;
    bridge.ask(move |reply| Control::Confirm(args.confirmation_id, args.decision, reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_settings(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<SettingsSnapshot, AppError> {
    let _: EmptyArgs = arguments(&window, &request, &["settings", "first-run"])?;
    bridge.ask(Control::Settings).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_set_hotkey(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<SettingsSnapshot, AppError> {
    let args: HotkeyArgs = arguments(&window, &request, &["settings", "first-run"])?;
    bridge.ask(move |reply| Control::Hotkey(args.hotkey, reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_permissions(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<PermissionSnapshot, AppError> {
    let _: EmptyArgs = arguments(&window, &request, &["settings", "first-run"])?;
    bridge.ask(Control::Permissions).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_request_permission(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<PermissionSnapshot, AppError> {
    let args: PermissionArgs = arguments(&window, &request, &["settings", "first-run"])?;
    let permission = match args.permission { PermissionRequest::Accessibility => Permission::Accessibility, PermissionRequest::ScreenRecording => Permission::ScreenRecording };
    bridge.ask(move |reply| Control::RequestPermission(permission, reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_complete_setup(window: WebviewWindow, request: Request<'_>, bridge: State<'_, Bridge>) -> Result<(), AppError> {
    let _: EmptyArgs = arguments(&window, &request, &["first-run"])?;
    bridge.ask(Control::CompleteSetup).await
}
