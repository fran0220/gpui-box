//! The strip along the bottom of a window.
//!
//! A status bar is where a desktop tool says what is true right now: which
//! branch, how many problems, whether the indexer is still running. Every one
//! of those facts belongs to the host. The bar draws what it is given and
//! invents nothing — an item with no state carries no state, rather than a
//! reassuring green dot nobody asked for.
//!
//! # Stale is not current
//!
//! [`AsyncValue`] already separates a value from what is happening to it,
//! which is what lets a failed refresh keep the last verified number on
//! screen. [`StatusItem::tracking`] reads that vocabulary straight: a value
//! whose status is `Refreshing` or `Error` while a value is still held is
//! drawn with its last verified text and the word `stale` beside it, and
//! publishes `stale` as its value. It is never drawn as current.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::display::progress_circle::ProgressCircle;
use crate::display::status::StatusDot;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::state::{AsyncStatus, AsyncValue};

/// How tall the strip is. The value occurs only here.
const HEIGHT: f32 = 26.0;

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Which group an item sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusGroup {
    Start,
    Centre,
    End,
}

impl StatusGroup {
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Centre => "centre",
            Self::End => "end",
        }
    }
}

enum Content {
    /// Words, and nothing else.
    Text,
    /// A tone-coloured dot and words.
    State(Tone),
    /// A ring, for work with or without a known extent.
    Progress {
        fraction: Option<f32>,
        count: Option<(usize, usize)>,
    },
    /// A control the typist can operate.
    Action,
    /// Whatever the caller put there, usually a `Popover` or a `Menu` that
    /// opens from the bar.
    Element(AnyElement),
}

/// One thing the bar says.
pub struct StatusItem {
    id: SharedString,
    label: SharedString,
    content: Content,
    icon: Option<Icon>,
    /// The name of the state the host gave this item, if it gave one.
    state: Option<SharedString>,
    stale: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for StatusItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StatusItem")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("state", &self.state)
            .field("stale", &self.stale)
            .finish()
    }
}

