use crate::{A11ySubtreeBuilder, Bounds, EditSnapshot, Pixels, SharedString, accesskit};
use std::{ops::Range, sync::Arc};
use unicode_bidi::{BidiInfo, Level};
use unicode_segmentation::UnicodeSegmentation;

const MAX_ACCESSIBLE_RUN_CHARS: usize = 255;

#[derive(Debug, Clone, PartialEq)]
struct AccessibleRun {
    value: Range<usize>,
    start_byte: usize,
    start_character: usize,
    character_lengths: Arc<[u8]>,
    word_starts: Arc<[u8]>,
    line: usize,
    direction: accesskit::TextDirection,
}

/// A snapshot of the text runs most recently published to AccessKit.
///
/// Keep this beside the text revision that produced it. Accessibility actions
/// may arrive after a new frame, so positions from a stale tree must not be
/// interpreted against different text.
#[derive(Clone, Debug)]
pub struct PublishedAccessibleText {
    source: SharedString,
    revision: u64,
    runs: Arc<[PublishedRun]>,
}

#[derive(Clone, Debug)]
struct PublishedRun {
    node: accesskit::NodeId,
    start_byte: usize,
    character_count: usize,
}

/// Revision-keyed logical text publication. Offscreen unchanged leaves are
/// retained in AccessKit; visible and previously visible leaves refresh their
/// geometry. Own one cache per text surface, never share it between windows.
#[derive(Default)]
pub struct AccessibleTextCache {
    source: SharedString,
    revision: u64,
    parent: Option<accesskit::NodeId>,
    direction: Option<accesskit::TextDirection>,
    rows: Vec<Range<usize>>,
    runs: Arc<[AccessibleRun]>,
    ids: Arc<[accesskit::NodeId]>,
    published: Option<PublishedAccessibleText>,
    visible: Vec<Range<usize>>,
    document: Option<EditSnapshot>,
    work: AccessibleTextWork,
}

/// Actual text work in the most recent accessible publication. Metadata and
/// parent child-id lists are separate, still linear work; these counters do
/// not claim an allocation bound for the entire AccessKit update.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessibleTextWork {
    /// UTF-8 bytes passed to Unicode segmentation and bidi resolution.
    pub segmented_bytes: usize,
    /// Actual persistent-snapshot byte comparisons used to locate the edit.
    pub compared_bytes: usize,
    /// TextRun payloads transmitted in this update.
    pub published_runs: usize,
    /// UTF-8 value bytes transmitted in TextRun payloads.
    pub published_text_bytes: usize,
    /// Existing TextRuns connected without retransmitting their payloads.
    pub retained_runs: usize,
}

impl AccessibleTextCache {
    /// Work performed in the last publication, not cumulative totals.
    pub fn work(&self) -> AccessibleTextWork {
        self.work
    }

    /// Publishes a persistent document, resegmenting only changed LF-delimited
    /// paragraphs when rows and direction outside the edit remain unchanged.
    /// LF is a Unicode bidi/word reset boundary; arbitrary visual rows are not.
    /// Source snapshots and run metadata remain complete, including offscreen
    /// paragraphs. Native actions are still checked against the exact revision.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_document(
        &mut self,
        builder: &mut A11ySubtreeBuilder,
        document: &EditSnapshot,
        anchor: usize,
        focus: usize,
        direction: accesskit::TextDirection,
        rows: &[Range<usize>],
        revision: u64,
        visible: Range<usize>,
        scale: f32,
        geometry: impl Fn(Range<usize>) -> Vec<Bounds<Pixels>>,
    ) -> Option<PublishedAccessibleText> {
        self.publish_document_regions(
            builder,
            document,
            anchor,
            focus,
            direction,
            rows,
            revision,
            vec![visible],
            scale,
            geometry,
        )
    }

    /// Publishes disjoint painted regions without treating omitted source gaps
    /// as visible. Logical text remains complete, including folded paragraphs.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_document_regions(
        &mut self,
        builder: &mut A11ySubtreeBuilder,
        document: &EditSnapshot,
        anchor: usize,
        focus: usize,
        direction: accesskit::TextDirection,
        rows: &[Range<usize>],
        revision: u64,
        visible: Vec<Range<usize>>,
        scale: f32,
        geometry: impl Fn(Range<usize>) -> Vec<Bounds<Pixels>>,
    ) -> Option<PublishedAccessibleText> {
        let difference = self
            .document
            .as_ref()
            .filter(|_| self.revision != revision)
            .map(|before| document.difference_from(before));
        let text = document.text();
        self.document = Some(document.clone());
        let result = publish_accessible_text_inner(
            builder,
            text,
            anchor,
            focus,
            direction,
            rows,
            revision,
            Some((&geometry, scale)),
            Some((self, visible)),
            difference.as_ref(),
        );
        if result.is_none() {
            self.document = None;
        }
        result
    }

    /// Publishes current selection and viewport geometry without rebuilding
    /// unchanged logical runs. `visible` must cover every byte whose geometry
    /// callback can return cells. Text/row/direction changes invalidate the
    /// logical cache; clipping and viewport changes refresh visible leaves.
    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &mut self,
        builder: &mut A11ySubtreeBuilder,
        text: &str,
        anchor: usize,
        focus: usize,
        direction: accesskit::TextDirection,
        rows: &[Range<usize>],
        revision: u64,
        visible: Range<usize>,
        scale: f32,
        geometry: impl Fn(Range<usize>) -> Vec<Bounds<Pixels>>,
    ) -> Option<PublishedAccessibleText> {
        self.document = None;
        publish_accessible_text_inner(
            builder,
            text,
            anchor,
            focus,
            direction,
            rows,
            revision,
            Some((&geometry, scale)),
            Some((self, vec![visible])),
            None,
        )
    }
}

