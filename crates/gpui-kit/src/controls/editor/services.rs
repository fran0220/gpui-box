//! Revision-paired language requests; transport and workspace policy stay with
//! the caller. Responses are data, never executable callbacks.

use std::ops::Range;

use gpui::{AnyElement, Context, EditSnapshot, SharedString, div, point, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ActiveTheme;

use super::{Editor, EditorEvent};
use crate::{
    overlay::popover,
    state::{AsyncStatus, AsyncValue},
    strings::{ActiveStrings, StringKey},
};

/// A caller-owned language operation requested by the editing surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorServiceKind {
    Completion,
    Hover,
    Definition,
    CodeActions,
}

/// Immutable request identity. Replies must match this id and text revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorServiceRequest {
    pub id: u64,
    pub revision: u64,
    pub kind: EditorServiceKind,
    pub position: usize,
    pub selection: Range<usize>,
    pub document: EditSnapshot,
}

/// Current-document replacement in the request's original UTF-8 coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorReplacement {
    pub range: Range<usize>,
    pub text: SharedString,
}

/// Stable caller identity, display label, and the effect of accepting a result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorServiceItem {
    pub id: SharedString,
    pub label: SharedString,
    pub detail: Option<SharedString>,
    pub effect: EditorServiceEffect,
}

/// Effects are either local atomic edits or caller-owned navigation/actions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorServiceEffect {
    Edits(Vec<EditorReplacement>),
    Definition {
        target: SharedString,
        range: Range<usize>,
    },
    Action(SharedString),
}

/// Hover content is plain caller text, not interpreted HTML or executable code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorHover {
    pub range: Range<usize>,
    pub contents: SharedString,
}

/// Service result paired with a request by `Editor::set_service_result`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorServiceResult {
    Items(Vec<EditorServiceItem>),
    Hover(EditorHover),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorDiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// Diagnostics are caller claims about one revision, not parser error nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorDiagnostic {
    pub id: SharedString,
    pub range: Range<usize>,
    pub message: SharedString,
    pub severity: EditorDiagnosticSeverity,
}

/// Caller-classified semantic token using the shared theme's code palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorSemanticToken {
    pub range: Range<usize>,
    pub class: gpui_kit_theme::SyntaxColor,
}

pub(super) struct ServicePopup {
    pub request: EditorServiceRequest,
    pub value: AsyncValue<EditorServiceResult, SharedString>,
    pub selected: usize,
    pub scroll: gpui::ScrollHandle,
}

impl Editor {
    /// Enables caller language requests and their keyboard/mouse bindings.
    pub fn language_services(mut self, enabled: bool) -> Self {
        self.services_enabled = enabled;
        self
    }

    /// Requests a language operation at a UTF-8 byte position. No service
    /// process, file, or network operation is started by Kit.
    pub fn request_service(
        &mut self,
        kind: EditorServiceKind,
        position: usize,
        cx: &mut Context<Self>,
    ) -> Option<EditorServiceRequest> {
        if !self.services_enabled || self.disabled {
            return None;
        }
        let area = self.area.read(cx);
        let document = area.document();
        if !valid_range(&document, &(position..position)) {
            return None;
        }
        self.next_service_request = self.next_service_request.wrapping_add(1);
        let request = EditorServiceRequest {
            id: self.next_service_request,
            revision: area.revision(),
            kind,
            position,
            selection: area.selected_range(),
            document,
        };
        let previous = self.service_popup.take().filter(|popup| {
            popup.request.kind == kind && popup.request.revision == request.revision
        });
        let mut value = AsyncValue::loading();
        if let Some(previous) = previous {
            value.value = previous.value.value;
            if value.value.is_some() {
                value.status = AsyncStatus::Refreshing;
            }
        }
        self.service_popup = Some(ServicePopup {
            request: request.clone(),
            value,
            selected: 0,
            scroll: gpui::ScrollHandle::new(),
        });
        cx.emit(EditorEvent::ServiceRequested(request.clone()));
        cx.notify();
        Some(request)
    }

