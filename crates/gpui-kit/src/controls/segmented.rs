//! A single choice presented as a strip of adjacent segments.
//!
//! A segmented control is a radio group that looks like a strip, so that is
//! what it publishes: a group of `Radio` nodes, exactly one of which is
//! checked. The choice is the caller's; the strip reports which segment was
//! asked for and draws whichever one the caller says holds.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface};

use crate::foundation::direction::ActiveDirection;
use crate::foundation::stepping::bounded_step;
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};
use crate::motion::{Flipping, flip};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One choice in the strip, identified by business identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    disabled: bool,
}

impl Segment {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Refuses the segment. A refused segment installs no handler and the
    /// keyboard steps over it.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

/// A strip where exactly one segment holds.
#[derive(IntoElement)]
pub struct SegmentedControl {
    ident: Ident,
    label: Option<SharedString>,
    segments: Vec<Segment>,
    selected: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for SegmentedControl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SegmentedControl")
            .field("ident", &self.ident)
            .field("segments", &self.segments.len())
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl SegmentedControl {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            segments: Vec::new(),
            selected: None,
            size: ControlSize::Md,
            disabled: false,
            on_select: None,
        }
    }

    /// What the whole strip is asking, for assistive technology.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn segments(mut self, segments: impl IntoIterator<Item = Segment>) -> Self {
        self.segments = segments.into_iter().collect();
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn actionable(&self) -> bool {
        !self.disabled && self.on_select.is_some()
    }

    fn selected_index(&self) -> Option<usize> {
        let id = self.selected.as_ref()?;
        self.segments.iter().position(|segment| &segment.id == id)
    }
}

impl Disableable for SegmentedControl {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for SegmentedControl {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

/// The segment `delta` steps away from `from`, skipping refusals and
/// stopping at the ends rather than wrapping onto the other side of the
/// strip, which a strip does not look like it does.
fn neighbour(segments: &[Segment], from: Option<usize>, delta: isize) -> Option<usize> {
    bounded_step(segments.len(), from, delta, |index| {
        segments[index].disabled
    })
}

/// The first segment that can be chosen, from whichever end.
fn edge(segments: &[Segment], from_start: bool) -> Option<usize> {
    if from_start {
        neighbour(segments, None, 1)
    } else {
        neighbour(segments, None, -1)
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let actionable = self.actionable();
        let strip_id = self.ident.semantic_id();
        // One background for the whole strip, drawn inside whichever segment
        // holds. Because it is the same element from frame to frame, changing
        // the choice moves it rather than redrawing it somewhere else.
        let selection = flip(self.ident.child("selection").semantic_id(), cx);

        let segments = self
            .segments
            .iter()
            .map(|segment| {
                let selected = self.selected.as_ref() == Some(&segment.id);
                let refused = self.disabled || segment.disabled;
                let ident = self.ident.child(segment.id.as_ref());
                let id = segment.id.clone();

                let fill = selected.then(|| {
                    div()
                        .absolute()
                        .inset_0()
                        .radius(&theme, Radius::Control)
                        .bg(theme.colors.raised)
                        .shadow(theme.shadow(gpui_kit_theme::Elevation::Raised).to_vec())
                        .flip(&selection, window, cx)
                });

                div()
                    .id(ident.element_id())
                    .row()
                    .justify_center()
                    .flex_none()
                    .relative()
                    .h(px(metrics.height - 2.0 * theme.borders.hairline))
                    .gap(px(metrics.gap))
                    .px(px(metrics.padding_x))
                    .radius(&theme, Radius::Control)
                    .text_size(px(metrics.font_size))
                    .text_color(if refused {
                        theme.colors.text_faint
                    } else if selected {
                        theme.colors.text
                    } else {
                        theme.colors.text_muted
                    })
                    .children(fill)
                    .when(segment.disabled, |element| {
                        element.opacity(theme.opacity.disabled)
                    })
                    .when(actionable && !segment.disabled, |element| {
                        element
                            .cursor_pointer()
                            .pressable(cx)
                            .when(!selected, |element| {
                                element.hover(|style| style.text_color(theme.colors.text))
                            })
                            .on_click({
                                let handler = self.on_select.clone().expect("checked above");
                                move |_, window, cx| handler(id.clone(), window, cx)
                            })
                    })
                    .children(segment.icon.map(|glyph| {
                        icon(glyph)
                            .size(px(metrics.icon_size * 0.9))
                            .text_color(if selected {
                                theme.colors.text
                            } else {
                                theme.colors.text_muted
                            })
                    }))
                    .child(segment.label.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Radio)
                            .parent(strip_id.clone())
                            .text(segment.label.clone())
                            .checked(selected)
                            .disabled(refused),
                    )
            })
            .collect::<Vec<_>>();

        let mut strip = div()
            .id(self.ident.child("strip").element_id())
            .row()
            .flex_none()
            .gap(px(2.0))
            .p(px(2.0))
            .radius(&theme, Radius::Control)
            .surface(&theme, Surface::Sunken)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(actionable, |element| {
                element.tab_index(0).focus_ring(&theme)
            })
            .children(segments);

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let items = self.segments.clone();
            let current = self.selected_index();
            // The strip runs in reading order, so the horizontal arrows step
            // with it. Up and down are the same two moves spelled on an axis
            // the reading direction does not touch.
            let direction = cx.layout_direction();
            strip.interactivity().on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                let next = match direction.arrow_step(key) {
                    Some(step) => neighbour(&items, current, step as isize),
                    None => match key {
                        "up" => neighbour(&items, current, -1),
                        "down" => neighbour(&items, current, 1),
                        "home" => edge(&items, true),
                        "end" => edge(&items, false),
                        _ => return,
                    },
                };
                if let Some(index) = next {
                    handler(items[index].id.clone(), window, cx);
                    cx.stop_propagation();
                }
            });
        }

        div()
            .column()
            .gap(px(theme.space(Space::Xs)))
            .child(strip)
            .semantic_in(cx, {
                let mut spec = NodeSpec::new(strip_id, Role::Group).disabled(self.disabled);
                if let Some(label) = self.label.clone() {
                    spec = spec.text(label);
                }
                spec
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments() -> Vec<Segment> {
        vec![
            Segment::new("day", "Day"),
            Segment::new("week", "Week").disabled(true),
            Segment::new("month", "Month"),
        ]
    }

    #[test]
    fn moving_steps_over_a_refused_segment() {
        assert_eq!(neighbour(&segments(), Some(0), 1), Some(2));
        assert_eq!(neighbour(&segments(), Some(2), -1), Some(0));
    }

    #[test]
    fn a_strip_has_ends_rather_than_wrapping() {
        assert_eq!(neighbour(&segments(), Some(2), 1), None);
        assert_eq!(neighbour(&segments(), Some(0), -1), None);
    }

    #[test]
    fn the_ends_are_the_first_segments_that_can_be_chosen() {
        assert_eq!(edge(&segments(), true), Some(0));
        assert_eq!(edge(&segments(), false), Some(2));
        let refused = vec![Segment::new("only", "Only").disabled(true)];
        assert_eq!(edge(&refused, true), None);
    }
}
