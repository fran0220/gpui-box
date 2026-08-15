//! Two panes and a divider the typist can move.
//!
//! Where the divider sits is the caller's answer, not the component's. The
//! split reports the ratio a drag or a keystroke asked for and draws whatever
//! the caller says is true, so a host that refuses a resize keeps the layout
//! that still holds.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AccessibleAction, AnyElement, App, ClickEvent, Global, InteractiveElement, IntoElement,
    MouseButton, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ActiveTheme;

use crate::foundation::{Disableable, FocusRing, Ident};
use crate::layout::measure;
use crate::strings::{ActiveStrings, StringKey};

/// How wide the grab area of the divider is, and how long the grip drawn
/// inside it is. Neither value repeats anywhere else.
pub(crate) const HANDLE: f32 = 7.0;
const GRIP: f32 = 24.0;

/// How far one arrow key moves the divider when the caller says nothing.
const DEFAULT_STEP: f32 = 24.0;

type ResizeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type CollapseHandler = Rc<dyn Fn(SplitSide, &mut Window, &mut App)>;

/// Which way the two panes are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitAxis {
    /// Side by side, with a vertical divider.
    #[default]
    Horizontal,
    /// Stacked, with a horizontal divider.
    Vertical,
}

/// One of the two panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSide {
    /// Left in a horizontal split, top in a vertical one.
    Start,
    /// Right in a horizontal split, bottom in a vertical one.
    End,
}

impl SplitSide {
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

/// Two panes separated by a divider.
#[derive(IntoElement)]
pub struct SplitPane {
    ident: Ident,
    axis: SplitAxis,
    ratio: f32,
    min_start: f32,
    min_end: f32,
    step: f32,
    collapsible: bool,
    handle_label: Option<SharedString>,
    start: Option<AnyElement>,
    end: Option<AnyElement>,
    disabled: bool,
    on_resize: Option<ResizeHandler>,
    on_collapse: Option<CollapseHandler>,
}

impl std::fmt::Debug for SplitPane {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SplitPane")
            .field("ident", &self.ident)
            .field("axis", &self.axis)
            .field("ratio", &self.ratio)
            .field("min", &(self.min_start, self.min_end))
            .field("collapsible", &self.collapsible)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_resize.is_some())
            .finish()
    }
}

impl SplitPane {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            axis: SplitAxis::Horizontal,
            ratio: 0.5,
            min_start: 0.0,
            min_end: 0.0,
            step: DEFAULT_STEP,
            collapsible: false,
            handle_label: None,
            start: None,
            end: None,
            disabled: false,
            on_resize: None,
            on_collapse: None,
        }
    }

    pub fn axis(mut self, axis: SplitAxis) -> Self {
        self.axis = axis;
        self
    }

    pub fn horizontal(self) -> Self {
        self.axis(SplitAxis::Horizontal)
    }

    pub fn vertical(self) -> Self {
        self.axis(SplitAxis::Vertical)
    }

    /// How much of the split the first pane takes, from 0 to 1.
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// The smallest each pane may be dragged to, in pixels.
    pub fn min_sizes(mut self, start: f32, end: f32) -> Self {
        self.min_start = start.max(0.0);
        self.min_end = end.max(0.0);
        self
    }

    /// How far one arrow key moves the divider, in pixels.
    pub fn step(mut self, step: f32) -> Self {
        if step > 0.0 {
            self.step = step;
        }
        self
    }

    /// Whether a double click on the divider reports a collapse.
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// What the divider is called, for a reader that has only the tree.
    pub fn handle_label(mut self, label: impl Into<SharedString>) -> Self {
        self.handle_label = Some(label.into());
        self
    }

    pub fn start(mut self, pane: impl IntoElement) -> Self {
        self.start = Some(pane.into_any_element());
        self
    }

    pub fn end(mut self, pane: impl IntoElement) -> Self {
        self.end = Some(pane.into_any_element());
        self
    }

    pub fn on_resize(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }

    /// Reports the pane a double click on the divider asked to collapse.
    pub fn on_collapse(
        mut self,
        handler: impl Fn(SplitSide, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_collapse = Some(Rc::new(handler));
        self
    }
}

