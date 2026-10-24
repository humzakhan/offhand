//! macOS Accessibility adapter. Native references are confined to the calling thread.
#![cfg(target_os = "macos")]

mod native;

use native::Object;
use offhand_platform::{
    Action, Element, ElementId, Error, Frame, Permission, Permissions, Platform, Rect, Result,
    Role, WindowId,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    process::Command,
};

/// macOS implementation of [`Platform`] (Accessibility API + `CoreGraphics`).
#[derive(Debug, Default)]
pub struct MacPlatform;

#[derive(Default)]
struct Cache {
    next: u64,
    windows: Vec<(WindowId, Object, Object)>,
    elements: HashMap<usize, Vec<(ElementId, Object)>>,
    element_count: usize,
    target: Option<WindowId>,
}
thread_local! { static CACHE: RefCell<Cache> = RefCell::new(Cache::default()); }

impl Cache {
    fn element_id(&mut self, object: &Object) -> Result<ElementId> {
        let bucket = self.elements.entry(object.hash()).or_default();
        if let Some((id, _)) = bucket.iter().find(|(_, old)| old.same(object)) {
            return Ok(id.clone());
        }
        if self.element_count >= 4096 {
            return Err(Error::Other(
                "accessibility reference limit reached; recapture target window".into(),
            ));
        }
        self.next += 1;
        let id = ElementId(format!("ax:{}", self.next));
        bucket.push((id.clone(), object.clone()));
        self.element_count += 1;
        Ok(id)
    }
    fn window(&mut self, id: WindowId) -> Result<(Object, Object)> {
        let (_, app, window) = self
            .windows
            .iter()
            .find(|(old, _, _)| *old == id)
            .ok_or(Error::WindowNotFound(id))?;
        let pair = (app.clone(), window.clone());
        if self.target != Some(id) {
            self.elements.clear();
            self.element_count = 0;
            self.target = Some(id);
        }
        Ok(pair)
    }
}

fn describe(object: &Object, cache: &mut Cache) -> Result<Element> {
    let native_role = object
        .attr("AXRole")?
        .text()
        .ok_or_else(|| Error::Other("accessibility role unavailable".into()))?;
    let subrole = object.optional_attr("AXSubrole")?.and_then(|v| v.text());
    let role =
        if subrole.as_deref() == Some("AXSecureTextField") || native_role == "AXSecureTextField" {
            Role::PasswordField
        } else {
            match native_role.as_str() {
                "AXButton" => Role::Button,
                "AXTextField" | "AXTextArea" => Role::TextField,
                "AXMenuItem" => Role::MenuItem,
                "AXLink" => Role::Link,
                "AXCheckBox" => Role::Checkbox,
                _ => Role::Other(native_role),
            }
        };
    let label = ["AXTitle", "AXDescription", "AXHelp"]
        .iter()
        .find_map(|name| {
            object
                .attr(name)
                .ok()
                .and_then(|v| v.text())
                .filter(|v| !v.is_empty())
        });
    Ok(Element {
        id: cache.element_id(object)?,
        role,
        label,
        bounds: object.bounds().unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }),
        enabled: object
            .attr("AXEnabled")
            .ok()
            .and_then(|v| v.boolean())
            .unwrap_or(false),
    })
}

