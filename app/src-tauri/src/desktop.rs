use offhand_core::{Target, UiState};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

use crate::{
    AppError,
    actor::{Bridge, Control},
};

#[derive(Debug)]
struct NativeMaterials(Vec<String>);

pub fn create_windows(app: &AppHandle) -> tauri::Result<()> {
    let mut materials = Vec::new();
    for (label, width, height, floating) in [
        ("command", 640.0, 96.0, true),
        ("overlay", 800.0, 600.0, true),
        ("confirm", 376.0, 230.0, true),
        ("first-run", 760.0, 480.0, false),
        ("settings", 780.0, 800.0, false),
    ] {
        let window = WebviewWindowBuilder::new(
            app,
            label,
            WebviewUrl::App(format!("index.html?surface={label}").into()),
        )
        .title(if label == "settings" {
            "Offhand Settings"
        } else {
            "Offhand"
        })
        .inner_size(width, height)
        .decorations(!floating)
        .transparent(floating)
        .shadow(label != "overlay")
        .always_on_top(floating)
        .skip_taskbar(floating)
        .resizable(!floating)
        .focused(false)
        .focusable(label != "overlay" && label != "confirm")
        .content_protected(floating)
        .visible(false)
        .build()?;
        if label == "overlay" {
            window.set_ignore_cursor_events(true)?;
        }
        if floating && label != "overlay" {
            // Native material is optional; the CSS graphite surface remains the fallback.
            #[cfg(target_os = "macos")]
            let native_material = window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![tauri::window::Effect::HudWindow],
                    radius: Some(14.0),
                    ..Default::default()
                })
                .is_ok();
            #[cfg(target_os = "windows")]
            let native_material = window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![tauri::window::Effect::Acrylic],
                    ..Default::default()
                })
                .is_ok();
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let native_material = false;
            if native_material {
                materials.push(label.to_owned());
            }
        }
        let handle = app.clone();
        let label = label.to_owned();
        window.on_window_event(move |event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if label == "confirm" || label == "command" {
                    let _ = handle.state::<Bridge>().0.send(Control::Stop(None));
                } else if let Some(w) = handle.get_webview_window(&label) {
                    let _ = w.hide();
                }
            }
            tauri::WindowEvent::Focused(false) if label == "command" => {
                let _ = handle.state::<Bridge>().0.send(Control::Blur);
            }
            _ => {}
        });
    }
    app.manage(NativeMaterials(materials));
    Ok(())
}

pub fn show_preferences(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn show_error(app: &AppHandle, error: &AppError) {
    place(app, None);
    if let Some(window) = app.get_webview_window("command") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    replay_error(app, error);
}

pub fn replay_error(app: &AppHandle, error: &AppError) {
    if let (Some(window), Ok(message)) = (
        app.get_webview_window("command"),
        serde_json::to_string(&error.message),
    ) {
        let _ = window.eval(format!(
            "window.__OFFHAND_ERROR__={message};window.dispatchEvent(new CustomEvent('offhand-error',{{detail:{message}}}))"
        ));
    }
}

pub fn hide(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.hide();
    }
}

pub fn place(app: &AppHandle, target: Option<&Target>) {
    let Ok(monitors) = app.available_monitors() else {
        return;
    };
    let cursor = app.cursor_position().ok();
    let point = target
        .map(|t| (t.bounds.x + t.bounds.w / 2.0, t.bounds.y + t.bounds.h / 2.0))
        .or_else(|| cursor.map(|p| (p.x, p.y)));
    let monitor = point
        .and_then(|(x, y)| {
            monitors.iter().find(|m| {
                x >= f64::from(m.position().x)
                    && y >= f64::from(m.position().y)
                    && x < f64::from(m.position().x) + f64::from(m.size().width)
                    && y < f64::from(m.position().y) + f64::from(m.size().height)
            })
        })
        .or_else(|| monitors.first());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let pos = monitor.position();
    let size = monitor.size();
    let work = monitor.work_area();
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.set_position(*pos);
        let _ = overlay.set_size(*size);
    }
    if let Some(command) = app.get_webview_window("command") {
        let x = f64::from(work.position.x) + (f64::from(work.size.width) - 640.0 * scale) / 2.0;
        let y = f64::from(work.position.y) + f64::from(work.size.height) / 4.0;
        let _ = command.set_position(PhysicalPosition::new(x, y));
    }
    if let Some(confirm) = app.get_webview_window("confirm") {
        let width = 376.0 * scale;
        let height = 230.0 * scale;
        let left = f64::from(work.position.x) + 12.0 * scale;
        let top = f64::from(work.position.y) + 12.0 * scale;
        let right = f64::from(work.position.x) + f64::from(work.size.width) - width - 12.0 * scale;
        let bottom =
            f64::from(work.position.y) + f64::from(work.size.height) - height - 12.0 * scale;
        let (x, y) = target.map_or((left + 24.0 * scale, top + 24.0 * scale), |t| {
            let beside = t.bounds.x + t.bounds.w + 20.0 * scale;
            let x = if beside <= right {
                beside
            } else {
                t.bounds.x - width - 20.0 * scale
            };
            (x, t.bounds.y - 22.0 * scale)
        });
        let _ = confirm.set_size(PhysicalSize::new(width, height));
        let _ = confirm.set_position(PhysicalPosition::new(
            x.clamp(left, right.max(left)),
            y.clamp(top, bottom.max(top)),
        ));
    }
}

