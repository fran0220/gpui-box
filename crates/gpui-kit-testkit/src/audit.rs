//! Invariants every published semantic tree must satisfy.
//!
//! These are the properties that make a tree usable by assistive technology
//! and by automation: an actionable node can be named and addressed, a value
//! is inside the range it reports, and nothing user-generated leaks verbatim.

use gpui_kit_semantics::{Node, Role, Snapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub id: String,
    pub problem: Problem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// An id that encodes list position breaks as soon as the list reorders.
    PositionalId,
    EmptyId,
    DuplicateId,
    /// An actionable control with nothing to announce.
    Unnamed,
    /// A control that reports a value outside the range it also reports.
    ValueOutOfRange,
    /// A range control that reports no bounds, while claiming a position.
    MissingRange,
    /// Text that survived redaction, which must never reach a snapshot.
    UnredactedSecret,
    /// A node that claims to be visible while occupying no space.
    ZeroSized,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let problem = match self.problem {
            Problem::PositionalId => "derives its id from list position",
            Problem::EmptyId => "has an empty id",
            Problem::DuplicateId => "shares its id with another node",
            Problem::Unnamed => "is actionable but has no accessible name",
            Problem::ValueOutOfRange => "reports a value outside its own range",
            Problem::MissingRange => "is a range control without bounds",
            Problem::UnredactedSecret => "publishes text that looks like a secret",
            Problem::ZeroSized => "is visible but occupies no space",
        };
        write!(formatter, "`{}` {problem}", self.id)
    }
}

/// Roles a user can operate, which therefore need a name.
fn is_actionable(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::MenuItem
            | Role::Tab
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
            | Role::Input
            | Role::Combobox
            | Role::Option
            | Role::Slider
    )
}

fn is_range(role: Role) -> bool {
    matches!(role, Role::Slider | Role::Progress)
}

pub fn audit(snapshot: &Snapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for node in &snapshot.nodes {
        let mut report = |problem: Problem| {
            findings.push(Finding {
                id: node.id.clone(),
                problem,
            })
        };

        if node.id.trim().is_empty() {
            report(Problem::EmptyId);
        } else if seen.contains(&node.id.as_str()) {
            report(Problem::DuplicateId);
        } else {
            seen.push(node.id.as_str());
        }

        if is_positional(&node.id) {
            report(Problem::PositionalId);
        }
        if is_actionable(node.role) && name_of(node).is_none() {
            report(Problem::Unnamed);
        }
        if is_range(node.role) {
            match (node.value_min, node.value_max, node.value_now) {
                (Some(min), Some(max), Some(now)) => {
                    if now < min || now > max || min > max {
                        report(Problem::ValueOutOfRange);
                    }
                }
                // An indeterminate wait has no position to report, and
                // inventing one would be a lie about progress.
                _ if node.busy && node.role == Role::Progress => {}
                _ => report(Problem::MissingRange),
            }
        }
        if let Some(text) = &node.text
            && looks_secret(text)
        {
            report(Problem::UnredactedSecret);
        }
        if node.visible && (node.bounds.width <= 0.0 || node.bounds.height <= 0.0) {
            report(Problem::ZeroSized);
        }
    }

    findings
}

fn name_of(node: &Node) -> Option<&str> {
    node.text
        .as_deref()
        .or(node.value.as_deref())
        .or(node.placeholder.as_deref())
        .filter(|name| !name.trim().is_empty())
}

/// An id whose last segment is a bare number describes where a row sits, not
/// what it is, so it changes when the list reorders.
fn is_positional(id: &str) -> bool {
    id.rsplit('.').next().is_some_and(|last| {
        !last.is_empty() && last.chars().all(|character| character.is_ascii_digit())
    })
}

