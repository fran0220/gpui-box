//! A single choice presented as a strip of adjacent segments.
//!
//! A segmented control is a radio group that looks like a strip, so that is
//! what it publishes: a group of `Radio` nodes, exactly one of which is
//! checked. The choice is the caller's; the strip reports which segment was
//! asked for and draws whichever one the caller says holds.

use std::rc::Rc;

use gpui::{
    App, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, TypeScale};

use crate::foundation::direction::ActiveDirection;
use crate::foundation::stepping::bounded_step;
use crate::foundation::{
    Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt, text as foundation_text,
};
use crate::motion::{Flipping, flip};
use crate::reactive::Binding;

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One choice in the strip, identified by business identity.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    tint: Option<Hsla>,
    disabled: bool,
}

impl Segment {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            tint: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// The colour this segment wears while it holds, in place of the accent.
    ///
    /// For a strip whose segments are colour-identified things — a Studio, a
    /// branch, an environment — where the answer is which one, and each one
    /// already has a colour the reader knows it by. The tint replaces the
    /// accent and nothing else: the raised pill, the resting and hover tones,
    /// and what the node publishes are what an untinted strip has, so a
    /// colour cannot turn one segment into a second segment shape. An
    /// untinted segment, and every segment that is not the current answer,
    /// stays on the accent language.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
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

    pub fn is_tinted(&self) -> bool {
        self.tint.is_some()
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

    /// Draws the segment the caller's [`Binding`] names, and writes the one
    /// that was picked. Sugar for [`Self::selected`] and [`Self::on_select`].
    pub fn bind(self, binding: &Binding<SharedString>, cx: &App) -> Self {
        let selected = binding.get(cx);
        let binding = binding.clone();
        self.selected(selected)
            .on_select(move |id, _window, cx| binding.set(cx, id))
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
        let selection = flip(self.ident.child("selection").semantic_id(), window, cx);

        let segments = self
            .segments
            .iter()
            .map(|segment| {
                let selected = self.selected.as_ref() == Some(&segment.id);
                let refused = self.disabled || segment.disabled;
                let ident = self.ident.child(segment.id.as_ref());
                let hover_group = ident.child("hover").semantic_id();
                let id = segment.id.clone();
                // The raised pill says which segment the strip is on; the
                // accent says the same thing in the one colour the rest of
                // the library reserves for "this is the current answer", so a
                // strip and a toggle group agree without being drawn alike.
                let label_color = if refused {
                    theme.colors.text_faint
                } else if selected {
                    segment.tint.unwrap_or(theme.colors.accent)
                } else {
                    theme.colors.text_muted
                };

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
                    .group(hover_group.clone())
                    .row()
                    .justify_center()
                    .flex_none()
                    .relative()
                    .h(px(metrics.height - 2.0 * theme.borders.hairline))
                    .gap(px(metrics.gap))
                    .px(px(metrics.padding_x))
                    .radius(&theme, Radius::Control)
                    .children(fill)
                    .when(segment.disabled, |element| {
                        element.opacity(theme.opacity.disabled)
                    })
                    .when(actionable && !segment.disabled, |element| {
                        element.cursor_pointer().pressable(cx).on_click({
                            let handler = self.on_select.clone().expect("checked above");
                            move |_, window, cx| handler(id.clone(), window, cx)
                        })
                    })
                    .children(segment.icon.map(|glyph| {
                        icon(glyph)
                            .size(px(metrics.icon_size * 0.9))
                            .text_color(label_color)
                    }))
                    .child(
                        foundation_text(&theme, TypeScale::Label, segment.label.clone())
                            .text_size(px(metrics.font_size))
                            .text_color(label_color)
                            .when(!refused && !selected, |element| {
                                element.group_hover(hover_group, |style| {
                                    style.text_color(theme.colors.text)
                                })
                            }),
                    )
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

        // The strip is as wide as its segments and no wider. Left to stretch,
        // it fills whatever column it was dropped into and the last segment
        // is followed by a run of empty track, which reads as a segment that
        // has lost its label.
        div()
            .column()
            .items_start()
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