impl Disableable for SplitPane {
    /// Freezes the divider. A frozen split installs no handler at all,
    /// including the keyboard one.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The narrowest and widest the first pane may be, as a fraction of `extent`.
///
/// Two minimums that together do not fit leave no room to move at all, so the
/// range collapses onto the point that splits the difference rather than
/// producing a reversed one.
pub(crate) fn limits(extent: f32, min_start: f32, min_end: f32) -> (f32, f32) {
    if extent <= 0.0 {
        return (0.0, 1.0);
    }
    let low = (min_start / extent).clamp(0.0, 1.0);
    let high = (1.0 - min_end / extent).clamp(0.0, 1.0);
    if low > high {
        let middle = (low + high) / 2.0;
        return (middle, middle);
    }
    (low, high)
}

/// The ratio a pointer at `position` asks for, stopped at whichever minimum it
/// runs into.
pub(crate) fn ratio_at(position: f32, origin: f32, extent: f32, low: f32, high: f32) -> f32 {
    if extent <= 0.0 {
        return low;
    }
    ((position - origin) / extent).clamp(low, high)
}

/// Which pane a double click asks to collapse.
///
/// The smaller of the two, because that is the one the divider already stands
/// nearest and the one collapsing costs the least.
pub(crate) fn collapse_target(ratio: f32) -> SplitSide {
    if ratio <= 0.5 {
        SplitSide::Start
    } else {
        SplitSide::End
    }
}

impl RenderOnce for SplitPane {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let horizontal = self.axis == SplitAxis::Horizontal;
        let ratio = self.ratio.clamp(0.0, 1.0);
        let actionable = !self.disabled && self.on_resize.is_some();
        let (min_start, min_end, step_px) = (self.min_start, self.min_end, self.step);

        // The divider needs the split's measured extent to turn a pointer
        // position into a ratio, and only prepaint knows it.
        let measured = measure::cell(&self.ident.semantic_id(), cx);
        let frame = measured.get();
        let extent = if horizontal {
            f32::from(frame.size.width)
        } else {
            f32::from(frame.size.height)
        };
        let (low, high) = limits(extent, min_start, min_end);
        let divider_ident = self.ident.child("divider");

