//! macOS adapter for [`offhand_platform::Platform`]. Compiles to empty on other OSes.
#![cfg(target_os = "macos")]

use offhand_platform::{Action, Element, Error, Frame, Platform, Rect, Result, WindowId};

/// macOS implementation of [`Platform`] (Accessibility API + `CoreGraphics`).
#[derive(Debug, Default)]
pub struct MacPlatform;

impl Platform for MacPlatform {
    fn tree(&self, _window: Option<WindowId>) -> Result<Vec<Element>> {
        Err(Error::Unimplemented)
    }

    fn capture(&self, _region: Option<Rect>) -> Result<Frame> {
        Err(Error::Unimplemented)
    }

    fn act(&self, _action: &Action) -> Result<()> {
        Err(Error::Unimplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_contract_tests() {
        offhand_platform::contract::run_contract_tests(&MacPlatform);
    }

    #[test]
    fn stub_is_unimplemented() {
        assert!(matches!(MacPlatform.tree(None), Err(Error::Unimplemented)));
    }
}
