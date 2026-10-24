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

/// The agent: drives a [`Platform`] with an [`Engine`].
#[derive(Debug)]
pub struct Agent<P: Platform, E: Engine> {
    /// OS adapter.
    pub platform: P,
    /// Inference backend.
    pub engine: E,
}
