//! Caller-owned fold identities and revision-tagged hard-line ranges.

use super::*;
use gpui::prelude::*;

/// A fold header and its body, in zero-based, half-open source line indices.
/// The first line remains visible. Identities must be unique and stable across
/// caller updates. Nested folds are supported; crossing ranges are refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorFold {
    pub id: SharedString,
    pub lines: Range<usize>,
}

impl Editor {
    /// Installs folds for the current revision atomically. Invalid/stale sets
    /// leave the previous set unchanged. Editing invalidates all fold ranges;
    /// callers can publish a fresh set from their parser's new revision.
    pub fn set_folds(
        &mut self,
        revision: u64,
        mut folds: Vec<EditorFold>,
        cx: &mut Context<Self>,
    ) -> bool {
        let area = self.area.read(cx);
        if revision != area.revision() || !valid_folds(&mut folds, area.document().line_count()) {
            return false;
        }
        self.collapsed_folds
            .retain(|id| folds.iter().any(|fold| fold.id == *id));
        self.folds = folds;
        self.apply_folds(cx);
        true
    }

    /// Changes one fold's transient visual state without changing text,
    /// selection or history. Disabled editors refuse interaction. Read-only
    /// editors still permit browsing. Navigation into hidden text expands it.
    pub fn set_fold_collapsed(
        &mut self,
        id: &str,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.disabled {
            return false;
        }
        let Some(fold) = self.folds.iter().find(|fold| fold.id.as_ref() == id) else {
            return false;
        };
        let id = fold.id.clone();
        let changed = if collapsed {
            self.collapsed_folds.insert(id.clone())
        } else {
            self.collapsed_folds.remove(&id)
        };
        if changed {
            self.apply_folds(cx);
            cx.emit(EditorEvent::FoldChanged { id, collapsed });
        }
        true
    }

    /// Whether a caller-identified fold is currently collapsed.
    pub fn is_fold_collapsed(&self, id: &str) -> bool {
        self.collapsed_folds.iter().any(|fold| fold.as_ref() == id)
    }

    fn apply_folds(&mut self, cx: &mut Context<Self>) {
        let omitted: Vec<_> = self
            .folds
            .iter()
            .filter(|fold| self.collapsed_folds.contains(&fold.id))
            .map(|fold| fold.lines.start + 1..fold.lines.end)
            .collect();
        self.area.update(cx, |area, cx| {
            let projection = (!omitted.is_empty()).then(|| {
                gpui::EditableLineProjection::new(area.document().line_count(), omitted)
                    .expect("validated fold ranges")
            });
            area.set_line_projection(projection, cx);
        });
        cx.notify();
    }

    pub(super) fn expand_caret_fold(&mut self, cx: &mut Context<Self>) {
        let area = self.area.read(cx);
        let line = area.document().line_at(area.cursor_offset());
        let ids: Vec<_> = self
            .folds
            .iter()
            .filter(|fold| {
                fold.lines.start < line
                    && line < fold.lines.end
                    && self.collapsed_folds.contains(&fold.id)
            })
            .map(|fold| fold.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        for id in ids {
            self.collapsed_folds.remove(&id);
            cx.emit(EditorEvent::FoldChanged {
                id,
                collapsed: false,
            });
        }
        self.apply_folds(cx);
    }

    pub(super) fn fold_toggle(
        &self,
        line: usize,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let fold = self.folds.iter().find(|fold| fold.lines.start == line)?;
        let id = fold.id.clone();
        let collapsed = self.collapsed_folds.contains(&id);
        let ident = self.ident.child("fold").child(&id);
        Some(
            div()
                .id(ident.element_id())
                .child(if collapsed { "+" } else { "−" })
                .when(!self.disabled, |element| {
                    element.on_click(cx.listener(move |editor, _, _, cx| {
                        editor.set_fold_collapsed(&id, !collapsed, cx);
                    }))
                })
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Button)
                        .text(fold.id.clone())
                        .expanded(!collapsed)
                        .disabled(self.disabled),
                )
                .into_any_element(),
        )
    }
}

fn valid_folds(folds: &mut [EditorFold], lines: usize) -> bool {
    folds.sort_by_key(|fold| (fold.lines.start, std::cmp::Reverse(fold.lines.end)));
    let mut ids = std::collections::HashSet::new();
    let mut ends = Vec::new();
    for fold in folds {
        if fold.id.is_empty()
            || !ids.insert(fold.id.clone())
            || fold.lines.start.saturating_add(1) >= fold.lines.end
            || fold.lines.end > lines
        {
            return false;
        }
        while ends.last().is_some_and(|end| *end <= fold.lines.start) {
            ends.pop();
        }
        if ends.last().is_some_and(|end| *end < fold.lines.end) {
            return false;
        }
        ends.push(fold.lines.end);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_ranges_allow_nesting_but_refuse_crossing_stale_geometry_and_duplicate_ids() {
        let fold = |id: &'static str, lines| EditorFold {
            id: id.into(),
            lines,
        };
        assert!(valid_folds(
            &mut [
                fold("inner", 3..7),
                fold("outer", 1..9),
                fold("tail", 9..12)
            ],
            12
        ));
        assert!(!valid_folds(&mut [fold("a", 1..7), fold("b", 6..9)], 12));
        assert!(!valid_folds(&mut [fold("a", 1..7), fold("a", 8..10)], 12));
        assert!(!valid_folds(&mut [fold("a", 1..13)], 12));
        assert!(!valid_folds(&mut [fold("a", 5..6)], 12));
    }
}
