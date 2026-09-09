//! Exact paragraph wrapping retained across edits and viewport changes.

use super::*;

/// Retains exact hard-paragraph layouts for one editable surface. Initial
/// layout and width/font changes shape every paragraph; later edits shape only
/// changed paragraphs or changed style partitions. Static/scroll updates reuse
/// the complete row index. A text edit still rebuilds line-index metadata, and
/// a single exceptionally long paragraph is still one shaping input.
#[derive(Default)]
pub struct EditableWrappedCache {
    state: Option<State>,
    indexed_lines: usize,
}

struct State {
    document: EditSnapshot,
    text_system: Arc<WindowTextSystem>,
    font_size: Pixels,
    width: Pixels,
    runs: Vec<TextRun>,
    run_starts: Vec<usize>,
    layout: EditableTextLayout,
}

fn run_starts(runs: &[TextRun]) -> Vec<usize> {
    let mut offset = 0;
    runs.iter()
        .map(|run| {
            let start = offset;
            offset += run.len;
            start
        })
        .collect()
}

fn styles<'a>(
    runs: &'a [TextRun],
    starts: &'a [usize],
    range: Range<usize>,
) -> impl Iterator<Item = TextRun> + 'a {
    let first = starts
        .partition_point(|start| *start <= range.start)
        .saturating_sub(1);
    runs.iter()
        .zip(starts)
        .skip(first)
        .take_while(move |(_, start)| **start < range.end)
        .filter_map(move |(run, start)| {
            let len = (start + run.len)
                .min(range.end)
                .saturating_sub((*start).max(range.start));
            (len > 0).then(|| TextRun { len, ..run.clone() })
        })
}

impl EditableWrappedCache {
    /// Number of hard-line metadata entries built by the most recent update.
    /// Separate from `EditableTextLayout::shaping_work`: an edit may reuse
    /// almost every shaped paragraph while rebuilding the exact row index.
    pub fn indexed_lines(&self) -> usize {
        self.indexed_lines
    }

    /// Drops retained layouts, for example after the host changes available
    /// font faces without changing the requested font/style identities.
    pub fn clear(&mut self) {
        self.state = None;
        self.indexed_lines = 0;
    }

