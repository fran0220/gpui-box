//! A bounded region with a themed scrollbar.
//!
//! The scroll area is honest about two different things a viewport can mean.
//! There is *nothing more* — the content fits, and no scrollbar is drawn or
//! published — and there is *more, off screen* — a scrollbar node carries how
//! far the content reaches and how far it has been scrolled, so a test can
//! tell the two apart instead of guessing from what happens to be visible.
//!
//! The scrollbar never covers what it hides. Its track is a sibling of the
//! viewport rather than a layer over it, and the gutter is reserved for every
//! axis the caller enabled, so turning a scrollbar on cannot reflow the
//! content that decided whether it was needed.
//!
//! This is the same rule [`List`](crate::data::List) follows: scroll position
//! is transient view state, so it is held in an application global keyed by
//! semantic identity rather than by the caller.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    RenderOnce, ScrollHandle, ScrollTarget, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Theme};

use crate::foundation::{Ident, window_state};
use crate::layout::measure;
use crate::layout::scroll_fade::{FadeEdges, ScrollFade};
use crate::motion::ScrollLink;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// Which way the content may be scrolled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollAxis {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

impl ScrollAxis {
    pub fn has_vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }

    pub fn has_horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }
}

/// A scrollable region.
#[derive(IntoElement)]
pub struct ScrollArea {
    ident: Ident,
    axis: ScrollAxis,
    label: Option<SharedString>,
    width: Option<f32>,
    height: Option<f32>,
    /// Whether the area is as tall as what it holds instead of as tall as it
    /// is offered.
    fit_height: bool,
    /// Something that already scrolls, whose position this area draws rather
    /// than scrolling its own content.
    target: Option<Rc<dyn ScrollTarget>>,
    content: Option<AnyElement>,
}

impl std::fmt::Debug for ScrollArea {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScrollArea")
            .field("ident", &self.ident)
            .field("axis", &self.axis)
            .field("label", &self.label)
            .field("size", &(self.width, self.height))
            .finish()
    }
}

