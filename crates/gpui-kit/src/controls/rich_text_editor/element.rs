//! Styled block layout and paint for [`super::RichTextEditor`].

use std::ops::Range;

use gpui::{
    App, Bounds, EditableTextLayout, Element, ElementId, ElementInputHandler, Entity, FontStyle,
    FontWeight, GlobalElementId, IntoElement, LayoutId, PaintQuad, Pixels, Point, SharedString,
    StrikethroughStyle, Style, TextAlign, TextRun, UnderlineStyle, Window, fill, font, point, px,
    relative, size,
};
use gpui_kit_theme::{ActiveTheme, Radius};

use crate::content::{
    RichTextAlignment, RichTextBlock, RichTextFormat, RichTextListKind, RichTextSelection,
};
use crate::foundation::direction::ActiveDirection;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

use super::projection::Projection;
use super::{RichTextDiagnosticSeverity, RichTextEditor};

pub(super) struct StoredBlockLayout {
    pub id: crate::content::RichTextBlockId,
    pub source: SharedString,
    pub top: Pixels,
    pub left: Pixels,
    pub width: Pixels,
    pub align: TextAlign,
    pub layout: EditableTextLayout,
}

pub(super) struct RichTextEditorElement {
    editor: Entity<RichTextEditor>,
}

impl RichTextEditorElement {
    pub fn new(editor: Entity<RichTextEditor>) -> Self {
        Self { editor }
    }
}

struct DiagnosticSlice {
    range: Range<usize>,
    severity: RichTextDiagnosticSeverity,
}

struct PaintedBlock {
    layout: Option<StoredBlockLayout>,
    screen_origin: Point<Pixels>,
    marker: Option<gpui::WrappedLine>,
    marker_origin: Point<Pixels>,
    marker_width: Pixels,
}

pub(super) struct PrepaintState {
    blocks: Vec<PaintedBlock>,
    selections: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
    content_height: Pixels,
    visible_rows: usize,
    scroll_offset: Pixels,
}

impl IntoElement for RichTextEditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichTextEditorElement {
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
        let rows = self.editor.read(cx).visible_rows();
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
        let direction = cx.layout_direction();
        let editor = self.editor.read(cx);
        let session = editor.session().read(cx);
        let document = session.document();
        let selection = session.selection().clone();
        let projection = Projection::new(document);
        let selection_range = projection
            .range_for_selection(&selection)
            .unwrap_or_default();
        let marked_range = session.marked_range().and_then(|range| {
            projection.range_for_selection(&RichTextSelection::new(
                range.start.clone(),
                range.end.clone(),
            ))
        });
        let empty_document = document
            .blocks()
            .iter()
            .all(|block| block.text().is_empty());
        let base = window.text_style();
        let font_size = base.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let paragraph_gap = px(theme.spacing.sm);
        let list_gap = px(theme.spacing.xs);
        let marker_width = px(theme.spacing.lg);
        let mut top = px(0.0);
        let mut blocks = Vec::with_capacity(document.blocks().len());

        for (index, block) in document.blocks().iter().enumerate() {
            let list = block.paragraph().list();
            let indent = list
                .map(|item| px(theme.spacing.lg) * (item.depth as f32 + 1.0))
                .unwrap_or(px(0.0));
            let marker_room = list.map(|_| marker_width).unwrap_or(px(0.0));
            let left = indent + marker_room;
            let width = (bounds.size.width - left).max(px(1.0));
            let align = resolve_alignment(block.paragraph().alignment(), direction.is_rtl());
            let projected = &projection.blocks()[index];
            let diagnostics = editor
                .diagnostic_items()
                .iter()
                .filter_map(|diagnostic| {
                    let flat = projection.range_for_selection(&RichTextSelection::new(
                        diagnostic.range.start.clone(),
                        diagnostic.range.end.clone(),
                    ))?;
                    let start = flat.start.max(projected.start);
                    let end = flat.end.min(projected.end);
                    (start < end).then(|| DiagnosticSlice {
                        range: start - projected.start..end - projected.start,
                        severity: diagnostic.severity,
                    })
                })
                .collect::<Vec<_>>();
            let local_marked = marked_range.as_ref().and_then(|range| {
                let start = range.start.max(projected.start);
                let end = range.end.min(projected.end);
                (start < end).then(|| start - projected.start..end - projected.start)
            });
            let show_placeholder = index == 0 && empty_document;
            let display_text = if show_placeholder {
                editor.placeholder_text().clone()
            } else {
                block.text().clone()
            };
            let runs = if show_placeholder {
                vec![TextRun {
                    len: display_text.len(),
                    font: base.font(),
                    color: theme.colors.text_placeholder,
                    background_color: None,
                    background_radius: None,
                    underline: None,
                    strikethrough: None,
                }]
            } else {
                runs_for_block(
                    block,
                    local_marked.as_ref(),
                    &diagnostics,
                    &base,
                    editor.is_disabled(),
                    &theme,
                )
            };
            let lines = window
                .text_system()
                .shape_text(display_text, font_size, &runs, Some(width), None)
                .map(|lines| lines.into_iter().collect::<Vec<_>>())
                .unwrap_or_default();
            let layout = EditableTextLayout::new(block.text(), lines, line_height);
            let marker = marker_for(index, document.blocks(), cx).and_then(|marker| {
                let marker_run = TextRun {
                    len: marker.len(),
                    font: base.font(),
                    color: if editor.is_disabled() {
                        theme.colors.text_disabled
                    } else {
                        theme.colors.text_faint
                    },
                    background_color: None,
                    background_radius: None,
                    underline: None,
                    strikethrough: None,
                };
                window
                    .text_system()
                    .shape_text(marker, font_size, &[marker_run], None, Some(1))
                    .ok()
                    .and_then(|mut lines| lines.pop())
            });
            let height = layout.height().max(line_height);
            blocks.push(PaintedBlock {
                layout: Some(StoredBlockLayout {
                    id: block.id().clone(),
                    source: block.text().clone(),
                    top,
                    left,
                    width,
                    align,
                    layout,
                }),
                screen_origin: point(bounds.left() + left, bounds.top() + top),
                marker,
                marker_origin: point(bounds.left() + indent, bounds.top() + top),
                marker_width,
            });
            top += height;
            if index + 1 < document.blocks().len() {
                top += if list.is_some() {
                    list_gap
                } else {
                    paragraph_gap
                };
            }
        }