fn run_end_character(run: &AccessibleRun) -> usize {
    run.start_character + run.character_lengths.len()
}

// Segment the document once, rather than rescanning it for every published
// run. Counting starts strictly before the word preserves prefix-grapheme
// semantics even if a word boundary is inside an extended grapheme.
fn indexed_word_starts(text: &str, graphemes: &[(usize, &str)]) -> (Vec<(usize, usize)>, usize) {
    let mut character = 0;
    let mut visited_bytes = 0;
    let words = text
        .unicode_word_indices()
        .map(|(offset, _)| {
            while character < graphemes.len() && graphemes[character].0 < offset {
                visited_bytes += graphemes[character].1.len();
                character += 1;
            }
            (offset, character)
        })
        .collect();
    (words, visited_bytes)
}

fn accessible_runs(
    text: &str,
    visual_rows: &[Range<usize>],
    fallback_direction: accesskit::TextDirection,
) -> Vec<AccessibleRun> {
    if text.is_empty() {
        return vec![AccessibleRun {
            value: 0..0,
            start_byte: 0,
            start_character: 0,
            character_lengths: Arc::default(),
            word_starts: Arc::default(),
            line: 0,
            direction: fallback_direction,
        }];
    }

    let graphemes = text.grapheme_indices(true).collect::<Vec<_>>();
    if graphemes
        .iter()
        .any(|(_, grapheme)| grapheme.len() > u8::MAX as usize)
    {
        // AccessKit stores each selectable unit's UTF-8 length in a u8. An
        // extended grapheme has no Unicode length limit, so an adversarially
        // long combining sequence cannot be represented truthfully.
        return Vec::new();
    }
    let fallback_level = Some(match fallback_direction {
        accesskit::TextDirection::RightToLeft => Level::rtl(),
        _ => Level::ltr(),
    });
    let bidi = BidiInfo::new(text, fallback_level);
    let (words, _) = indexed_word_starts(text, &graphemes);
    let mut runs = Vec::new();
    for (line, row) in visual_rows.iter().enumerate() {
        let mut start =
            graphemes.partition_point(|(offset, grapheme)| offset + grapheme.len() <= row.start);
        let row_end = graphemes.partition_point(|(offset, _)| *offset < row.end);
        while start < row_end {
            let level = bidi.levels[graphemes[start].0];
            let direction = if level.is_rtl() {
                accesskit::TextDirection::RightToLeft
            } else {
                accesskit::TextDirection::LeftToRight
            };
            let limit = (start + MAX_ACCESSIBLE_RUN_CHARS).min(row_end);
            let direction_end = graphemes[start + 1..limit]
                .iter()
                .position(|(offset, _)| bidi.levels[*offset].is_rtl() != level.is_rtl())
                .map(|offset| start + offset + 1)
                .unwrap_or(limit);
            let end = direction_end;
            let start_byte = graphemes[start].0;
            let end_byte = graphemes
                .get(end)
                .map(|(offset, _)| *offset)
                .unwrap_or(text.len());
            runs.push(AccessibleRun {
                value: start_byte..end_byte,
                start_byte,
                start_character: start,
                character_lengths: graphemes[start..end]
                    .iter()
                    .map(|(_, grapheme)| grapheme.len() as u8)
                    .collect(),
                word_starts: words[words.partition_point(|(offset, _)| *offset < start_byte)
                    ..words.partition_point(|(offset, _)| *offset < end_byte)]
                    .iter()
                    .map(|(_, character)| (character - start) as u8)
                    .collect(),
                line,
                direction,
            });
            start = end;
        }
    }
    // A terminating hard break introduces another (empty) logical line. It
    // needs its own position: the position before the break and the caret on
    // the following empty line must not collapse onto the same TextRun.
    if graphemes
        .last()
        .is_some_and(|(_, grapheme)| grapheme.ends_with('\n'))
    {
        runs.push(AccessibleRun {
            value: text.len()..text.len(),
            start_byte: text.len(),
            start_character: graphemes.len(),
            character_lengths: Arc::default(),
            word_starts: Arc::default(),
            line: visual_rows.len(),
            direction: fallback_direction,
        });
    }
    runs
}

