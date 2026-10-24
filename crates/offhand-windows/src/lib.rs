//! Windows UI Automation adapter; COM state stays on the constructing thread.
#![cfg(windows)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use offhand_platform::{
    Action, Element, ElementId, Error, Frame, Permission, Permissions, Platform, Rect, Result,
    Role, WindowId,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::c_void,
    mem::size_of,
    ptr,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND},
        Security::{GetTokenInformation, GetSidSubAuthorityCount, GetSidSubAuthority, TOKEN_QUERY, TOKEN_MANDATORY_LABEL, TokenIntegrityLevel},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize, SAFEARRAY,
            },
            Ole::{
                SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
                SafeArrayUnaccessData,
            },
            Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION},
        },
        UI::{
            Accessibility::{
                CUIAutomation8, IUIAutomation2, IUIAutomationElement, IUIAutomationInvokePattern,
                IUIAutomationTreeWalker, UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId,
                UIA_EditControlTypeId, UIA_HyperlinkControlTypeId, UIA_InvokePatternId,
                UIA_MenuItemControlTypeId,
            },
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
                KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY,
            },
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow, SW_RESTORE,
                SW_SHOWNORMAL, SetForegroundWindow, ShowWindow,
            },
        },
    },
    core::PCWSTR,
};

/// Windows implementation of [`Platform`] using UI Automation and `SendInput`.
#[derive(Debug, Default)]
pub struct WindowsPlatform;