        let content_height = top.max(line_height);
        let (min_rows, max_rows) = editor.row_limits();
        let content_rows =
            ((f32::from(content_height) / f32::from(line_height)).ceil() as usize).max(1);
        let visible_rows = content_rows.clamp(min_rows, max_rows);
        let mut scroll_offset = editor
            .scroll_offset()
            .min((content_height - bounds.size.height).max(px(0.0)))
            .max(px(0.0));
        if selection.is_caret()
            && let Some(block) = blocks.iter().find(|block| {
                block
                    .layout
                    .as_ref()
                    .is_some_and(|layout| layout.id == selection.head.block)
            })
            && let Some(layout) = block.layout.as_ref()
        {
            let caret = layout.layout.position_for_offset_aligned(
                selection.head.offset,
                layout.align,
                layout.width,
            );
            let caret_top = layout.top + caret.y;
            if caret_top < scroll_offset {
                scroll_offset = caret_top;
            }
            if caret_top + line_height > scroll_offset + bounds.size.height {
                scroll_offset = caret_top + line_height - bounds.size.height;
            }
        }

        let mut selections = Vec::new();
        let mut cursor = None;
        for block in &mut blocks {
            let layout = block
                .layout
                .as_ref()
                .expect("layout is present until paint");
            block.screen_origin.y -= scroll_offset;
            block.marker_origin.y -= scroll_offset;
            let projected = projection
                .block(&layout.id)
                .expect("layout came from projected document");
            let start = selection_range.start.max(projected.start);
            let end = selection_range.end.min(projected.end);
            if start < end {
                selections.extend(
                    layout
                        .layout
                        .bounds_for_range(
                            start - projected.start..end - projected.start,
                            block.screen_origin,
                            layout.align,
                            layout.width,
                        )
                        .into_iter()
                        .map(|bounds| fill(bounds, theme.colors.selected)),
                );
            }
            if selection.is_caret() && selection.head.block == layout.id {
                cursor = Some(fill(
                    layout.layout.caret_bounds_aligned(
                        selection.head.offset,
                        block.screen_origin,
                        px(1.5),
                        layout.align,
                        layout.width,
                    ),
                    theme.colors.accent,
                ));
            }
        }

        PrepaintState {
            blocks,
            selections,
            cursor,
            content_height,
            visible_rows,
            scroll_offset,
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
        let (focus, disabled) = {
            let editor = self.editor.read(cx);
            (editor.focus_handle().clone(), editor.is_disabled())
        };
        if !disabled {
            window.handle_input(
                &focus,
                ElementInputHandler::new(bounds, self.editor.clone()),
                cx,
            );
        }

        let line_height = window.line_height();
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for selection in prepaint.selections.drain(..) {
                window.paint_quad(selection);
            }
            for block in &mut prepaint.blocks {
                if let Some(marker) = block.marker.take() {
                    marker
                        .paint(
                            block.marker_origin,
                            line_height,
                            TextAlign::Right,
                            Some(Bounds::new(
                                block.marker_origin,
                                size(block.marker_width, line_height),
                            )),
                            window,
                            cx,
                        )
                        .ok();
                }
                if let Some(layout) = block.layout.as_ref() {
                    for (line, top) in layout.layout.painted_lines() {
                        line.paint(
                            point(block.screen_origin.x, block.screen_origin.y + top),
                            line_height,
                            layout.align,
                            Some(Bounds::new(
                                block.screen_origin,
                                size(layout.width, layout.layout.height()),
                            )),
                            window,
                            cx,
                        )
                        .ok();
                    }
                }
            }
            if !disabled
                && focus.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });

        let layouts = prepaint
            .blocks
            .iter_mut()
            .filter_map(|block| block.layout.take())
            .collect();
        let content_height = prepaint.content_height;
        let visible_rows = prepaint.visible_rows;
        let scroll_offset = prepaint.scroll_offset;
        self.editor.update(cx, |editor, cx| {
            let changed = editor.set_last_layouts(
                layouts,
                bounds,
                content_height,
                visible_rows,
                scroll_offset,
            );
            if changed {
                cx.notify();
            }
        });
    }
}

