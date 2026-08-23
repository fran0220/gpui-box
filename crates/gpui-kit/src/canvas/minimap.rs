//! A caller-owned overview of a larger canvas.
//!
//! Marks and the viewport rectangle are already normalized by the host into
//! the `0..=1` square. Clicking reports the normalized point; the host pans.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Surface};

use crate::foundation::{Ident, StyledExt};
use crate::layout::measure;
use crate::strings::{ActiveStrings, StringKey};

type PanHandler = Rc<dyn Fn(f32, f32, &mut Window, &mut App)>;

/// One already-normalized mark on the overview.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapMark {
    pub id: SharedString,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl MinimapMark {
    pub fn new(id: impl Into<SharedString>, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            x,
            y,
            width,
            height,
        }
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
                div()
                    .absolute()
                    .left(relative(mark.x.clamp(0.0, 1.0)))
                    .top(relative(mark.y.clamp(0.0, 1.0)))
                    .w(relative(mark.width.clamp(0.04, 1.0)))
                    .h(relative(mark.height.clamp(0.04, 1.0)))
                    .bg(theme.colors.accent.opacity(0.7))
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
        let viewport = self.view.map(|view| {
            div()
                .absolute()
                .left(relative(view.x.clamp(0.0, 1.0)))
                .top(relative(view.y.clamp(0.0, 1.0)))
                .w(relative(view.width.clamp(0.04, 1.0)))
                .h(relative(view.height.clamp(0.04, 1.0)))
                .border(px(theme.borders.hairline))
                .border_color(theme.colors.danger)
        });
        let measured = measure::cell(&self.ident.semantic_id(), window, cx);
        let handler = self.on_pan;
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
            .radius(&theme, Radius::Small)
            .surface(&theme, Surface::Overlay)
            .elevation(&theme, Elevation::Raised)
            .overflow_hidden()
            .children(marks)
            .children(viewport)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status)
                    .text(cx.strings().text(StringKey::GraphMinimap))
                    .value("minimap"),
            );
        if let Some(handler) = handler {
            frame = frame.on_mouse_down_with_pointer_capture(
                MouseButton::Left,
                move |event, window, cx| {
                    let bounds = measured.get();
                    let width = f32::from(bounds.size.width).max(1.0);
                    let height = f32::from(bounds.size.height).max(1.0);
                    let x = ((f32::from(event.position.x) - f32::from(bounds.origin.x)) / width)
                        .clamp(0.0, 1.0);
                    let y = ((f32::from(event.position.y) - f32::from(bounds.origin.y)) / height)
                        .clamp(0.0, 1.0);
                    handler(x, y, window, cx);
                },
            );
        }
        frame
    }
}