impl ScrollArea {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            axis: ScrollAxis::Vertical,
            label: None,
            width: None,
            height: None,
            fit_height: false,
            target: None,
            content: None,
        }
    }

    /// Draws this area's scrollbar for something that scrolls itself.
    ///
    /// A virtualized list holds its own scroll position, because it decides
    /// which rows exist from where it has been scrolled to. Wrapping one in an
    /// overflowing container gives it a second, disagreeing position and a
    /// thumb sized from a content height nobody has measured. Bound to the
    /// list's own [`ScrollTarget`], the area stops scrolling and only reports:
    /// the numbers on the gutter, and in the semantic tree, are the list's.
    pub fn bound_to(mut self, target: impl ScrollTarget + 'static) -> Self {
        self.target = Some(Rc::new(target));
        self
    }

    pub fn axis(mut self, axis: ScrollAxis) -> Self {
        self.axis = axis;
        self
    }

    pub fn vertical(self) -> Self {
        self.axis(ScrollAxis::Vertical)
    }

    pub fn horizontal(self) -> Self {
        self.axis(ScrollAxis::Horizontal)
    }

    pub fn both(self) -> Self {
        self.axis(ScrollAxis::Both)
    }

    /// What the region is called, for a reader that has only the tree.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Bounds the viewport. Without a bound on the scrolled axis the region
    /// grows to its content and never scrolls at all.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Makes the area as tall as its content rather than as tall as the space
    /// around it.
    ///
    /// By default the area fills the height it is offered, which is what a
    /// pane wants. A parent that states no height offers none, and the area
    /// then has nowhere to draw: it disappears, taking its content with it.
    /// A caller placing one in a column that grows with its children says so
    /// here.
    pub fn fit_height(mut self) -> Self {
        self.fit_height = true;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let bound = self.target.is_some();
        // The area's own handle exists whether or not it is used, because the
        // viewport element has to be told about it before the branch below
        // decides whether it scrolls.
        let own_handle = scroll_handle(&self.ident, window, cx);
        let target: Rc<dyn ScrollTarget> = match self.target.clone() {
            Some(target) => target,
            None => Rc::new(own_handle.clone()),
        };
        let offset = target.scroll_offset();
        let max = target.max_scroll_offset();
        // Whether a scrollbar is needed depends on a size only layout knows,
        // so the viewport is measured during prepaint and the frame that
        // learns a new size asks for one more.
        let measured = measure::cell(&self.ident.child("viewport").semantic_id(), window, cx);
        let viewport = measured.get().size;

        let content = div()
            .id(self.ident.child("content").element_id())
            .when(self.axis.has_vertical(), |element| element.min_h(px(0.0)))
            .when(!self.axis.has_horizontal(), |element| element.w_full())
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.child("content").semantic_id(), Role::Group)
                    .parent(self.ident.semantic_id()),
            )
            .children(self.content);

        let fit_height = self.fit_height;
        let viewport_element = div()
            .id(self.ident.child("viewport").element_id())
            .w_full()
            .when(!fit_height, |element| element.h_full())
            // A bound area scrolls nothing: the thing it is bound to already
            // does, and a second scrolling container over the same content
            // would fight it.
            .when(!bound && self.axis.has_vertical(), |element| {
                element.overflow_y_scroll()
            })
            .when(!bound && self.axis.has_horizontal(), |element| {
                element.overflow_x_scroll()
            })
            .when(!bound, |element| element.track_scroll(&own_handle))
            .child(content);

        // Which edges hide something is a function of the offset and nothing
        // else: it does not animate, it does not ask for a frame, and
        // scrolling back takes it away again because the offset went back. It
        // is information — there is content past this edge — so it is not
        // suppressed under reduced motion.
        //
        // The content fades rather than a wash being laid over it. A gradient
        // tinted with the colour of whatever is behind the region says nothing
        // on a surface that colour does not match, which is how a scrolled
        // panel in the dark theme came to show a hard cut through the last row
        // of glyphs and no cue at all.
        let band = px(theme.effects.edge_fade_band);
        let hides = |distance: f32| ScrollLink::over(band).progress(px(distance)) > 0.0;
        let edges = FadeEdges {
            top: self.axis.has_vertical() && hides(-f32::from(offset.y)),
            bottom: self.axis.has_vertical()
                && hides((f32::from(max.y) + f32::from(offset.y)).max(0.0)),
            left: self.axis.has_horizontal() && hides(-f32::from(offset.x)),
            right: self.axis.has_horizontal()
                && hides((f32::from(max.x) + f32::from(offset.x)).max(0.0)),
        };
        let viewport_element = ScrollFade::inside(self.ident.child("fade"))
            .edges(edges)
            .when(fit_height, |fade| fade.fit_height())
            .child(viewport_element);

        let viewport_frame = div()
            .relative()
            .on_children_prepainted({
                let measured = Rc::clone(&measured);
                move |bounds, window, _| {
                    if let Some(first) = bounds.first() {
                        measure::record(&measured, *first, window);
                    }
                }
            })
            .when(!self.fit_height, |element| element.flex_1())
            .min_w(px(0.0))
            .min_h(px(0.0))
            // The viewport stays the first child: the prepaint above measures
            // whichever child comes first, and it is the viewport that decides
            // whether a scrollbar is needed.
            .child(viewport_element);

        let vertical = self.axis.has_vertical().then(|| {
            bar(
                &self.ident,
                "vertical",
                true,
                f32::from(viewport.height),
                f32::from(max.y),
                -f32::from(offset.y),
                &target,
                offset,
                &theme,
                window,
                cx,
            )
        });
        let horizontal = self.axis.has_horizontal().then(|| {
            bar(
                &self.ident,
                "horizontal",
                false,
                f32::from(viewport.width),
                f32::from(max.x),
                -f32::from(offset.x),
                &target,
                offset,
                &theme,
                window,
                cx,
            )
        });

        let body = div()
            .flex()
            .flex_row()
            .items_stretch()
            .flex_1()
            .min_h(px(0.0))
            .child(viewport_frame)
            .children(vertical);

        div()
            .id(self.ident.element_id())
            .flex()
            .flex_col()
            .when_some(self.width, |element, width| element.w(px(width)))
            .when_some(self.height, |element, height| element.h(px(height)))
            .when(
                self.width.is_none() && self.height.is_none() && !self.fit_height,
                |element| element.size_full(),
            )
            .when(self.fit_height && self.width.is_none(), |element| {
                element.w_full()
            })
            .child(body)
            .children(horizontal)
            .semantic_in(cx, {
                let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Region);
                if let Some(label) = self.label.clone() {
                    spec = spec.text(label);
                }
                spec
            })
    }
}

