use offhand_core::ConfirmationDecision;
use offhand_platform::Permission;
use serde::Deserialize;
use tauri::{ipc::{InvokeBody, Request}, State, WebviewWindow};

use crate::{actor::{Bridge, Control}, settings::{PermissionSnapshot, SettingsSnapshot}, AppError};

fn authorize(window: &WebviewWindow, request: &Request<'_>, labels: &[&str], fields: &[&str]) -> Result<(), AppError> {
    if !labels.contains(&window.label()) { return Err(AppError::with("forbidden","This window cannot use that command")); }
    match request.body() {
        InvokeBody::Json(value) => {
            if let Some(object) = value.as_object() {
                if object.keys().any(|key| !fields.contains(&key.as_str())) { return Err(AppError::input("Unknown command argument")); }
                Ok(())
            } else { Err(AppError::input("Arguments must be an object")) }
        }
        InvokeBody::Raw(_) => Err(AppError::input("Binary arguments are not accepted")),
    }
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_submit(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>, command: String) -> Result<(), AppError> {
    authorize(&window,&request,&["command"],&["command"])?;
    bridge.ask(move |reply| Control::Submit(command,reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_stop(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>) -> Result<(), AppError> {
    authorize(&window,&request,&["command"],&[])?;
    bridge.ask(|reply| Control::Stop(Some(reply))).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_confirm(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>, confirmation_id: u64, decision: ConfirmationDecision) -> Result<(), AppError> {
    authorize(&window,&request,&["confirm"],&["confirmation_id","decision"])?;
    bridge.ask(move |reply| Control::Confirm(confirmation_id,decision,reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_settings(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>) -> Result<SettingsSnapshot, AppError> {
    authorize(&window,&request,&["settings","first-run"],&[])?;
    bridge.ask(Control::Settings).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_set_hotkey(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>, hotkey: String) -> Result<SettingsSnapshot, AppError> {
    authorize(&window,&request,&["settings","first-run"],&["hotkey"])?;
    bridge.ask(move |reply| Control::Hotkey(hotkey,reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_permissions(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>) -> Result<PermissionSnapshot, AppError> {
    authorize(&window,&request,&["settings","first-run"],&[])?;
    bridge.ask(Control::Permissions).await
}
#[derive(Debug,Clone,Copy,Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRequest { Accessibility, ScreenRecording }
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_request_permission(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>, permission: PermissionRequest) -> Result<PermissionSnapshot, AppError> {
    authorize(&window,&request,&["settings","first-run"],&["permission"])?;
    let permission = match permission { PermissionRequest::Accessibility => Permission::Accessibility, PermissionRequest::ScreenRecording => Permission::ScreenRecording };
    bridge.ask(move |reply| Control::RequestPermission(permission,reply)).await
}
#[tauri::command(rename_all = "snake_case")]
pub async fn offhand_complete_setup(window: WebviewWindow, request: Request<'_>, bridge: State<'_,Bridge>) -> Result<(), AppError> {
    authorize(&window,&request,&["first-run"],&[])?;
    bridge.ask(Control::CompleteSetup).await
}
