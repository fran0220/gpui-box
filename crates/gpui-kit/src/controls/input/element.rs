//! The painted surface of a [`super::TextInput`].
//!
//! GPUI has no built-in editable text element, so the input shapes its own
//! line and paints the selection and caret around it. Doing the shaping here,
//! rather than in the view, keeps the measured layout next to the bounds the
//! same frame produced, which is what a hit test and the input method both
//! need.

use gpui::{
    App, Bounds, Element, ElementId, ElementInputHandler, Entity, GlobalElementId, IntoElement,
    LayoutId, PaintQuad, Pixels, ShapedLine, Style, TextRun, UnderlineStyle, Window, fill, point,
    px, relative, size,
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
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
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
            (input.placeholder_text().clone(), theme.colors.text_faint)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
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
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        // Long text scrolls under a fixed frame, so the caret stays visible
        // instead of being painted outside the control.
        let cursor_x = line.x_for_index(cursor);
        let previous = self.input.read(cx).scroll_offset();
        let mut scroll_offset = if custom_visual {
            px(0.0)
        } else {
            previous.min(line.width - bounds.size.width).max(px(0.0))
        };
        if !custom_visual {
            if cursor_x - scroll_offset > bounds.size.width {
                scroll_offset = cursor_x - bounds.size.width;
            }
            if cursor_x < scroll_offset {
                scroll_offset = cursor_x;
            }
            if line.width <= bounds.size.width {
                scroll_offset = px(0.0);
            }
        }

        let left = bounds.left() - scroll_offset;
        let (selection, cursor) = if custom_visual {
            (None, None)
        } else if selected.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(left + cursor_x, bounds.top()),
                        size(px(1.5), bounds.size.height),
                    ),
                    theme.colors.accent,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(left + line.x_for_index(selected.start), bounds.top()),
                        point(left + line.x_for_index(selected.end), bounds.bottom()),
                    ),
                    theme.colors.selected,
                )),
                None,
            )
        };

        PrepaintState {
            line: Some(line),
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
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(selection);
            }
            if let Some(line) = prepaint.line.take() {
                if !prepaint.custom_visual {
                    line.paint(
                        point(bounds.origin.x - scroll_offset, bounds.origin.y),
                        window.line_height(),
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
                }
                self.input.update(cx, |input, _| {
                    input.set_last_layout(line, bounds);
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