struct State {
    automation: Option<IUIAutomation2>,
    elements: HashMap<ElementId, IUIAutomationElement>,
    windows: HashMap<WindowId, u32>,
    target: Option<WindowId>,
}
impl State {
    fn new() -> Result<Self> {
        // SAFETY: initialization and every COM call occur on this same TLS-owned thread.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(native_error)?;
        // SAFETY: the current apartment is initialized; the class/interface IDs are matching UIA types.
        let automation = match unsafe {
            CoCreateInstance::<_, IUIAutomation2>(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
        } {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: balances the successful initialization above on this thread.
                unsafe { CoUninitialize() };
                return Err(native_error(error));
            }
        };
        let state = Self {
            automation: Some(automation),
            elements: HashMap::new(),
            windows: HashMap::new(),
            target: None,
        };
        // SAFETY: UIA object belongs to the current initialized apartment; values are milliseconds.
        unsafe { state.automation()?.SetConnectionTimeout(200) }.map_err(native_error)?;
        // SAFETY: same UIA ownership invariant; bounds provider transaction stalls.
        unsafe { state.automation()?.SetTransactionTimeout(200) }.map_err(native_error)?;
        Ok(state)
    }
    fn automation(&self) -> Result<&IUIAutomation2> {
        self.automation
            .as_ref()
            .ok_or_else(|| Error::Other("UI Automation is closed".into()))
    }
    fn target(&mut self, id: WindowId) -> Result<HWND> {
        let hwnd = hwnd(id)?;
        let expected = self.windows.get(&id).ok_or(Error::WindowNotFound(id))?;
        if window_pid(hwnd)? != *expected || *expected == own_pid() {
            return Err(Error::WindowNotFound(id));
        }
        if self.target != Some(id) {
            self.elements.clear();
            self.target = Some(id);
        }
        Ok(hwnd)
    }
    fn describe(&mut self, object: &IUIAutomationElement) -> Result<Element> {
        let id = runtime_id(object)?;
        if !self.elements.contains_key(&id) && self.elements.len() >= 4096 {
            return Err(Error::Other(
                "UI Automation reference limit reached; recapture target".into(),
            ));
        }
        // SAFETY: object is retained in this apartment for the duration of every property call.
        let password = unsafe { object.CurrentIsPassword() }
            .map_err(native_error)?
            .as_bool();
        // SAFETY: live apartment-local UIA receiver.
        let native_role = unsafe { object.CurrentControlType() }.map_err(native_error)?;
        let role = if password {
            Role::PasswordField
        } else if native_role == UIA_ButtonControlTypeId {
            Role::Button
        } else if native_role == UIA_EditControlTypeId {
            Role::TextField
        } else if native_role == UIA_MenuItemControlTypeId {
            Role::MenuItem
        } else if native_role == UIA_HyperlinkControlTypeId {
            Role::Link
        } else if native_role == UIA_CheckBoxControlTypeId {
            Role::Checkbox
        } else {
            Role::Other(format!("UIA:{}", native_role.0))
        };
        // SAFETY: live apartment-local UIA receiver; BSTR ownership is handled by bindings.
        let label = unsafe { object.CurrentName() }
            .map_err(native_error)?
            .to_string();
        // SAFETY: UIA returns desktop physical pixel coordinates without DPI virtualization.
        let bounds = unsafe { object.CurrentBoundingRectangle() }.map_err(native_error)?;
        // SAFETY: live apartment-local UIA receiver.
        let enabled = unsafe { object.CurrentIsEnabled() }
            .map_err(native_error)?
            .as_bool();
        self.elements.insert(id.clone(), object.clone());
        Ok(Element {
            id,
            role,
            label: (!label.is_empty()).then_some(label),
            enabled,
            bounds: Rect {
                x: f64::from(bounds.left),
                y: f64::from(bounds.top),
                w: f64::from(bounds.right) - f64::from(bounds.left),
                h: f64::from(bounds.bottom) - f64::from(bounds.top),
            },
        })
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.elements.clear();
        self.automation.take();
        // SAFETY: all COM references were released before balancing this thread's CoInitializeEx.
        unsafe { CoUninitialize() };
    }
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
fn with_state<T>(operation: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
    STATE.with_borrow_mut(|slot| {
        if slot.is_none() {
            *slot = Some(State::new()?);
        }
        operation(
            slot.as_mut()
                .ok_or_else(|| Error::Other("UI Automation unavailable".into()))?,
        )
    })
}
// Result::map_err transfers ownership of its source error to this converter.
#[allow(clippy::needless_pass_by_value)]
fn native_error(error: windows::core::Error) -> Error {
    if error.code().0 == 0x8007_0005_u32 as i32 {
        Error::PermissionDenied("Windows denied access (target may have higher integrity)".into())
    } else {
        Error::Other(format!("Windows API: {error}"))
    }
}
fn own_pid() -> u32 {
    // SAFETY: GetCurrentProcessId has no preconditions.
    unsafe { GetCurrentProcessId() }
}
fn hwnd(id: WindowId) -> Result<HWND> {
    let raw = usize::try_from(id.0).map_err(|_| Error::WindowNotFound(id))?;
    let hwnd = HWND(raw as *mut c_void);
    // SAFETY: IsWindow validates the opaque OS handle before any window operation.
    if hwnd.0.is_null() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err(Error::WindowNotFound(id));
    }
    Ok(hwnd)
}
fn window_id(hwnd: HWND) -> WindowId {
    WindowId(hwnd.0 as usize as u64)
}
fn window_pid(hwnd: HWND) -> Result<u32> {
    let mut pid = 0;
    // SAFETY: HWND is an opaque OS handle; pid points to writable initialized storage.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    if thread == 0 || pid == 0 {
        Err(Error::WindowNotFound(window_id(hwnd)))
    } else {
        Ok(pid)
    }
}