struct UpdatedRuns {
    runs: Arc<[AccessibleRun]>,
    prefix: usize,
    old_suffix: usize,
    new_suffix: usize,
    segmented_bytes: usize,
}

fn update_runs(
    cache: &AccessibleTextCache,
    text: &str,
    rows: &[Range<usize>],
    direction: accesskit::TextDirection,
    difference: &crate::EditDifference,
) -> Option<UpdatedRuns> {
    let old = cache.source.as_ref();
    if old.is_empty() || text.is_empty() || cache.runs.is_empty() {
        return None;
    }
    if difference.replaced.is_empty() && difference.inserted.is_empty() && cache.rows == rows {
        return Some(UpdatedRuns {
            runs: cache.runs.clone(),
            prefix: cache.runs.len(),
            old_suffix: cache.runs.len(),
            new_suffix: cache.runs.len(),
            segmented_bytes: 0,
        });
    }
    let start = old[..difference.replaced.start]
        .rfind('\n')
        .map_or(0, |at| at + 1);
    let old_end = old[difference.replaced.end..]
        .find('\n')
        .map_or(old.len(), |at| difference.replaced.end + at + 1);
    let byte_delta = text.len() as isize - old.len() as isize;
    let end = old_end.checked_add_signed(byte_delta)?;
    let old_first = cache.rows.partition_point(|row| row.end <= start);
    let first = rows.partition_point(|row| row.end <= start);
    let old_last = if old_end == old.len() {
        cache.rows.len()
    } else {
        cache.rows.partition_point(|row| row.start < old_end)
    };
    let last = if end == text.len() {
        rows.len()
    } else {
        rows.partition_point(|row| row.start < end)
    };
    if old_first != first
        || cache.rows[..old_first] != rows[..first]
        || cache.rows.len() - old_last != rows.len() - last
        || cache.rows[old_last..]
            .iter()
            .zip(&rows[last..])
            .any(|(old, new)| {
                old.start.checked_add_signed(byte_delta) != Some(new.start)
                    || old.end.checked_add_signed(byte_delta) != Some(new.end)
            })
        || rows.get(first).is_none_or(|row| row.start != start)
        || rows
            .get(last.saturating_sub(1))
            .is_none_or(|row| row.end != end)
    {
        return None;
    }
    let prefix = cache.runs.partition_point(|run| run.start_byte < start);
    let old_suffix = if old_end == old.len() {
        cache.runs.len()
    } else {
        cache.runs.partition_point(|run| run.start_byte < old_end)
    };
    let character_start = cache.runs.get(prefix).map_or_else(
        || run_end_character(cache.runs.last().expect("nonempty runs")),
        |run| run.start_character,
    );
    let old_character_end = cache.runs.get(old_suffix).map_or_else(
        || run_end_character(cache.runs.last().expect("nonempty runs")),
        |run| run.start_character,
    );
    let local_rows: Vec<_> = rows[first..last]
        .iter()
        .map(|row| row.start - start..row.end - start)
        .collect();
    let mut changed = accessible_runs(&text[start..end], &local_rows, direction);
    if changed.is_empty() {
        return None;
    }
    let new_character_end =
        character_start + run_end_character(changed.last().expect("nonempty changed runs"));
    if end < text.len() && changed.last().is_some_and(|run| run.value.is_empty()) {
        changed.pop();
    }
    let mut runs = Vec::with_capacity(prefix + changed.len() + cache.runs.len() - old_suffix);
    runs.extend_from_slice(&cache.runs[..prefix]);
    runs.extend(changed.into_iter().map(|mut run| {
        run.value = run.value.start + start..run.value.end + start;
        run.start_byte += start;
        run.start_character += character_start;
        run.line += first;
        run
    }));
    let new_suffix = runs.len();
    let character_delta = new_character_end as isize - old_character_end as isize;
    let line_delta = last as isize - old_last as isize;
    runs.extend(cache.runs[old_suffix..].iter().cloned().map(|mut run| {
        run.value = run
            .value
            .start
            .checked_add_signed(byte_delta)
            .expect("shifted source")
            ..run
                .value
                .end
                .checked_add_signed(byte_delta)
                .expect("shifted source");
        run.start_byte = run.value.start;
        run.start_character = run
            .start_character
            .checked_add_signed(character_delta)
            .expect("shifted character");
        run.line = run
            .line
            .checked_add_signed(line_delta)
            .expect("shifted line");
        run
    }));
    Some(UpdatedRuns {
        runs: runs.into(),
        prefix,
        old_suffix,
        new_suffix,
        segmented_bytes: end - start,
    })
}