fn looks_secret(text: &str) -> bool {
    const PREFIXES: [&str; 5] = ["sk-", "pk-", "ghp_", "xoxb-", "Bearer "];
    PREFIXES.iter().any(|prefix| text.contains(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit_semantics::{Node, Rect};

    fn node(id: &str, role: Role) -> Node {
        Node {
            id: id.into(),
            role,
            parent: None,
            labels: None,
            text: None,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            visible: true,
            focused: false,
            disabled: false,
            selected: false,
            hovered: false,
            pressed: false,
            checked: None,
            expanded: None,
            value: None,
            placeholder: None,
            value_min: None,
            value_max: None,
            value_now: None,
            level: None,
            busy: false,
            invalid: false,
            required: false,
        }
    }

    fn snapshot(nodes: Vec<Node>) -> Snapshot {
        Snapshot {
            generation: 1,
            nodes,
        }
    }

    fn problems(snapshot: &Snapshot) -> Vec<Problem> {
        audit(snapshot)
            .into_iter()
            .map(|finding| finding.problem)
            .collect()
    }

    #[test]
    fn a_well_formed_tree_reports_nothing() {
        let mut button = node("settings.save", Role::Button);
        button.text = Some("Save".into());
        assert!(audit(&snapshot(vec![button])).is_empty());
    }

    #[test]
    fn positional_ids_are_rejected() {
        let mut row = node("providers.0", Role::Button);
        row.text = Some("Anthropic".into());
        assert_eq!(problems(&snapshot(vec![row])), vec![Problem::PositionalId]);
    }

    #[test]
    fn an_actionable_node_must_be_nameable() {
        assert_eq!(
            problems(&snapshot(vec![node("settings.save", Role::Button)])),
            vec![Problem::Unnamed]
        );
    }

    #[test]
    fn a_placeholder_is_an_acceptable_name_for_a_field() {
        let mut field = node("search", Role::Input);
        field.placeholder = Some("Search providers".into());
        assert!(audit(&snapshot(vec![field])).is_empty());
    }

    #[test]
    fn duplicate_ids_are_reported_once_for_the_repeat() {
        let mut first = node("row", Role::Group);
        first.text = Some("A".into());
        let second = first.clone();
        assert_eq!(
            problems(&snapshot(vec![first, second])),
            vec![Problem::DuplicateId]
        );
    }

    #[test]
    fn an_indeterminate_wait_needs_no_position() {
        let mut spinner = node("saving", Role::Progress);
        spinner.busy = true;
        assert!(audit(&snapshot(vec![spinner])).is_empty());
    }

    #[test]
    fn a_determinate_progress_still_needs_its_bounds() {
        let mut progress = node("upload", Role::Progress);
        progress.value_now = Some(0.5);
        assert_eq!(
            problems(&snapshot(vec![progress])),
            vec![Problem::MissingRange]
        );
    }

    #[test]
    fn a_range_control_must_report_bounds_that_contain_its_value() {
        let mut slider = node("volume", Role::Slider);
        slider.text = Some("Volume".into());
        assert_eq!(
            problems(&snapshot(vec![slider.clone()])),
            vec![Problem::MissingRange]
        );

        slider.value_min = Some(0.0);
        slider.value_max = Some(1.0);
        slider.value_now = Some(1.5);
        assert_eq!(
            problems(&snapshot(vec![slider.clone()])),
            vec![Problem::ValueOutOfRange]
        );

        slider.value_now = Some(0.5);
        assert!(audit(&snapshot(vec![slider])).is_empty());
    }

    #[test]
    fn a_secret_that_survived_redaction_is_reported() {
        let mut callout = node("error", Role::Status);
        callout.text = Some("sk-live-not-a-real-key".into());
        assert_eq!(
            problems(&snapshot(vec![callout])),
            vec![Problem::UnredactedSecret]
        );
    }

    #[test]
    fn a_visible_node_must_occupy_space() {
        let mut group = node("panel", Role::Group);
        group.bounds.height = 0.0;
        assert_eq!(problems(&snapshot(vec![group])), vec![Problem::ZeroSized]);
    }
}