struct RuntimeArray(*mut SAFEARRAY);
impl Drop for RuntimeArray {
    fn drop(&mut self) {
        // SAFETY: owns the SAFEARRAY returned by GetRuntimeId, never destroyed elsewhere.
        let _ = unsafe { SafeArrayDestroy(self.0) };
    }
}
fn runtime_id(object: &IUIAutomationElement) -> Result<ElementId> {
    // SAFETY: live apartment-local receiver; returns an owned SAFEARRAY of i32 runtime IDs.
    let array = RuntimeArray(unsafe { object.GetRuntimeId() }.map_err(native_error)?);
    if array.0.is_null() {
        return Err(Error::Other("UIA runtime ID unavailable".into()));
    }
    // SAFETY: UIA runtime IDs have one dimension; bounds are read before accessing data.
    let low = unsafe { SafeArrayGetLBound(array.0, 1) }.map_err(native_error)?;
    // SAFETY: same live one-dimensional SAFEARRAY.
    let high = unsafe { SafeArrayGetUBound(array.0, 1) }.map_err(native_error)?;
    let count = i64::from(high) - i64::from(low) + 1;
    if !(1..=256).contains(&count) {
        return Err(Error::Other("invalid UIA runtime ID length".into()));
    }
    let mut data = ptr::null_mut();
    // SAFETY: live SAFEARRAY and writable pointer; locks it until UnaccessData below.
    unsafe { SafeArrayAccessData(array.0, &raw mut data) }.map_err(native_error)?;
    if data.is_null() {
        // SAFETY: balances successful lock of the live array, even for invalid provider data.
        let _ = unsafe { SafeArrayUnaccessData(array.0) };
        return Err(Error::Other("UIA runtime ID data unavailable".into()));
    }
    // SAFETY: UIA specifies VT_I4 entries, count came from its validated bounds; array is locked.
    let values = unsafe { std::slice::from_raw_parts(data.cast::<i32>(), count as usize) };
    let id = ElementId(format!("uia:{values:?}"));
    // SAFETY: balances successful SafeArrayAccessData before the owner destroys the array.
    unsafe { SafeArrayUnaccessData(array.0) }.map_err(native_error)?;
    Ok(id)
}