        let mut divider = div()
            .id(divider_ident.element_id())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .when(horizontal, |element| element.w(px(HANDLE)).h_full())
            .when(!horizontal, |element| element.h(px(HANDLE)).w_full())
            .bg(theme.colors.panel)
            .child(
                div()
                    .rounded_full()
                    .bg(theme.colors.divider)
                    .when(horizontal, |grip| {
                        grip.w(px(theme.borders.hairline)).h(px(GRIP))
                    })
                    .when(!horizontal, |grip| {
                        grip.h(px(theme.borders.hairline)).w(px(GRIP))
                    }),
            )
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .hover(|style| style.bg(theme.colors.hover))
                    .focus_ring(&theme)
            });

        // A drag has to be followed across the whole split, not just the few
        // pixels of the handle, so the divider only records that one started
        // and the frame does the following.
        let dragging = drag_flag(&self.ident, cx);
        if actionable {
            let started = Rc::clone(&dragging);
            divider = divider.on_mouse_down(MouseButton::Left, move |_, _, _| started.set(true));
        }

        if let (true, Some(handler)) = (actionable, self.on_resize.clone()) {
            let bounds = Rc::clone(&measured);
            divider = divider.on_key_down(move |event, window, cx| {
                let frame = bounds.get();
                let extent = if horizontal {
                    f32::from(frame.size.width)
                } else {
                    f32::from(frame.size.height)
                };
                let (low, high) = limits(extent, min_start, min_end);
                let step = if extent > 0.0 { step_px / extent } else { 0.0 };
                let backward = if horizontal { "left" } else { "up" };
                let forward = if horizontal { "right" } else { "down" };
                let next = match event.keystroke.key.as_str() {
                    key if key == backward => ratio - step,
                    key if key == forward => ratio + step,
                    "home" => low,
                    "end" => high,
                    _ => return,
                }
                .clamp(low, high);
                cx.stop_propagation();
                if (next - ratio).abs() < f32::EPSILON {
                    return;
                }
                handler(next, window, cx);
            });
        }

        if let (true, Some(handler)) = (actionable, self.on_resize.clone()) {
            let bounds = Rc::clone(&measured);
            divider = divider.on_a11y_action(AccessibleAction::Increment, move |_, window, cx| {
                let frame = bounds.get();
                let extent = if horizontal {
                    f32::from(frame.size.width)
                } else {
                    f32::from(frame.size.height)
                };
                let (low, high) = limits(extent, min_start, min_end);
                let step = if extent > 0.0 { step_px / extent } else { 0.0 };
                let next = (ratio + step).clamp(low, high);
                if (next - ratio).abs() >= f32::EPSILON {
                    handler(next, window, cx);
                }
            });
        }
        if let (true, Some(handler)) = (actionable, self.on_resize.clone()) {
            let bounds = Rc::clone(&measured);
            divider = divider.on_a11y_action(AccessibleAction::Decrement, move |_, window, cx| {
                let frame = bounds.get();
                let extent = if horizontal {
                    f32::from(frame.size.width)
                } else {
                    f32::from(frame.size.height)
                };
                let (low, high) = limits(extent, min_start, min_end);
                let step = if extent > 0.0 { step_px / extent } else { 0.0 };
                let next = (ratio - step).clamp(low, high);
                if (next - ratio).abs() >= f32::EPSILON {
                    handler(next, window, cx);
                }
            });
        }

        if let (true, true, Some(handler)) =
            (actionable, self.collapsible, self.on_collapse.clone())
        {
            divider = divider.on_click(move |event: &ClickEvent, window, cx| {
                if event.click_count() < 2 {
                    return;
                }
                handler(collapse_target(ratio), window, cx);
            });
        }

        let divider = divider.semantic_in(
            cx,
            NodeSpec::new(divider_ident.semantic_id(), Role::Splitter)
                .parent(self.ident.semantic_id())
                .disabled(!actionable)
                .text(
                    self.handle_label
                        .clone()
                        .unwrap_or_else(|| cx.strings().text(StringKey::SplitResizeHandle)),
                )
                .orientation(if horizontal {
                    gpui::accesskit::Orientation::Vertical
                } else {
                    gpui::accesskit::Orientation::Horizontal
                })
                .range(low, high, ratio.clamp(low, high)),
        );

        // A pane with no share of the split drops its content rather than
        // drawing it at zero size, so nothing invisible stays addressable.
        let start = (ratio > 0.0).then_some(self.start).flatten().map(|pane| {
            pane_frame(&self.ident, "start", cx)
                .when(horizontal, |element| element.w(relative(ratio)).h_full())
                .when(!horizontal, |element| element.h(relative(ratio)).w_full())
                .child(pane)
        });
        let end = (ratio < 1.0)
            .then_some(self.end)
            .flatten()
            .map(|pane| pane_frame(&self.ident, "end", cx).flex_1().child(pane));

        let mut frame = div()
            .on_children_prepainted({
                let measured = Rc::clone(&measured);
                move |bounds, window, _| {
                    if let Some(first) = bounds.first() {
                        measure::record(&measured, *first, window);
                    }
                }
            })
            .id(self.ident.element_id())
            .size_full()
            .overflow_hidden();

        if let (true, Some(handler)) = (actionable, self.on_resize.clone()) {
            let bounds = Rc::clone(&measured);
            let held = Rc::clone(&dragging);
            frame = frame.on_mouse_move(move |event, window, cx| {
                if !held.get() {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    held.set(false);
                    return;
                }
                let frame = bounds.get();
                let (origin, extent) = if horizontal {
                    (f32::from(frame.left()), f32::from(frame.size.width))
                } else {
                    (f32::from(frame.top()), f32::from(frame.size.height))
                };
                let position = if horizontal {
                    f32::from(event.position.x)
                } else {
                    f32::from(event.position.y)
                };
                let (low, high) = limits(extent, min_start, min_end);
                let next = ratio_at(position, origin, extent, low, high);
                // A pane already at its minimum has nowhere left to go, so
                // dragging further reports nothing rather than a ratio the
                // caller would only have to clamp itself.
                if (next - ratio).abs() < f32::EPSILON {
                    return;
                }
                handler(next, window, cx);
            });
            let released = Rc::clone(&dragging);
            frame = frame.on_mouse_up(MouseButton::Left, move |_, _, _| released.set(false));
        }

        frame.child(
            div()
                .flex()
                .when(horizontal, |element| element.flex_row())
                .when(!horizontal, |element| element.flex_col())
                .size_full()
                .items_stretch()
                .overflow_hidden()
                .children(start)
                .child(divider)
                .children(end)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.semantic_id(), Role::Group)
                        .value(format!("{ratio:.3}")),
                ),
        )
    }
}