fn walk(
    object: &Object,
    depth: usize,
    output: &mut Vec<Element>,
    cache: &mut Cache,
    remaining: &mut usize,
) {
    if *remaining == 0 || depth >= 32 || native::tree_deadline_expired() { return; }
    *remaining -= 1;
    let Ok(pid) = object.pid() else { return; };
    if own_pid().ok() == Some(pid) { return; }
    let Ok(element) = describe(object, cache) else { return; };
    if native::tree_deadline_expired() || output.iter().any(|old| old.id == element.id) { return; }
    output.push(element);
    if depth + 1 >= 32 || *remaining == 0 { return; }
    let Ok(children) = object.children(*remaining) else { return; };
    for child in children {
        if *remaining == 0 || native::tree_deadline_expired() { break; }
        walk(&child, depth + 1, output, cache, remaining);
    }
}
fn validate_text(text: &str) -> Result<()> {
    if text.chars().take(2001).count() > 2000 {
        return Err(Error::Other("text exceeds 2000 characters".into()));
    }
    if text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\t')) {
        return Err(Error::Other("unsupported text control character".into()));
    }
    Ok(())
}
fn own_pid() -> Result<i32> {
    i32::try_from(std::process::id()).map_err(|e| Error::Other(e.to_string()))
}
fn target(window: Option<WindowId>) -> Result<WindowId> {
    window.map_or_else(|| MacPlatform.foreground_window(), Ok)
}
fn verify_input_target() -> Result<()> {
    native::require_trust()?;
    CACHE.with_borrow(|cache| {
        let target = cache
            .target
            .ok_or_else(|| Error::Other("no restored target window".into()))?;
        let (_, app, window) = cache
            .windows
            .iter()
            .find(|(id, _, _)| *id == target)
            .ok_or(Error::WindowNotFound(target))?;
        let front = Object::system()?.ax_attr("AXFocusedApplication")?;
        if front.pid()? != app.pid()?
            || front.pid()? == own_pid()?
            || !front.ax_attr("AXFocusedWindow")?.same(window)
        {
            return Err(Error::Other("target window lost foreground".into()));
        }
        Ok(())
    })
}
fn launch(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || name.starts_with('-')
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
    {
        return Err(Error::Other("invalid application name".into()));
    }
    let status = Command::new("/usr/bin/open")
        .arg("-a")
        .arg(name)
        .status()
        .map_err(|e| Error::Other(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Other("application launch failed".into()))
    }
}

impl Platform for MacPlatform {
    fn tree(&self, window: Option<WindowId>) -> Result<Vec<Element>> {
        native::require_trust()?;
        let id = target(window)?;
        native::with_tree_deadline(|| CACHE.with_borrow_mut(|cache| {
            let (_, window) = cache.window(id)?;
            let mut nodes = Vec::new();
            walk(&window, 0, &mut nodes, cache, &mut 2000);
            // Preserve identities for live nodes, release references absent from the fresh snapshot.
            let live: HashSet<_> = nodes.iter().map(|node| node.id.clone()).collect();
            cache.elements.retain(|_, bucket| {
                bucket.retain(|(id, _)| live.contains(id));
                !bucket.is_empty()
            });
            cache.element_count = cache.elements.values().map(Vec::len).sum();
            Ok(nodes)
        }))
    }
    fn capture(&self, _region: Option<Rect>) -> Result<Frame> {
        Err(Error::Unimplemented)
    }
    fn act(&self, action: &Action) -> Result<()> {
        native::require_trust()?;
        match action {
            Action::OpenApp(name) => launch(name),
            Action::FocusWindow(id) => CACHE.with_borrow_mut(|cache| {
                let (app, window) = cache.window(*id)?;
                if app.pid()? == own_pid()? {
                    return Err(Error::WindowNotFound(*id));
                }
                app.set_true("AXFrontmost")?;
                window.perform("AXRaise")?;
                let _ = app.set("AXFocusedWindow", &window);
                let front = Object::system()?.ax_attr("AXFocusedApplication")?;
                if front.pid()? != app.pid()? || !front.ax_attr("AXFocusedWindow")?.same(&window) {
                    return Err(Error::Other("target window could not be restored".into()));
                }
                Ok(())
            }),
            Action::Press(id) => CACHE.with_borrow(|cache| {
                let object = cache
                    .elements
                    .values()
                    .flatten()
                    .find(|(old, _)| old == id)
                    .map(|(_, object)| object)
                    .ok_or_else(|| Error::ElementNotFound(id.clone()))?;
                if object.pid()? == own_pid()? {
                    return Err(Error::ElementNotFound(id.clone()));
                }
                if object.attr("AXEnabled")?.boolean() != Some(true) {
                    return Err(Error::Other("element is disabled".into()));
                }
                object.perform("AXPress")
            }),
            Action::Type(text) => {
                validate_text(text)?;
                // One Unicode scalar per pair avoids splitting a surrogate pair.
                for scalar in text.chars() {
                    verify_input_target()?;
                    match scalar {
                        '\n' => native::keyboard(36, 0, None)?,
                        '\t' => native::keyboard(48, 0, None)?,
                        _ => {
                            let mut units = [0; 2];
                            native::keyboard(0, 0, Some(scalar.encode_utf16(&mut units)))?;
                        }
                    }
                }
                Ok(())
            }
            Action::Key(chord) => {
                let (key, flags) = key(chord)?;
                verify_input_target()?;
                native::keyboard(key, flags, None)
            }
            Action::Click { .. } | Action::Scroll { .. } => Err(Error::Unimplemented),
        }
    }
    fn foreground_window(&self) -> Result<WindowId> {
        native::require_trust()?;
        let app = Object::system()?.ax_attr("AXFocusedApplication")?;
        if app.pid()? == own_pid()? {
            return Err(Error::Other(
                "Offhand is foreground; capture the external target before activation".into(),
            ));
        }
        let window = app.ax_attr("AXFocusedWindow")?;
        CACHE.with_borrow_mut(|cache| {
            if let Some((id, _, _)) = cache.windows.iter().find(|(_, _, old)| old.same(&window)) {
                return Ok(*id);
            }
            // A closed AX window can retain a valid app PID, so probe the window itself.
            cache.windows.retain(|(_, app, window)| app.pid().is_ok() && window.attr("AXRole").is_ok());
            if cache.windows.len() >= 128 {
                if let Some(index) = cache.windows.iter().position(|(id, _, _)| Some(*id) != cache.target) {
                    // Removing the tuple releases both retained CF references.
                    cache.windows.remove(index);
                }
            }
            cache.next += 1;
            let id = WindowId(cache.next);
            cache.windows.push((id, app, window));
            Ok(id)
        })
    }
    fn focused_element(&self, window: Option<WindowId>) -> Result<Option<Element>> {
        native::require_trust()?;
        let id = target(window)?;
        CACHE.with_borrow_mut(|cache| {
            let (app, target) = cache.window(id)?;
            if !app.ax_attr("AXFocusedWindow")?.same(&target) {
                return Ok(None);
            }
            let Ok(focused) = app.ax_attr("AXFocusedUIElement") else {
                return Ok(None);
            };
            if focused.pid()? == own_pid()? {
                return Ok(None);
            }
            describe(&focused, cache).map(Some)
        })
    }
    fn permissions(&self) -> Permissions {
        Permissions {
            accessibility: Some(native::trusted()),
            screen_recording: Some(native::recording()),
        }
    }
    fn request_permission(&self, permission: Permission) -> Result<()> {
        match permission {
            Permission::Accessibility => {
                let status = Command::new("/usr/bin/open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility").status().map_err(|e| Error::Other(e.to_string()))?;
                if !status.success() {
                    return Err(Error::Other("could not open Accessibility settings".into()));
                }
            }
            Permission::ScreenRecording => native::request_recording(),
        }
        Ok(())
    }
}