fn walk(
    state: &mut State,
    walker: &IUIAutomationTreeWalker,
    object: &IUIAutomationElement,
    depth: usize,
    output: &mut Vec<Element>,
    seen: &mut HashSet<ElementId>,
    remaining: &mut usize,
) -> Result<()> {
    if depth >= 32 || *remaining == 0 {
        return Ok(());
    }
    *remaining -= 1;
    // SAFETY: live receiver on its initialized apartment thread.
    if unsafe { object.CurrentProcessId() }.map_err(native_error)? as u32 == own_pid() {
        return Ok(());
    }
    let element = state.describe(object)?;
    if !seen.insert(element.id.clone()) {
        return Ok(());
    }
    output.push(element);
    if depth + 1 >= 32 || *remaining == 0 {
        return Ok(());
    }
    // SAFETY: live walker and element on the same initialized apartment thread.
    let mut child = optional_element(unsafe { walker.GetFirstChildElement(object) })?;
    while let Some(current) = child {
        if *remaining == 0 {
            break;
        }
        walk(state, walker, &current, depth + 1, output, seen, remaining)?;
        // SAFETY: current remains retained until after its sibling is obtained.
        child = optional_element(unsafe { walker.GetNextSiblingElement(&current) })?;
    }
    Ok(())
}
fn foreground() -> Result<HWND> {
    // SAFETY: GetForegroundWindow has no preconditions.
    let current = unsafe { GetForegroundWindow() };
    let pid = window_pid(current)?;
    if pid == own_pid() {
        return Err(Error::Other(
            "capture the external target before activating Offhand".into(),
        ));
    }
    Ok(current)
}
fn verify_input_target() -> Result<()> {
    with_state(|state| {
        let id = state
            .target
            .ok_or_else(|| Error::Other("no restored target window".into()))?;
        let expected = state.target(id)?;
        if foreground()? != expected {
            return Err(Error::Other("target window lost foreground".into()));
        }
        // Probe process access too: access-denied must not be mistaken for successful input.
        // SAFETY: query-only access to an existing external process, handle closed below.
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                window_pid(expected)?,
            )
        }
        .map_err(native_error)?;
        let process = OwnedHandle(process);
        let target_integrity = integrity_level(process.0)?;
        // SAFETY: GetCurrentProcess returns a valid pseudo-handle, which must not be closed.
        let current = unsafe { GetCurrentProcess() };
        if target_integrity > integrity_level(current)? {
            return Err(Error::PermissionDenied("target has higher Windows integrity; input is blocked".into()));
        }
        Ok(())
    })
}
struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: owns a successful OpenProcess/OpenProcessToken result, closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}
fn integrity_level(process: HANDLE) -> Result<u32> {
    let mut token = HANDLE::default();
    // SAFETY: process is a live handle and token is writable out storage.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) }.map_err(native_error)?;
    let token = OwnedHandle(token);
    let mut required = 0;
    // SAFETY: null information buffer probes the required byte count in writable out storage.
    let _ = unsafe { GetTokenInformation(token.0, TokenIntegrityLevel, None, 0, &raw mut required) };
    if required < size_of::<TOKEN_MANDATORY_LABEL>() as u32 || required > 4096 {
        return Err(Error::Other("invalid Windows integrity information size".into()));
    }
    // usize storage provides the native pointer alignment required by TOKEN_MANDATORY_LABEL.
    let mut storage = vec![0_usize; (required as usize).div_ceil(size_of::<usize>())];
    // SAFETY: allocated aligned buffer is at least required bytes and token is live.
    unsafe { GetTokenInformation(token.0, TokenIntegrityLevel, Some(storage.as_mut_ptr().cast()), required, &raw mut required) }.map_err(native_error)?;
    // SAFETY: OS returned a TOKEN_MANDATORY_LABEL in a sufficiently large aligned buffer.
    let label = unsafe { &*storage.as_ptr().cast::<TOKEN_MANDATORY_LABEL>() };
    // SAFETY: TokenIntegrityLevel supplies a valid SID retained within storage.
    let count = unsafe { *GetSidSubAuthorityCount(label.Label.Sid) };
    if count == 0 { return Err(Error::Other("empty Windows integrity SID".into())); }
    // SAFETY: last subauthority index is within the validated SID count and storage is live.
    Ok(unsafe { *GetSidSubAuthority(label.Label.Sid, u32::from(count - 1)) })
}
fn optional_element(value: windows::core::Result<IUIAutomationElement>) -> Result<Option<IUIAutomationElement>> {
    match value {
        Ok(value) => Ok(Some(value)),
        // A successful native null child/sibling becomes E_POINTER in windows-rs.
        Err(error) if error.code().0 == 0x8000_4003_u32 as i32 => Ok(None),
        Err(error) => Err(native_error(error)),
    }
}
fn input(key: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(key),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
fn send(inputs: &[INPUT]) -> Result<()> {
    verify_input_target()?;
    // SAFETY: initialized INPUT records, stable slice, correct ABI structure size.
    let count = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if count as usize == inputs.len() {
        Ok(())
    } else {
        release_partial_input(&inputs[..(count as usize).min(inputs.len())]);
        Err(Error::PermissionDenied("SendInput was blocked or partially delivered (integrity/UIPI or desktop restriction); action was not retried".into()))
    }
}
fn release_partial_input(inserted: &[INPUT]) {
    let mut held = HashMap::new();
    for event in inserted {
        if event.r#type != INPUT_KEYBOARD { continue; }
        // SAFETY: the tag above identifies the active KEYBDINPUT member of this INPUT union.
        let keyboard = unsafe { event.Anonymous.ki };
        let key = (keyboard.wVk.0, keyboard.wScan);
        if keyboard.dwFlags.contains(KEYEVENTF_KEYUP) {
            held.remove(&key);
        } else {
            held.insert(key, keyboard.dwFlags);
        }
    }
    let releases: Vec<_> = held.into_iter().map(|((key, scan), flags)| input(key, scan, flags | KEYEVENTF_KEYUP)).collect();
    if !releases.is_empty() {
        // SAFETY: initialized keyboard release records and exact ABI size. Only release
        // events are sent; the failed text/key action is never replayed.
        let _ = unsafe { SendInput(&releases, size_of::<INPUT>() as i32) };
    }
}
fn modifier_input(key: u16, up: bool) -> INPUT {
    let mut flags = if key == 0x5b { KEYEVENTF_EXTENDEDKEY } else { KEYBD_EVENT_FLAGS(0) };
    if up { flags |= KEYEVENTF_KEYUP; }
    input(key, 0, flags)
}
fn launch(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || name.starts_with('-')
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
    {
        return Err(Error::Other("invalid application name".into()));
    }
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // SAFETY: NUL-terminated strings live through call; no arguments, path, shell command or parent HWND.
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(name.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "application launch failed: {}",
            result.0 as isize
        )))
    }
}
impl Platform for WindowsPlatform {
    fn tree(&self, window: Option<WindowId>) -> Result<Vec<Element>> {
        let id = window.map_or_else(|| self.foreground_window(), Ok)?;
        with_state(|state| {
            let hwnd = state.target(id)?;
            // SAFETY: HWND was validated; UIA receiver belongs to current initialized apartment.
            let root =
                unsafe { state.automation()?.ElementFromHandle(hwnd) }.map_err(native_error)?;
            // SAFETY: apartment-local UIA receiver.
            let walker = unsafe { state.automation()?.RawViewWalker() }.map_err(native_error)?;
            let mut nodes = Vec::new();
            let mut seen = HashSet::new();
            walk(state, &walker, &root, 0, &mut nodes, &mut seen, &mut 2000)?;
            state.elements.retain(|id, _| seen.contains(id));
            Ok(nodes)
        })
    }
    fn capture(&self, _region: Option<Rect>) -> Result<Frame> {
        Err(Error::Unimplemented)
    }
    fn act(&self, action: &Action) -> Result<()> {
        match action {
            Action::OpenApp(name) => launch(name),
            Action::FocusWindow(id) => with_state(|state| {
                let hwnd = state.target(*id)?;
                // SAFETY: HWND was validated and belongs to the captured external process.
                if unsafe { IsIconic(hwnd) }.as_bool() {
                    // SAFETY: validated HWND; restore only the exact captured target.
                    let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
                }
                // SAFETY: validated HWND. Windows may refuse foreground activation; detect that below.
                if !unsafe { SetForegroundWindow(hwnd) }.as_bool() || foreground()? != hwnd {
                    return Err(Error::Other(
                        "Windows refused to restore the target window".into(),
                    ));
                }
                Ok(())
            }),
            Action::Press(id) => with_state(|state| {
                let object = state
                    .elements
                    .get(id)
                    .ok_or_else(|| Error::ElementNotFound(id.clone()))?;
                // SAFETY: retained UIA element in its owning COM apartment.
                if unsafe { object.CurrentProcessId() }.map_err(native_error)? as u32 == own_pid() {
                    return Err(Error::ElementNotFound(id.clone()));
                }
                // SAFETY: retained UIA element; disabled controls must not be invoked.
                if !unsafe { object.CurrentIsEnabled() }
                    .map_err(native_error)?
                    .as_bool()
                {
                    return Err(Error::Other("element is disabled".into()));
                }
                // SAFETY: typed pattern ID matches the requested interface, both in same apartment.
                let invoke: IUIAutomationInvokePattern =
                    unsafe { object.GetCurrentPatternAs(UIA_InvokePatternId) }
                        .map_err(native_error)?;
                // SAFETY: retained Invoke pattern on current initialized COM apartment.
                unsafe { invoke.Invoke() }.map_err(native_error)
            }),
            Action::Type(text) => {
                for scalar in text.chars() {
                    let mut units = [0; 2];
                    let mut events = Vec::new();
                    for &unit in scalar.encode_utf16(&mut units).iter() {
                        events.push(input(0, unit, KEYEVENTF_UNICODE));
                        events.push(input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
                    }
                    send(&events)?;
                }
                Ok(())
            }
            Action::Key(chord) => {
                let (key, modifiers, extended) = key(chord)?;
                let mut events: Vec<_> = modifiers
                    .iter()
                    .map(|&key| modifier_input(key, false))
                    .collect();
                let flag = if extended {
                    KEYEVENTF_EXTENDEDKEY
                } else {
                    KEYBD_EVENT_FLAGS(0)
                };
                events.push(input(key, 0, flag));
                events.push(input(key, 0, flag | KEYEVENTF_KEYUP));
                events.extend(
                    modifiers
                        .iter()
                        .rev()
                        .map(|&key| modifier_input(key, true)),
                );
                send(&events)
            }
            Action::Click { .. } | Action::Scroll { .. } => Err(Error::Unimplemented),
        }
    }
    fn foreground_window(&self) -> Result<WindowId> {
        let hwnd = foreground()?;
        let id = window_id(hwnd);
        let pid = window_pid(hwnd)?;
        with_state(|state| {
            if state.windows.len() >= 128 && !state.windows.contains_key(&id) {
                return Err(Error::Other("window reference limit reached".into()));
            }
            state.windows.insert(id, pid);
            Ok(id)
        })
    }
    fn focused_element(&self, window: Option<WindowId>) -> Result<Option<Element>> {
        let id = window.map_or_else(|| self.foreground_window(), Ok)?;
        with_state(|state| {
            let expected = state.target(id)?;
            if foreground()? != expected {
                return Ok(None);
            }
            // SAFETY: UIA receiver is apartment-local and initialized.
            let Some(object) = optional_element(unsafe { state.automation()?.GetFocusedElement() })? else {
                return Ok(None);
            };
            // SAFETY: retained UIA element; process identity checked before exposing its properties.
            if unsafe { object.CurrentProcessId() }.map_err(native_error)? as u32
                != window_pid(expected)?
            {
                return Ok(None);
            }
            state.describe(&object).map(Some)
        })
    }
    fn permissions(&self) -> Permissions {
        Permissions::default()
    }
    fn request_permission(&self, _permission: Permission) -> Result<()> {
        Ok(())
    }
}
fn key(chord: &str) -> Result<(u16, Vec<u16>, bool)> {
    let parts: Vec<_> = chord.split('+').collect();
    let mut modifiers = Vec::new();
    for modifier in &parts[..parts.len().saturating_sub(1)] {
        let key = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "primary" => 0x11,
            "alt" => 0x12,
            "shift" => 0x10,
            "meta" => 0x5b,
            _ => return Err(Error::Other("unsupported key modifier".into())),
        };
        if !modifiers.contains(&key) {
            modifiers.push(key);
        }
    }
    let base = parts
        .last()
        .copied()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let code = match base.as_str() {
        "enter" => 0x0d,
        "tab" => 0x09,
        "space" => 0x20,
        "backspace" => 0x08,
        "delete" => 0x2e,
        "left" | "arrowleft" => 0x25,
        "up" | "arrowup" => 0x26,
        "right" | "arrowright" => 0x27,
        "down" | "arrowdown" => 0x28,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "f1" => 0x70,
        "f2" => 0x71,
        "f3" => 0x72,
        "f4" => 0x73,
        "f5" => 0x74,
        "f6" => 0x75,
        "f7" => 0x76,
        "f8" => 0x77,
        "f9" => 0x78,
        "f10" => 0x79,
        "f11" => 0x7a,
        "f12" => 0x7b,
        _ if base.len() == 1 && base.as_bytes()[0].is_ascii_alphanumeric() => {
            u16::from(base.as_bytes()[0].to_ascii_uppercase())
        }
        _ => return Err(Error::Other("unsupported or reserved key".into())),
    };
    Ok((
        code,
        modifiers,
        (0x21..=0x28).contains(&code) || code == 0x2e,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passes_contract_tests() {
        offhand_platform::contract::run_contract_tests(&WindowsPlatform);
    }
}