#[derive(Default)]
struct Dragging(RefCell<HashMap<SharedString, Rc<Cell<bool>>>>);

impl Global for Dragging {}

/// Whether a drag of this split's divider is in flight.
///
/// A `RenderOnce` builder is rebuilt every frame, so the press that starts a
/// drag and the moves that continue it are in different builds and need
/// somewhere outside the frame to agree.
fn drag_flag(ident: &Ident, cx: &mut App) -> Rc<Cell<bool>> {
    if !cx.has_global::<Dragging>() {
        cx.set_global(Dragging::default());
    }
    let mut flags = cx.global::<Dragging>().0.borrow_mut();
    Rc::clone(flags.entry(ident.semantic_id()).or_default())
}

fn pane_frame(ident: &Ident, side: &str, cx: &App) -> gpui::Stateful<gpui::Div> {
    let pane = ident.child(side);
    div()
        .flex()
        .flex_col()
        .flex_none()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .semantic_in(
            cx,
            NodeSpec::new(pane.semantic_id(), Role::Group).parent(ident.semantic_id()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_cannot_be_dragged_past_its_minimum() {
        let (low, high) = limits(400.0, 100.0, 80.0);
        assert_eq!(low, 0.25);
        assert_eq!(high, 0.8);
        assert_eq!(ratio_at(0.0, 0.0, 400.0, low, high), 0.25);
        assert_eq!(ratio_at(400.0, 0.0, 400.0, low, high), 0.8);
        assert_eq!(ratio_at(200.0, 0.0, 400.0, low, high), 0.5);
    }

    #[test]
    fn two_minimums_that_do_not_fit_leave_no_room_to_move() {
        let (low, high) = limits(100.0, 80.0, 80.0);
        assert_eq!(low, high, "the divider has nowhere to go");
        assert_eq!(ratio_at(0.0, 0.0, 100.0, low, high), low);
        assert_eq!(ratio_at(100.0, 0.0, 100.0, low, high), low);
    }

    #[test]
    fn an_unmeasured_split_reports_the_whole_range() {
        assert_eq!(limits(0.0, 100.0, 100.0), (0.0, 1.0));
    }

    #[test]
    fn a_double_click_collapses_the_smaller_pane() {
        assert_eq!(collapse_target(0.2), SplitSide::Start);
        assert_eq!(collapse_target(0.9), SplitSide::End);
        assert_eq!(collapse_target(0.5), SplitSide::Start);
    }
}
