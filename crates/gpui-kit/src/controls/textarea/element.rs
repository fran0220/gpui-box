//! The painted surface of a [`super::TextArea`].
//!
//! GPUI has no built-in editable text element, so the area shapes its own
//! wrapped text and paints the selection and caret around it. Doing the
//! shaping here, rather than in the view, keeps the measured layout next to
//! the bounds the same frame produced, which is what a hit test, the visual
//! motion keys, and the input method all need.

use gpui::{
    App, Bounds, EditableTextLayout, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, HighlightStyle, IntoElement, LayoutId, PaintQuad, Pixels, Style, TextRun,
    TextStyle, UnderlineStyle, Window, fill, point, px, relative,
};
use gpui_kit_theme::ActiveTheme;

use super::{TextArea, TextAreaWrap, text_edit};

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
    document_layout: Option<EditableTextLayout>,
    source_text: gpui::SharedString,
    rows: std::sync::Arc<[std::ops::Range<usize>]>,
    indexed_rows: usize,
    cursors: Vec<PaintQuad>,
    selection: Vec<PaintQuad>,
    scroll_offset: Pixels,
    horizontal_scroll_offset: Pixels,
    visible_rows: usize,
    visual_transform: gpui::VisualTransform,
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
        let selections = area.selections();
        let cursor = area.cursor_offset();
        let marked = area.marked_range();
        let (min_rows, max_rows) = area.row_limits();
        let empty = content.is_empty();
        let style = window.text_style();
        let wrap = area.wrap_mode();

        let (display_text, text_color) = if empty {
            (
                area.placeholder_text().clone(),
                theme.colors.text_placeholder,
            )
        } else {
            (content, style.color)
        };

        let mut base = style.clone();
        base.color = text_color;
        let highlights = if empty { &[] } else { area.highlights() };
        // Caller-owned source colours and IME composition share one run
        // partition, so highlighting cannot shift the geometry used by the
        // caret, selection, hit testing, accessibility, or the input method.
        let runs = text_runs(
            display_text.len(),
            &base,
            highlights,
            marked,
            px(theme.measures.text_decoration_width),
        );

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let mut layout = if wrap == TextAreaWrap::None && !empty {
            let document = area.document();
            let (_, visible) = area.source_viewport(line_height, bounds.size.height);
            let layout = EditableTextLayout::unwrapped_projected(
                document,
                window.text_system().clone(),
                font_size,
                line_height,
                runs,
                visible,
                area.source_projection(),
            )
            .expect("current source line projection");
            // Width and painting share these same shaped visible rows.
            layout.painted_lines().for_each(drop);
            layout
        } else if !empty {
            area.wrapped_cache.borrow_mut().update(
                area.document(),
                window.text_system().clone(),
                font_size,
                line_height,
                runs,
                bounds.size.width,
            )
        } else {
            area.wrapped_cache.borrow_mut().clear();
            let lines = window
                .text_system()
                .shape_text(
                    display_text,
                    font_size,
                    &runs,
                    (wrap == TextAreaWrap::Soft).then_some(bounds.size.width),
                    None,
                )
                .map(|lines| lines.into_iter().collect::<Vec<_>>())
                .unwrap_or_default();
            EditableTextLayout::new(source_text.as_ref(), lines, line_height)
        };
        // Empty document geometry must not use the placeholder's glyphs.
        let document_layout = empty.then(|| {
            let lines = window
                .text_system()
                .shape_text("".into(), font_size, &[style.to_run(0)], None, None)
                .map(|lines| lines.into_iter().collect())
                .unwrap_or_default();
            EditableTextLayout::new("", lines, line_height)
        });
        // A placeholder never grows the frame: only what was typed does.
        let visible_rows = if empty {
            min_rows
        } else {
            layout.total_rows().clamp(min_rows, max_rows)
        };

        // Browsing does not move or reveal the caret. Only explicit editing
        // and selection/navigation intent asks this frame to reveal it.
        let scroll_offset = if area.reveal_caret {
            layout.scroll_offset_to_reveal(cursor, bounds.size.height, area.scroll_offset())
        } else {
            area.scroll_offset()
                .clamp(px(0.0), (layout.height() - bounds.size.height).max(px(0.0)))
        };
        let horizontal_scroll_offset = match wrap {
            TextAreaWrap::Soft => px(0.0),
            TextAreaWrap::None if area.reveal_caret => layout.horizontal_scroll_offset_to_reveal(
                cursor,
                bounds.size.width,
                area.horizontal_scroll_offset(),
            ),
            TextAreaWrap::None => area.horizontal_scroll_offset().clamp(
                px(0.0),
                (area.known_text_width.max(layout.text_width()) - bounds.size.width).max(px(0.0)),
            ),
        };

        let origin = point(
            bounds.left() - horizontal_scroll_offset,
            bounds.top() - scroll_offset,
        );
        if wrap == TextAreaWrap::Soft && !empty {
            let first = (scroll_offset / line_height).floor().max(0.0) as usize;
            let end = ((scroll_offset + bounds.size.height) / line_height)
                .ceil()
                .max(0.0) as usize;
            layout.set_painted_rows(first..end);
        }
        // Row topology and cell geometry must describe the same prepaint,
        // including the first frame after an edit or wrap-width change.
        let (rows, indexed_rows) = if area
            .last_layout
            .as_ref()
            .is_some_and(|previous| layout.shares_row_index_with(previous))
        {
            (area.last_layout_rows.clone(), 0)
        } else {
            let rows: std::sync::Arc<[_]> = layout.visual_rows(&source_text).into();
            let count = rows.len();
            (rows, count)
        };
        let accessible_geometry = text_edit::AccessibleTextGeometry::capture_ranges(
            source_text.clone(),
            window.scale_factor(),
            layout.painted_source_ranges(),
            |range| {
                layout.bounds_for_range(range, origin, gpui::TextAlign::Left, bounds.size.width)
            },
        );
        *area
            .accessible_geometry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(super::AccessibleLayout {
            geometry: accessible_geometry,
            rows: area.line_projection.is_none().then(|| rows.clone()),
        });
        let visible = layout.painted_source_ranges();
        let cursors = selections
            .iter()
            .filter(|(range, _)| range.is_empty())
            .filter(|(range, _)| {
                empty
                    || visible
                        .iter()
                        .any(|visible| visible.start <= range.start && range.start <= visible.end)
            })
            .map(|(range, _)| {
                let position = if *range == area.edit.selection() {
                    area.edit.native_selection().head
                } else {
                    gpui::NativeTextPosition {
                        utf16_offset: gpui::offset_to_utf16(&source_text, range.start),
                        ..Default::default()
                    }
                };
                fill(
                    layout
                        .native_position_bounds(
                            &source_text,
                            position,
                            origin,
                            px(theme.measures.caret_width),
                            gpui::TextAlign::Left,
                            bounds.size.width,
                        )
                        .unwrap_or_else(|| {
                            layout.caret_bounds(range.start, origin, px(theme.measures.caret_width))
                        }),
                    theme.colors.accent,
                )
            })
            .collect();

        let selection = selections
            .iter()
            .flat_map(|(range, _)| {
                layout.painted_bounds_for_range(
                    range.clone(),
                    origin,
                    gpui::TextAlign::Left,
                    bounds.size.width,
                )
            })
            .map(|bounds| fill(bounds, theme.colors.selected))
            .collect();

        PrepaintState {
            layout: Some(layout),
            document_layout,
            source_text,
            rows,
            indexed_rows,
            cursors,
            selection,
            scroll_offset,
            horizontal_scroll_offset,
            visible_rows,
            visual_transform: window.visual_transform(),
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
                ElementInputHandler::new(bounds, self.area.clone())
                    .with_visual_transform(prepaint.visual_transform),
                cx,
            );
        }

        let scroll_offset = prepaint.scroll_offset;
        let horizontal_scroll_offset = prepaint.horizontal_scroll_offset;
        let visible_rows = prepaint.visible_rows;
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for selection in prepaint.selection.drain(..) {
                window.paint_quad(selection);
            }
            if let Some(layout) = prepaint.layout.take() {
                for (line, top) in layout.painted_lines() {
                    line.paint(
                        point(
                            bounds.origin.x - horizontal_scroll_offset,
                            bounds.origin.y + top - scroll_offset,
                        ),
                        window.line_height(),
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
                }
                let caret_width = px(cx.theme().measures.caret_width);
                self.area.update(cx, |area, cx| {
                    let grew = area.visible_rows() != visible_rows;
                    let scroll_changed = area.scroll_dirty || area.scroll_offset() != scroll_offset;
                    let horizontal_scroll_changed =
                        area.horizontal_scroll_offset() != horizontal_scroll_offset;
                    area.scroll_dirty = false;
                    area.reveal_caret = false;
                    area.known_text_width = area.known_text_width.max(layout.text_width());
                    area.set_visible_rows(visible_rows);
                    area.set_scroll_offset(scroll_offset);
                    area.set_horizontal_scroll_offset(horizontal_scroll_offset);
                    let layout_changed = area.set_last_layout(
                        prepaint.document_layout.take().unwrap_or(layout),
                        prepaint.source_text.clone(),
                        bounds,
                        caret_width,
                        prepaint.rows.clone(),
                        prepaint.indexed_rows,
                        prepaint.visual_transform,
                    );
                    // Geometry and accessibility consumers need one
                    // corrective frame when shaped rows, bounds, or scrolling
                    // change. Notifying on every paint would redraw forever.
                    if grew || layout_changed || scroll_changed || horizontal_scroll_changed {
                        if layout_changed || scroll_changed || horizontal_scroll_changed {
                            cx.emit(crate::controls::textarea::TextAreaEvent::GeometryChanged);
                        }
                        cx.notify();
                    }
                });
            }
            if !disabled && focus_handle.is_focused(window) {
                for cursor in prepaint.cursors.drain(..) {
                    window.paint_quad(cursor);
                }
            }
        });
    }
}

