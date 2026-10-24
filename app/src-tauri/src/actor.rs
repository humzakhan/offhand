use std::{
    rc::Rc,
    str::FromStr,
    sync::mpsc,
    time::{Duration, Instant},
};

use offhand_core::{Agent, ConfirmationDecision, UiState};
use offhand_infer::NoopEngine;
use offhand_platform::{Action, Element, Frame, Permission, Permissions, Platform, Rect, WindowId};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    AppError, desktop,
    settings::{PermissionSnapshot, Preferences, SettingsSnapshot},
};

type Reply<T> = mpsc::Sender<Result<T, AppError>>;
#[derive(Debug, Clone)]
pub struct Bridge(pub mpsc::Sender<Control>);
#[derive(Debug)]
pub enum Control {
    Summon,
    Blur,
    Pause,
    Resume,
    Decide(u64, ConfirmationDecision),
    Quit,
    Submit(String, Reply<()>),
    Stop(Option<Reply<()>>),
    Confirm(u64, ConfirmationDecision, Reply<()>),
    Settings(Reply<SettingsSnapshot>),
    Hotkey(String, Reply<SettingsSnapshot>),
    Permissions(Reply<PermissionSnapshot>),
    RequestPermission(Permission, Reply<PermissionSnapshot>),
    CompleteSetup(Reply<()>),
}
impl Bridge {
    pub async fn ask<T: Send + 'static>(
        &self,
        message: impl FnOnce(Reply<T>) -> Control + Send + 'static,
    ) -> Result<T, AppError> {
        let sender = self.0.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let (tx, rx) = mpsc::channel();
            sender
                .send(message(tx))
                .map_err(|_| AppError::actor("Agent thread is unavailable"))?;
            rx.recv().map_err(|_| {
                AppError::actor("Agent thread closed before acknowledging the request")
            })?
        })
        .await
        .map_err(AppError::actor)?
    }
}

