//! Frozen cross-platform contract for Offhand.
//!
//! Every OS adapter (`offhand-macos`, `offhand-windows`) implements [`Platform`]
//! and must pass the shared suite in [`contract`].

pub mod contract;

/// Result alias for platform operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors an adapter can return.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The operation is not implemented by this adapter yet.
    #[error("not implemented")]
    Unimplemented,
    /// The OS denied access, for example a missing accessibility or screen-recording grant.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// The referenced element no longer exists.
    #[error("element not found: {0:?}")]
    ElementNotFound(ElementId),
    /// The referenced window no longer exists.
    #[error("window not found: {0:?}")]
    WindowNotFound(WindowId),
    /// Any other adapter-specific failure.
    #[error("platform error: {0}")]
    Other(String),
}

/// Opaque identifier of a UI element, stable within one `tree` snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementId(pub String);

/// Opaque identifier of a top-level window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

/// Rectangle in desktop physical pixels, top-left origin; coordinates may be negative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// Normalised role of a UI element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// A push button.
    Button,
    /// An editable text field.
    TextField,
    /// A secure text field.
    PasswordField,
    /// A menu entry.
    MenuItem,
    /// A hyperlink.
    Link,
    /// A checkbox.
    Checkbox,
    /// Any role without a dedicated variant, carrying the native role name.
    Other(String),
}

/// A node in the accessibility tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Identifier of this element.
    pub id: ElementId,
    /// Normalised role.
    pub role: Role,
    /// Human-readable label, if the OS exposes one.
    pub label: Option<String>,
    /// Bounds in screen coordinates.
    pub bounds: Rect,
    /// Whether the element accepts input.
    pub enabled: bool,
}

/// An input action the agent can perform.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Launch an application by name.
    OpenApp(String),
    /// Restore a captured external window.
    FocusWindow(WindowId),
    /// Invoke the default action of an element (accessibility press).
    Press(ElementId),
    /// Click at screen coordinates.
    Click {
        /// Horizontal position.
        x: f64,
        /// Vertical position.
        y: f64,
    },
    /// Type text into the focused element.
    Type(String),
    /// Press a shared-vocabulary key or chord, for example `"primary+s"`.
    Key(String),
    /// Scroll by the given deltas.
    Scroll {
        /// Horizontal delta.
        dx: f64,
        /// Vertical delta.
        dy: f64,
    },
}

/// A captured screen image in RGBA8 layout.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel data, 4 bytes per pixel, row-major.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rgba_len", &self.rgba.len())
            .finish()
    }
}

/// A fixed native permission flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Accessibility tree and input access.
    Accessibility,
    /// Optional future screen capture access.
    ScreenRecording,
}
/// Native grant status; None means no grant is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Permissions {
    /// None when the OS requires no explicit accessibility grant.
    pub accessibility: Option<bool>,
    /// None when the OS requires no explicit capture grant.
    pub screen_recording: Option<bool>,
}

/// The OS surface Offhand needs: read the UI, capture pixels, perform actions.
pub trait Platform {
    /// Snapshot the accessibility tree, optionally limited to one window.
    fn tree(&self, window: Option<WindowId>) -> Result<Vec<Element>>;
    /// Capture the screen, optionally limited to a region.
    fn capture(&self, region: Option<Rect>) -> Result<Frame>;
    /// Perform an action.
    fn act(&self, action: &Action) -> Result<()>;
    /// Capture the foreground external window before Offhand activates.
    fn foreground_window(&self) -> Result<WindowId> {
        Err(Error::Unimplemented)
    }
    fn focused_element(&self, _window: Option<WindowId>) -> Result<Option<Element>> {
        Ok(None)
    }
    /// Query fixed native grant statuses without prompting.
    fn permissions(&self) -> Permissions {
        Permissions::default()
    }
    fn request_permission(&self, _permission: Permission) -> Result<()> {
        Err(Error::Unimplemented)
    }
}
