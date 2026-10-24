//! Deterministic, accessibility-first single-command runner.

use offhand_infer::{Embedder, Engine};
use offhand_platform::{Action, Element, Platform, WindowId};
use offhand_policy::Risk;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

static NEXT_CONFIRMATION_ID: AtomicU64 = AtomicU64::new(1);

/// Minimum cosine score for semantic fallback; provisional until benched on both OSes.
pub const MIN_EMBED_SCORE: f32 = 0.45;

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
    Running {
        step: u32,
        total: u32,
        target: Option<Target>,
    },
    AwaitConfirm {
        confirmation_id: u64,
        label: String,
        app: String,
        detail: Option<String>,
        target: Option<Target>,
    },
    Blocked {
        label: String,
        target: Option<Target>,
    },
    Stuck {
        query: String,
        cloud_available: bool,
    },
    Paused,
    Done {
        summary: String,
        undoable: bool,
    },
}

/// An explicit decision for exactly one pending action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationDecision {
    Allow,
    Skip,
    Stop,
}

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
enum Command {
    Open(String),
    Click(String),
    Type(String),
    Key(String),
    Press(String),
}

impl Command {
    fn parse(input: &str) -> Option<Self> {
        let input = input.trim_start();
        let split = input.find(char::is_whitespace)?;
        let verb = input[..split].to_ascii_lowercase();
        // Consume only the separator, preserving the text suffix verbatim.
        let text = input[split..].strip_prefix(char::is_whitespace)?;
        if text.is_empty() {
            return None;
        }
        match verb.as_str() {
            "open" => Some(Self::Open(text.trim().to_owned())),
            "click" => Some(Self::Click(text.to_owned())),
            "type" => Some(Self::Type(text.to_owned())),
            "press" => {
                let text = text.trim();
                if reserved_key(text) {
                    return None;
                }
                if let Some(key) = canonical_key(text) {
                    Some(Self::Key(key))
                } else if text.contains('+') {
                    None
                } else {
                    Some(Self::Press(text.to_owned()))
                }
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
    input
        .split('+')
        .any(|part| matches!(part.trim().to_ascii_lowercase().as_str(), "esc" | "escape"))
}

fn canonical_key(input: &str) -> Option<String> {
    let parts: Vec<_> = input.split('+').collect();
    let (base, modifiers) = parts.split_last()?;
    let mut output = Vec::new();
    for modifier in modifiers {
        let modifier = modifier.to_ascii_lowercase();
        if !matches!(
            modifier.as_str(),
            "ctrl" | "alt" | "shift" | "meta" | "primary"
        ) || output.contains(&modifier)
        {
            return None;
        }
        output.push(modifier);
    }
    let lower = base.to_ascii_lowercase();
    let name = match lower.as_str() {
        "enter" => "Enter",
        "tab" => "Tab",
        "space" => "Space",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "left" => "Left",
        "right" => "Right",
        "up" => "Up",
        "down" => "Down",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        _ => {
            let function_key = lower
                .strip_prefix('f')
                .and_then(|n| n.parse::<u8>().ok())
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
struct Pending {
    action: Action,
    element: Option<Element>,
    window: Option<WindowId>,
}

impl Pending {
    fn target(&self) -> Option<Target> {
        self.element.as_ref().map(|element| Target {
            label: element
                .label
                .clone()
                .unwrap_or_else(|| "Focused field".into()),
            bounds: Bounds {
                x: element.bounds.x,
                y: element.bounds.y,
                w: element.bounds.w,
                h: element.bounds.h,
            },
        })
    }
}

#[derive(Clone, Copy)]
enum Stage {
    Resolve,
    Ready { focus_wait: Option<u8> },
}

/// Drives a platform on its owning thread with optional local label ranking.
pub struct Agent<P: Platform, E: Engine> {
    platform: P,
    _engine: E,
    embedder: Option<Box<dyn Embedder>>,
    state: UiState,
    subscribers: Vec<Sender<UiState>>,
    command: Option<Command>,
    window: Option<WindowId>,
    pending: Option<Pending>,
    stage: Stage,
    confirmation_id: u64,
    approved: bool,
    revalidation_mismatches: u8,
    focus_failures: u8,
}

// Do not expose command text or typed secrets through diagnostic formatting.
impl<P: Platform, E: Engine> std::fmt::Debug for Agent<P, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<P: Platform, E: Engine> Agent<P, E> {
    /// Construct a runner on the same thread that owns the platform.
    #[must_use]
    pub fn new(platform: P, engine: E) -> Self {
        Self {
            platform,
            _engine: engine,
            embedder: offhand_infer::default_embedder(),
            state: UiState::Idle,
            subscribers: Vec::new(),
            command: None,
            window: None,
            pending: None,
            stage: Stage::Resolve,
            confirmation_id: 0,
            approved: false,
            revalidation_mismatches: 0,
            focus_failures: 0,
        }
    }

    /// Override the default local embedder used to rank accessibility labels.
    ///
    /// Deterministic matches take priority. Otherwise, label commands use the
    /// highest-ranked enabled target only at or above [`MIN_EMBED_SCORE`], then
    /// enter the existing policy and confirmation path. Embedding errors leave
    /// the command unmatched. This does not enable VLM selection or capture.
    #[must_use]
    pub fn with_embedder(mut self, embedder: Box<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    fn match_label<'a>(&self, tree: &'a [Element], query: &str) -> Option<&'a Element> {
        if let Some(element) = unique_match(tree, query) {
            return Some(element);
        }
        let embedder = self.embedder.as_deref()?;
        if query.is_empty() {
            return None;
        }
        // Keep native element identity; embeddings rank only enabled, labeled targets.
        let candidates: Vec<_> = tree
            .iter()
            .filter_map(|element| {
                element
                    .label
                    .as_deref()
                    .filter(|label| element.enabled && !label.trim().is_empty())
                    .map(|label| (element, label))
            })
            .collect();
        let labels: Vec<_> = candidates.iter().map(|(_, label)| *label).collect();
        let scores = offhand_infer::shortlist_with_scores(embedder, query, &labels, 1).ok()?;
        scores
            .first()
            .filter(|(_, score)| *score >= MIN_EMBED_SCORE)
            .map(|&(index, _)| candidates[index].0)
    }

    /// Current public state; accepting a command does not imply completion.
    #[must_use]
    pub fn state(&self) -> &UiState {
        &self.state
    }

    /// Subscribe to the current state immediately and every subsequent transition.
    pub fn subscribe(&mut self) -> Receiver<UiState> {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(self.state.clone());
        self.subscribers.push(tx);
        rx
    }

    fn emit(&mut self, state: UiState) {
        self.state = state;
        self.subscribers
            .retain(|tx| tx.send(self.state.clone()).is_ok());
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
        if self.command.is_some() {
            self.running(None);
        } else {
            self.stuck("Use open <app>, click <label>, type <text>, or press <key-or-label>");
        }
        Ok(())
    }

    /// Resolve or dispatch at most one native action, without sleeping or VLM calls.
    pub fn tick(&mut self) -> Result<()> {
        if !matches!(self.state, UiState::Running { .. }) {
            return Ok(());
        }
        let result = self.advance();
        if result.is_err() {
            self.stuck("Native operation failed; inspect permissions and retry explicitly");
        }
        result
    }

    fn advance(&mut self) -> Result<()> {
        match self.stage {
            Stage::Resolve => {
                if let Some(pending) = self.resolve()? {
                    self.present(pending)?;
                }
            }
            Stage::Ready { focus_wait } => {
                let Some(previous) = self.pending.clone() else {
                    return Ok(());
                };
                // Own-process exclusion can make the foreground query unavailable
                // while the command bar is still hiding. Restore the pinned window.
                if let Some(window) = previous.window
                    && self.platform.foreground_window().ok() != Some(window)
                {
                    if let Some(waited) = focus_wait {
                        if waited >= 4 {
                            self.stuck(
                                "The captured window could not be restored after five ticks",
                            );
                        } else {
                            self.stage = Stage::Ready {
                                focus_wait: Some(waited + 1),
                            };
                        }
                        return Ok(());
                    }
                    let action = Action::FocusWindow(window);
                    if offhand_policy::classify(&action, None) != Risk::Safe {
                        self.stuck("Window restoration was rejected");
                        return Ok(());
                    }
                    // Record the attempt before dispatch so failures cannot retry forever.
                    self.focus_failures += 1;
                    if self.platform.act(&action).is_err() {
                        if self.focus_failures >= 2 {
                            self.stuck(
                                "Window restoration failed twice; inspect permissions and retry explicitly",
                            );
                        }
                        return Ok(());
                    }
                    self.focus_failures = 0;
                    self.stage = Stage::Ready {
                        focus_wait: Some(0),
                    };
                    return Ok(());
                }
                self.focus_failures = 0;
                let Some(current) = self.resolve()? else {
                    return Ok(());
                };
                if current != previous {
                    self.revalidation_mismatches += 1;
                    if self.revalidation_mismatches >= 3 {
                        self.stuck(
                            "The target changed three times during revalidation; retry explicitly",
                        );
                    } else {
                        self.present(current)?;
                    }
                    return Ok(());
                }
                self.revalidation_mismatches = 0;
                match offhand_policy::classify(&current.action, current.element.as_ref()) {
                    Risk::Block => {
                        self.present(current)?;
                    }
                    Risk::Confirm if !self.approved => {
                        self.present(current)?;
                    }
                    Risk::Safe | Risk::Confirm => self.dispatch(&current)?,
                }
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, current: &Pending) -> Result<()> {
        // A snapshot may have taken time; recheck immediately before focused input.
        if let Some(window) = current.window
            && self.platform.foreground_window()? != window
        {
            self.stuck("The target window changed during revalidation; retry explicitly");
            return Ok(());
        }
        self.approved = false;
        self.pending = None;
        self.platform.act(&current.action)?;
        // Dispatch success is terminal even if verification cannot read a tree.
        // In particular, Resume must never repeat already-inserted text or a launch.
        self.clear_task();
        let summary = if self.platform.tree(current.window).is_ok() {
            "Action dispatched; verify the result in the app"
        } else {
            "Action dispatched; result unverified because the tree is unavailable"
        };
        self.emit(UiState::Done {
            summary: summary.into(),
            undoable: false,
        });
        Ok(())
    }

    fn resolve(&mut self) -> Result<Option<Pending>> {
        let Some(command) = self.command.clone() else {
            return Ok(None);
        };
        if let Command::Open(name) = command {
            return Ok(Some(Pending {
                action: Action::OpenApp(name),
                element: None,
                window: None,
            }));
        }
        let Some(window) = self.window else {
            self.stuck("No external target window was captured");
            return Ok(None);
        };
        let (action, element) = match command {
            Command::Click(label) | Command::Press(label) => {
                let tree = self.platform.tree(Some(window))?;
                let Some(element) = self.match_label(&tree, label.trim()) else {
                    self.stuck("No unique enabled accessibility label matched; refine the command and retry");
                    return Ok(None);
                };
                (Action::Press(element.id.clone()), Some(element.clone()))
            }
            Command::Type(text) => (
                Action::Type(text),
                self.platform.focused_element(Some(window))?,
            ),
            Command::Key(key) => (
                Action::Key(key),
                self.platform.focused_element(Some(window))?,
            ),
            Command::Open(_) => return Ok(None),
        };
        Ok(Some(Pending {
            action,
            element,
            window: Some(window),
        }))
    }

    fn present(&mut self, pending: Pending) -> Result<()> {
        self.approved = false;
        let target = pending.target();
        let label = if matches!(pending.action, Action::Press(_) | Action::Click { .. }) {
            pending
                .element
                .as_ref()
                .and_then(|element| element.label.clone())
        } else {
            None
        }
        .unwrap_or_else(|| {
            self.command
                .as_ref()
                .map_or_else(|| "Action".into(), Command::label)
        });
        let detail = confirmation_detail(&pending);
        let risk = offhand_policy::classify(&pending.action, pending.element.as_ref());
        self.pending = Some(pending);
        self.stage = Stage::Ready { focus_wait: None };
        match risk {
            Risk::Safe => self.running(target),
            Risk::Confirm => {
                self.confirmation_id = NEXT_CONFIRMATION_ID
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                    .map_err(|_| Error::ConfirmationExhausted)?;
                self.emit(UiState::AwaitConfirm {
                    confirmation_id: self.confirmation_id,
                    label,
                    app: "active app".into(),
                    detail,
                    target,
                });
            }
            Risk::Block => self.emit(UiState::Blocked { label, target }),
        }
        Ok(())
    }

    /// Accept a decision only for the currently pending confirmation.
    pub fn respond(&mut self, confirmation_id: u64, decision: ConfirmationDecision) -> Result<()> {
        if !matches!(self.state, UiState::AwaitConfirm { confirmation_id: id, .. } if id == confirmation_id)
        {
            return Err(Error::StaleConfirmation);
        }
        match decision {
            ConfirmationDecision::Allow => {
                self.approved = true;
                self.running(self.pending.as_ref().and_then(Pending::target));
            }
            ConfirmationDecision::Skip => {
                self.clear_task();
                self.emit(UiState::Done {
                    summary: "Skipped".into(),
                    undoable: false,
                });
            }
            ConfirmationDecision::Stop => self.stop(),
        }
        Ok(())
    }

    /// Pause a live task and revoke any pending approval.
    pub fn pause(&mut self) {
        if matches!(
            self.state,
            UiState::Running { .. } | UiState::AwaitConfirm { .. }
        ) {
            self.pending = None;
            self.approved = false;
            self.stage = Stage::Resolve;
            self.emit(UiState::Paused);
        }
    }

    /// Explicitly retry resolution from Paused or Stuck; never reuse an approval.
    pub fn resume(&mut self) -> Result<()> {
        if matches!(self.state, UiState::Paused | UiState::Stuck { .. }) {
            if self.command.is_some() {
                self.pending = None;
                self.approved = false;
                self.revalidation_mismatches = 0;
                self.focus_failures = 0;
                self.stage = Stage::Resolve;
                self.running(None);
            } else {
                self.stop();
            }
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
        self.focus_failures = 0;
        self.revalidation_mismatches = 0;
        self.pending = None;
        self.approved = false;
        self.stage = Stage::Resolve;
    }

    fn running(&mut self, target: Option<Target>) {
        self.emit(UiState::Running {
            step: 1,
            total: 1,
            target,
        });
    }

    fn stuck(&mut self, query: &str) {
        self.pending = None;
        self.approved = false;
        self.stage = Stage::Resolve;
        self.emit(UiState::Stuck {
            query: query.into(),
            cloud_available: false,
        });
    }
}

fn confirmation_detail(pending: &Pending) -> Option<String> {
    let detail = match &pending.action {
        Action::Type(text) => {
            let password = pending.element.as_ref().is_some_and(|element| {
                element.role == offhand_platform::Role::PasswordField
                    || element
                        .label
                        .as_ref()
                        .is_some_and(|label| label.to_lowercase().contains("password"))
            });
            if password {
                Some("Password field")
            } else if pending.element.is_none() {
                Some("No focused field")
            } else if text.contains(['\n', '\r']) {
                Some("Multiline text")
            } else {
                None
            }
        }
        Action::Key(_) => Some("Key shortcut"),
        Action::OpenApp(_) => Some("System settings"),
        Action::Press(_)
        | Action::Click { .. }
        | Action::FocusWindow(_)
        | Action::Scroll { .. } => None,
    };
    detail.map(str::to_owned)
}

fn unique_match<'a>(tree: &'a [Element], query: &str) -> Option<&'a Element> {
    if query.is_empty() {
        return None;
    }
    let lower = query.to_lowercase();
    // Include disabled matches in the exact tiers: never redirect an exact
    // disabled target to a different enabled element with a longer label.
    for case_sensitive in [true, false] {
        let matches: Vec<_> = tree
            .iter()
            .filter(|element| {
                element.label.as_ref().is_some_and(|label| {
                    if case_sensitive {
                        label == query
                    } else {
                        label.to_lowercase() == lower
                    }
                })
            })
            .collect();
        if !matches.is_empty() {
            return if matches.len() == 1 && matches[0].enabled {
                Some(matches[0])
            } else {
                None
            };
        }
    }
    let mut matches = tree.iter().filter(|e| {
        e.enabled
            && e.label
                .as_ref()
                .is_some_and(|label| label.to_lowercase().contains(&lower))
    });
    let first = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(first)
    }
}