/// One scrollbar gutter, with a thumb only while there is something to scroll.
///
/// The gutter is reserved whether or not the thumb is drawn, so showing a
/// scrollbar never takes space away from the content that decided it was
/// needed.
#[allow(clippy::too_many_arguments)]
fn bar(
    ident: &Ident,
    axis: &str,
    vertical: bool,
    viewport: f32,
    max: f32,
    scrolled: f32,
    target: &Rc<dyn ScrollTarget>,
    offset: Point<Pixels>,
    theme: &Theme,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let bar_ident = ident.child("scrollbar").child(axis);
    let content = viewport + max;
    let overflowing = max > 0.5 && viewport > 0.0;
    let track = measure::cell(&bar_ident.semantic_id(), window, cx);

    let fraction = if content > 0.0 {
        (viewport / content).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let position = if max > 0.0 {
        (scrolled / max).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // The thumb rests at part strength and comes up to full while the pointer
    // is anywhere in the region, which is how it stays legible without being
    // the loudest thing in a panel that is mostly text.
    let hover_group = bar_ident.child("hover").semantic_id();
    let thumb = overflowing.then(|| {
        let resting = theme.resting_track();
        let active = theme.colors.track;
        let track_width = theme.measures.scrollbar_track;
        let thumb_width = theme.measures.scrollbar_thumb;
        div()
            .absolute()
            .rounded_full()
            .bg(resting)
            .group_hover(hover_group.clone(), move |style| style.bg(active))
            .when(vertical, |element| {
                element
                    .w(px(thumb_width))
                    .left(px((track_width - thumb_width) / 2.0))
                    .min_h(px(theme.measures.scrollbar_min_thumb))
                    .h(relative(fraction))
                    .top(relative(position * (1.0 - fraction)))
            })
            .when(!vertical, |element| {
                element
                    .h(px(thumb_width))
                    .top(px((track_width - thumb_width) / 2.0))
                    .min_w(px(theme.measures.scrollbar_min_thumb))
                    .w(relative(fraction))
                    .left(relative(position * (1.0 - fraction)))
            })
    });

    // The gutter paints nothing. A reserved lane that carried a surface
    // colour drew a stripe down the side of every scrolling region whether or
    // not there was anything to scroll, which is a border by another name.
    let mut gutter = div()
        .id(bar_ident.element_id())
        .relative()
        .size_full()
        .children(thumb);

    if overflowing {
        // A list that measures its rows as they arrive learns it is taller
        // than it thought while the thumb is being dragged, which would move
        // the thumb out from under the pointer holding it.
        gutter = gutter
            .on_mouse_down(MouseButton::Left, {
                let target = Rc::clone(target);
                move |_, _, _| target.set_scroll_dragging(true)
            })
            .on_mouse_up(MouseButton::Left, {
                let target = Rc::clone(target);
                move |_, _, _| target.set_scroll_dragging(false)
            })
            .on_mouse_up_out(MouseButton::Left, {
                let target = Rc::clone(target);
                move |_, _, _| target.set_scroll_dragging(false)
            });

        let target = Rc::clone(target);
        let track = Rc::clone(&track);
        gutter = gutter.on_mouse_move(move |event, window, _| {
            if event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let bounds = track.get();
            let (origin, extent, pointer) = if vertical {
                (
                    f32::from(bounds.top()),
                    f32::from(bounds.size.height),
                    f32::from(event.position.y),
                )
            } else {
                (
                    f32::from(bounds.left()),
                    f32::from(bounds.size.width),
                    f32::from(event.position.x),
                )
            };
            if extent <= 0.0 {
                return;
            }
            let travel = (extent * (1.0 - fraction)).max(f32::EPSILON);
            let next = (((pointer - origin) - travel * fraction / 2.0) / travel).clamp(0.0, 1.0);
            let scrolled = -next * max;
            target.set_scroll_offset(if vertical {
                gpui::point(offset.x, px(scrolled))
            } else {
                gpui::point(px(scrolled), offset.y)
            });
            window.refresh();
        });
    }

    // A scrollbar exists only where there is something to scroll: an absent
    // node is how a test reads "there is nothing more".
    if overflowing {
        gutter = gutter.semantic_in(
            cx,
            NodeSpec::new(bar_ident.semantic_id(), Role::Scrollbar)
                .parent(ident.semantic_id())
                .text(cx.strings().text(if vertical {
                    StringKey::ScrollbarVertical
                } else {
                    StringKey::ScrollbarHorizontal
                }))
                .value(cx.strings().format(
                    StringKey::CountOfTotal,
                    &[
                        cx.numbers().decimal(f64::from(scrolled), 0).as_ref(),
                        cx.numbers().decimal(f64::from(max), 0).as_ref(),
                    ],
                ))
                .range(0.0, max, scrolled.clamp(0.0, max)),
        );
    }

    div()
        .on_children_prepainted({
            let track = Rc::clone(&track);
            move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    measure::record(&track, *first, window);
                }
            }
        })
        .flex_none()
        .group(hover_group)
        // The thumb stops short of both ends of its lane. A thumb that runs
        // into the corner of a rounded container crosses outside the shape it
        // is drawn in, and that corner is where the container's own radius is
        // already saying the region stops.
        .when(vertical, |element| {
            element
                .w(px(theme.measures.scrollbar_track))
                .h_full()
                .py(px(theme.space(gpui_kit_theme::Space::Sm)))
        })
        .when(!vertical, |element| {
            element
                .h(px(theme.measures.scrollbar_track))
                .w_full()
                .px(px(theme.space(gpui_kit_theme::Space::Sm)))
        })
        .child(gutter)
        .into_any_element()
}