impl StatusItem {
    fn build(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        content: Content,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content,
            icon: None,
            state: None,
            stale: false,
            disabled: false,
            on_click: None,
        }
    }

    pub fn text(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::build(id, label, Content::Text)
    }

    /// A dot and words. The tone is the host's judgement, never the bar's.
    pub fn state(id: impl Into<SharedString>, label: impl Into<SharedString>, tone: Tone) -> Self {
        let mut item = Self::build(id, label, Content::State(tone));
        item.state = Some(SharedString::new_static(tone.name()));
        item
    }

    /// A ring. Without a fraction or a count the extent is unknown, and is
    /// drawn as unknown rather than as a ring that happens to be part full.
    pub fn progress(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::build(
            id,
            label,
            Content::Progress {
                fraction: None,
                count: None,
            },
        )
    }

    /// A control. Without `on_click` it installs no handler and is refused.
    pub fn action(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::build(id, label, Content::Action)
    }

    /// Whatever the caller wants in the strip, usually a `Popover` or a `Menu`
    /// that opens from it. The element publishes its own identities.
    pub fn element(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        element: impl IntoElement,
    ) -> Self {
        Self::build(id, label, Content::Element(element.into_any_element()))
    }

    pub fn fraction(mut self, fraction: f32) -> Self {
        if let Content::Progress { fraction: slot, .. } = &mut self.content {
            *slot = Some(fraction.clamp(0.0, 1.0));
        }
        self
    }

    pub fn count(mut self, done: usize, total: usize) -> Self {
        if let Content::Progress { count, .. } = &mut self.content {
            *count = Some((done, total));
        }
        self
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// The name of the state the host says this item is in. Publishing it is
    /// how a test reads what the host claimed rather than what was painted.
    pub fn state_name(mut self, state: impl Into<SharedString>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Marks the text as the last verified value rather than the current one.
    pub fn stale(mut self, stale: bool) -> Self {
        self.stale = stale;
        self
    }

    /// Takes the text and the staleness from a value the host is refreshing.
    ///
    /// A refresh that failed keeps the last verified text on screen and marks
    /// it stale; a value the host never had shows the item's own label.
    pub fn tracking<E>(mut self, value: &AsyncValue<SharedString, E>) -> Self {
        if let Some(held) = value.value.clone() {
            self.label = held;
        }
        self.stale = value.is_stale();
        self.state = Some(SharedString::new_static(status_name(&value.status)));
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn is_stale(&self) -> bool {
        self.stale
    }
}

impl Disableable for StatusItem {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The name of an async status, for a test that reads the tree.
fn status_name<E>(status: &AsyncStatus<E>) -> &'static str {
    match status {
        AsyncStatus::Idle => "idle",
        AsyncStatus::Loading => "loading",
        AsyncStatus::Refreshing => "refreshing",
        AsyncStatus::Ready => "ready",
        AsyncStatus::Empty => "empty",
        AsyncStatus::Unavailable(_) => "unavailable",
        AsyncStatus::Error(_) => "error",
    }
}

/// The strip along the bottom of a window.
#[derive(IntoElement)]
pub struct StatusBar {
    ident: Ident,
    label: Option<SharedString>,
    groups: [Vec<StatusItem>; 3],
    disabled: bool,
}

impl std::fmt::Debug for StatusBar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StatusBar")
            .field("ident", &self.ident)
            .field(
                "items",
                &self.groups.iter().map(Vec::len).collect::<Vec<_>>(),
            )
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl StatusBar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            groups: [Vec::new(), Vec::new(), Vec::new()],
            disabled: false,
        }
    }

    /// What the strip is, for a reader that has only the tree.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn item(mut self, group: StatusGroup, item: StatusItem) -> Self {
        self.groups[index(group)].push(item);
        self
    }

    pub fn items(
        mut self,
        group: StatusGroup,
        items: impl IntoIterator<Item = StatusItem>,
    ) -> Self {
        self.groups[index(group)].extend(items);
        self
    }

    pub fn start(self, items: impl IntoIterator<Item = StatusItem>) -> Self {
        self.items(StatusGroup::Start, items)
    }

    pub fn centre(self, items: impl IntoIterator<Item = StatusItem>) -> Self {
        self.items(StatusGroup::Centre, items)
    }

    pub fn end(self, items: impl IntoIterator<Item = StatusItem>) -> Self {
        self.items(StatusGroup::End, items)
    }

    fn item_element(&self, item: StatusItem, theme: &Theme, cx: &mut App) -> AnyElement {
        let ident = self.ident.child(item.id.as_ref());
        let disabled = self.disabled || item.disabled;

        if let Content::Action = item.content {
            let mut button = Button::new(ident.clone())
                .label(item.label.clone())
                .ghost()
                .xs()
                .disabled(disabled);
            if let Some(glyph) = item.icon {
                button = button.icon(glyph);
            }
            if let (false, Some(handler)) = (disabled, item.on_click.clone()) {
                button = button.on_click(move |window, cx| handler(window, cx));
            }
            return button.into_any_element();
        }

        let text_color = if disabled {
            theme.colors.text_faint
        } else {
            theme.colors.text_muted
        };

        let mut row = div()
            .row()
            .flex_none()
            .h(px(theme.control.get(ControlSize::Xs).height))
            .px_token(theme, Space::Xs)
            .gap_token(theme, Space::Xs)
            .radius(theme, Radius::Control)
            .type_scale(theme, TypeScale::Caption)
            .text_color(text_color)
            .children(
                item.icon
                    .map(|glyph| icon(glyph).size(px(11.0)).text_color(text_color)),
            );

        match &item.content {
            Content::State(tone) => row = row.child(StatusDot::new(*tone)),
            Content::Progress { fraction, count } => {
                let mut ring = ProgressCircle::new(ident.child("progress"))
                    .label(item.label.clone())
                    .xs();
                if let Some((done, total)) = count {
                    ring = ring.count(*done, *total);
                } else if let Some(fraction) = fraction {
                    ring = ring.fraction(*fraction);
                }
                row = row.child(ring);
            }
            _ => {}
        }

        if let Content::Element(element) = item.content {
            row = row.child(element);
        } else {
            row = row.child(item.label.clone());
        }

        // A stale value keeps its text and says so beside it. Removing the
        // text would lose the last thing the host actually verified.
        if item.stale {
            row = row.child(
                div()
                    .flex_none()
                    .px(px(theme.spacing.xs / 2.0))
                    .radius(theme, Radius::Small)
                    .bg(theme.colors.warning.opacity(0.16))
                    .text_color(theme.colors.warning)
                    .child(SharedString::new_static("stale")),
            );
        }

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Status)
            .parent(self.ident.semantic_id())
            .disabled(disabled)
            .text(item.label.clone());
        // The bar states what the host claimed and nothing else: an item the
        // host gave no state to publishes no state.
        if item.stale {
            spec = spec.value("stale");
        } else if let Some(state) = item.state.clone() {
            spec = spec.value(state);
        }

        row.semantic_in(cx, spec).into_any_element()
    }
}