pub fn render(app: &AppHandle, ui: &UiState) {
    let target = match ui {
        UiState::Running { target, .. }
        | UiState::AwaitConfirm { target, .. }
        | UiState::Blocked { target, .. } => target.as_ref(),
        _ => None,
    };
    if !matches!(ui, UiState::Idle) {
        place(app, target);
    }
    for label in ["command", "overlay", "confirm"] {
        let visible = match label {
            "command" => matches!(ui, UiState::Listening),
            "confirm" => matches!(ui, UiState::AwaitConfirm { .. } | UiState::Blocked { .. }),
            _ => !matches!(ui, UiState::Idle | UiState::Listening),
        };
        if let Some(w) = app.get_webview_window(label) {
            if visible {
                let _ = w.show();
                if label == "command" {
                    let _ = w.set_focus();
                }
            } else {
                let _ = w.hide();
            }
        }
    }
}

pub fn tray(
    app: &AppHandle,
    ui: &UiState,
    recent: &[(String, std::time::Instant)],
    hotkey: &str,
) -> tauri::Result<()> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "run",
        format!("Run command…    {hotkey}"),
        true,
        None::<&str>,
    )?)?;
    if matches!(
        ui,
        UiState::Running { .. } | UiState::Paused | UiState::Stuck { .. }
    ) {
        let paused = matches!(ui, UiState::Paused | UiState::Stuck { .. });
        menu.append(&MenuItem::with_id(
            app,
            if paused { "resume" } else { "pause" },
            if paused { "Resume" } else { "Pause" },
            true,
            None::<&str>,
        )?)?;
    }
    let history = Submenu::new(app, "Recent", !recent.is_empty())?;
    for (summary, time) in recent.iter().rev().take(3) {
        let minutes = time.elapsed().as_secs() / 60;
        let age = if minutes == 0 {
            "just now".into()
        } else {
            format!("{minutes}m ago")
        };
        history.append(&MenuItem::new(
            app,
            format!("{summary} · {age}"),
            false,
            None::<&str>,
        )?)?;
    }
    menu.append(&history)?;
    menu.append(&MenuItem::new(
        app,
        "● Local · Qwen3.5-4B unavailable",
        false,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "settings",
        "Settings…",
        true,
        Some("CommandOrControl+,"),
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        "Quit Offhand",
        true,
        Some("CommandOrControl+Q"),
    )?)?;
    if let Some(tray) = app.tray_by_id("offhand") {
        tray.set_menu(Some(menu))?;
    } else {
        let mut builder = TrayIconBuilder::with_id("offhand")
            .tooltip("Offhand · local tree commands")
            .menu(&menu)
            .show_menu_on_left_click(true)
            .on_menu_event(|app, event| {
                let message = match event.id.as_ref() {
                    "run" => Control::Summon,
                    "pause" => Control::Pause,
                    "resume" => Control::Resume,
                    "settings" => {
                        show_preferences(app, "settings");
                        return;
                    }
                    "quit" => Control::Quit,
                    _ => return,
                };
                let _ = app.state::<Bridge>().0.send(message);
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        builder.build(app)?;
    }
    Ok(())
}

fn sync_geometry(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay")
        && let Ok(Some(monitor)) = overlay.current_monitor()
    {
        let pos = monitor.position();
        let size = monitor.size();
        let work = monitor.work_area();
        let scale = monitor.scale_factor();
        let bottom = (f64::from(pos.y) + f64::from(size.height)
            - f64::from(work.position.y)
            - f64::from(work.size.height))
            / scale
            + 20.0;
        let geometry = serde_json::json!({"x":pos.x,"y":pos.y,"scale":scale,"bottom":bottom});
        let _ = overlay.eval(format!("window.__OFFHAND_MONITOR__={geometry};window.dispatchEvent(new CustomEvent('offhand-monitor',{{detail:window.__OFFHAND_MONITOR__}}))"));
    }
}

pub fn emit(app: &AppHandle, revision: u64, ui: &UiState, armed: bool) {
    sync_geometry(app);
    let materials = app.state::<NativeMaterials>();
    for label in ["command", "confirm"] {
        if let Some(window) = app.get_webview_window(label) {
            let material = if materials.0.iter().any(|name| name == label) {
                "native"
            } else {
                "fallback"
            };
            let _ = window.eval(format!(
                "document.documentElement.dataset.material='{material}'"
            ));
        }
    }
    let _ = app.emit(
        "offhand://state",
        serde_json::json!({"revision":revision,"ui":ui,"armed":armed}),
    );
}
