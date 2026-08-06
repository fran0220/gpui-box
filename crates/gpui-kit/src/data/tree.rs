//! A hierarchy whose open branches are caller-owned.
//!
//! The tree reports the node that was activated and the state its disclosure
//! should take next. It renders exactly the set the caller passed to
//! [`Tree::expanded`], so a host that refuses to open a branch leaves it shut.
//!
//! A collapsed node renders none of its children, and publishes none of them
//! either, so asserting that a child is absent means something.

use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Transformation, Window, div, prelude::FluentBuilder, px,
    radians,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Theme};

use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};

type ToggleHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One node, identified by business identity rather than by its place in the
/// hierarchy, so moving a branch does not rename what hangs under it.
#[derive(Debug, Clone)]
pub struct TreeNode {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    disabled: bool,
    children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            disabled: false,
            children: Vec::new(),
        }
    }

    pub fn child(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = TreeNode>) -> Self {
        self.children.extend(children);
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A disclosure hierarchy.
#[derive(IntoElement)]
pub struct Tree {
    ident: Ident,
    nodes: Vec<TreeNode>,
    expanded: Vec<SharedString>,
    selected: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    on_toggle: Option<ToggleHandler>,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for Tree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Tree")
            .field("ident", &self.ident)
            .field("nodes", &self.nodes.len())
            .field("expanded", &self.expanded)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl Tree {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            nodes: Vec::new(),
            expanded: Vec::new(),
            selected: None,
            size: ControlSize::Md,
            disabled: false,
            on_toggle: None,
            on_select: None,
        }
    }

    pub fn node(mut self, node: TreeNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn nodes(mut self, nodes: impl IntoIterator<Item = TreeNode>) -> Self {
        self.nodes.extend(nodes);
        self
    }

    pub fn expanded(mut self, ids: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded = ids.into_iter().collect();
        self
    }

    pub fn expanded_ids<S: AsRef<str>>(mut self, ids: &[S]) -> Self {
        self.expanded = ids
            .iter()
            .map(|id| SharedString::from(id.as_ref().to_string()))
            .collect();
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Disableable for Tree {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Tree {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

/// One node as it is actually shown: what the keyboard can reach.
#[derive(Debug, Clone)]
struct Visible {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    disabled: bool,
    /// Root nodes are level 1, matching how assistive technology counts.
    level: u32,
    open: bool,
    has_children: bool,
    parent: Option<SharedString>,
    first_child: Option<SharedString>,
}

/// The nodes a frame shows, in the order the keyboard walks them.
///
/// A collapsed branch contributes only itself, which is why a move never lands
/// on something the typist cannot see.
fn flatten(
    nodes: &[TreeNode],
    expanded: &[SharedString],
    level: u32,
    parent: Option<&SharedString>,
    out: &mut Vec<Visible>,
) {
    for node in nodes {
        let open = expanded.contains(&node.id);
        let has_children = !node.children.is_empty();
        out.push(Visible {
            id: node.id.clone(),
            label: node.label.clone(),
            icon: node.icon,
            disabled: node.disabled,
            level,
            open: open && has_children,
            has_children,
            parent: parent.cloned(),
            first_child: node.children.first().map(|child| child.id.clone()),
        });
        if open && has_children {
            flatten(&node.children, expanded, level + 1, Some(&node.id), out);
        }
    }
}

/// What a keystroke reports: a selection, a disclosure change, or nothing.
enum Move {
    Select(SharedString),
    Toggle(SharedString, bool),
}

fn keystroke_move(key: &str, visible: &[Visible], selected: Option<&SharedString>) -> Option<Move> {
    let at = visible
        .iter()
        .position(|node| Some(&node.id) == selected)
        .filter(|_| selected.is_some());
    match key {
        "up" | "down" => {
            let delta: isize = if key == "down" { 1 } else { -1 };
            let from = match at {
                Some(at) => at as isize + delta,
                // Entering from outside, a move lands on the end it travels
                // away from.
                None if delta > 0 => 0,
                None => visible.len() as isize - 1,
            };
            step(visible, from, delta).map(Move::Select)
        }
        "home" => step(visible, 0, 1).map(Move::Select),
        "end" => step(visible, visible.len() as isize - 1, -1).map(Move::Select),
        "right" => {
            let node = visible.get(at?)?;
            if node.has_children && !node.open {
                Some(Move::Toggle(node.id.clone(), true))
            } else {
                node.first_child
                    .clone()
                    .filter(|_| node.open)
                    .map(Move::Select)
            }
        }
        "left" => {
            let node = visible.get(at?)?;
            if node.has_children && node.open {
                Some(Move::Toggle(node.id.clone(), false))
            } else {
                node.parent.clone().map(Move::Select)
            }
        }
        _ => None,
    }
}

/// The first node from `from` in `delta`'s direction that accepts selection.
fn step(visible: &[Visible], from: isize, delta: isize) -> Option<SharedString> {
    let mut index = from;
    while index >= 0 && (index as usize) < visible.len() {
        let node = &visible[index as usize];
        if !node.disabled {
            return Some(node.id.clone());
        }
        index += delta;
    }
    None
}

impl RenderOnce for Tree {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let mut visible = Vec::new();
        flatten(&self.nodes, &self.expanded, 1, None, &mut visible);

        let mut stack = div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .text_size(px(metrics.font_size));

        if !self.disabled && (self.on_select.is_some() || self.on_toggle.is_some()) {
            let nodes = visible.clone();
            let selected = self.selected.clone();
            let select = self.on_select.clone();
            let toggle = self.on_toggle.clone();
            stack = stack.on_key_down(move |event, window, cx| {
                let Some(next) =
                    keystroke_move(event.keystroke.key.as_str(), &nodes, selected.as_ref())
                else {
                    return;
                };
                match next {
                    Move::Select(id) => {
                        if Some(&id) == selected.as_ref() {
                            return;
                        }
                        let Some(handler) = select.as_ref() else {
                            return;
                        };
                        handler(id, window, cx);
                    }
                    Move::Toggle(id, open) => {
                        let Some(handler) = toggle.as_ref() else {
                            return;
                        };
                        handler(id, open, window, cx);
                    }
                }
                cx.stop_propagation();
            });
        }

        for node in &visible {
            stack = stack.child(self.node_element(node, &theme, metrics.icon_size, cx));
        }

        stack.semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Tree))
    }
}

impl Tree {
    fn node_element(
        &self,
        node: &Visible,
        theme: &Theme,
        icon_size: f32,
        cx: &mut App,
    ) -> impl IntoElement {
        let ident = self.ident.child(node.id.as_ref());
        let selected = self.selected.as_ref() == Some(&node.id);
        let disabled = self.disabled || node.disabled;
        let selectable = !disabled && self.on_select.is_some();
        let toggleable = !disabled && node.has_children && self.on_toggle.is_some();
        let color = if disabled {
            theme.colors.text_faint
        } else {
            theme.colors.text
        };

        let chevron = node.has_children.then(|| {
            let toggle = ident.child("toggle");
            let mut glyph = div()
                .id(toggle.element_id())
                .row()
                .flex_none()
                .size(px(icon_size))
                .child(
                    icon(Icon::AltArrowRight)
                        .size(px(icon_size))
                        .text_color(theme.colors.text_muted)
                        .when(node.open, |glyph| {
                            glyph.with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                        }),
                )
                .when(toggleable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
                        .focus_ring(theme)
                });

            if let (true, Some(handler)) = (toggleable, self.on_toggle.clone()) {
                let id = node.id.clone();
                let open = node.open;
                let keyed = Rc::clone(&handler);
                let keyed_id = id.clone();
                glyph = glyph.on_click(move |_, window, cx| {
                    handler(id.clone(), !open, window, cx);
                    // A disclosure is not a selection, so the row underneath
                    // must not also report one.
                    cx.stop_propagation();
                });
                glyph = glyph.on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        keyed(keyed_id.clone(), !open, window, cx);
                        cx.stop_propagation();
                    }
                });
            }

            glyph.semantic_in(
                cx,
                NodeSpec::new(toggle.semantic_id(), Role::Button)
                    .parent(ident.semantic_id())
                    .text(node.label.clone())
                    .expanded(node.open)
                    .disabled(!toggleable),
            )
        });

        let mut row = div()
            .id(ident.element_id())
            .row()
            .w_full()
            .h(px(theme.control.get(self.size).height))
            .pr(px(theme.space(Space::Sm)))
            // The indent is the only thing that says how deep a node sits, so
            // it steps once per level from the left edge.
            .pl(px(theme.space(Space::Sm)
                + node.level.saturating_sub(1) as f32
                    * theme.space(Space::Md)))
            .gap(px(theme.space(Space::Xs)))
            .text_color(color)
            .when(selected, |element| element.bg(theme.colors.selected))
            .when(disabled, |element| element.opacity(theme.opacity.disabled))
            .when(selectable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .when(!selected, |element| {
                        element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
                    })
                    .focus_ring(theme)
            })
            .children(chevron)
            // A leaf still lines up with its siblings, which is what makes the
            // indent readable as depth rather than as decoration.
            .when(!node.has_children, |element| {
                element.child(div().flex_none().size(px(icon_size)))
            })
            .children(
                node.icon
                    .map(|glyph| icon(glyph).size(px(icon_size)).text_color(color)),
            )
            .child(div().flex_1().overflow_hidden().child(node.label.clone()));

        if let (true, Some(handler)) = (selectable, self.on_select.clone()) {
            let id = node.id.clone();
            row = row.on_click(move |_, window, cx| handler(id.clone(), window, cx));
        }

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::TreeItem)
            .parent(
                node.parent
                    .as_ref()
                    .map_or(self.ident.semantic_id(), |parent| {
                        self.ident.child(parent.as_ref()).semantic_id()
                    }),
            )
            .text(node.label.clone())
            .selected(selected)
            .disabled(disabled)
            .level(node.level);
        // Only a node that has something to disclose claims a disclosure
        // state; a leaf that reported `expanded: false` would look shut.
        if node.has_children {
            spec = spec.expanded(node.open);
        }

        row.semantic_in(cx, spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<TreeNode> {
        vec![
            TreeNode::new("workspace", "Workspace").children([
                TreeNode::new("src", "src").children([TreeNode::new("lib", "lib.rs")]),
                TreeNode::new("docs", "docs"),
            ]),
            TreeNode::new("target", "target").disabled(true),
        ]
    }

    fn visible(expanded: &[&str]) -> Vec<Visible> {
        let expanded: Vec<SharedString> = expanded
            .iter()
            .map(|id| SharedString::from(id.to_string()))
            .collect();
        let mut out = Vec::new();
        flatten(&sample(), &expanded, 1, None, &mut out);
        out
    }

    #[test]
    fn a_collapsed_branch_contributes_only_itself() {
        let nodes = visible(&[]);
        let ids: Vec<&str> = nodes.iter().map(|node| node.id.as_ref()).collect();
        assert_eq!(ids, vec!["workspace", "target"]);
    }

    #[test]
    fn an_open_branch_levels_its_children_one_deeper() {
        let nodes = visible(&["workspace"]);
        assert_eq!(nodes[0].level, 1);
        assert_eq!(nodes[1].level, 2);
        assert_eq!(nodes[1].parent.as_deref(), Some("workspace"));
    }

    #[test]
    fn a_move_down_skips_a_refusal_and_stops_at_the_end() {
        let nodes = visible(&["workspace"]);
        let from = SharedString::from("docs");
        // `target` refuses selection and is the last node, so the move lands
        // nowhere rather than wrapping.
        assert!(keystroke_move("down", &nodes, Some(&from)).is_none());
    }

    #[test]
    fn right_opens_a_shut_branch_and_then_descends() {
        let shut = visible(&[]);
        let workspace = SharedString::from("workspace");
        match keystroke_move("right", &shut, Some(&workspace)) {
            Some(Move::Toggle(id, next)) => {
                assert_eq!(id.as_ref(), "workspace");
                assert!(next);
            }
            _ => panic!("right must open a shut branch"),
        }

        let open = visible(&["workspace"]);
        match keystroke_move("right", &open, Some(&workspace)) {
            Some(Move::Select(id)) => assert_eq!(id.as_ref(), "src"),
            _ => panic!("right must descend into an open branch"),
        }
    }

    #[test]
    fn left_shuts_an_open_branch_and_otherwise_ascends() {
        let open = visible(&["workspace"]);
        let src = SharedString::from("src");
        match keystroke_move("left", &open, Some(&src)) {
            Some(Move::Select(id)) => assert_eq!(id.as_ref(), "workspace"),
            _ => panic!("left must ascend from a leaf"),
        }

        let deeper = visible(&["workspace", "src"]);
        match keystroke_move("left", &deeper, Some(&src)) {
            Some(Move::Toggle(id, next)) => {
                assert_eq!(id.as_ref(), "src");
                assert!(!next);
            }
            _ => panic!("left must shut an open branch"),
        }
    }
}
