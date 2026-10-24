//! Deterministic, accessibility-first single-command runner.

use offhand_infer::Engine;
use offhand_platform::{Action, Element, Platform, WindowId};
use offhand_policy::Risk;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver, Sender};

/// Escalation ladder retained for future local inference backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Direct accessibility actions (the only enabled tier).
    Accessibility,
    /// Local element selection (unavailable).
    Choose,
    /// Local visual grounding (unavailable).
    Ground,
    /// Opt-in cloud fallback (unavailable).
    Cloud,
}

/// Desktop physical pixels, with top-left origin and possible negative coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A resolved accessibility target suitable for a highlight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub label: String,
    pub bounds: Bounds,
}

/// Public runner state shared by the CLI and desktop shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UiState {
    Idle,
    Listening,
    Running { step: u32, total: u32, target: Option<Target> },
    AwaitConfirm {
        confirmation_id: u64,
        label: String,
        app: String,
        detail: Option<String>,
        target: Option<Target>,
    },
    Blocked { label: String, target: Option<Target> },
    Stuck { query: String, cloud_available: bool },
    Paused,
    Done { summary: String, undoable: bool },
}

/// An explicit decision for exactly one pending action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationDecision { Allow, Skip, Stop }

/// Runner failures. Resolution failures also publish a Stuck state.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Platform(#[from] offhand_platform::Error),
    #[error("confirmation is no longer pending")]
    StaleConfirmation,
    #[error("confirmation identifier exhausted")]
    ConfirmationExhausted,
}

/// Result of accepting or advancing a runner operation.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
enum Command { Open(String), Click(String), Type(String), Key(String), Press(String) }

impl Command {
    fn parse(input: &str) -> Option<Self> {
        let input = input.trim_start();
        let split = input.find(char::is_whitespace)?;
        let verb = input[..split].to_ascii_lowercase();
        // Consume only the separator, preserving the text suffix verbatim.
        let text = input[split..].strip_prefix(char::is_whitespace)?;
        if text.is_empty() { return None; }
        match verb.as_str() {
            "open" => Some(Self::Open(text.to_owned())),
            "click" => Some(Self::Click(text.to_owned())),
            "type" => Some(Self::Type(text.to_owned())),
            "press" => {
                let text = text.trim();
                if reserved_key(text) { return None; }
                if let Some(key) = canonical_key(text) { Some(Self::Key(key)) }
                else if text.contains('+') { None }
                else { Some(Self::Press(text.to_owned())) }
            }
            _ => None,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Open(name) => format!("Open {name}"),
            Self::Click(label) | Self::Press(label) => label.clone(),
            Self::Type(_) => "Type text".into(),
            Self::Key(key) => format!("Press {key}"),
        }
    }
}

fn reserved_key(input: &str) -> bool {
    input.split('+').any(|part| matches!(part.trim().to_ascii_lowercase().as_str(), "esc" | "escape"))
}

fn canonical_key(input: &str) -> Option<String> {
    let parts: Vec<_> = input.split('+').collect();
    let (base, modifiers) = parts.split_last()?;
    let mut output = Vec::new();
    for modifier in modifiers {
        let modifier = modifier.to_ascii_lowercase();
        if !matches!(modifier.as_str(), "ctrl" | "alt" | "shift" | "meta" | "primary")
            || output.contains(&modifier) { return None; }
        output.push(modifier);
    }
    let lower = base.to_ascii_lowercase();
    let name = match lower.as_str() {
        "enter" => "Enter", "tab" => "Tab", "space" => "Space",
        "backspace" => "Backspace", "delete" => "Delete",
        "left" => "Left", "right" => "Right", "up" => "Up", "down" => "Down",
        "home" => "Home", "end" => "End", "pageup" => "PageUp", "pagedown" => "PageDown",
        _ => {
            let function_key = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=12).contains(&n));
            if function_key || (lower.len() == 1 && lower.as_bytes()[0].is_ascii_alphanumeric()) {
                output.push(lower.to_ascii_uppercase());
                return Some(output.join("+"));
            }
            return None;
        }
    };
    output.push(name.to_owned());
    Some(output.join("+"))
}

#[derive(Clone, PartialEq)]
struct Pending { action: Action, element: Option<Element>, window: Option<WindowId> }

impl Pending {
    fn target(&self) -> Option<Target> {
        self.element.as_ref().map(|element| Target {
            label: element.label.clone().unwrap_or_else(|| "Focused field".into()),
            bounds: Bounds { x: element.bounds.x, y: element.bounds.y, w: element.bounds.w, h: element.bounds.h },
        })
    }
}