fn index(group: StatusGroup) -> usize {
    match group {
        StatusGroup::Start => 0,
        StatusGroup::Centre => 1,
        StatusGroup::End => 2,
    }
}

impl Disableable for StatusBar {
    /// Refuses every item in the strip. A refused item installs no handler.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for StatusBar {
    fn render(mut self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let count: usize = self.groups.iter().map(Vec::len).sum();
        let groups = std::mem::take(&mut self.groups);
        let mut strip = div()
            .id(self.ident.element_id())
            .row()
            .w_full()
            .flex_none()
            .h(px(HEIGHT))
            .items_center()
            .px_token(&theme, Space::Sm)
            .gap_token(&theme, Space::Sm)
            .bg(theme.colors.panel)
            .border_t(px(theme.borders.hairline))
            .border_color(theme.colors.hairline);

        for (position, items) in groups.into_iter().enumerate() {
            let elements: Vec<AnyElement> = items
                .into_iter()
                .map(|item| self.item_element(item, &theme, cx))
                .collect();
            strip = strip.child(
                div()
                    .row()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .gap_token(&theme, Space::Xs)
                    // The centre group takes the slack so the end group stays
                    // pinned to the right edge whatever the middle holds.
                    .when(position == 1, |group| group.flex_1().justify_center())
                    .when(position == 2, |group| group.justify_end())
                    .children(elements),
            );
        }

        let mut spec =
            NodeSpec::new(self.ident.semantic_id(), Role::Toolbar).value(count.to_string());
        if let Some(label) = self.label.clone() {
            spec = spec.text(label);
        }
        strip.semantic_in(cx, spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refresh_that_failed_keeps_the_last_verified_text_and_marks_it() {
        let mut value = AsyncValue::<SharedString, String>::ready("main@a1b2c3".into());
        value.refresh();
        value.fail_refresh("the host is unreachable".into());

        let item = StatusItem::text("vcs", "unknown").tracking(&value);
        assert_eq!(item.label().as_ref(), "main@a1b2c3");
        assert!(item.is_stale());
    }

    #[test]
    fn a_value_that_is_current_is_not_stale() {
        let value = AsyncValue::<SharedString, String>::ready("main@a1b2c3".into());
        let item = StatusItem::text("vcs", "unknown").tracking(&value);
        assert!(!item.is_stale());
    }

    #[test]
    fn a_state_carries_the_tone_the_host_chose() {
        let item = StatusItem::state("build", "Build passing", Tone::Success);
        assert_eq!(item.state.as_deref(), Some("success"));
    }

    #[test]
    fn an_item_with_no_state_claims_none() {
        assert!(StatusItem::text("branch", "main").state.is_none());
    }

    #[test]
    fn every_async_status_has_a_name_a_test_can_read() {
        assert_eq!(
            status_name(&AsyncStatus::<String>::Refreshing),
            "refreshing"
        );
        assert_eq!(
            status_name(&AsyncStatus::Unavailable::<String>("no".into())),
            "unavailable"
        );
    }
}
