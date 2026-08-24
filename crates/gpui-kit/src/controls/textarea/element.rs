//! The painted surface of a [`super::TextArea`].
//!
//! GPUI has no built-in editable text element, so the area shapes its own
//! wrapped text and paints the selection and caret around it. Doing the
//! shaping here, rather than in the view, keeps the measured layout next to
//! the bounds the same frame produced, which is what a hit test, the visual
//! motion keys, and the input method all need.

use gpui::{
    App, Bounds, EditableTextLayout, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, IntoElement, LayoutId, PaintQuad, Pixels, Style, TextRun, UnderlineStyle,
    Window, fill, point, px, relative,
};
use gpui_kit_theme::ActiveTheme;

use super::{TextArea, text_edit};

pub struct TextAreaElement {
    area: Entity<TextArea>,
}

impl TextAreaElement {
    pub fn new(area: Entity<TextArea>) -> Self {
        Self { area }
    }
}

pub struct PrepaintState {
    layout: Option<EditableTextLayout>,
    source_text: gpui::SharedString,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    scroll_offset: Pixels,
    visible_rows: usize,
}

impl IntoElement for TextAreaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextAreaElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // The row count is measured, not guessed: it comes from the last
        // frame that knew how wide the area actually was.
        let rows = self.area.read(cx).visible_rows();
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = (window.line_height() * rows as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = cx.theme().clone();
        let area = self.area.read(cx);
        let content = area.value().clone();
        let source_text = content.clone();
        let selected = area.selected_range();
        let cursor = area.cursor_offset();
        let marked = area.marked_range();
        let (min_rows, max_rows) = area.row_limits();
        let empty = content.is_empty();
        let style = window.text_style();

        let (display_text, text_color) = if empty {
            (
                area.placeholder_text().clone(),
                theme.colors.text_placeholder,
            )
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            background_radius: None,
            underline: None,
            strikethrough: None,
        };
        // An input method underlines what it is still composing, so the
        // typist can see which characters are not yet committed.
        let runs = match marked {
            Some(marked) if marked.end <= display_text.len() => vec![
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            _ => vec![run],
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let lines = window
            .text_system()
            .shape_text(
                display_text,
                font_size,
                &runs,
                Some(bounds.size.width),
                None,
            )
            .map(|lines| lines.into_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let layout = EditableTextLayout::new(source_text.as_ref(), lines, line_height);
        // A placeholder never grows the frame: only what was typed does.
        let visible_rows = if empty {
            min_rows
        } else {
            layout.total_rows().clamp(min_rows, max_rows)
        };

        // Long text scrolls under a fixed frame, so the caret stays visible
        // instead of being painted outside the control.
        let scroll_offset = layout.scroll_offset_to_reveal(
            cursor,
            bounds.size.height,
            self.area.read(cx).scroll_offset(),
        );

        let origin = point(bounds.left(), bounds.top() - scroll_offset);
        let accessible_geometry = text_edit::AccessibleTextGeometry::capture(
            source_text.clone(),
            window.scale_factor(),
            |range| {
                layout.bounds_for_range(range, origin, gpui::TextAlign::Left, bounds.size.width)
            },
        );
        *area
            .accessible_geometry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(accessible_geometry);
        let cursor = (selected.is_empty()).then(|| {
            fill(
                layout.caret_bounds(cursor, origin, px(1.5)),
                theme.colors.accent,
            )
        });

        let selection = layout
            .bounds_for_range(selected, origin, gpui::TextAlign::Left, bounds.size.width)
            .into_iter()
            .map(|bounds| fill(bounds, theme.colors.selected))
            .collect();

        PrepaintState {
            layout: Some(layout),
            source_text,
            cursor,
            selection,
            scroll_offset,
            visible_rows,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, disabled) = {
            let area = self.area.read(cx);
            (area.focus_handle.clone(), area.disabled)
        };
        if !disabled {
            window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, self.area.clone()),
                cx,
            );
        }

        let scroll_offset = prepaint.scroll_offset;
        let visible_rows = prepaint.visible_rows;
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for selection in prepaint.selection.drain(..) {
                window.paint_quad(selection);
            }
            if let Some(layout) = prepaint.layout.take() {
                for (line, top) in layout.painted_lines() {
                    line.paint(
                        point(bounds.origin.x, bounds.origin.y + top - scroll_offset),
                        window.line_height(),
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
                }
                self.area.update(cx, |area, cx| {
                    let grew = area.visible_rows() != visible_rows;
                    area.set_visible_rows(visible_rows);
                    area.set_scroll_offset(scroll_offset);
                    let accessibility_layout_changed =
                        area.set_last_layout(layout, prepaint.source_text.clone(), bounds);
                    // Only a changed height needs another frame; notifying on
                    // every frame would redraw forever. Accessibility needs
                    // one more frame when newly shaped visual rows become
                    // available to the parent node.
                    if grew || accessibility_layout_changed {
                        cx.notify();
                    }
                });
            }
            if !disabled
                && focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });
    }
}