fn resolve_alignment(alignment: RichTextAlignment, rtl: bool) -> TextAlign {
    match alignment {
        RichTextAlignment::Start if rtl => TextAlign::Right,
        RichTextAlignment::Start => TextAlign::Left,
        RichTextAlignment::Center => TextAlign::Center,
        RichTextAlignment::End if rtl => TextAlign::Left,
        RichTextAlignment::End => TextAlign::Right,
    }
}

fn marker_for(index: usize, blocks: &[RichTextBlock], cx: &App) -> Option<SharedString> {
    let item = blocks[index].paragraph().list()?;
    match item.kind {
        RichTextListKind::Unordered => Some(cx.strings().text(StringKey::RichTextBullet)),
        RichTextListKind::Ordered => {
            let mut ordinal = 1;
            for previous in blocks[..index].iter().rev() {
                match previous.paragraph().list() {
                    Some(candidate)
                        if candidate.kind == RichTextListKind::Ordered
                            && candidate.depth == item.depth =>
                    {
                        ordinal += 1;
                    }
                    Some(candidate) if candidate.depth > item.depth => {}
                    _ => break,
                }
            }
            Some(cx.strings().format(
                StringKey::RichTextOrderedMarker,
                &[cx.numbers().count(ordinal).as_ref()],
            ))
        }
    }
}

fn runs_for_block(
    block: &RichTextBlock,
    marked: Option<&Range<usize>>,
    diagnostics: &[DiagnosticSlice],
    base: &gpui::TextStyle,
    disabled: bool,
    theme: &gpui_kit_theme::Theme,
) -> Vec<TextRun> {
    if block.text().is_empty() {
        return vec![TextRun {
            len: 0,
            font: base.font(),
            color: base.color,
            background_color: None,
            background_radius: None,
            underline: None,
            strikethrough: None,
        }];
    }
    let mut boundaries = vec![0, block.text().len()];
    let mut end = 0;
    for run in block.styles().runs() {
        end += run.len;
        boundaries.push(end);
    }
    if let Some(marked) = marked {
        boundaries.extend([marked.start, marked.end]);
    }
    for diagnostic in diagnostics {
        boundaries.extend([diagnostic.range.start, diagnostic.range.end]);
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
            let style = block.styles().style_at(range.start);
            let mut run_font = if style.format(RichTextFormat::Code) {
                font(theme.typography.mono.clone())
            } else {
                base.font()
            };
            run_font.fallbacks = Some(gpui_kit_assets::text_fallbacks());
            if style.format(RichTextFormat::Bold) {
                run_font.weight = FontWeight::BOLD;
            }
            if style.format(RichTextFormat::Italic) {
                run_font.style = FontStyle::Italic;
            }
            let diagnostic = diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.range.start < range.end && diagnostic.range.end > range.start
                })
                .max_by_key(|diagnostic| diagnostic.severity);
            let diagnostic_color = diagnostic.map(|diagnostic| match diagnostic.severity {
                RichTextDiagnosticSeverity::Info => theme.colors.info,
                RichTextDiagnosticSeverity::Warning => theme.colors.warning,
                RichTextDiagnosticSeverity::Error => theme.colors.danger,
            });
            let link = style.link().is_some();
            let underlined = style.format(RichTextFormat::Underline) || link;
            let composing =
                marked.is_some_and(|marked| marked.start < range.end && marked.end > range.start);
            Some(TextRun {
                len: range.len(),
                font: run_font,
                color: if disabled {
                    theme.colors.text_disabled
                } else if style.format(RichTextFormat::Code) {
                    theme.colors.syntax.inline
                } else if link {
                    theme.colors.accent
                } else {
                    base.color
                },
                background_color: style
                    .format(RichTextFormat::Code)
                    .then_some(theme.colors.syntax.inline_wash),
                background_radius: style
                    .format(RichTextFormat::Code)
                    .then_some(px(theme.radius(Radius::Small))),
                underline: diagnostic_color
                    .map(|color| UnderlineStyle {
                        color: Some(color),
                        thickness: px(1.0),
                        wavy: true,
                    })
                    .or_else(|| {
                        (composing || underlined).then_some(UnderlineStyle {
                            color: Some(if link && !disabled {
                                theme.colors.accent
                            } else {
                                base.color
                            }),
                            thickness: px(1.0),
                            wavy: false,
                        })
                    }),
                strikethrough: style
                    .format(RichTextFormat::Strike)
                    .then_some(StrikethroughStyle {
                        thickness: px(1.0),
                        color: Some(base.color),
                    }),
            })
        })
        .collect()
}