/// Returns whether every selectable grapheme can be represented by AccessKit.
pub fn accessible_text_is_representable(text: &str) -> bool {
    !text
        .graphemes(true)
        .any(|grapheme| grapheme.len() > u8::MAX as usize)
}

/// Publishes logical text runs and the current selection below an accessible
/// text element.
///
/// `visual_rows` contains UTF-8 ranges in visual row order. Runs are split at
/// bidirectional boundaries and AccessKit's 255-character run limit.
pub fn publish_accessible_text(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
    visual_rows: &[Range<usize>],
    revision: u64,
) -> Option<PublishedAccessibleText> {
    publish_accessible_text_inner(
        builder,
        text,
        anchor_byte,
        focus_byte,
        fallback_direction,
        visual_rows,
        revision,
        None,
        None,
        None,
    )
}

/// Publishes selectable text with per-grapheme geometry.
///
/// Bounds returned by `bounds_for_range` are in GPUI logical pixels. They are
/// scaled here to the physical coordinates used by the parent AccessKit node.
#[allow(clippy::too_many_arguments)]
pub fn publish_accessible_text_with_geometry(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
    visual_rows: &[Range<usize>],
    revision: u64,
    scale_factor: f32,
    bounds_for_range: impl Fn(Range<usize>) -> Vec<Bounds<Pixels>>,
) -> Option<PublishedAccessibleText> {
    publish_accessible_text_inner(
        builder,
        text,
        anchor_byte,
        focus_byte,
        fallback_direction,
        visual_rows,
        revision,
        Some((&bounds_for_range, scale_factor)),
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn publish_accessible_text_inner(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    anchor_byte: usize,
    focus_byte: usize,
    fallback_direction: accesskit::TextDirection,
    visual_rows: &[Range<usize>],
    revision: u64,
    geometry: Option<(&dyn Fn(Range<usize>) -> Vec<Bounds<Pixels>>, f32)>,
    mut cache: Option<(&mut AccessibleTextCache, Vec<Range<usize>>)>,
    difference: Option<&crate::EditDifference>,
) -> Option<PublishedAccessibleText> {
    let reused = cache.as_ref().is_some_and(|(cache, _)| {
        cache.revision == revision
            && cache.parent == Some(builder.synthetic_node_id(0u8))
            && cache.direction == Some(fallback_direction)
            && (std::ptr::eq(cache.source.as_ref(), text) || cache.source.as_ref() == text)
            && cache.rows == visual_rows
    });
    let updated = cache.as_ref().and_then(|(cache, _)| {
        (cache.parent == Some(builder.synthetic_node_id(0u8))
            && cache.direction == Some(fallback_direction))
        .then(|| {
            difference.and_then(|difference| {
                update_runs(cache, text, visual_rows, fallback_direction, difference)
            })
        })
        .flatten()
    });
    let mut work = AccessibleTextWork {
        compared_bytes: difference.map_or(0, |difference| difference.compared_bytes),
        segmented_bytes: if reused {
            0
        } else {
            updated
                .as_ref()
                .map_or(text.len(), |updated| updated.segmented_bytes)
        },
        ..Default::default()
    };
    let runs: Arc<[AccessibleRun]> = if reused {
        cache.as_ref().expect("reused cache").0.runs.clone()
    } else if let Some(updated) = &updated {
        updated.runs.clone()
    } else {
        accessible_runs(text, visual_rows, fallback_direction).into()
    };
    if runs.is_empty() {
        if let Some((cache, _)) = cache.as_mut() {
            cache.work = work;
        }
        return None;
    }
    let run_count = runs.len();
    let old_index = |index: usize| {
        if reused {
            Some(index)
        } else {
            updated.as_ref().and_then(|updated| {
                if index < updated.prefix {
                    Some(index)
                } else if index >= updated.new_suffix {
                    Some(updated.old_suffix + index - updated.new_suffix)
                } else {
                    None
                }
            })
        }
    };
    let run_ids: Arc<[accesskit::NodeId]> = if reused {
        cache.as_ref().expect("reused cache").0.ids.clone()
    } else {
        runs.iter()
            .enumerate()
            .map(|(index, run)| {
                if let Some(old) = old_index(index) {
                    return cache.as_ref().expect("incremental cache").0.ids[old];
                }
                builder.synthetic_node_id((
                    revision,
                    run.line,
                    run.start_character,
                    run.character_lengths.len(),
                ))
            })
            .collect()
    };
    for run in 0..run_count {
        let accessible_run = &runs[run];
        let visible = cache.as_ref().is_none_or(|(_, visible)| {
            visible.iter().any(|visible| {
                accessible_run.value.start < visible.end && visible.start < accessible_run.value.end
            })
        });
        let was_visible = cache.as_ref().is_some_and(|(cache, _)| {
            old_index(run).is_some_and(|index| {
                let old = &cache.runs[index];
                cache
                    .visible
                    .iter()
                    .any(|visible| old.value.start < visible.end && visible.start < old.value.end)
            })
        });
        if old_index(run).is_some()
            && !visible
            && !was_visible
            && builder.retain_child(run_ids[run])
        {
            work.retained_runs += 1;
            continue;
        }
        work.published_runs += 1;
        work.published_text_bytes += accessible_run.value.len();
        let mut node = accesskit::Node::new(accesskit::Role::TextRun);
        node.set_text_direction(accessible_run.direction);
        node.set_value(&text[accessible_run.value.clone()]);
        node.set_character_lengths(accessible_run.character_lengths.to_vec());
        if !accessible_run.word_starts.is_empty() {
            node.set_word_starts(accessible_run.word_starts.to_vec());
        }
        if let Some((bounds_for_range, scale)) = geometry.filter(|_| visible) {
            let mut positions = Vec::with_capacity(accessible_run.character_lengths.len());
            let mut widths = Vec::with_capacity(accessible_run.character_lengths.len());
            let mut advance = 0.0;
            let mut union: Option<Bounds<Pixels>> = None;
            for (offset, grapheme) in text[accessible_run.value.clone()].grapheme_indices(true) {
                positions.push(advance * scale);
                let range = accessible_run.start_byte + offset
                    ..accessible_run.start_byte + offset + grapheme.len();
                let cells = bounds_for_range(range);
                let width = cells.iter().map(|cell| cell.size.width.0).sum::<f32>();
                widths.push(width);
                advance += width;
                for cell in cells {
                    union = Some(match union {
                        Some(bounds) => bounds.union(&cell),
                        None => cell,
                    });
                }
            }
            if let Some(bounds) = union {
                let normalization = if advance > 0.0 {
                    bounds.size.width.0 / advance
                } else {
                    1.0
                };
                let mut normalized_advance = 0.0;
                for (position, width) in positions.iter_mut().zip(&mut widths) {
                    *position = normalized_advance * scale;
                    *width *= normalization;
                    normalized_advance += *width;
                    *width *= scale;
                }
                node.set_bounds(accesskit::Rect {
                    x0: (bounds.left().0 * scale) as f64,
                    y0: (bounds.top().0 * scale) as f64,
                    x1: (bounds.right().0 * scale) as f64,
                    y1: (bounds.bottom().0 * scale) as f64,
                });
                node.set_character_positions(positions);
                node.set_character_widths(widths);
            }
        }
        if run > 0 && runs[run - 1].line == accessible_run.line {
            node.set_previous_on_line(run_ids[run - 1]);
        }
        if run + 1 < run_count && runs[run + 1].line == accessible_run.line {
            node.set_next_on_line(run_ids[run + 1]);
        }
        builder.push_child(run_ids[run], node);
    }
    let anchor = accessible_position(text, anchor_byte, &runs, |run| run_ids[run]);
    let focus = accessible_position(text, focus_byte, &runs, |run| run_ids[run]);
    builder
        .parent_node()
        .set_text_selection(accesskit::TextSelection { anchor, focus });
    let published = if reused {
        cache.as_ref().expect("reused cache").0.published.clone()
    } else {
        Some(PublishedAccessibleText {
            source: cache
                .as_ref()
                .and_then(|(cache, _)| cache.document.as_ref())
                .map_or_else(|| text.into(), |document| document.text().clone()),
            revision,
            runs: runs
                .iter()
                .zip(run_ids.iter().copied())
                .map(|(run, node)| PublishedRun {
                    node,
                    start_byte: run.start_byte,
                    character_count: run.character_lengths.len(),
                })
                .collect(),
        })
    };
    if let Some((cache, visible)) = cache.as_mut() {
        if !reused {
            cache.source = published.as_ref().expect("published runs").source.clone();
            cache.revision = revision;
            cache.parent = Some(builder.synthetic_node_id(0u8));
            cache.direction = Some(fallback_direction);
            cache.rows = visual_rows.to_vec();
            cache.runs = runs;
            cache.ids = run_ids;
            cache.published = published.clone();
        }
        cache.visible = visible.clone();
        cache.work = work;
    }
    published
}

fn accessible_position(
    text: &str,
    byte_offset: usize,
    runs: &[AccessibleRun],
    node_id: impl Fn(usize) -> accesskit::NodeId,
) -> accesskit::TextPosition {
    let byte_offset = byte_offset.min(text.len());
    let mut run = runs
        .partition_point(|run| run.start_byte < byte_offset)
        .saturating_sub(1);
    let local = byte_offset.saturating_sub(runs[run].start_byte);
    let mut bytes = 0;
    let mut character_index = runs[run]
        .character_lengths
        .iter()
        .take_while(|length| {
            let before = bytes;
            bytes += **length as usize;
            before < local
        })
        .count();
    // Prefix semantics round an interior byte of a selectable unit forward.
    // For CRLF that can reach the next logical line before its LF byte ends.
    if character_index == runs[run].character_lengths.len()
        && runs
            .get(run + 1)
            .is_some_and(|next| next.line != runs[run].line)
    {
        run += 1;
        character_index = 0;
    }
    accesskit::TextPosition {
        node: node_id(run),
        character_index,
    }
}

fn byte_offset_for_accessible_position(
    text: &str,
    position: accesskit::TextPosition,
    runs: &[PublishedRun],
) -> Option<usize> {
    let run = runs.iter().find(|run| run.node == position.node)?;
    if position.character_index > run.character_count {
        return None;
    }
    Some(
        text[run.start_byte..]
            .grapheme_indices(true)
            .nth(position.character_index)
            .map(|(offset, _)| run.start_byte + offset)
            .unwrap_or(text.len()),
    )
}

/// Resolves an AccessKit text position only against the exact value and
/// revision for which its synthetic run ids were published.
pub fn byte_offset_for_published_position(
    current_text: &str,
    current_revision: u64,
    published: &PublishedAccessibleText,
    position: accesskit::TextPosition,
) -> Option<usize> {
    (current_text == published.source.as_ref() && current_revision == published.revision)
        .then(|| byte_offset_for_accessible_position(current_text, position, &published.runs))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_updates_equal_full_unicode_segmentation_for_insert_delete_and_line_changes() {
        let texts = [
            "head\ncan't e\u{301} אבג DEF\r\n👩‍💻 tail\n".to_string(),
            format!("head\n{}'word אבג\n終わり", "a".repeat(270)),
            "\n\nend".to_string(),
        ];
        for text in texts {
            for direction in [
                accesskit::TextDirection::LeftToRight,
                accesskit::TextDirection::RightToLeft,
            ] {
                let rows = hard_rows(&text);
                let cache = AccessibleTextCache {
                    source: text.clone().into(),
                    rows: rows.clone(),
                    runs: accessible_runs(&text, &rows, direction).into(),
                    ..Default::default()
                };
                let boundaries: Vec<_> = text
                    .char_indices()
                    .map(|(at, _)| at)
                    .chain([text.len()])
                    .collect();
                for (index, &start) in boundaries.iter().enumerate() {
                    for end in [start, *boundaries.get(index + 2).unwrap_or(&text.len())] {
                        for inserted in ["", "界\nאב", "\u{301}", "\r\n"] {
                            let mut changed = text.clone();
                            changed.replace_range(start..end, inserted);
                            let new_text = changed.as_str();
                            let difference = crate::EditDifference {
                                replaced: start..end,
                                inserted: inserted.into(),
                                compared_bytes: 0,
                                shared_bytes: 0,
                            };
                            let new_rows = hard_rows(new_text);
                            let update =
                                update_runs(&cache, new_text, &new_rows, direction, &difference)
                                    .expect("nonempty paragraph update");
                            assert_eq!(
                                update.runs.as_ref(),
                                accessible_runs(new_text, &new_rows, direction),
                                "edit {start}..{end} -> {inserted:?}, direction {direction:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ten_thousand_paragraph_edit_segments_only_changed_text_and_shares_other_payloads() {
        for count in [1000, 10000] {
            let text = format!(
                "[\n{}{{\"tail\":7}}\n]",
                "{\"asymmetric\":\"界\",\"value\":13},\n".repeat(count)
            );
            let mut buffer = crate::EditBuffer::new(crate::EditRules::default());
            buffer.set_text(&text);
            let before = buffer.snapshot();
            let rows = hard_rows(&text);
            let cache = AccessibleTextCache {
                source: text.clone().into(),
                rows: rows.clone(),
                runs: accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight).into(),
                ..Default::default()
            };
            let at = text.find("13").expect("fixture number");
            buffer.replace(at..at + 1, "987", crate::EditCause::Programmatic);
            let after = buffer.snapshot();
            let difference = after.difference_from(&before);
            let updated = update_runs(
                &cache,
                after.text(),
                &hard_rows(after.text()),
                accesskit::TextDirection::LeftToRight,
                &difference,
            )
            .expect("incremental update");
            assert!(updated.segmented_bytes < 40);
            assert!(difference.compared_bytes < 8192);
            assert!(updated.runs.len() - updated.new_suffix >= count);
            assert!(Arc::ptr_eq(
                &cache.runs[updated.old_suffix].character_lengths,
                &updated.runs[updated.new_suffix].character_lengths
            ));
            assert_eq!(
                updated.runs.as_ref(),
                accessible_runs(
                    after.text(),
                    &hard_rows(after.text()),
                    accesskit::TextDirection::LeftToRight
                )
            );
        }
    }

    #[test]
    fn indexed_words_preserve_global_boundaries_across_runs() {
        for text in [
            format!("{}tail e\u{301}👩‍💻 אבג123 abc\r\n界中文", "a".repeat(254)),
            "can't a\u{301}b אבג DEF\nfoo_bar 12.34 ไทย".into(),
        ] {
            let mut rows = hard_rows(&text);
            // A visual break inside a word must not invent a new word start.
            rows.splice(0..1, [0..2, 2..rows[0].end]);
            let runs = accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight);
            for run in runs {
                let expected = text
                    .unicode_word_indices()
                    .filter(|(offset, _)| {
                        run.start_byte <= *offset && *offset < run.start_byte + run.value.len()
                    })
                    .map(|(offset, _)| {
                        (text[..offset].graphemes(true).count() - run.start_character) as u8
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    run.word_starts.as_ref(),
                    expected,
                    "{}",
                    &text[run.value.clone()]
                );
            }
        }
    }

    #[test]
    fn large_accessible_word_index_visits_each_byte_at_most_once() {
        for rows in [1000, 10000] {
            let text = format!(
                "[\n{}{{\"tail\":7}}\n]",
                "{\"asymmetric\":\"界\",\"value\":13},\n".repeat(rows)
            );
            let graphemes = text.grapheme_indices(true).collect::<Vec<_>>();
            let (words, visited_bytes) = indexed_word_starts(&text, &graphemes);
            assert!(visited_bytes <= text.len());
            assert_eq!(words.len(), rows * 4 + 2);
            let runs = accessible_runs(
                &text,
                &hard_rows(&text),
                accesskit::TextDirection::LeftToRight,
            );
            assert_eq!(
                runs.iter().map(|run| run.word_starts.len()).sum::<usize>(),
                words.len()
            );
            assert_eq!(
                runs.iter().map(|run| run.value.len()).sum::<usize>(),
                text.len()
            );
        }
    }

    fn hard_rows(text: &str) -> Vec<Range<usize>> {
        let mut rows = Vec::new();
        let mut start = 0;
        for (offset, grapheme) in text.grapheme_indices(true) {
            if grapheme.ends_with('\n') {
                rows.push(start..offset + grapheme.len());
                start = offset + grapheme.len();
            }
        }
        if start < text.len() || rows.is_empty() {
            rows.push(start..text.len());
        }
        rows
    }

    #[test]
    fn indexed_positions_match_global_prefix_semantics_at_every_unicode_byte() {
        let text = format!("{}e\u{301}👩‍💻 אבג\r\nDEF\n", "x".repeat(270));
        let rows = hard_rows(&text);
        let runs = accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight);
        for offset in 0..=text.len() {
            let character = text
                .grapheme_indices(true)
                .take_while(|(at, _)| *at < offset)
                .count();
            let index = runs
                .iter()
                .enumerate()
                .find_map(|(index, run)| {
                    let end = run_end_character(run);
                    (character < end
                        || (character == end
                            && runs.get(index + 1).is_none_or(|next| next.line == run.line)))
                    .then_some(index)
                })
                .unwrap_or(runs.len() - 1);
            let actual = accessible_position(&text, offset, &runs, |index| {
                accesskit::NodeId(index as u64)
            });
            assert_eq!(
                actual.node,
                accesskit::NodeId(index as u64),
                "byte {offset}"
            );
            assert_eq!(
                actual.character_index,
                character - runs[index].start_character,
                "byte {offset}"
            );
        }
    }

    #[test]
    fn accessible_positions_round_trip_utf8_text() {
        let text = format!("{}e\u{301}👩‍💻\nאב", "x".repeat(255));
        let nodes = |run| accesskit::NodeId(100 + run as u64);
        let rows = hard_rows(&text);
        let runs = accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight);
        let run_ids = (0..runs.len()).map(nodes).collect::<Vec<_>>();
        let published = runs
            .iter()
            .zip(&run_ids)
            .map(|(run, node)| PublishedRun {
                node: *node,
                start_byte: run.start_byte,
                character_count: run.character_lengths.len(),
            })
            .collect::<Vec<_>>();
        for offset in [0, 255, 258, 269, 270, text.len()] {
            let position = accessible_position(&text, offset, &runs, nodes);
            assert_eq!(
                byte_offset_for_accessible_position(&text, position, &published),
                Some(offset)
            );
        }
    }

    #[test]
    fn accessible_runs_use_graphemes_and_do_not_link_hard_lines() {
        let text = "e\u{301}👩‍💻\nאב";
        let rows = hard_rows(text);
        let runs = accessible_runs(text, &rows, accesskit::TextDirection::LeftToRight);
        assert_eq!(runs.len(), 2);
        assert_eq!(&text[runs[0].value.clone()], "e\u{301}👩‍💻\n");
        assert_eq!(runs[0].character_lengths.as_ref(), [3, 11, 1]);
        assert_eq!(&text[runs[1].value.clone()], "אב");
        assert_eq!(runs[1].direction, accesskit::TextDirection::RightToLeft);
    }

    #[test]
    fn trailing_lf_and_crlf_publish_a_distinct_empty_line() {
        for text in ["a\n", "a\r\n"] {
            let rows = hard_rows(text);
            let runs = accessible_runs(text, &rows, accesskit::TextDirection::LeftToRight);
            assert_eq!(runs.len(), 2);
            assert_eq!(&text[runs[0].value.clone()], text);
            assert_eq!(&text[runs[1].value.clone()], "");
            let ids = [accesskit::NodeId(1), accesskit::NodeId(2)];
            let published = runs
                .iter()
                .zip(ids)
                .map(|(run, node)| PublishedRun {
                    node,
                    start_byte: run.start_byte,
                    character_count: run.character_lengths.len(),
                })
                .collect::<Vec<_>>();
            let end = accessible_position(text, text.len(), &runs, |run| ids[run]);
            assert_eq!(end.node, ids[1]);
            assert_eq!(end.character_index, 0);
            assert_eq!(
                byte_offset_for_accessible_position(text, end, &published),
                Some(text.len())
            );
        }
    }

    #[test]
    fn unrepresentable_graphemes_are_not_published() {
        let text = format!("a{}", "\u{301}".repeat(128));
        assert!(text.len() > u8::MAX as usize);
        let rows = std::iter::once(0..text.len()).collect::<Vec<_>>();
        assert!(accessible_runs(&text, &rows, accesskit::TextDirection::LeftToRight).is_empty());
    }
}
