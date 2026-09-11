//! Platform-independent Android protocol rules, tested without an emulator.

use std::ops::Range;

/// Android selection endpoints are anchor/head UTF-16 offsets, not a sorted
/// range. Android supplies no visual affinity, so use downstream explicitly.
pub fn android_selection(text: &str, start: i32, end: i32) -> Option<gpui::NativeTextSelection> {
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    utf16_range(text, start.min(end)..start.max(end))?;
    let position = |utf16_offset| gpui::NativeTextPosition {
        utf16_offset,
        affinity: gpui::TextAffinity::Downstream,
    };
    Some(gpui::NativeTextSelection {
        anchor: position(start),
        head: position(end),
    })
}

/// Configuration.UI_MODE_NIGHT_MASK / UI_MODE_NIGHT_YES. An undefined night
/// mode follows Android's non-night default; unrelated uiMode bits are ignored.
pub fn android_appearance(ui_mode: i32) -> gpui::WindowAppearance {
    if ui_mode & 0x30 == 0x20 {
        gpui::WindowAppearance::Dark
    } else {
        gpui::WindowAppearance::Light
    }
}

/// Android MotionEvent actions, after removing the pointer-index bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerAction {
    Down,
    Up,
    Move,
    Cancel,
}

/// Returns the affected pointer indexes. Android's POINTER_DOWN/UP applies to
/// the action index only; MOVE and CANCEL apply to every current pointer.
pub fn motion_pointers(action: i32, count: usize) -> Vec<(usize, PointerAction)> {
    let index = ((action >> 8) & 255) as usize;
    match action & 255 {
        0 | 5 if index < count => vec![(index, PointerAction::Down)],
        1 | 6 if index < count => vec![(index, PointerAction::Up)],
        2 => (0..count).map(|i| (i, PointerAction::Move)).collect(),
        3 => (0..count).map(|i| (i, PointerAction::Cancel)).collect(),
        _ => Vec::new(),
    }
}

/// Android's cursor offset is relative to the end of the insertion for positive
/// values, or its start otherwise. All values here count UTF-16 code units.
pub fn insertion_cursor(start: usize, inserted: usize, cursor: i32, total: usize) -> usize {
    let base = if cursor > 0 { start + inserted } else { start };
    let offset = if cursor > 0 {
        cursor as i64 - 1
    } else {
        cursor as i64
    };
    (base as i64 + offset).clamp(0, total as i64) as usize
}

/// Rebase a UTF-16 selection or composing span after deleting another span.
/// Surrounding-text deletion must not collapse an untouched selected range.
pub fn selection_after_deletion(selection: Range<usize>, deleted: Range<usize>) -> Range<usize> {
    let offset = |position: usize| {
        if position <= deleted.start {
            position
        } else if position >= deleted.end {
            position - deleted.len()
        } else {
            deleted.start
        }
    };
    offset(selection.start)..offset(selection.end)
}

/// Preserve endpoint direction and each existing affinity when surrounding
/// text is removed. Deletion changes offsets, not which endpoint is moving.
pub fn native_selection_after_deletion(
    mut selection: gpui::NativeTextSelection,
    deleted: Range<usize>,
) -> gpui::NativeTextSelection {
    for position in [&mut selection.anchor, &mut selection.head] {
        let offset = position.utf16_offset;
        position.utf16_offset = selection_after_deletion(offset..offset, deleted.clone()).start;
    }
    selection
}

/// Validates UTF-16 boundaries, rejecting a selection inside a surrogate pair.
pub fn utf16_range(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    if range.start > range.end {
        return None;
    }
    let mut units = 0;
    let mut start = None;
    let mut end = None;
    for (byte, ch) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
    {
        if units == range.start {
            start = Some(byte);
        }
        if units == range.end {
            end = Some(byte);
        }
        units += ch.len_utf16();
    }
    Some(start?..end?)
}

/// Draw eligibility is independent of Activity existence. A resumed Activity
/// can temporarily have no Surface, and a Surface can precede onResume.
#[derive(Default, Debug)]
pub struct SurfaceLifecycle {
    active: bool,
    size: Option<(u32, u32)>,
    generation: u64,
}