fn key(chord: &str) -> Result<(u16, u64)> {
    let parts: Vec<_> = chord.split('+').collect();
    let mut flags = 0;
    for modifier in &parts[..parts.len().saturating_sub(1)] {
        flags |= match modifier.to_ascii_lowercase().as_str() {
            "ctrl" => 1 << 18,
            "alt" => 1 << 19,
            "shift" => 1 << 17,
            "meta" | "primary" => 1 << 20,
            _ => return Err(Error::Other("unsupported key modifier".into())),
        };
    }
    let base = parts
        .last()
        .copied()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let code = match base.as_str() {
        "enter" => 36,
        "tab" => 48,
        "space" => 49,
        "backspace" => 51,
        "delete" => 117,
        "left" | "arrowleft" => 123,
        "right" | "arrowright" => 124,
        "down" | "arrowdown" => 125,
        "up" | "arrowup" => 126,
        "home" => 115,
        "end" => 119,
        "pageup" => 116,
        "pagedown" => 121,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        "a" => 0,
        "s" => 1,
        "d" => 2,
        "f" => 3,
        "h" => 4,
        "g" => 5,
        "z" => 6,
        "x" => 7,
        "c" => 8,
        "v" => 9,
        "b" => 11,
        "q" => 12,
        "w" => 13,
        "e" => 14,
        "r" => 15,
        "y" => 16,
        "t" => 17,
        "1" => 18,
        "2" => 19,
        "3" => 20,
        "4" => 21,
        "6" => 22,
        "5" => 23,
        "9" => 25,
        "7" => 26,
        "8" => 28,
        "0" => 29,
        "o" => 31,
        "u" => 32,
        "i" => 34,
        "p" => 35,
        "l" => 37,
        "j" => 38,
        "k" => 40,
        "n" => 45,
        "m" => 46,
        _ => return Err(Error::Other("unsupported or reserved key".into())),
    };
    Ok((code, flags))
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
        assert!(matches!(
            MacPlatform.capture(None),
            Err(Error::Unimplemented)
        ));
    }
}
