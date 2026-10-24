//! Windows adapter for [`offhand_platform::Platform`]. Compiles to empty on other OSes.
#![cfg(windows)]

use offhand_platform::{Action, Element, Error, Frame, Platform, Rect, Result, WindowId};

/// Windows implementation of [`Platform`] (UI Automation + Windows Graphics Capture).
#[derive(Debug, Default)]
pub struct WindowsPlatform;

impl Platform for WindowsPlatform {
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
