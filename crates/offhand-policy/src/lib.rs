//! Action risk policy: decides whether an action is safe, needs confirmation, or is blocked.

use offhand_platform::{Action, Element};

/// Risk level of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Run without asking.
    Safe,
    /// Ask the user first.
    Confirm,
    /// Never run.
    Block,
}

/// Labels containing any of these words make a press or click irreversible enough to confirm.
const RISKY_WORDS: [&str; 8] = [
    "send", "delete", "remove", "buy", "pay", "purchase", "checkout", "submit",
];

/// Classify every native action. Block never has an approval bypass.
#[must_use]
pub fn classify(action: &Action, target: Option<&Element>) -> Risk {
    let label = target.and_then(|e| e.label.as_deref()).unwrap_or_default().to_lowercase();
    match action {
        Action::FocusWindow(_) => Risk::Safe,
        Action::OpenApp(name) => {
            let name = name.trim();
            if name.is_empty() || name.starts_with('-') || name.contains(['/', '\\', ':'])
                || name.chars().any(char::is_control) { return Risk::Block; }
            let name = name.to_lowercase();
            let name = name.strip_suffix(".app").or_else(|| name.strip_suffix(".exe")).unwrap_or(&name);
            let normalized: String = name.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).collect();
            if ["systemsettings", "systempreferences", "settings", "controlpanel", "control"].contains(&normalized.as_str()) {
                Risk::Confirm
            } else { Risk::Safe }
        }
        Action::Type(text) => {
            if target.is_none() || target.is_some_and(|e| e.role == offhand_platform::Role::PasswordField)
                || label.contains("password") || text.contains(['\n', '\r']) { Risk::Confirm }
            else { Risk::Safe }
        }
        Action::Press(_) | Action::Click { .. } => {
            let normalized = label.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
            if normalized.contains("empty trash") || normalized.contains("empty recycle bin") { Risk::Block }
            else if RISKY_WORDS.iter().any(|word| label.contains(word)) { Risk::Confirm }
            else { Risk::Safe }
        }
        Action::Key(key) => {
            let key = key.to_ascii_lowercase();
            if key.split('+').any(|part| matches!(part, "escape" | "esc")) { Risk::Block }
            else if matches!(key.as_str(), "tab" | "left" | "right" | "up" | "down" | "home" | "end" | "pageup" | "pagedown") { Risk::Safe }
            else { Risk::Confirm }
        }
        Action::Scroll { .. } => Risk::Safe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use offhand_platform::{ElementId, Rect, Role};

    fn field(label: &str) -> Element {
        Element {
            id: ElementId("1".into()),
            role: Role::TextField,
            label: Some(label.into()),
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            enabled: true,
        }
    }

    fn press() -> Action {
        Action::Press(ElementId("1".into()))
    }

    fn click() -> Action {
        Action::Click { x: 1.0, y: 1.0 }
    }

    #[test]
    fn typing_into_password_needs_confirm() {
        for label in ["Password", "PASSWORD", "Confirm password"] {
            let e = field(label);
            assert_eq!(classify(&Action::Type("x".into()), Some(&e)), Risk::Confirm);
        }
    }

    #[test]
    fn typing_into_ordinary_field_is_safe() {
        let e = field("Search");
        assert_eq!(classify(&Action::Type("x".into()), Some(&e)), Risk::Safe);
    }

    #[test]
    fn press_or_click_on_risky_label_needs_confirm() {
        for word in [
            "send", "delete", "remove", "buy", "pay", "purchase", "checkout", "submit",
        ] {
            for label in [
                word.to_string(),
                word.to_uppercase(),
                format!("Please {word} now"),
            ] {
                let e = field(&label);
                assert_eq!(classify(&press(), Some(&e)), Risk::Confirm, "press {label}");
                assert_eq!(classify(&click(), Some(&e)), Risk::Confirm, "click {label}");
            }
        }
    }

    #[test]
    fn press_or_click_on_ordinary_label_is_safe() {
        let e = field("Open");
        assert_eq!(classify(&press(), Some(&e)), Risk::Safe);
        assert_eq!(classify(&click(), Some(&e)), Risk::Safe);
    }

    #[test]
    fn password_label_does_not_gate_press() {
        let e = field("Password");
        assert_eq!(classify(&press(), Some(&e)), Risk::Safe);
    }

    #[test]
    fn risky_label_does_not_gate_type() {
        let e = field("Send to");
        assert_eq!(classify(&Action::Type("x".into()), Some(&e)), Risk::Safe);
    }

    #[test]
    fn missing_target_or_label_is_safe() {
        assert_eq!(classify(&press(), None), Risk::Safe);
        let mut e = field("Delete");
        e.label = None;
        assert_eq!(classify(&press(), Some(&e)), Risk::Safe);
    }
}