    /// Updates exact wrapping with the same text system and byte-style
    /// partition used to paint. Viewport restriction is applied afterward via
    /// `EditableTextLayout::set_painted_rows`, including after caret reveal.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        document: EditSnapshot,
        text_system: Arc<WindowTextSystem>,
        font_size: Pixels,
        line_height: Pixels,
        runs: Vec<TextRun>,
        width: Pixels,
    ) -> EditableTextLayout {
        self.indexed_lines = 0;
        let compatible = self.state.as_ref().filter(|old| {
            old.font_size == font_size
                && old.width == width
                && Arc::ptr_eq(&old.text_system, &text_system)
        });
        if let Some(old) = compatible
            && document.shares_storage_with(&old.document)
            && old.runs == runs
        {
            let mut layout = old.layout.clone();
            layout.line_height = line_height;
            layout.work = Some(EditableTextWork::default());
            return layout;
        }
        let same_document =
            compatible.is_some_and(|old| document.shares_storage_with(&old.document));
        let changed = compatible.filter(|_| !same_document).map(|old| {
            let edit = document.difference_from(&old.document);
            (
                old.document.line_at(edit.replaced.start),
                old.document.line_at(edit.replaced.end) + 1,
                document.line_at(edit.replaced.start + edit.inserted.len()) + 1,
            )
        });
        let starts = run_starts(&runs);
        let mut lines = Vec::with_capacity(document.line_count());
        let mut source_starts = Vec::with_capacity(document.line_count());
        let mut rows = Vec::with_capacity(document.line_count());
        let mut total_rows = 0;
        let mut work = EditableTextWork::default();
        for line in 0..document.line_count() {
            let mut range = document.line_range(line).expect("indexed paragraph");
            source_starts.push(range.start);
            if line + 1 < document.line_count() {
                range.end -= 1; // LF belongs to the logical row, not the shaper.
            }
            let previous = compatible.and_then(|old| {
                let index = if same_document {
                    Some(line)
                } else {
                    changed.and_then(|(prefix, old_suffix, new_suffix)| {
                        if line < prefix {
                            Some(line)
                        } else if line >= new_suffix {
                            Some(old_suffix + line - new_suffix)
                        } else {
                            None
                        }
                    })
                }?;
                let previous = old.layout.lines.get(index)?;
                let old_start = old.layout.starts[index];
                styles(&runs, &starts, range.clone())
                    .eq(styles(
                        &old.runs,
                        &old.run_starts,
                        old_start..old_start + previous.len(),
                    ))
                    .then(|| previous.clone())
            });
            let shaped = previous.unwrap_or_else(|| {
                let text = document.slice(range.clone()).expect("paragraph bytes");
                let paragraph_runs: Vec<_> = styles(&runs, &starts, range).collect();
                work.shaped_lines += 1;
                work.shaped_bytes += text.len();
                Arc::new(
                    text_system
                        .shape_text(text.into(), font_size, &paragraph_runs, Some(width), None)
                        .ok()
                        .and_then(|mut lines| lines.pop())
                        .unwrap_or_default(),
                )
            });
            rows.push(total_rows);
            total_rows += shaped.wrap_boundaries().len() + 1;
            lines.push(shaped);
        }
        self.indexed_lines = lines.len();
        let measured_width = lines
            .iter()
            .map(|line| line.unwrapped_layout.width)
            .fold(px(0.0), Pixels::max);
        let layout = EditableTextLayout {
            lines: lines.into(),
            starts: source_starts.into(),
            rows: rows.into(),
            total_rows,
            text_len: document.len(),
            line_height,
            source: None,
            paint_rows: None,
            work: Some(work),
            width: measured_width,
        };
        self.state = Some(State {
            document,
            text_system,
            font_size,
            width,
            runs,
            run_starts: starts,
            layout: layout.clone(),
        });
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestAppContext, TextStyle};

    #[crate::test]
    fn wrapped_large_file_reuses_rows_and_shapes_only_the_edited_paragraph(
        cx: &mut TestAppContext,
    ) {
        let text_system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        for count in [1000, 10000] {
            let paragraph = "asymmetric 界 words and more words\n";
            let mut document = EditSnapshot::new(&format!("{}tail אב", paragraph.repeat(count)));
            let mut cache = EditableWrappedCache::default();
            let update = |cache: &mut EditableWrappedCache, document: &EditSnapshot| {
                cache.update(
                    document.clone(),
                    text_system.clone(),
                    px(14.0),
                    px(20.0),
                    vec![TextStyle::default().to_run(document.len())],
                    px(100.0),
                )
            };
            let initial = update(&mut cache, &document);
            assert_eq!(initial.shaping_work().shaped_lines, count + 1);
            assert_eq!(document.materialized_bytes(), 0);
            let mut steady = update(&mut cache, &document);
            assert_eq!(steady.shaping_work(), EditableTextWork::default());
            assert_eq!(cache.indexed_lines(), 0);
            assert!(steady.shares_row_index_with(&initial));
            steady.set_painted_rows(21..28);
            assert_eq!(steady.painted_source_ranges().len(), 7);
            assert!(steady.painted_lines().count() <= 7);
            let cells = steady.painted_bounds_for_range(
                0..document.len(),
                point(px(0.0), px(0.0)),
                TextAlign::Left,
                px(100.0),
            );
            assert!(!cells.is_empty());
            assert!(
                cells
                    .iter()
                    .all(|cell| cell.top() >= px(420.0) && cell.bottom() <= px(560.0))
            );
            let start = document
                .line_range(count / 3)
                .expect("middle paragraph")
                .start;
            document.replace(start..start + 1, "XYZ");
            let edited = update(&mut cache, &document);
            assert_eq!(
                edited.shaping_work(),
                EditableTextWork {
                    shaped_lines: 1,
                    shaped_bytes: paragraph.len() - 1 + 2,
                }
            );
            assert_eq!(cache.indexed_lines(), count + 1);
            assert_eq!(document.materialized_bytes(), 0);
        }
    }

    #[crate::test]
    fn incremental_wrap_matches_fresh_unicode_geometry_after_splits_merges_and_style_changes(
        cx: &mut TestAppContext,
    ) {
        let text_system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        let mut document =
            EditSnapshot::new("alpha 界 words\nאב e\u{301} more words\nlast 😀 tail\n");
        let mut cache = EditableWrappedCache::default();
        for (replacement, insertion, width, size) in [
            (0..0, "", 77.0, 14.0),
            (1..2, "long\nnew ", 77.0, 14.0),
            (0..9, "join ", 77.0, 14.0),
            (0..0, "", 131.0, 14.0),
            (0..0, "", 131.0, 18.0),
        ] {
            if !insertion.is_empty() || !replacement.is_empty() {
                document.replace(replacement, insertion);
            }
            let runs = vec![TextStyle::default().to_run(document.len())];
            let cached = cache.update(
                document.clone(),
                text_system.clone(),
                px(size),
                px(22.0),
                runs.clone(),
                px(width),
            );
            let fresh = EditableTextLayout::new(
                document.text(),
                text_system
                    .shape_text(
                        document.text().clone(),
                        px(size),
                        &runs,
                        Some(px(width)),
                        None,
                    )
                    .expect("fresh shape")
                    .into_vec(),
                px(22.0),
            );
            assert_eq!(
                cached.visual_rows(document.text()),
                fresh.visual_rows(document.text())
            );
            assert_eq!(cached.height(), fresh.height());
            for (offset, _) in document.text().char_indices() {
                assert_eq!(
                    cached.position_for_offset(offset),
                    fresh.position_for_offset(offset),
                    "offset {offset}"
                );
            }
        }
        let first = document.line_range(0).expect("first line").end;
        let mut bold = TextStyle::default().to_run(document.len() - first);
        bold.font.weight = crate::FontWeight::BOLD;
        bold.color = crate::red();
        let runs = vec![TextStyle::default().to_run(first), bold];
        let changed = cache.update(
            document.clone(),
            text_system.clone(),
            px(18.0),
            px(22.0),
            runs.clone(),
            px(131.0),
        );
        assert!(changed.shaping_work().shaped_lines > 0);
        let fresh = EditableTextLayout::new(
            document.text(),
            text_system
                .shape_text(
                    document.text().clone(),
                    px(18.0),
                    &runs,
                    Some(px(131.0)),
                    None,
                )
                .expect("styled shape")
                .into_vec(),
            px(22.0),
        );
        assert_eq!(
            changed.visual_rows(document.text()),
            fresh.visual_rows(document.text())
        );
        for (left, right) in changed
            .lines
            .iter()
            .zip(fresh.lines.iter())
            .filter(|(line, _)| line.len() > 0)
        {
            assert!(
                left.decoration_runs
                    .iter()
                    .map(|run| run.color)
                    .eq(right.decoration_runs.iter().map(|run| run.color))
            );
            assert!(
                left.unwrapped_layout
                    .runs
                    .iter()
                    .map(|run| run.font_id)
                    .eq(right.unwrapped_layout.runs.iter().map(|run| run.font_id))
            );
        }
    }
}
