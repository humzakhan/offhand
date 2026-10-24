//! Small ownership-checked wrappers around the C Accessibility/CoreGraphics APIs.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use offhand_platform::{Error, Rect, Result};
use objc2_application_services::{AXUIElement, AXValue, AXValueType, AXIsProcessTrusted};
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use std::{
    ffi::{CString, c_char, c_void},
    ptr::{self, NonNull},
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








    fn AXUIElementGetTypeID() -> usize;
    fn AXValueGetTypeID() -> usize;

    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGGetDisplaysWithPoint(point: Point, max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayBounds(display: u32) -> CRect;
    fn CGDisplayPixelsWide(display: u32) -> usize;




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
        let ax = unsafe { AXUIElement::new_system_wide() };
        let raw = (&raw const *ax).cast();
        // Transfer the create reference into our type-erased CF owner.
        std::mem::forget(ax);
        let object = Self::owned(raw)?;
        object.timeout();
        Ok(object)
    }
    fn ax(&self) -> Result<&AXUIElement> {
        // SAFETY: self is a live CF object and is type-checked before creating an AX reference.
        if unsafe { CFGetTypeID(self.0) != AXUIElementGetTypeID() } {
            return Err(Error::Other("expected accessibility element".into()));
        }
        // SAFETY: the type check above established AXUIElement and self retains its lifetime.
        Ok(unsafe { &*self.0.cast::<AXUIElement>() })
    }
    fn timeout(&self) {
        if let Ok(ax) = self.ax() {
            // SAFETY: validated AX element remains retained for the duration of this call.
            unsafe { ax.set_messaging_timeout(0.15) };
        }
    }
    pub(super) fn attr(&self, name: &str) -> Result<Self> {
        self.optional_attr(name)?.ok_or_else(|| Error::Other(format!("accessibility attribute unavailable: {name}")))
    }
    pub(super) fn optional_attr(&self, name: &str) -> Result<Option<Self>> {
        let name = Self::string(name)?;
        let mut value = ptr::null();
        // SAFETY: self/name are live references; value is writable out storage.
        let code = unsafe { self.ax()?.copy_attribute_value(&*name.0.cast(), NonNull::from(&mut value).cast()) }.0;
        if code == -25205 || code == -25212 { return Ok(None); }
        status(code)?;
        Self::owned(value).map(Some)
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
            self.ax()?.copy_attribute_values(&*name.0.cast(), 0, native_limit, NonNull::from(&mut raw).cast()).0
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
        // SAFETY: type checks accept every live CF object.
        if unsafe { CFGetTypeID(self.0) != CFBooleanGetTypeID() } { return None; }
        // SAFETY: the type check above established CFBoolean; self retains its lifetime.
        Some(unsafe { CFBooleanGetValue(self.0) != 0 })
    }
    pub(super) fn pid(&self) -> Result<i32> {
        let mut pid = 0;
        // SAFETY: valid AX reference and writable pid storage.
        status(unsafe { self.ax()?.pid(NonNull::from(&mut pid)) }.0)?;
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
        status(unsafe { self.ax()?.perform_action(&*action.0.cast()) }.0)
    }
    pub(super) fn set(&self, name: &str, value: &Self) -> Result<()> {
        let name = Self::string(name)?;
        // SAFETY: live AX receiver, string key and CF value for duration of call.
        status(unsafe { self.ax()?.set_attribute_value(&*name.0.cast(), &*value.0.cast()) }.0)
    }
    pub(super) fn set_true(&self, name: &str) -> Result<()> {
        let name = Self::string(name)?;
        // SAFETY: live AX receiver/string key; kCFBooleanTrue is an immortal CFBoolean.
        status(unsafe { self.ax()?.set_attribute_value(&*name.0.cast(), &*kCFBooleanTrue.cast()) }.0)
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
            (&*position.0.cast::<AXValue>()).value(AXValueType::CGPoint, NonNull::from(&mut p).cast())
                && (&*size.0.cast::<AXValue>()).value(AXValueType::CGSize, NonNull::from(&mut s).cast())
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
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|()| Error::Other("keyboard event source unavailable".into()))?;
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.clone(), key, down)
            .map_err(|()| Error::Other("keyboard event unavailable".into()))?;
        event.set_flags(CGEventFlags::from_bits_retain(flags));
        if let Some(text) = text {
            // The caller supplies complete Unicode scalars, including paired surrogates.
            event.set_string_from_utf16_unchecked(text);
        }
        event.post(CGEventTapLocation::HID);
    }
    Ok(())
}