#[derive(Clone, Copy)]
enum Stage { Resolve, Ready { focus_attempted: bool } }

/// Drives a platform on its owning thread; inference is deliberately unused in T0.
pub struct Agent<P: Platform, E: Engine> {
    platform: P,
    _engine: E,
    state: UiState,
    subscribers: Vec<Sender<UiState>>,
    command: Option<Command>,
    window: Option<WindowId>,
    pending: Option<Pending>,
    stage: Stage,
    confirmation_id: u64,
    approved: bool,
}

// Do not expose command text or typed secrets through diagnostic formatting.
impl<P: Platform, E: Engine> std::fmt::Debug for Agent<P, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent").field("state", &self.state).finish_non_exhaustive()
    }
}

impl<P: Platform, E: Engine> Agent<P, E> {
    /// Construct a runner on the same thread that owns the platform.
    #[must_use]
    pub fn new(platform: P, engine: E) -> Self {
        Self { platform, _engine: engine, state: UiState::Idle, subscribers: Vec::new(),
            command: None, window: None, pending: None, stage: Stage::Resolve,
            confirmation_id: 0, approved: false }
    }

    /// Current public state; accepting a command does not imply completion.
    #[must_use]
    pub fn state(&self) -> &UiState { &self.state }

    /// Subscribe to the current state immediately and every subsequent transition.
    pub fn subscribe(&mut self) -> Receiver<UiState> {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(self.state.clone());
        self.subscribers.push(tx);
        rx
    }

    fn emit(&mut self, state: UiState) {
        self.state = state;
        self.subscribers.retain(|tx| tx.send(self.state.clone()).is_ok());
    }

    /// Capture the external window before the shell activates its command bar.
    pub fn listen(&mut self) -> Result<()> {
        self.clear_task();
        self.window = self.platform.foreground_window().ok();
        self.emit(UiState::Listening);
        Ok(())
    }

    /// Replace the previous task and approval with one deterministic command.
    pub fn submit(&mut self, command: &str) -> Result<()> {
        if !matches!(self.state, UiState::Listening) {
            self.window = self.platform.foreground_window().ok();
        }
        self.clear_task();
        self.command = Command::parse(command);
        if self.command.is_some() { self.running(None); }
        else { self.stuck("Use open <app>, click <label>, type <text>, or press <key-or-label>"); }
        Ok(())
    }

    /// Resolve or dispatch at most one native action, without sleeping or inference.
    pub fn tick(&mut self) -> Result<()> {
        if !matches!(self.state, UiState::Running { .. }) { return Ok(()); }
        let result = self.advance();
        if result.is_err() { self.stuck("Native operation failed; inspect permissions and retry explicitly"); }
        result
    }