impl SurfaceLifecycle {
    pub fn resume(&mut self) {
        self.active = true;
    }
    pub fn pause(&mut self) {
        self.active = false;
    }
    pub fn attach(&mut self, width: u32, height: u32) -> u64 {
        self.generation += 1;
        self.size = Some((width, height));
        self.generation
    }
    pub fn detach(&mut self) {
        self.size = None;
    }
    pub fn can_draw(&self) -> bool {
        self.active && self.size.is_some_and(|(w, h)| w > 0 && h > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_selection_keeps_anchor_head_and_validates_utf16() {
        let selection = android_selection("a😀中z", 4, 1).expect("valid reversed selection");
        assert_eq!(selection.anchor.utf16_offset, 4);
        assert_eq!(selection.head.utf16_offset, 1);
        assert_eq!(selection.anchor.affinity, gpui::TextAffinity::Downstream);
        assert_eq!(selection.head.affinity, gpui::TextAffinity::Downstream);
        let forward = android_selection("a😀中z", 1, 4).expect("valid forward selection");
        assert_eq!(forward.anchor, selection.head);
        assert_eq!(forward.head, selection.anchor);
        assert!(android_selection("a😀中z", 4, 2).is_none());
        assert!(android_selection("a😀中z", -1, 3).is_none());
        let mut selected = selection;
        selected.anchor.affinity = gpui::TextAffinity::Upstream;
        let shifted = native_selection_after_deletion(selected, 0..1);
        assert_eq!(shifted.anchor.utf16_offset, 3);
        assert_eq!(shifted.head.utf16_offset, 0);
        assert_eq!(shifted.anchor.affinity, gpui::TextAffinity::Upstream);
        assert_eq!(shifted.head.affinity, gpui::TextAffinity::Downstream);
    }

    #[test]
    fn configuration_night_mode_ignores_other_ui_mode_bits() {
        assert_eq!(android_appearance(0x23), gpui::WindowAppearance::Dark);
        assert_eq!(android_appearance(0x13), gpui::WindowAppearance::Light);
        assert_eq!(android_appearance(0x03), gpui::WindowAppearance::Light);
    }

    #[test]
    fn pointer_up_does_not_end_other_fingers() {
        assert_eq!(
            motion_pointers(6 | (2 << 8), 3),
            vec![(2, PointerAction::Up)]
        );
        assert_eq!(
            motion_pointers(5 | (1 << 8), 3),
            vec![(1, PointerAction::Down)]
        );
        assert_eq!(
            motion_pointers(3, 3),
            vec![
                (0, PointerAction::Cancel),
                (1, PointerAction::Cancel),
                (2, PointerAction::Cancel)
            ]
        );
        assert_eq!(
            motion_pointers(2, 2),
            vec![(0, PointerAction::Move), (1, PointerAction::Move)]
        );
        assert!(motion_pointers(6 | (3 << 8), 3).is_empty());
    }

    #[test]
    fn android_cursor_offsets_are_not_byte_offsets() {
        assert_eq!(insertion_cursor(3, 2, 1, 10), 5);
        assert_eq!(insertion_cursor(3, 2, 0, 10), 3);
        assert_eq!(insertion_cursor(3, 2, -2, 10), 1);
        assert_eq!(insertion_cursor(3, 2, 99, 10), 10);
        assert_eq!(utf16_range("a😀中z", 1..3), Some(1..5));
        assert_eq!(utf16_range("a😀中z", 2..3), None);
        assert_eq!(utf16_range("a😀中z", 3..5), Some(5..9));
    }

    #[test]
    fn surrounding_deletion_preserves_selection_and_composing_span() {
        assert_eq!(selection_after_deletion(3..7, 8..11), 3..7);
        assert_eq!(selection_after_deletion(3..7, 1..3), 1..5);
        assert_eq!(selection_after_deletion(3..7, 3..7), 3..3);
        assert_eq!(selection_after_deletion(4..9, 2..6), 2..5);
    }

    #[test]
    fn surface_recreation_and_background_never_draw_stale_surface() {
        let mut life = SurfaceLifecycle::default();
        assert_eq!(life.attach(400, 700), 1);
        assert!(!life.can_draw());
        life.resume();
        assert!(life.can_draw());
        life.detach();
        assert!(!life.can_draw());
        assert_eq!(life.attach(700, 400), 2);
        assert!(life.can_draw());
        life.pause();
        assert!(!life.can_draw());
        life.attach(0, 400);
        life.resume();
        assert!(!life.can_draw());
    }
}