    /// Publishes a reply only while its request and document revision remain
    /// current. Refresh errors retain the last verified result for inspection.
    pub fn set_service_result(
        &mut self,
        request: u64,
        mut result: AsyncValue<EditorServiceResult, SharedString>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(popup) = self.service_popup.as_mut() else {
            return false;
        };
        if popup.request.id != request || popup.request.revision != self.area.read(cx).revision() {
            return false;
        }
        if let Some(value) = &result.value {
            let valid = match value {
                EditorServiceResult::Hover(hover) => {
                    popup.request.kind == EditorServiceKind::Hover
                        && valid_range(&popup.request.document, &hover.range)
                }
                EditorServiceResult::Items(items) => {
                    let mut ids = std::collections::HashSet::new();
                    popup.request.kind != EditorServiceKind::Hover
                        && items.iter().all(|item| {
                            !item.id.is_empty()
                                && ids.insert(item.id.clone())
                                && match (&item.effect, popup.request.kind) {
                                    (
                                        EditorServiceEffect::Edits(edits),
                                        EditorServiceKind::Completion
                                        | EditorServiceKind::CodeActions,
                                    ) => {
                                        let mut ranges: Vec<_> =
                                            edits.iter().map(|edit| edit.range.clone()).collect();
                                        ranges.sort_by_key(|range| (range.start, range.end));
                                        !ranges.is_empty()
                                            && ranges.iter().all(|range| {
                                                let document = &popup.request.document;
                                                valid_range(&popup.request.document, range)
                                                    && [range.start, range.end].into_iter().all(
                                                        |offset| {
                                                            offset == 0
                                                                || document.next_grapheme_boundary(
                                                                    document
                                                                        .previous_grapheme_boundary(
                                                                            offset,
                                                                        ),
                                                                ) == offset
                                                        },
                                                    )
                                            })
                                            && ranges.windows(2).all(|pair| {
                                                pair[0].end <= pair[1].start && pair[0] != pair[1]
                                            })
                                    }
                                    (
                                        EditorServiceEffect::Definition { target, range },
                                        EditorServiceKind::Definition,
                                    ) => !target.is_empty() && range.start <= range.end,
                                    (
                                        EditorServiceEffect::Action(_),
                                        EditorServiceKind::CodeActions,
                                    ) => true,
                                    _ => false,
                                }
                        })
                }
            };
            if !valid {
                return false;
            }
        }
        if matches!(result.status, AsyncStatus::Error(_)) && result.value.is_none() {
            result.value = popup.value.value.clone();
        }
        if matches!(&result.value, Some(EditorServiceResult::Items(items)) if items.is_empty())
            && matches!(result.status, AsyncStatus::Ready)
        {
            result = AsyncValue::empty();
        }
        popup.value = result;
        popup.selected = 0;
        cx.notify();
        true
    }

    /// Dismisses the current popup and invalidates its outstanding reply.
    pub fn dismiss_service(&mut self, cx: &mut Context<Self>) {
        self.service_popup = None;
        cx.notify();
    }

    /// Accepts a stable result identity. Edits are one undo transaction;
    /// navigation and external actions are emitted for the caller to handle.
    pub fn accept_service_item(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        if self.disabled {
            return false;
        }
        let Some(popup) = self.service_popup.as_ref() else {
            return false;
        };
        if popup.request.revision != self.area.read(cx).revision()
            || !matches!(popup.value.status, AsyncStatus::Ready)
        {
            return false;
        }
        let Some(EditorServiceResult::Items(items)) = &popup.value.value else {
            return false;
        };
        let Some(item) = items.iter().find(|item| item.id.as_ref() == id).cloned() else {
            return false;
        };
        let revision = popup.request.revision;
        match &item.effect {
            EditorServiceEffect::Edits(edits) => {
                if self.read_only
                    || !self.apply_edits(
                        revision,
                        edits
                            .iter()
                            .map(|edit| (edit.range.clone(), edit.text.clone())),
                        cx,
                    )
                {
                    return false;
                }
            }
            EditorServiceEffect::Definition { target, range } => {
                cx.emit(EditorEvent::DefinitionRequested {
                    target: target.clone(),
                    range: range.clone(),
                })
            }
            EditorServiceEffect::Action(action) => {
                cx.emit(EditorEvent::CodeActionRequested(action.clone()))
            }
        }
        self.service_popup = None;
        cx.emit(EditorEvent::ServiceAccepted(item.id));
        cx.notify();
        true
    }

    /// Publishes revision-paired diagnostic ranges and messages. Stale or
    /// malformed batches are refused without dropping the previous batch.
    pub fn set_diagnostics(
        &mut self,
        revision: u64,
        diagnostics: Vec<EditorDiagnostic>,
        cx: &mut Context<Self>,
    ) -> bool {
        let area = self.area.read(cx);
        let document = area.document();
        let mut ids = std::collections::HashSet::new();
        if revision != area.revision()
            || diagnostics.iter().any(|diagnostic| {
                diagnostic.id.is_empty()
                    || !ids.insert(diagnostic.id.clone())
                    || !valid_range(&document, &diagnostic.range)
            })
        {
            return false;
        }
        self.diagnostics = Some((revision, diagnostics));
        cx.notify();
        true
    }