fn text_runs(
    len: usize,
    base: &TextStyle,
    highlights: &[(std::ops::Range<usize>, HighlightStyle)],
    marked: Option<std::ops::Range<usize>>,
    decoration_width: Pixels,
) -> Vec<TextRun> {
    if len == 0 {
        return vec![base.to_run(0)];
    }
    let marked = marked.filter(|range| range.end <= len);
    let mut boundaries = vec![0, len];
    for (range, _) in highlights {
        boundaries.extend([range.start, range.end]);
    }
    if let Some(range) = marked.as_ref() {
        boundaries.extend([range.start, range.end]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    boundaries
        .windows(2)
        .filter_map(|edge| {
            let range = edge[0]..edge[1];
            if range.is_empty() {
                return None;
            }
            let highlight = highlights
                .iter()
                .find(|(highlighted, _)| {
                    highlighted.start <= range.start && range.end <= highlighted.end
                })
                .map(|(_, style)| *style);
            let mut style = base.clone();
            if let Some(highlight) = highlight {
                style = style.highlight(highlight);
            }
            if marked
                .as_ref()
                .is_some_and(|marked| marked.start <= range.start && range.end <= marked.end)
            {
                style.underline = Some(UnderlineStyle {
                    color: Some(style.color),
                    thickness: decoration_width,
                    wavy: false,
                });
            }
            let mut run = style.to_run(range.len());
            run.background_radius = highlight.and_then(|style| style.background_radius);
            Some(run)
        })
        .collect()
}