    fn advance(&mut self) -> Result<()> {
        match self.stage {
            Stage::Resolve => {
                if let Some(pending) = self.resolve()? { self.present(pending)?; }
            }
            Stage::Ready { focus_attempted } => {
                let Some(previous) = self.pending.clone() else { return Ok(()); };
                if let Some(window) = previous.window {
                    if self.platform.foreground_window()? != window {
                        if focus_attempted { self.stuck("The captured window could not be restored"); return Ok(()); }
                        let action = Action::FocusWindow(window);
                        if offhand_policy::classify(&action, None) != Risk::Safe {
                            self.stuck("Window restoration was rejected"); return Ok(());
                        }
                        self.platform.act(&action)?;
                        self.stage = Stage::Ready { focus_attempted: true };
                        return Ok(());
                    }
                }
                let Some(current) = self.resolve()? else { return Ok(()); };
                if current != previous { self.present(current)?; return Ok(()); }
                match offhand_policy::classify(&current.action, current.element.as_ref()) {
                    Risk::Block => { self.present(current)?; }
                    Risk::Confirm if !self.approved => { self.present(current)?; }
                    Risk::Safe | Risk::Confirm => {
                        // This is the sole task dispatch path. Approval is consumed even on failure.
                        self.approved = false;
                        self.pending = None;
                        self.platform.act(&current.action)?;
                        // Refresh for subsequent resolution, but never retry an uncertain dispatch.
                        let refreshed = self.platform.tree(current.window);
                        if refreshed.is_err() {
                            self.stuck("Action dispatched, but the refreshed tree is unavailable; verify manually");
                        } else {
                            self.command = None;
                            self.emit(UiState::Done { summary: "Action dispatched; verify the result in the app".into(), undoable: false });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn resolve(&mut self) -> Result<Option<Pending>> {
        let Some(command) = self.command.clone() else { return Ok(None); };
        if let Command::Open(name) = command {
            return Ok(Some(Pending { action: Action::OpenApp(name), element: None, window: None }));
        }
        let Some(window) = self.window else { self.stuck("No external target window was captured"); return Ok(None); };
        let (action, element) = match command {
            Command::Click(label) | Command::Press(label) => {
                let tree = self.platform.tree(Some(window))?;
                let Some(element) = unique_match(&tree, label.trim()) else {
                    self.stuck("No unique enabled accessibility label matched; refine the command and retry");
                    return Ok(None);
                };
                (Action::Press(element.id.clone()), Some(element.clone()))
            }
            Command::Type(text) => (Action::Type(text), self.platform.focused_element(Some(window))?),
            Command::Key(key) => (Action::Key(key), self.platform.focused_element(Some(window))?),
            Command::Open(_) => return Ok(None),
        };
        Ok(Some(Pending { action, element, window: Some(window) }))
    }

    fn present(&mut self, pending: Pending) -> Result<()> {
        self.approved = false;
        let target = pending.target();
        let label = self.command.as_ref().map_or_else(|| "Action".into(), Command::label);
        let risk = offhand_policy::classify(&pending.action, pending.element.as_ref());
        self.pending = Some(pending);
        self.stage = Stage::Ready { focus_attempted: false };
        match risk {
            Risk::Safe => self.running(target),
            Risk::Confirm => {
                self.confirmation_id = self.confirmation_id.checked_add(1).ok_or(Error::ConfirmationExhausted)?;
                self.emit(UiState::AwaitConfirm { confirmation_id: self.confirmation_id, label,
                    app: "active app".into(), detail: None, target });
            }
            Risk::Block => self.emit(UiState::Blocked { label, target }),
        }
        Ok(())
    }

    /// Accept a decision only for the currently pending confirmation.
    pub fn respond(&mut self, confirmation_id: u64, decision: ConfirmationDecision) -> Result<()> {
        if !matches!(self.state, UiState::AwaitConfirm { confirmation_id: id, .. } if id == confirmation_id) {
            return Err(Error::StaleConfirmation);
        }
        match decision {
            ConfirmationDecision::Allow => {
                self.approved = true;
                self.running(self.pending.as_ref().and_then(Pending::target));
            }
            ConfirmationDecision::Skip => {
                self.clear_task();
                self.emit(UiState::Done { summary: "Skipped".into(), undoable: false });
            }
            ConfirmationDecision::Stop => self.stop(),
        }
        Ok(())
    }

    /// Pause a live task and revoke any pending approval.
    pub fn pause(&mut self) {
        if matches!(self.state, UiState::Running { .. } | UiState::AwaitConfirm { .. }) {
            self.pending = None;
            self.approved = false;
            self.stage = Stage::Resolve;
            self.emit(UiState::Paused);
        }
    }

    /// Explicitly retry resolution from Paused or Stuck; never reuse an approval.
    pub fn resume(&mut self) -> Result<()> {
        if matches!(self.state, UiState::Paused | UiState::Stuck { .. }) && self.command.is_some() {
            self.pending = None;
            self.approved = false;
            self.stage = Stage::Resolve;
            self.running(None);
        }
        Ok(())
    }

    /// Cancel all pending work and approval immediately.
    pub fn stop(&mut self) {
        self.clear_task();
        self.window = None;
        self.emit(UiState::Idle);
    }

    fn clear_task(&mut self) {
        self.command = None;
        self.pending = None;
        self.approved = false;
        self.stage = Stage::Resolve;
    }

    fn running(&mut self, target: Option<Target>) {
        self.emit(UiState::Running { step: 1, total: 1, target });
    }

    fn stuck(&mut self, query: &str) {
        self.pending = None;
        self.approved = false;
        self.stage = Stage::Resolve;
        self.emit(UiState::Stuck { query: query.into(), cloud_available: false });
    }
}

fn unique_match<'a>(tree: &'a [Element], query: &str) -> Option<&'a Element> {
    if query.is_empty() { return None; }
    let exact: Vec<_> = tree.iter().filter(|e| e.enabled && e.label.as_deref() == Some(query)).collect();
    if exact.len() == 1 { return exact.first().copied(); }
    if exact.len() > 1 { return None; }
    let lower = query.to_lowercase();
    let mut matches = tree.iter().filter(|e| e.enabled && e.label.as_ref().is_some_and(|label| label.to_lowercase().contains(&lower)));
    let first = matches.next()?;
    if matches.next().is_some() { None } else { Some(first) }
}
