//! A caller-owned overview of a larger canvas.
//!
//! Marks and the viewport rectangle are already normalized by the host into
//! the `0..=1` square. Clicking reports the normalized point; when the
//! overview is focusable, the arrow keys report a point one twentieth of the
//! canvas away from the current viewport centre. The host applies every pan.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ColorChoice, Elevation, Radius, Surface, Variant};

use crate::foundation::{FocusRing, Ident, Pressable, StyledExt};
use crate::layout::measure;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type PanHandler = Rc<dyn Fn(f32, f32, &mut Window, &mut App)>;

/// A minimap is normalized, so a fixed normalized step remains independent
/// of the host's canvas units while still offering twenty stops per axis.
const KEYBOARD_PAN_STEP: f32 = 0.05;

/// One already-normalized mark on the overview.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapMark {
    pub id: SharedString,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Optional caller-owned category colour. A mark without one remains
    /// neutral; a mark with one resolves through the same palette tier as a
    /// graph node, so identity survives the change in scale.
    pub color: Option<ColorChoice>,
}

impl MinimapMark {
    pub fn new(id: impl Into<SharedString>, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            x,
            y,
            width,
            height,
            color: None,
        }
    }

    pub fn color(mut self, color: impl Into<ColorChoice>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// The visible window, already normalized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapView {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl MinimapView {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Keeps a normalized viewport rectangle inside the minimap without changing
/// the extent it represents. Clamp the extent first, then seat its origin in
/// the remaining space: clamping width against `1 - x` panics when a caller
/// truthfully reports a view whose origin has reached the far edge.
pub(super) fn bounded_view(view: MinimapView) -> MinimapView {
    let width = view.width.clamp(0.04, 1.0);
    let height = view.height.clamp(0.04, 1.0);
    MinimapView {
        x: view.x.clamp(0.0, 1.0 - width),
        y: view.y.clamp(0.0, 1.0 - height),
        width,
        height,
    }
}

/// What a click on the overview reported.
#[derive(Debug, Clone, PartialEq)]
pub enum MinimapEvent {
    Pan { x: f32, y: f32 },
}

/// A compact overview of host-owned marks.
#[derive(IntoElement)]
pub struct Minimap {
    ident: Ident,
    marks: Vec<MinimapMark>,
    view: Option<MinimapView>,
    on_pan: Option<PanHandler>,
}

impl Minimap {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            marks: Vec::new(),
            view: None,
            on_pan: None,
        }
    }

    pub fn marks(mut self, marks: impl IntoIterator<Item = MinimapMark>) -> Self {
        self.marks = marks.into_iter().collect();
        self
    }

    pub fn view(mut self, view: MinimapView) -> Self {
        self.view = Some(view);
        self
    }

    pub fn on_pan(mut self, handler: impl Fn(f32, f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_pan = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Minimap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let marks = self
            .marks
            .iter()
            .map(|mark| {
                let colors = mark
                    .color
                    .as_ref()
                    .map(|color| theme.variant_colors(Variant::Light, color));
                div()
                    .absolute()
                    .left(relative(mark.x.clamp(0.0, 1.0)))
                    .top(relative(mark.y.clamp(0.0, 1.0)))
                    .w(relative(mark.width.clamp(0.04, 1.0)))
                    .h(relative(mark.height.clamp(0.04, 1.0)))
                    .radius(&theme, Radius::Small)
                    .bg(colors.map_or(theme.colors.loader_placeholder, |colors| colors.background))
                    .semantic_in(
                        cx,
                        NodeSpec::new(
                            self.ident
                                .child("mark")
                                .child(mark.id.as_ref())
                                .semantic_id(),
                            Role::Status,
                        )
                        .text(mark.id.clone()),
                    )
            })
            .collect::<Vec<_>>();
        // The viewport is where the reader is, not an alarm, and it is kept
        // inside the square: a rectangle that overhangs the overview reports
        // a view of somewhere the overview does not describe.
        //
        // It is drawn as a hole in a veil rather than as a filled rectangle.
        // A fill sits *on* the marks it covers, so the part of the canvas the
        // reader is actually looking at was the part hardest to read; veiling
        // the rest instead leaves the view clear and dims what is off screen,
        // which is what the two regions mean. The veil is four bands rather
        // than one shape because a rectangular hole is not a shape a fill can
        // have.
        let viewport = self.view.map(|view| {
            let MinimapView {
                x,
                y,
                width,
                height,
            } = bounded_view(view);
            let veil = theme
                .colors
                .canvas
                .opacity(theme.effects.node_overview_veil_alpha);
            let band = |left: f32, top: f32, wide: f32, tall: f32| {
                div()
                    .absolute()
                    .left(relative(left))
                    .top(relative(top))
                    .w(relative(wide.max(0.0)))
                    .h(relative(tall.max(0.0)))
                    .bg(veil)
            };
            div()
                .absolute()
                .inset_0()
                .child(band(0.0, 0.0, 1.0, y))
                .child(band(0.0, y + height, 1.0, 1.0 - y - height))
                .child(band(0.0, y, x, height))
                .child(band(x + width, y, 1.0 - x - width, height))
                .child(
                    div()
                        .absolute()
                        .left(relative(x))
                        .top(relative(y))
                        .w(relative(width))
                        .h(relative(height))
                        .radius(&theme, Radius::Small)
                        .bg(theme
                            .color_wash(theme.colors.accent, gpui_kit_theme::SemanticWash::Faint)),
                )
        });
        let measured = measure::cell(&self.ident.semantic_id(), window, cx);
        let handler = self.on_pan;
        let (keyboard_x, keyboard_y) = self.view.map_or((0.5, 0.5), |view| {
            (
                (view.x + view.width / 2.0).clamp(0.0, 1.0),
                (view.y + view.height / 2.0).clamp(0.0, 1.0),
            )
        });
        let horizontal = cx.numbers().decimal(f64::from(keyboard_x * 100.0), 0);
        let vertical = cx.numbers().decimal(f64::from(keyboard_y * 100.0), 0);
        let position = cx.strings().format(
            StringKey::GraphMinimapPosition,
            &[horizontal.as_ref(), vertical.as_ref()],
        );
        let actionable = handler.is_some();
        let mut spec = NodeSpec::new(
            self.ident.semantic_id(),
            if actionable {
                Role::Slider
            } else {
                Role::Status
            },
        )
        .text(cx.strings().text(StringKey::GraphMinimap))
        .value(position);
        if actionable {
            // The semantic protocol has one scalar range. Publish the
            // horizontal centre there, and keep both axes in the value text
            // that a reader hears.
            spec = spec.range(0.0, 1.0, keyboard_x);
        }
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
            .relative()
            .w(px(160.0))
            .h(px(100.0))
            // The minimap is a detached overlay panel, so it takes the same
            // rounding as a card or popover rather than a tiny control.
            .radius(&theme, Radius::Card)
            .surface(&theme, Surface::Overlay)
            .elevation(&theme, Elevation::Raised)
            .overflow_hidden()
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .focus_ring(&theme)
            })
            // The prepaint hook reports child bounds. Keep a full-frame
            // measuring child first so normalized pointer coordinates are
            // based on the overview, not whichever mark happens to come first.
            .child(div().absolute().inset_0())
            .children(marks)
            .children(viewport)
            .semantic_in(cx, spec);
        if let Some(handler) = handler {
            let pointer = Rc::clone(&handler);
            frame = frame
                .on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, window, cx| {
                    let bounds = measured.get();
                    let width = f32::from(bounds.size.width).max(1.0);
                    let height = f32::from(bounds.size.height).max(1.0);
                    let x = ((f32::from(event.position.x) - f32::from(bounds.origin.x)) / width)
                        .clamp(0.0, 1.0);
                    let y = ((f32::from(event.position.y) - f32::from(bounds.origin.y)) / height)
                        .clamp(0.0, 1.0);
                    pointer(x, y, window, cx);
                })
                .on_key_down(move |event, window, cx| {
                    let (x, y) = match event.keystroke.key.as_str() {
                        "left" => (keyboard_x - KEYBOARD_PAN_STEP, keyboard_y),
                        "right" => (keyboard_x + KEYBOARD_PAN_STEP, keyboard_y),
                        "up" => (keyboard_x, keyboard_y - KEYBOARD_PAN_STEP),
                        "down" => (keyboard_x, keyboard_y + KEYBOARD_PAN_STEP),
                        _ => return,
                    };
                    handler(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0), window, cx);
                    cx.stop_propagation();
                });
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_keeps_the_category_the_caller_gave_it() {
        let mark = MinimapMark::new("ingest", 0.1, 0.2, 0.3, 0.4).color("teal");
        assert_eq!(mark.color, Some(ColorChoice::Palette("teal".into())));
    }

    #[test]
    fn a_view_at_the_far_edge_keeps_its_extent_inside_the_minimap() {
        let view = bounded_view(MinimapView::new(1.0, 1.0, 0.0, 0.25));
        assert_eq!(view, MinimapView::new(0.96, 0.75, 0.04, 0.25));
    }
}
