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

/// Classify an action, given the element it targets if known.
#[must_use]
pub fn classify(action: &Action, target: Option<&Element>) -> Risk {
    if matches!(action, Action::Type(_))
        && target
            .and_then(|e| e.label.as_deref())
            .is_some_and(|l| l.to_lowercase().contains("password"))
    {
        return Risk::Confirm;
    }
    Risk::Safe
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

    #[test]
    fn typing_into_password_needs_confirm() {
        let e = field("Password");
        assert_eq!(classify(&Action::Type("x".into()), Some(&e)), Risk::Confirm);
    }

    #[test]
    fn typing_into_ordinary_field_is_safe() {
        let e = field("Search");
        assert_eq!(classify(&Action::Type("x".into()), Some(&e)), Risk::Safe);
    }
}