    /// Semantic tokens override parser colors only for their current revision.
    pub fn set_semantic_tokens(
        &mut self,
        revision: u64,
        mut tokens: Vec<EditorSemanticToken>,
        cx: &mut Context<Self>,
    ) -> bool {
        let area = self.area.read(cx);
        let document = area.document();
        tokens.sort_by_key(|token| token.range.start);
        if revision != area.revision()
            || tokens
                .iter()
                .any(|token| !valid_range(&document, &token.range))
            || tokens
                .windows(2)
                .any(|pair| pair[0].range.end > pair[1].range.start)
        {
            return false;
        }
        self.semantic_tokens = Some((revision, tokens));
        cx.notify();
        true
    }

    pub(super) fn diagnostic_spans(
        &self,
        base: (u64, Vec<(Range<usize>, gpui::HighlightStyle)>),
        cx: &Context<Self>,
    ) -> (u64, Vec<(Range<usize>, gpui::HighlightStyle)>) {
        let area = self.area.read(cx);
        let revision = area.revision();
        let theme = cx.theme();
        let line_height = px(theme
            .type_style(gpui_kit_theme::TypeScale::Code)
            .line_height);
        let ranges = area.visible_source_ranges(line_height, line_height * self.rows as f32);
        let spans = ranges
            .into_iter()
            .flat_map(|range| self.diagnostic_spans_in(base.clone(), range, cx).1)
            .collect();
        (revision, spans)
    }