type ScrollHandles = HashMap<SharedString, ScrollHandle>;

/// How far the region with this identity has been scrolled, in pixels down
/// and across from the start of its content.
///
/// This is what a scroll-linked value reads: pair it with
/// [`ScrollLink`] to collapse a header or fade a
/// heading as the content moves under it. A region that has never rendered
/// reports zero, which is where it will be when it does.
pub fn scroll_offset(ident: impl Into<Ident>, window: &Window, cx: &mut App) -> Point<Pixels> {
    let offset = scroll_handle(&ident.into(), window, cx).offset();
    gpui::point(-offset.x, -offset.y)
}

/// Puts the region with this identity at `offset`, measured the same way
/// [`scroll_offset`] reports it.
///
/// The counterpart to reading, for a host restoring where the user was: a
/// reopened document belongs where they left it, not at the top. It takes
/// effect on the next frame, and a region that has never rendered remembers
/// the position until it does.
pub fn scroll_to(ident: impl Into<Ident>, offset: Point<Pixels>, window: &Window, cx: &mut App) {
    scroll_handle(&ident.into(), window, cx).set_offset(gpui::point(-offset.x, -offset.y));
}

/// The scroll position of the region with this identity.
///
/// The same handle [`scroll_offset`] and [`scroll_to`] address, for a
/// component that has to hand it to the element it is about to build — a menu
/// that scrolls, and so has an edge that may be hiding a row — without
/// carrying a field for a region whose existence depends on what it was given
/// to show.
pub(crate) fn scroll_handle(ident: &Ident, window: &Window, cx: &mut App) -> ScrollHandle {
    window_state::with(
        window.window_handle().window_id(),
        cx,
        |handles: &mut ScrollHandles| handles.entry(ident.semantic_id()).or_default().clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_axis_knows_which_gutters_it_reserves() {
        assert!(ScrollAxis::Vertical.has_vertical());
        assert!(!ScrollAxis::Vertical.has_horizontal());
        assert!(ScrollAxis::Both.has_vertical() && ScrollAxis::Both.has_horizontal());
    }
}
