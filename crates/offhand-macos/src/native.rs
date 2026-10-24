//! Small ownership-checked wrappers around the C Accessibility/CoreGraphics APIs.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use offhand_platform::{Error, Rect, Result};
use std::{
    ffi::{CString, c_char, c_void},
    ptr,
};

type Ref = *const c_void;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Size {
    w: f64,
    h: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CRect {
    origin: Point,
    size: Size,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(value: Ref) -> Ref;
    fn CFRelease(value: Ref);
    fn CFEqual(a: Ref, b: Ref) -> u8;
    fn CFHash(value: Ref) -> usize;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(value: Ref) -> u8;
    fn CFStringCreateWithCString(allocator: Ref, text: *const c_char, encoding: u32) -> Ref;
    fn CFStringGetLength(value: Ref) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(value: Ref, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFArrayGetCount(value: Ref) -> isize;
    fn CFArrayGetValueAtIndex(value: Ref, index: isize) -> Ref;
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateSystemWide() -> Ref;
    fn AXUIElementCopyAttributeValue(element: Ref, attr: Ref, value: *mut Ref) -> i32;
    fn AXUIElementCopyAttributeValues(
        element: Ref,
        attr: Ref,
        index: isize,
        max: isize,
        value: *mut Ref,
    ) -> i32;
    fn AXUIElementSetAttributeValue(element: Ref, attr: Ref, value: Ref) -> i32;
    fn AXUIElementPerformAction(element: Ref, action: Ref) -> i32;
    fn AXUIElementSetMessagingTimeout(element: Ref, seconds: f32) -> i32;
    fn AXUIElementGetPid(element: Ref, pid: *mut i32) -> i32;
    fn AXUIElementGetTypeID() -> usize;
    fn AXValueGetTypeID() -> usize;
    fn AXValueGetValue(value: Ref, kind: u32, output: *mut c_void) -> bool;
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGGetDisplaysWithPoint(point: Point, max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayBounds(display: u32) -> CRect;
    fn CGDisplayPixelsWide(display: u32) -> usize;
    fn CGEventCreateKeyboardEvent(source: Ref, key: u16, down: bool) -> Ref;
    fn CGEventKeyboardSetUnicodeString(event: Ref, length: usize, text: *const u16);
    fn CGEventSetFlags(event: Ref, flags: u64);
    fn CGEventPost(tap: u32, event: Ref);
    static kCFBooleanTrue: Ref;
}

/// An owned CF/AX reference; never leaves its creating thread.
#[derive(Debug)]
pub(super) struct Object(Ref);
impl Clone for Object {
    fn clone(&self) -> Self {
        // SAFETY: self owns a live CF reference; retain creates another ownership claim.
        Self(unsafe { CFRetain(self.0) })
    }
}
impl Drop for Object {
    fn drop(&mut self) {
        // SAFETY: every Object owns exactly one non-null create/copy/retain reference.
        unsafe { CFRelease(self.0) };
    }
}
impl Object {
    fn owned(value: Ref) -> Result<Self> {
        if value.is_null() {
            Err(Error::Other("native object unavailable".into()))
        } else {
            Ok(Self(value))
        }
    }
    fn string(text: &str) -> Result<Self> {
        let text = CString::new(text).map_err(|_| Error::Other("invalid native string".into()))?;
        // SAFETY: NUL-terminated input lives through the call; default allocator is valid.
        Self::owned(unsafe { CFStringCreateWithCString(ptr::null(), text.as_ptr(), 0x0800_0100) })
    }
    pub(super) fn system() -> Result<Self> {
        // SAFETY: no arguments; returns a newly owned AX reference.
        let object = Self::owned(unsafe { AXUIElementCreateSystemWide() })?;
        object.timeout();
        Ok(object)
    }
    fn timeout(&self) {
        // SAFETY: self is an AX element at all call sites; timeout bounds provider stalls.
        unsafe { AXUIElementSetMessagingTimeout(self.0, 0.15) };
    }
    pub(super) fn attr(&self, name: &str) -> Result<Self> {
        let name = Self::string(name)?;
        let mut value = ptr::null();
        // SAFETY: self/name are live references; value is writable out storage.
        status(unsafe { AXUIElementCopyAttributeValue(self.0, name.0, &raw mut value) })?;
        Self::owned(value)
    }
    pub(super) fn ax_attr(&self, name: &str) -> Result<Self> {
        let value = self.attr(name)?;
        // SAFETY: CF type checks accept any live CF object.
        if unsafe { CFGetTypeID(value.0) != AXUIElementGetTypeID() } {
            return Err(Error::Other(
                "accessibility attribute is not an element".into(),
            ));
        }
        value.timeout();
        Ok(value)
    }
    pub(super) fn children(&self, limit: usize) -> Result<Vec<Self>> {
        let limit = limit.min(2000);
        let native_limit = isize::try_from(limit).map_err(|e| Error::Other(e.to_string()))?;
        let name = Self::string("AXChildren")?;
        let mut raw = ptr::null();
        // SAFETY: valid AX receiver, CFString key and out pointer; limit is at most 2000.
        let result = unsafe {
            AXUIElementCopyAttributeValues(self.0, name.0, 0, native_limit, &raw mut raw)
        };
        if result == -25205 || result == -25212 {
            return Ok(Vec::new());
        }
        status(result)?;
        let array = Self::owned(raw)?;
        // SAFETY: array is a live CF object; checked before using CFArray operations.
        if unsafe { CFGetTypeID(array.0) != CFArrayGetTypeID() } {
            return Ok(Vec::new());
        }
        // SAFETY: type was checked above.
        let count = unsafe { CFArrayGetCount(array.0) }.max(0) as usize;
        let mut result = Vec::new();
        for index in 0..count.min(limit) {
            let index = isize::try_from(index).map_err(|e| Error::Other(e.to_string()))?;
            // SAFETY: index is in bounds; array retains its borrowed child during retain.
            let child = unsafe { CFArrayGetValueAtIndex(array.0, index) };
            // SAFETY: array children are CF references; validate AX type before wrapping.
            if !child.is_null() && unsafe { CFGetTypeID(child) == AXUIElementGetTypeID() } {
                // SAFETY: child is live and retained by array until this call completes.
                let child = Self::owned(unsafe { CFRetain(child) })?;
                child.timeout();
                result.push(child);
            }
        }
        Ok(result)
    }
    pub(super) fn text(&self) -> Option<String> {
        // SAFETY: self is live; string operations follow a successful type check.
        if unsafe { CFGetTypeID(self.0) != CFStringGetTypeID() } {
            return None;
        }
        // SAFETY: self is a CFString and the encoding is UTF-8.
        let size =
            unsafe { CFStringGetMaximumSizeForEncoding(CFStringGetLength(self.0), 0x0800_0100) };
        if !(0..=1_048_576).contains(&size) {
            return None;
        }
        let mut bytes = vec![0_u8; size as usize + 1];
        // SAFETY: writable buffer is size+1 bytes, matching the supplied capacity.
        if unsafe { CFStringGetCString(self.0, bytes.as_mut_ptr().cast(), size + 1, 0x0800_0100) }
            == 0
        {
            return None;
        }
        let end = bytes.iter().position(|&b| b == 0)?;
        String::from_utf8(bytes[..end].to_vec()).ok()
    }
    pub(super) fn boolean(&self) -> Option<bool> {
        // SAFETY: boolean access follows a successful CFBoolean type check.
        (unsafe { CFGetTypeID(self.0) == CFBooleanGetTypeID() })
            .then(|| unsafe { CFBooleanGetValue(self.0) != 0 })
    }
    pub(super) fn pid(&self) -> Result<i32> {
        let mut pid = 0;
        // SAFETY: valid AX reference and writable pid storage.
        status(unsafe { AXUIElementGetPid(self.0, &raw mut pid) })?;
        Ok(pid)
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        // SAFETY: both references are live CF objects.
        unsafe { CFEqual(self.0, other.0) != 0 }
    }
    pub(super) fn hash(&self) -> usize {
        // SAFETY: live CF object.
        unsafe { CFHash(self.0) }
    }
    pub(super) fn perform(&self, action: &str) -> Result<()> {
        let action = Self::string(action)?;
        // SAFETY: receiver is an AX element and action a live CFString.
        status(unsafe { AXUIElementPerformAction(self.0, action.0) })
    }
    pub(super) fn set(&self, name: &str, value: &Self) -> Result<()> {
        let name = Self::string(name)?;
        // SAFETY: live AX receiver, string key and CF value for duration of call.
        status(unsafe { AXUIElementSetAttributeValue(self.0, name.0, value.0) })
    }
    pub(super) fn set_true(&self, name: &str) -> Result<()> {
        let name = Self::string(name)?;
        // SAFETY: live AX receiver/string key; kCFBooleanTrue is an immortal CFBoolean.
        status(unsafe { AXUIElementSetAttributeValue(self.0, name.0, kCFBooleanTrue) })
    }
    pub(super) fn bounds(&self) -> Result<Rect> {
        let position = self.attr("AXPosition")?;
        let size = self.attr("AXSize")?;
        // SAFETY: live CF objects; verify AXValue types before extracting.
        if unsafe {
            CFGetTypeID(position.0) != AXValueGetTypeID()
                || CFGetTypeID(size.0) != AXValueGetTypeID()
        } {
            return Err(Error::Other("invalid accessibility geometry".into()));
        }
        let mut p = Point::default();
        let mut s = Size::default();
        // SAFETY: output types match kAXValueCGPointType (1) and CGSizeType (2).
        if !unsafe {
            AXValueGetValue(position.0, 1, (&raw mut p).cast())
                && AXValueGetValue(size.0, 2, (&raw mut s).cast())
        } {
            return Err(Error::Other("missing accessibility geometry".into()));
        }
        let mut display = 0;
        let mut count = 0;
        // SAFETY: one display slot and count storage are valid writable outputs.
        let rc = unsafe { CGGetDisplaysWithPoint(p, 1, &raw mut display, &raw mut count) };
        if rc != 0 || count == 0 {
            return Err(Error::Other("element display unavailable".into()));
        }
        // SAFETY: display ID was returned by CoreGraphics.
        let (frame, pixels) = unsafe { (CGDisplayBounds(display), CGDisplayPixelsWide(display)) };
        let pixels = u32::try_from(pixels).map_err(|e| Error::Other(e.to_string()))?;
        let scale = f64::from(pixels) / frame.size.w;
        if !scale.is_finite() || scale <= 0.0 {
            return Err(Error::Other("invalid display scale".into()));
        }
        Ok(Rect {
            x: p.x * scale,
            y: p.y * scale,
            w: s.w * scale,
            h: s.h * scale,
        })
    }
}

fn status(code: i32) -> Result<()> {
    if code == 0 {
        Ok(())
    } else if code == -25211 {
        Err(Error::PermissionDenied("Accessibility".into()))
    } else {
        Err(Error::Other(format!("Accessibility error {code}")))
    }
}
pub(super) fn trusted() -> bool {
    // SAFETY: no arguments or preconditions; does not prompt.
    unsafe { AXIsProcessTrusted() }
}
pub(super) fn recording() -> bool {
    // SAFETY: no arguments or preconditions; does not prompt.
    unsafe { CGPreflightScreenCaptureAccess() }
}
pub(super) fn request_recording() {
    // SAFETY: fixed OS permission prompt; no pointer arguments.
    unsafe { CGRequestScreenCaptureAccess() };
}
pub(super) fn require_trust() -> Result<()> {
    if trusted() {
        Ok(())
    } else {
        Err(Error::PermissionDenied("Accessibility".into()))
    }
}
pub(super) fn keyboard(key: u16, flags: u64, text: Option<&[u16]>) -> Result<()> {
    for down in [true, false] {
        // SAFETY: null selects default source; key is a validated virtual key code.
        let event = Object::owned(unsafe { CGEventCreateKeyboardEvent(ptr::null(), key, down) })?;
        // SAFETY: live CGEvent reference with no aliasing mutation on another thread.
        unsafe { CGEventSetFlags(event.0, flags) };
        if let Some(text) = text {
            // SAFETY: UTF-16 slice remains valid through call, length matches allocation.
            unsafe { CGEventKeyboardSetUnicodeString(event.0, text.len(), text.as_ptr()) };
        }
        // SAFETY: event is live; kCGHIDEventTap is 0. AX trust was checked by caller.
        unsafe { CGEventPost(0, event.0) };
    }
    Ok(())
}
