//! A navigation rail of sectioned places, expanded or collapsed to icons.
//!
//! Where the typist is, is caller-owned. The sidebar reports the place that was
//! picked and marks whatever the caller says is current, so a host that refuses
//! a move keeps the place that still holds highlighted.
//!
//! Collapsing is a change of drawing, never of substance. A collapsed rail
//! shows glyphs and reaches the label through a [`Tooltip`](crate::overlay::Tooltip), and every item
//! still publishes its full name, so nothing becomes unaddressable by being
//! made narrow.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::display::badge::Badge;
use crate::foundation::{Disableable, Ident, StyledExt};
use crate::overlay::Tooltipped;

/// How wide the rail is expanded, and how wide it is collapsed to glyphs.
/// Neither value repeats anywhere else.
const EXPANDED_WIDTH: f32 = 240.0;
const COLLAPSED_WIDTH: f32 = 52.0;

/// How far a nested item is indented from its parent in the expanded rail.
const NESTING_INDENT: f32 = 16.0;

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One place in the rail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarItem {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    badge: Option<SharedString>,
    disabled: bool,
    children: Vec<SidebarItem>,
}

impl SidebarItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            disabled: false,
            children: Vec::new(),
        }
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// A count or a state shown next to the label, such as how many runs a
    /// place holds.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Nested places, one level deep.
    ///
    /// A rail deeper than that stops being a rail, so anything nested inside a
    /// child is dropped here rather than drawn at a depth the sidebar cannot
    /// lay out or publish.
    pub fn children(mut self, children: impl IntoIterator<Item = SidebarItem>) -> Self {
        self.children = children
            .into_iter()
            .map(|child| SidebarItem {
                children: Vec::new(),
                ..child
            })
            .collect();
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// A titled run of places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSection {
    id: SharedString,
    title: Option<SharedString>,
    items: Vec<SidebarItem>,
}

impl SidebarSection {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: None,
            items: Vec::new(),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn item(mut self, item: SidebarItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = SidebarItem>) -> Self {
        self.items.extend(items);
        self
    }
}

/// A collapsible navigation rail.
#[derive(IntoElement)]
pub struct Sidebar {
    ident: Ident,
    sections: Vec<SidebarSection>,
    active: Option<SharedString>,
    collapsed: bool,
    header: Option<AnyElement>,
    footer: Option<AnyElement>,
    disabled: bool,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for Sidebar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sidebar")
            .field("ident", &self.ident)
            .field("sections", &self.sections.len())
            .field("active", &self.active)
            .field("collapsed", &self.collapsed)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl Sidebar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            sections: Vec::new(),
            active: None,
            collapsed: false,
            header: None,
            footer: None,
            disabled: false,
            on_select: None,
        }
    }

    pub fn section(mut self, section: SidebarSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn sections(mut self, sections: impl IntoIterator<Item = SidebarSection>) -> Self {
        self.sections.extend(sections);
        self
    }

    /// The place the caller says is current. The sidebar marks it and never
    /// moves it.
    pub fn active(mut self, id: impl Into<SharedString>) -> Self {
        self.active = Some(id.into());
        self
    }

    /// Draws glyphs only, with each label reachable as hover help.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// The slot at the bottom of the rail, kept in both widths.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn item_element(
        &self,
        item: &SidebarItem,
        level: u32,
        theme: &Theme,
        cx: &mut App,
    ) -> AnyElement {
        let metrics = theme.control.get(ControlSize::Md);
        let ident = self.ident.child(item.id.as_ref());
        let active = self.active.as_ref() == Some(&item.id);
        let disabled = self.disabled || item.disabled;
        let actionable = !disabled && self.on_select.is_some();
        let color = if disabled {
            theme.colors.text_faint
        } else if active {
            theme.colors.text
        } else {
            theme.colors.text_muted
        };
        let indent = if self.collapsed || level == 1 {
            0.0
        } else {
            NESTING_INDENT
        };

        let mut row = div()
            .id(ident.element_id())
            .flex()
            .flex_row()
            .items_center()
            .h(px(metrics.height))
            .w_full()
            .gap(px(theme.space(Space::Sm)))
            .pl(px(theme.space(Space::Sm) + indent))
            .pr(px(theme.space(Space::Sm)))
            .radius(theme, Radius::Control)
            .text_size(px(metrics.font_size))
            .text_color(color)
            .when(self.collapsed, |element| element.justify_center())
            .when(active, |element| element.bg(theme.colors.selected))
            .when(disabled, |element| element.opacity(theme.opacity.disabled))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .w(px(metrics.icon_size))
                    .justify_center()
                    .children(
                        item.icon
                            .map(|glyph| icon(glyph).size(px(metrics.icon_size)).text_color(color)),
                    ),
            )
            .when(!self.collapsed, |element| {
                element
                    .child(div().flex_1().overflow_hidden().child(item.label.clone()))
                    .children(item.badge.clone().map(|badge| Badge::new(badge).neutral()))
            })
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.bg(theme.colors.hover))
                    .focus(|style| style.shadow(theme.selected_ring()))
            });

        // A narrow rail hides the wording, so the wording has to reach the
        // typist some other way.
        if self.collapsed {
            row = row.tip(ident.clone(), item.label.clone());
        }

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let id = item.id.clone();
            let click = Rc::clone(&handler);
            let clicked = id.clone();
            row = row
                .on_click(move |_, window, cx| click(clicked.clone(), window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(id.clone(), window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Link)
            .parent(self.ident.semantic_id())
            .selected(active)
            .disabled(disabled)
            .level(level)
            // A collapsed rail draws a glyph and still says what it is.
            .text(item.label.clone());
        if let Some(badge) = item.badge.clone() {
            spec = spec.value(badge);
        }

        row.semantic_in(cx, spec).into_any_element()
    }
}

impl Disableable for Sidebar {
    /// Freezes the whole rail. A frozen rail installs no handler at all.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut body = div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(theme.space(Space::Md)))
            .overflow_hidden();

        for section in &self.sections {
            let section_ident = self.ident.child(section.id.as_ref());
            let mut rows: Vec<AnyElement> = Vec::new();
            for item in &section.items {
                rows.push(self.item_element(item, 1, &theme, cx));
                for child in &item.children {
                    rows.push(self.item_element(child, 2, &theme, cx));
                }
            }

            // A collapsed rail has no room for a caption, so the run is
            // separated by a rule instead of titled.
            let heading = section
                .title
                .clone()
                .filter(|_| !self.collapsed)
                .map(|title| {
                    div()
                        .px(px(theme.space(Space::Sm)))
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(title.clone())
                        .semantic_in(
                            cx,
                            NodeSpec::new(section_ident.semantic_id(), Role::Heading)
                                .parent(self.ident.semantic_id())
                                .level(1)
                                .text(title),
                        )
                });

            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme.space(Space::Xs)))
                    .children(heading)
                    .children(rows),
            );
        }

        div()
            .id(self.ident.element_id())
            .flex()
            .flex_col()
            .h_full()
            .w(px(if self.collapsed {
                COLLAPSED_WIDTH
            } else {
                EXPANDED_WIDTH
            }))
            .gap(px(theme.space(Space::Md)))
            .p(px(theme.space(Space::Sm)))
            .bg(theme.colors.panel)
            .border_r(px(theme.borders.hairline))
            .border_color(theme.colors.hairline)
            .children(self.header)
            .child(body)
            .children(self.footer)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List).expanded(!self.collapsed),
            )
    }
}
