//! Agent loop for Offhand.

use offhand_infer::Engine;
use offhand_platform::Platform;

/// Escalation ladder, cheapest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Act directly from the accessibility tree.
    Accessibility,
    /// Let the local model choose among accessibility elements.
    Choose,
    /// Let the local model ground a screen coordinate from pixels.
    Ground,
    /// Opt-in cloud fallback.
    Cloud,
}

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target { pub label: String, pub bounds: Bounds }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UiState {
    Idle, Listening,
    Running { step: u32, total: u32, target: Option<Target> },
    AwaitConfirm { confirmation_id: u64, label: String, app: String, detail: Option<String>, target: Option<Target> },
    Blocked { label: String, target: Option<Target> },
    Stuck { query: String, cloud_available: bool },
    Paused,
    Done { summary: String, undoable: bool },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationDecision { Allow, Skip, Stop }
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Platform(#[from] offhand_platform::Error),
    #[error("confirmation is no longer pending")]
    StaleConfirmation,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub struct Agent<P: Platform, E: Engine> {
    platform: P,
    engine: E,
    state: UiState,
    subscribers: Vec<std::sync::mpsc::Sender<UiState>>,
}
impl<P: Platform, E: Engine> Agent<P, E> {
    pub fn new(platform: P, engine: E) -> Self { Self { platform, engine, state: UiState::Idle, subscribers: Vec::new() } }
    pub fn state(&self) -> &UiState { &self.state }
    pub fn subscribe(&mut self) -> std::sync::mpsc::Receiver<UiState> {
        let (tx, rx) = std::sync::mpsc::channel(); let _ = tx.send(self.state.clone()); self.subscribers.push(tx); rx
    }
    fn emit(&mut self, state: UiState) { self.state = state; self.subscribers.retain(|tx| tx.send(self.state.clone()).is_ok()); }
    pub fn listen(&mut self) -> Result<()> { self.emit(UiState::Listening); Ok(()) }
    pub fn submit(&mut self, _command: &str) -> Result<()> { self.emit(UiState::Stuck { query: "Runner initializing".into(), cloud_available: false }); Ok(()) }
    pub fn tick(&mut self) -> Result<()> { let _ = (&self.platform, &self.engine); Ok(()) }
    pub fn respond(&mut self, _confirmation_id: u64, _decision: ConfirmationDecision) -> Result<()> { Err(Error::StaleConfirmation) }
    pub fn pause(&mut self) { self.emit(UiState::Paused); }
    pub fn resume(&mut self) -> Result<()> { Ok(()) }
    pub fn stop(&mut self) { self.emit(UiState::Idle); }
}