#[derive(Debug)]
struct SharedPlatform<P>(Rc<P>);
impl<P: Platform> Platform for SharedPlatform<P> {
    fn tree(&self, window: Option<WindowId>) -> offhand_platform::Result<Vec<Element>> {
        self.0.tree(window)
    }
    fn capture(&self, region: Option<Rect>) -> offhand_platform::Result<Frame> {
        self.0.capture(region)
    }
    fn act(&self, action: &Action) -> offhand_platform::Result<()> {
        self.0.act(action)
    }
    fn foreground_window(&self) -> offhand_platform::Result<WindowId> {
        self.0.foreground_window()
    }
    fn focused_element(
        &self,
        window: Option<WindowId>,
    ) -> offhand_platform::Result<Option<Element>> {
        self.0.focused_element(window)
    }
    fn permissions(&self) -> Permissions {
        self.0.permissions()
    }
    fn request_permission(&self, permission: Permission) -> offhand_platform::Result<()> {
        self.0.request_permission(permission)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[derive(Debug, Default)]
struct UnsupportedPlatform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl Platform for UnsupportedPlatform {
    fn tree(&self, _: Option<WindowId>) -> offhand_platform::Result<Vec<Element>> {
        Err(offhand_platform::Error::Other(
            "Offhand requires macOS or Windows".into(),
        ))
    }
    fn capture(&self, _: Option<Rect>) -> offhand_platform::Result<Frame> {
        Err(offhand_platform::Error::Unimplemented)
    }
    fn act(&self, _: &Action) -> offhand_platform::Result<()> {
        Err(offhand_platform::Error::Other(
            "Offhand requires macOS or Windows".into(),
        ))
    }
}

pub fn start(app: AppHandle, receiver: mpsc::Receiver<Control>, bridge: Bridge) {
    std::thread::spawn(move || {
        // Native adapters and COM-bearing objects are constructed and remain on this thread.
        #[cfg(target_os = "macos")]
        #[allow(clippy::default_constructed_unit_structs)]
        let platform = offhand_macos::MacPlatform::default();
        #[cfg(target_os = "windows")]
        #[allow(clippy::default_constructed_unit_structs)]
        let platform = offhand_windows::WindowsPlatform::default();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let platform = UnsupportedPlatform;
        run(&app, &receiver, &bridge, platform);
    });
}

fn bind(
    app: &AppHandle,
    key: &str,
    bridge: &Bridge,
    action: impl Fn() -> Control + Send + Sync + 'static,
) -> Result<(), AppError> {
    let shortcut = Shortcut::from_str(key).map_err(AppError::shortcut)?;
    let tx = bridge.0.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_, _, event| {
            if event.state == ShortcutState::Pressed {
                let _ = tx.send(action());
            }
        })
        .map_err(AppError::shortcut)
}
fn unbind(app: &AppHandle, key: &str) -> Result<(), AppError> {
    let key = Shortcut::from_str(key).map_err(AppError::shortcut)?;
    app.global_shortcut()
        .unregister(key)
        .map_err(AppError::shortcut)
}
fn ensure_escape(app: &AppHandle, bridge: &Bridge, registered: &mut bool) -> Result<(), AppError> {
    if !*registered {
        bind(app, "Escape", bridge, || Control::Stop(None))?;
        *registered = true;
    }
    Ok(())
}
fn new_hotkey(
    app: &AppHandle,
    bridge: &Bridge,
    prefs: &mut Preferences,
    hotkey: &str,
) -> Result<SettingsSnapshot, AppError> {
    let key = Shortcut::from_str(hotkey).map_err(AppError::shortcut)?;
    if hotkey.len() > 80
        || !hotkey.contains('+')
        || ["Escape", "Tab", "CommandOrControl+Enter"]
            .iter()
            .filter_map(|s| Shortcut::from_str(s).ok())
            .any(|reserved| reserved == key)
    {
        return Err(AppError::shortcut(
            "Choose a modified shortcut other than Escape, Tab, or the approval shortcut",
        ));
    }
    if Shortcut::from_str(&prefs.hotkey).ok() == Some(key) {
        if !app.global_shortcut().is_registered(key) { bind(app, hotkey, bridge, || Control::Summon)?; }
        return Ok(prefs.snapshot());
    }
    // Keep the old registration until the replacement is registered and saved.
    bind(app, hotkey, bridge, || Control::Summon)?;
    let old = prefs.clone();
    hotkey.clone_into(&mut prefs.hotkey);
    if let Err(error) = prefs.save(app) {
        *prefs = old;
        let _ = unbind(app, hotkey);
        return Err(error);
    }
    if app.global_shortcut().is_registered(old.hotkey.as_str())
        && let Err(error) = unbind(app, &old.hotkey) {
        *prefs = old;
        let _ = prefs.save(app);
        let _ = unbind(app, hotkey);
        return Err(error);
    }
    Ok(prefs.snapshot())
}
fn is_summon_command(command: &str, hotkey: &str) -> bool {
    let Some((verb, chord)) = command.trim().split_once(char::is_whitespace) else {
        return false;
    };
    if !verb.eq_ignore_ascii_case("press") {
        return false;
    }
    let parts: Vec<_> = chord
        .trim()
        .split('+')
        .map(|p| match p.to_ascii_lowercase().as_str() {
            "ctrl" => "Control".into(),
            "primary" => "CommandOrControl".into(),
            "meta" => "Super".into(),
            _ => p.to_owned(),
        })
        .collect();
    matches!((Shortcut::from_str(&parts.join("+")), Shortcut::from_str(hotkey)), (Ok(a),Ok(b)) if a == b)
}

#[allow(clippy::too_many_lines)] // One owner loop keeps controls ordered before every native tick.
fn run<P: Platform>(
    app: &AppHandle,
    receiver: &mpsc::Receiver<Control>,
    bridge: &Bridge,
    platform: P,
) {
    let platform = Rc::new(platform);
    let mut agent = Agent::new(SharedPlatform(Rc::clone(&platform)), NoopEngine);
    let states = agent.subscribe();
    let mut shell_error = None;
    let mut prefs = match Preferences::load(app) {
        Ok(prefs) => prefs,
        Err(error) => {
            report_error(app, &mut shell_error, error);
            Preferences::default()
        }
    };
    if let Err(error) = bind(app, &prefs.hotkey, bridge, || Control::Summon) {
        report_error(app, &mut shell_error, error);
    }
    if !prefs.first_run_complete {
        desktop::show_preferences(app, "first-run");
    }
    let mut revision = 0_u64;
    let mut ui = UiState::Idle;
    let mut last_tick = Instant::now();
    let mut last_emit = Instant::now();
    let mut last_tray = Instant::now();
    let mut done_since = None;
    let mut recent = Vec::new();
    let mut escape = false;
    let mut primary = false;
    let mut skip = false;
    let mut command_visible = false;
    loop {
        let first = match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(message) => Some(message),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        // Drain all controls before tick: stop/pause cannot sit behind a native action.
        for message in first.into_iter().chain(receiver.try_iter()) {
            match message {
                Control::Summon => {
                    shell_error = None;
                    agent.stop();
                    command_visible = false;
                    match ensure_escape(app, bridge, &mut escape)
                        .and_then(|()| agent.listen().map_err(AppError::core))
                    {
                        Ok(()) => {
                            command_visible = true;
                        }
                        Err(error) => {
                            agent.stop();
                            report_error(app, &mut shell_error, error);
                        }
                    }
                }
                Control::Submit(command, reply) => {
                    let result = if command.trim().is_empty()
                        || command.len() > 8192
                        || command.contains('\0')
                    {
                        Err(AppError::input("Enter a command between 1 and 8192 bytes"))
                    } else if is_summon_command(&command, &prefs.hotkey) {
                        Err(AppError::input(
                            "The summon shortcut cannot be sent to another app",
                        ))
                    } else {
                        ensure_escape(app, bridge, &mut escape).and_then(|()| {
                            // Set before hide: its queued blur must never stop the submitted run.
                            command_visible = false;
                            desktop::hide(app, "command");
                            agent.submit(&command).map_err(AppError::core)
                        })
                    };
                    if result.is_err() {
                        command_visible = true;
                        if let Some(w) = app.get_webview_window("command") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    if result.is_ok() {
                        shell_error = None;
                    }
                    let _ = reply.send(result);
                }
                Control::Blur => {
                    if shell_error.take().is_some() {
                        desktop::hide(app, "command");
                    }
                    if command_visible && matches!(agent.state(), UiState::Listening) {
                        command_visible = false;
                        agent.stop();
                    }
                }
                Control::Stop(reply) => {
                    shell_error = None;
                    desktop::hide(app, "command");
                    if !matches!(
                        agent.state(),
                        UiState::Idle | UiState::Listening | UiState::Done { .. }
                    ) {
                        remember(&mut recent, "✕ Stopped");
                    }
                    command_visible = false;
                    agent.stop();
                    if let Some(reply) = reply {
                        let _ = reply.send(Ok(()));
                    }
                }
                Control::Confirm(id, decision, reply) => {
                    let result = if matches!(agent.state(), UiState::Blocked { .. })
                        && id == 0
                        && decision == ConfirmationDecision::Stop
                    {
                        agent.stop();
                        Ok(())
                    } else {
                        agent.respond(id, decision).map_err(AppError::core)
                    };
                    let _ = reply.send(result);
                }
                Control::Pause => {
                    if matches!(agent.state(), UiState::Running { .. }) {
                        agent.pause();
                    }
                }
                Control::Resume => {
                    if matches!(agent.state(), UiState::Paused | UiState::Stuck { .. })
                        && let Err(error) = agent.resume()
                    {
                        report_error(app, &mut shell_error, AppError::core(error));
                    }
                }
                Control::Decide(id, decision) => {
                    // The binding carries the ID that was visible when it was installed.
                    // A queued key can never approve a replacement confirmation.
                    let _ = agent.respond(id, decision);
                }
                Control::Settings(reply) => {
                    let _ = reply.send(Ok(prefs.snapshot()));
                }
                Control::Hotkey(hotkey, reply) => {
                    let result = new_hotkey(app, bridge, &mut prefs, &hotkey);
                    let _ = desktop::tray(app, agent.state(), &recent, &prefs.hotkey);
                    let _ = reply.send(result);
                }
                Control::Permissions(reply) => {
                    let _ = reply.send(Ok(platform.permissions().into()));
                }
                Control::RequestPermission(permission, reply) => {
                    let result = platform
                        .request_permission(permission)
                        .map(|()| platform.permissions().into())
                        .map_err(AppError::core);
                    let _ = reply.send(result);
                }
                Control::CompleteSetup(reply) => {
                    prefs.first_run_complete = true;
                    let result = prefs.save(app);
                    if result.is_ok() {
                        desktop::hide(app, "first-run");
                    } else {
                        prefs.first_run_complete = false;
                    }
                    let _ = reply.send(result);
                }
                Control::Quit => {
                    agent.stop();
                    app.exit(0);
                    return;
                }
            }
        }
        if last_tick.elapsed() >= Duration::from_millis(110) {
            if agent.tick().is_err() {
                // Core enters Stuck and retains the task for an explicit retry.
                remember(&mut recent, "✕ Native operation failed");
            }
            last_tick = Instant::now();
        }
        if let Some(next) = states.try_iter().last() {
            if next == ui && revision != 0 {
                // A late webview still receives the periodic state below.
            } else {
                ui = next;
                revision += 1;
                if matches!(ui, UiState::Idle) {
                    if escape {
                        let _ = unbind(app, "Escape");
                        escape = false;
                    }
                } else if let Err(error) = ensure_escape(app, bridge, &mut escape) {
                    agent.stop();
                    report_error(app, &mut shell_error, error);
                    continue;
                }
                // State-scoped keys never remain installed after the applicable state ends.
                if primary {
                    let _ = unbind(app, "CommandOrControl+Enter");
                    primary = false;
                }
                if skip {
                    let _ = unbind(app, "Tab");
                    skip = false;
                }
                if let UiState::AwaitConfirm {
                    confirmation_id, ..
                } = &ui
                {
                    let id = *confirmation_id;
                    primary = bind(app, "CommandOrControl+Enter", bridge, move || {
                        Control::Decide(id, ConfirmationDecision::Allow)
                    })
                    .is_ok();
                    skip = bind(app, "Tab", bridge, move || {
                        Control::Decide(id, ConfirmationDecision::Skip)
                    })
                    .is_ok();
                } else if matches!(ui, UiState::Paused | UiState::Stuck { .. }) {
                    primary =
                        bind(app, "CommandOrControl+Enter", bridge, || Control::Resume).is_ok();
                }
                done_since = if let UiState::Done { summary, .. } = &ui {
                    remember(&mut recent, &format!("✓ {summary}"));
                    Some((revision, Instant::now()))
                } else {
                    None
                };
                if let UiState::Blocked { label, .. } = &ui {
                    remember(&mut recent, &format!("✕ Blocked: {label}"));
                }
                desktop::render(app, &ui);
                if let Some(error) = &shell_error {
                    desktop::show_error(app, error);
                }
                desktop::emit(app, revision, &ui);
                let _ = desktop::tray(app, &ui, &recent, &prefs.hotkey);
            }
        }
        if done_since.is_some_and(|(r, t)| r == revision && t.elapsed() >= Duration::from_secs(3)) {
            agent.stop();
            done_since = None;
        }
        if last_emit.elapsed() >= Duration::from_millis(250) {
            if app.webview_windows().values().any(|window| window.is_visible().unwrap_or(false)) {
                desktop::emit(app, revision, &ui);
            }
            if let Some(error) = &shell_error {
                desktop::replay_error(app, error);
            }
            last_emit = Instant::now();
        }
        if last_tray.elapsed() >= Duration::from_secs(60) {
            let _ = desktop::tray(app, &ui, &recent, &prefs.hotkey);
            last_tray = Instant::now();
        }
    }
}
fn remember(recent: &mut Vec<(String, Instant)>, summary: &str) {
    recent.push((summary.chars().take(90).collect(), Instant::now()));
    if recent.len() > 3 {
        recent.remove(0);
    }
}

fn report_error(app: &AppHandle, current: &mut Option<AppError>, error: AppError) {
    desktop::show_error(app, &error);
    *current = Some(error);
}
