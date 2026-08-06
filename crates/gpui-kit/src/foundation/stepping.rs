//! Moving a keyboard selection along a strip of choices.
//!
//! A strip has ends. Arrowing past the last choice onto the first would move
//! the selection somewhere the typist never pointed, so movement stops instead
//! of wrapping. A menu is the other case and wraps deliberately, which is why
//! [`crate::overlay::popover::step`] is a separate rule rather than an option
//! on this one.

/// The index `delta` steps away from `from`, skipping every index `refused`
/// answers for and stopping at the ends.
///
/// Entering with nothing selected lands on the end the movement travels away
/// from: a step to the right enters at the first choice, a step to the left at
/// the last.
pub(crate) fn bounded_step(
    count: usize,
    from: Option<usize>,
    delta: isize,
    refused: impl Fn(usize) -> bool,
) -> Option<usize> {
    let count = count as isize;
    if count == 0 || delta == 0 {
        return None;
    }
    let start = from.map_or(if delta > 0 { -1 } else { count }, |index| index as isize);
    let mut index = start + delta;
    while (0..count).contains(&index) {
        if !refused(index as usize) {
            return Some(index as usize);
        }
        index += delta;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_stops_at_the_ends_rather_than_wrapping() {
        let open = |_| false;
        assert_eq!(bounded_step(3, Some(2), 1, open), None);
        assert_eq!(bounded_step(3, Some(0), -1, open), None);
    }

    #[test]
    fn entering_lands_on_the_end_the_movement_came_from() {
        let open = |_| false;
        assert_eq!(bounded_step(3, None, 1, open), Some(0));
        assert_eq!(bounded_step(3, None, -1, open), Some(2));
    }

    #[test]
    fn a_refused_choice_is_stepped_over_rather_than_landed_on() {
        assert_eq!(bounded_step(3, Some(0), 1, |index| index == 1), Some(2));
        assert_eq!(bounded_step(2, None, 1, |_| true), None);
    }
}
