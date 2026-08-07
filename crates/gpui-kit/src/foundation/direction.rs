//! Which way the interface reads, and the logical styling that follows.
//!
//! GPUI has no notion of a reading direction: `Styled` exposes `pl`/`pr`,
//! `ml`/`mr`, `border_l`/`border_r` and a `TextAlign` of `Left`, `Center` and
//! `Right`, all of them physical, and taffy's logical properties are not
//! surfaced. So the direction is a global here, reaching components the same
//! way [`Theme`](gpui_kit_theme::Theme) does: a host sets it once, components
//! read it during render, and nothing is threaded through a builder argument.
//!
//! The vocabulary is *start* and *end* rather than left and right. Start is
//! where reading begins, so it is the left edge in a left-to-right interface
//! and the right edge in a right-to-left one. Anything that is genuinely
//! physical — the edge a vertical scrollbar sits on, the axis of a chart —
//! keeps saying left and right, because those do not move when the reading
//! direction does.

use gpui::{App, Global, Pixels, Styled, TextAlign};

/// The direction the interface reads in.
///
/// Left to right is the default, so a host that never sets one renders
/// exactly as it did before this existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayoutDirection {
    #[default]
    LeftToRight,
    RightToLeft,
}

impl LayoutDirection {
    pub fn is_rtl(self) -> bool {
        matches!(self, Self::RightToLeft)
    }

    pub fn is_ltr(self) -> bool {
        matches!(self, Self::LeftToRight)
    }

    /// The physical edge that reading begins at.
    pub fn start(self) -> PhysicalSide {
        match self {
            Self::LeftToRight => PhysicalSide::Left,
            Self::RightToLeft => PhysicalSide::Right,
        }
    }

    /// The physical edge that reading ends at.
    pub fn end(self) -> PhysicalSide {
        self.start().opposite()
    }

    /// How far one step through reading order moves when a physical arrow key
    /// is pressed. `None` for a key that is not a horizontal arrow.
    ///
    /// A `Left` arrow means "previous" while reading left to right and "next"
    /// while reading right to left, so a component that steps a selection asks
    /// this instead of matching on the key name.
    pub fn arrow_step(self, key: &str) -> Option<i32> {
        let forward = match self {
            Self::LeftToRight => 1,
            Self::RightToLeft => -1,
        };
        match key {
            "right" => Some(forward),
            "left" => Some(-forward),
            _ => None,
        }
    }
}

/// An edge named by reading order rather than by geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalSide {
    Start,
    End,
}

impl LogicalSide {
    pub fn resolve(self, direction: LayoutDirection) -> PhysicalSide {
        match self {
            Self::Start => direction.start(),
            Self::End => direction.end(),
        }
    }
}

/// An edge that does not move when the reading direction does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicalSide {
    Left,
    Right,
}

impl PhysicalSide {
    pub fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    pub fn is_left(self) -> bool {
        matches!(self, Self::Left)
    }
}

/// The one place the active direction lives.
#[derive(Debug, Default)]
struct Direction(LayoutDirection);

impl Global for Direction {}

/// Reads the active reading direction from any context that dereferences to
/// [`App`], the way [`ActiveTheme`](gpui_kit_theme::ActiveTheme) reads the
/// theme.
pub trait ActiveDirection {
    fn layout_direction(&self) -> LayoutDirection;

    fn is_rtl(&self) -> bool {
        self.layout_direction().is_rtl()
    }
}

impl ActiveDirection for App {
    fn layout_direction(&self) -> LayoutDirection {
        // Defaulted rather than required, so a host that installed nothing
        // still gets the left-to-right layout it had before.
        self.try_global::<Direction>()
            .map(|direction| direction.0)
            .unwrap_or_default()
    }
}

/// Installs the direction global at its default. Idempotent.
pub fn install(cx: &mut App) {
    if !cx.has_global::<Direction>() {
        cx.set_global(Direction::default());
    }
}

/// Sets the reading direction and repaints every window.
pub fn set_layout_direction(direction: LayoutDirection, cx: &mut App) {
    cx.set_global(Direction(direction));
    cx.refresh_windows();
}