    fn diagnostic_spans_in(
        &self,
        mut base: (u64, Vec<(Range<usize>, gpui::HighlightStyle)>),
        visible: Range<usize>,
        cx: &Context<Self>,
    ) -> (u64, Vec<(Range<usize>, gpui::HighlightStyle)>) {
        let revision = self.area.read(cx).revision();
        let theme = cx.theme();
        base.1
            .retain(|(range, _)| range.start < visible.end && visible.start < range.end);
        if base.0 != revision {
            base = (revision, Vec::new());
        }
        if let Some((version, tokens)) = &self.semantic_tokens
            && *version == revision
        {
            base.1.extend(
                tokens
                    .iter()
                    .skip(tokens.partition_point(|token| token.range.end <= visible.start))
                    .take_while(|token| token.range.start < visible.end)
                    .map(|token| {
                        (
                            token.range.clone(),
                            gpui::HighlightStyle {
                                color: Some(theme.colors.syntax.get(token.class)),
                                ..Default::default()
                            },
                        )
                    }),
            );
        }
        let diagnostics: Vec<_> = self
            .diagnostics
            .as_ref()
            .filter(|(version, _)| *version == revision)
            .iter()
            .flat_map(|(_, diagnostics)| diagnostics.iter())
            .filter(|diagnostic| {
                diagnostic.range.start < visible.end && visible.start < diagnostic.range.end
            })
            .collect();
        let mut edges: Vec<_> = base
            .1
            .iter()
            .map(|(range, _)| range)
            .chain(diagnostics.iter().map(|diagnostic| &diagnostic.range))
            .flat_map(|range| [range.start.max(visible.start), range.end.min(visible.end)])
            .collect();
        edges.sort_unstable();
        edges.dedup();
        let spans = edges
            .windows(2)
            .filter_map(|edge| {
                let range = edge[0]..edge[1];
                let base_style = base
                    .1
                    .iter()
                    .rev()
                    .find(|(span, _)| span.start <= range.start && range.end <= span.end)
                    .map(|(_, style)| *style);
                let diagnostic = diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.range.start <= range.start && range.end <= diagnostic.range.end
                    })
                    .min_by_key(|diagnostic| match diagnostic.severity {
                        EditorDiagnosticSeverity::Error => 0,
                        EditorDiagnosticSeverity::Warning => 1,
                        EditorDiagnosticSeverity::Information => 2,
                        EditorDiagnosticSeverity::Hint => 3,
                    });
                if base_style.is_none() && diagnostic.is_none() {
                    return None;
                }
                let mut style = base_style.unwrap_or_default();
                if let Some(diagnostic) = diagnostic {
                    style.underline = Some(gpui::UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(match diagnostic.severity {
                            EditorDiagnosticSeverity::Error => theme.colors.danger,
                            EditorDiagnosticSeverity::Warning => theme.colors.warning,
                            _ => theme.colors.accent,
                        }),
                        wavy: true,
                    });
                }
                Some((range, style))
            })
            .collect();
        (revision, spans)
    }

    pub(super) fn move_service_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(popup) = self.service_popup.as_mut()
            && let Some(EditorServiceResult::Items(items)) = &popup.value.value
            && !items.is_empty()
        {
            popup.selected =
                (popup.selected as isize + delta).rem_euclid(items.len() as isize) as usize;
            popup.scroll.scroll_to_item(popup.selected);
            cx.notify();
        }
    }

    pub(super) fn accept_selected_service(&mut self, cx: &mut Context<Self>) {
        let id = self
            .service_popup
            .as_ref()
            .and_then(|popup| match &popup.value.value {
                Some(EditorServiceResult::Items(items)) => {
                    items.get(popup.selected).map(|item| item.id.clone())
                }
                _ => None,
            });
        if let Some(id) = id {
            self.accept_service_item(&id, cx);
        }
    }

    pub(super) fn service_surface(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let popup = self.service_popup.as_ref()?;
        let area = self.area.read(cx);
        if popup.request.revision != area.revision() {
            return None;
        }
        let bounds = area.bounds_for_position(popup.request.position)?;
        let theme = cx.theme().clone();
        let viewport = area.viewport_bounds()?;
        let width = px(theme.measures.compact_overlay_width).min(viewport.size.width);
        let left = bounds
            .left()
            .min(viewport.right() - width)
            .max(viewport.left());
        let ident = self.ident.child("service");
        let mut children = Vec::new();
        if popup.request.kind == EditorServiceKind::Hover
            && let Some((revision, diagnostics)) = &self.diagnostics
            && *revision == popup.request.revision
        {
            for diagnostic in diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.range.contains(&popup.request.position))
            {
                children.push(
                    div()
                        .child(diagnostic.message.clone())
                        .semantic_in(
                            cx,
                            NodeSpec::new(
                                ident
                                    .child("diagnostic")
                                    .child(&diagnostic.id)
                                    .semantic_id(),
                                Role::Status,
                            )
                            .text(diagnostic.message.clone()),
                        )
                        .into_any_element(),
                );
            }
        }
        if let Some(result) = &popup.value.value {
            match result {
                EditorServiceResult::Hover(hover) => children.push(
                    div()
                        .child(hover.contents.clone())
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("hover").semantic_id(), Role::Tooltip)
                                .text(hover.contents.clone()),
                        )
                        .into_any_element(),
                ),
                EditorServiceResult::Items(items) => {
                    for (index, item) in items.iter().enumerate() {
                        let item_id = item.id.clone();
                        let row_id = ident.child(&item.id);
                        let enabled = matches!(popup.value.status, AsyncStatus::Ready)
                            && !(self.read_only
                                && matches!(item.effect, EditorServiceEffect::Edits(_)));
                        use gpui::prelude::*;
                        children.push(
                            div()
                                .id(row_id.element_id())
                                .p(px(theme.spacing.xs))
                                .when(index == popup.selected, |row| row.bg(theme.colors.sunken))
                                .child(item.label.clone())
                                .children(item.detail.clone())
                                .when(enabled, |row| {
                                    row.on_click(cx.listener(move |editor, _, _, cx| {
                                        editor.accept_service_item(&item_id, cx);
                                    }))
                                })
                                .semantic_in(
                                    cx,
                                    NodeSpec::new(row_id.semantic_id(), Role::MenuItem)
                                        .text(item.label.clone())
                                        .disabled(!enabled),
                                )
                                .into_any_element(),
                        );
                    }
                }
            }
        }
        let status = match &popup.value.status {
            AsyncStatus::Idle => Some(cx.strings().text(StringKey::StateViewIdle)),
            AsyncStatus::Loading => Some(cx.strings().text(StringKey::Loading)),
            AsyncStatus::Refreshing => Some(cx.strings().text(StringKey::StateViewRefreshing)),
            AsyncStatus::Empty => Some(cx.strings().text(StringKey::StateViewEmpty)),
            AsyncStatus::Unavailable(reason) => Some(
                format!(
                    "{}: {reason}",
                    cx.strings().text(StringKey::StateViewUnavailable)
                )
                .into(),
            ),
            AsyncStatus::Error(reason) => Some(reason.clone()),
            AsyncStatus::Ready => None,
        };
        if let Some(status) = status {
            children.push(
                div()
                    .child(status.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("status").semantic_id(), Role::Status)
                            .text(status),
                    )
                    .into_any_element(),
            );
        }
        use gpui::prelude::*;
        let body = div()
            .id(ident.child("body").element_id())
            .max_h(px(theme.measures.compact_menu_max_height))
            .overflow_y_scroll()
            .track_scroll(&popup.scroll)
            .children(children);
        let content = popover::card_flush(ident.clone(), &theme)
            .p(px(theme.spacing.sm))
            .w(width)
            .child(popover::menu_body(&ident, &popup.scroll, body))
            .into_any_element();
        Some(popover::at(
            ident.child("anchor").element_id(),
            &theme,
            point(left, bounds.bottom()),
            content,
        ))
    }
}

fn valid_range(document: &EditSnapshot, range: &Range<usize>) -> bool {
    range.start <= range.end
        && range.end <= document.len()
        && [range.start, range.end].into_iter().all(|offset| {
            offset == document.len()
                || document
                    .byte_chunks(offset..offset + 1)
                    .next()
                    .is_some_and(|bytes| bytes[0] & 0xc0 != 0x80)
        })
}
