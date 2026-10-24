//! Inference engine interface.
//!
//! The `llama-cpp-2` backend lands after the week-1 spike (ADR-0002); until then this crate
//! only defines the [`Engine`] trait.

use offhand_platform::{Element, Frame};

/// Result alias for inference operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Inference errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No backend is available yet.
    #[error("not implemented")]
    Unimplemented,
    /// The backend failed.
    #[error("inference failed: {0}")]
    Backend(String),
}

/// The option the model picked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Choice {
    /// Index into the `options` slice passed to [`Engine::choose`].
    pub index: usize,
    /// Probability assigned to that option, in `0.0..=1.0`.
    pub prob: f64,
}

/// A local vision-language model backend.
pub trait Engine {
    /// Pick the best element for `task` from `options`, given the current screen.
    fn choose(&self, frame: &Frame, task: &str, options: &[Element]) -> Result<Choice>;
    /// Locate the screen point to act on for `task` when the accessibility tree is not enough.
    fn ground(&self, frame: &Frame, task: &str) -> Result<(f64, f64)>;
}