/// Logical box model helpers, named by reading order.
///
/// Each one takes the direction explicitly rather than reaching for the
/// global, because a component has already read it once by the time it styles
/// anything and passing it keeps these usable in a test with no application.
pub trait DirectionalExt: Styled + Sized {
    /// A flex row that runs in reading order.
    fn row_reading(self, direction: LayoutDirection) -> Self {
        let element = self.flex().items_center();
        if direction.is_rtl() {
            element.flex_row_reverse()
        } else {
            element.flex_row()
        }
    }

    /// Padding on the edge reading begins at.
    fn ps(self, direction: LayoutDirection, length: Pixels) -> Self {
        if direction.is_rtl() {
            self.pr(length)
        } else {
            self.pl(length)
        }
    }

    /// Padding on the edge reading ends at.
    fn pe(self, direction: LayoutDirection, length: Pixels) -> Self {
        if direction.is_rtl() {
            self.pl(length)
        } else {
            self.pr(length)
        }
    }

    /// Margin on the edge reading begins at.
    fn ms(self, direction: LayoutDirection, length: Pixels) -> Self {
        if direction.is_rtl() {
            self.mr(length)
        } else {
            self.ml(length)
        }
    }

    /// Margin on the edge reading ends at.
    fn me(self, direction: LayoutDirection, length: Pixels) -> Self {
        if direction.is_rtl() {
            self.ml(length)
        } else {
            self.mr(length)
        }
    }

    /// A border on the edge reading begins at.
    fn border_s(self, direction: LayoutDirection, width: Pixels) -> Self {
        if direction.is_rtl() {
            self.border_r(width)
        } else {
            self.border_l(width)
        }
    }

    /// A border on the edge reading ends at.
    fn border_e(self, direction: LayoutDirection, width: Pixels) -> Self {
        if direction.is_rtl() {
            self.border_l(width)
        } else {
            self.border_r(width)
        }
    }

    /// Text aligned to where reading begins.
    fn text_start(self, direction: LayoutDirection) -> Self {
        self.text_align(match direction {
            LayoutDirection::LeftToRight => TextAlign::Left,
            LayoutDirection::RightToLeft => TextAlign::Right,
        })
    }

    /// Text aligned to where reading ends.
    fn text_end(self, direction: LayoutDirection) -> Self {
        self.text_align(match direction {
            LayoutDirection::LeftToRight => TextAlign::Right,
            LayoutDirection::RightToLeft => TextAlign::Left,
        })
    }
}

impl<T: Styled + Sized> DirectionalExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_direction_reads_left_to_right() {
        assert_eq!(LayoutDirection::default(), LayoutDirection::LeftToRight);
        assert!(!LayoutDirection::default().is_rtl());
        assert_eq!(LayoutDirection::default().start(), PhysicalSide::Left);
    }

    #[test]
    fn a_logical_edge_swaps_and_a_physical_one_does_not() {
        let ltr = LayoutDirection::LeftToRight;
        let rtl = LayoutDirection::RightToLeft;

        assert_eq!(LogicalSide::Start.resolve(ltr), PhysicalSide::Left);
        assert_eq!(LogicalSide::Start.resolve(rtl), PhysicalSide::Right);
        assert_eq!(LogicalSide::End.resolve(ltr), PhysicalSide::Right);
        assert_eq!(LogicalSide::End.resolve(rtl), PhysicalSide::Left);

        // A physical edge is the same edge in either reading direction: that
        // is the whole reason it is spelled physically.
        assert_eq!(PhysicalSide::Right.opposite(), PhysicalSide::Left);
        assert_eq!(PhysicalSide::Right, PhysicalSide::Right);
    }

    #[test]
    fn arrow_keys_step_the_way_the_reading_direction_says() {
        let ltr = LayoutDirection::LeftToRight;
        let rtl = LayoutDirection::RightToLeft;

        assert_eq!(ltr.arrow_step("right"), Some(1));
        assert_eq!(ltr.arrow_step("left"), Some(-1));
        assert_eq!(rtl.arrow_step("right"), Some(-1));
        assert_eq!(rtl.arrow_step("left"), Some(1));

        // A vertical arrow is not a reading-order move in either direction.
        assert_eq!(ltr.arrow_step("up"), None);
        assert_eq!(rtl.arrow_step("down"), None);
        assert_eq!(rtl.arrow_step("home"), None);
    }
}
