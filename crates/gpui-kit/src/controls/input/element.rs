//! The painted surface of a [`super::TextInput`].
//!
//! GPUI has no built-in editable text element, so the input shapes its own
//! line and paints the selection and caret around it. Doing the shaping here,
//! rather than in the view, keeps the measured layout next to the bounds the
//! same frame produced, which is what a hit test and the input method both
//! need.

use gpui::{
    App, Bounds, EditableTextLayout, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, IntoElement, LayoutId, PaintQuad, Pixels, Style, TextRun, UnderlineStyle,
    Window, fill, point, px, relative,
};
use gpui_kit_theme::ActiveTheme;

use super::TextInput;

pub struct TextElement {
    input: Entity<TextInput>,
}

impl TextElement {
    pub fn new(input: Entity<TextInput>) -> Self {
        Self { input }
    }
}

pub struct PrepaintState {
    layout: Option<EditableTextLayout>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    scroll_offset: Pixels,
    custom_visual: bool,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
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
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = if self.input.read(cx).visual_slots().is_some() {
            relative(1.0).into()
        } else {
            window.line_height().into()
        };
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
        let input = self.input.read(cx);
        let custom_visual = input.visual_slots().is_some();
        let content = input.display_text();
        let selected = input.selected_range();
        let cursor = input.display_offset(input.cursor_offset());
        let marked = input
            .marked_range()
            .map(|range| input.display_offset(range.start)..input.display_offset(range.end));
        let selected = input.display_offset(selected.start)..input.display_offset(selected.end);
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (
                input.placeholder_text().clone(),
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
            .shape_text(display_text.clone(), font_size, &runs, None, None)
            .map(|lines| lines.into_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let layout = EditableTextLayout::new(display_text.as_ref(), lines, line_height);

        // Long text scrolls under a fixed frame, so the caret stays visible
        // instead of being painted outside the control.
        let previous = self.input.read(cx).scroll_offset();
        let scroll_offset = if custom_visual {
            px(0.0)
        } else {
            layout.horizontal_scroll_offset_to_reveal(cursor, bounds.size.width, previous)
        };

        let origin = point(bounds.left() - scroll_offset, bounds.top());
        let (selection, cursor) = if custom_visual {
            (Vec::new(), None)
        } else if selected.is_empty() {
            (
                Vec::new(),
                Some(fill(
                    layout.caret_bounds(cursor, origin, px(1.5)),
                    theme.colors.accent,
                )),
            )
        } else {
            (
                layout
                    .bounds_for_range(selected, origin, gpui::TextAlign::Left, bounds.size.width)
                    .into_iter()
                    .map(|bounds| fill(bounds, theme.colors.selected))
                    .collect(),
                None,
            )
        };

        PrepaintState {
            layout: Some(layout),
            cursor,
            selection,
            scroll_offset,
            custom_visual,
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
            let input = self.input.read(cx);
            (input.focus_handle.clone(), input.disabled)
        };
        if !disabled {
            window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, self.input.clone()),
                cx,
            );
        }

        let scroll_offset = prepaint.scroll_offset;
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for selection in prepaint.selection.drain(..) {
                window.paint_quad(selection);
            }
            if let Some(layout) = prepaint.layout.take() {
                if !prepaint.custom_visual {
                    for (line, top) in layout.painted_lines() {
                        line.paint(
                            point(bounds.origin.x - scroll_offset, bounds.origin.y + top),
                            layout.line_height(),
                            gpui::TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .ok();
                    }
                }
                self.input.update(cx, |input, _| {
                    input.set_last_layout(layout, bounds);
                    input.set_scroll_offset(scroll_offset);
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
