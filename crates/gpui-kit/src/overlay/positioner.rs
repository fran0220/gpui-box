//! Which side an anchored surface goes on, and how much room it has there.
//!
//! GPUI's [`anchored`](gpui::anchored) already keeps a floating surface inside
//! the window: it flips and shifts one that would hang off an edge. That is
//! enough for a surface whose size does not depend on the answer.
//!
//! It is not enough for a surface that has to be *built* differently depending
//! on where it lands. A select menu that flips above its trigger has a
//! different amount of room than one that opens below it, and the list inside
//! it has to be bounded to that room before it is laid out — bounding it
//! afterwards is a frame of the wrong height that the reader sees. Such a
//! component needs the decision, not just its effect, and it needs it in the
//! same frame it is building.
//!
//! That is what this answers: given where the trigger was measured and how
//! much of the window there is, which side, which edge to hang from, how wide,
//! and how tall.
//!
//! # Which edge it hangs from is decided, not assumed
//!
//! `anchored` also slides a surface back inside the window, which keeps it on
//! screen and quietly unhooks it from its trigger: a menu that was under a
//! chip is now under the chip and 40 points to the left of it, pointing at
//! nothing. So the trailing edge is resolved here too, on the same rule as the
//! side. A surface that would run off is lined up with its trigger's other
//! edge instead, where it is still attached to the thing that opened it.
//!
//! # It is only ever as good as the measurement
//!
//! A trigger that has not been laid out has no bounds, so the first frame of a
//! surface is placed against the whole usable window and the trigger's
//! prepaint asks for the corrective frame. [`Room::measured`] says which of
//! the two a caller is looking at, because "there is room below" and "nobody
//! has looked yet" are different answers.

use gpui::{Bounds, Pixels, Window};

use crate::overlay::layer::{Hang, Placement};

/// Which side of the trigger a surface is asked to go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// Under the trigger, which is where a menu goes unless it cannot.
    #[default]
    Below,
    /// Over it.
    Above,
    /// To the trigger's leading edge, for a submenu.
    Before,
    /// To its trailing edge.
    After,
}

impl Side {
    /// The side a surface flips to when this one does not fit.
    pub fn opposite(self) -> Self {
        match self {
            Self::Below => Self::Above,
            Self::Above => Self::Below,
            Self::Before => Self::After,
            Self::After => Self::Before,
        }
    }

    /// Whether the surface is stacked with the trigger rather than beside it.
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Below | Self::Above)
    }
}

impl From<Side> for Placement {
    /// Vertical sides are the two an [`Overlay`](crate::overlay::Overlay)
    /// draws directly. A horizontal one has no overlay placement of its own,
    /// so it falls to the side a caller would stack it on.
    fn from(side: Side) -> Self {
        match side {
            Side::Above => Placement::Above,
            _ => Placement::Below,
        }
    }
}

/// What a surface asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Positioner {
    /// Where it would rather be.
    pub side: Side,
    /// Which of the trigger's edges it would rather hang from.
    pub hang: Hang,
    /// How tall it would be if nothing stopped it.
    pub height: f32,
    /// The narrowest it is worth drawing. A surface is at least this wide even
    /// when its trigger is narrower, because a menu the width of a small
    /// button is a column of cut-off words.
    pub min_width: f32,
    /// How far it stays from the window's edge.
    pub margin: f32,
    /// How far it stays from the trigger.
    pub gap: f32,
}

impl Positioner {
    /// A surface `height` tall that would rather be below its trigger.
    pub fn below(height: f32) -> Self {
        Self {
            side: Side::Below,
            hang: Hang::Start,
            height,
            min_width: 0.0,
            margin: 0.0,
            gap: 0.0,
        }
    }

    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Which of the trigger's edges the surface would rather hang from. It
    /// keeps that edge when there is room for it there.
    pub fn hang(mut self, hang: Hang) -> Self {
        self.hang = hang;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// How far the surface stays from the window edge and from its trigger.
    pub fn spacing(mut self, margin: f32, gap: f32) -> Self {
        self.margin = margin;
        self.gap = gap.max(0.0);
        self
    }

    /// Resolves against the window the trigger was measured in.
    pub fn resolve(self, window: &Window, trigger: Bounds<Pixels>) -> Room {
        let viewport = window.viewport_size();
        self.resolve_in(
            f32::from(viewport.width),
            f32::from(viewport.height),
            trigger,
        )
    }

    /// The same decision against a stated window, which is what makes it
    /// testable without one.
    pub fn resolve_in(self, width: f32, height: f32, trigger: Bounds<Pixels>) -> Room {
        let usable_width = (width - self.margin * 2.0).max(0.0);
        let usable_height = (height - self.margin * 2.0).max(0.0);
        let trigger_width = f32::from(trigger.size.width);
        let surface_width = trigger_width.max(self.min_width).min(usable_width);
        let measured = trigger_width > 0.0 && f32::from(trigger.size.height) > 0.0;

        if !measured {
            // Nobody has laid the trigger out, so there is no side to prefer.
            // The surface is bounded by the window and the trigger's prepaint
            // asks for the frame that knows better.
            return Room {
                side: self.side,
                hang: self.hang,
                width: surface_width,
                height: self.height.min((usable_height - self.gap).max(0.0)),
                measured: false,
                flipped: false,
                rehung: false,
            };
        }

        let (near, far) = if self.side.is_vertical() {
            (
                (f32::from(trigger.top()) - self.margin - self.gap).max(0.0),
                (height - self.margin - f32::from(trigger.bottom()) - self.gap).max(0.0),
            )
        } else {
            (
                (f32::from(trigger.left()) - self.margin - self.gap).max(0.0),
                (width - self.margin - f32::from(trigger.right()) - self.gap).max(0.0),
            )
        };
        let (preferred, alternative) = match self.side {
            Side::Above | Side::Before => (near, far),
            Side::Below | Side::After => (far, near),
        };

        // It stays where it was asked to go if it fits there, and otherwise
        // goes wherever there is more room. A surface that fits on neither
        // side is not moved for the sake of moving it.
        let flipped = preferred < self.height && alternative > preferred;
        let available = if flipped { alternative } else { preferred };

        // The same rule sideways. Room from a trigger's leading edge runs to
        // the far side of the window; room from its trailing edge runs back
        // the other way.
        let leading = (width - self.margin - f32::from(trigger.left())).max(0.0);
        let trailing = (f32::from(trigger.right()) - self.margin).max(0.0);
        let (edge, other_edge) = match self.hang {
            Hang::Start => (leading, trailing),
            Hang::End => (trailing, leading),
        };
        let rehung = edge < surface_width && other_edge > edge;

        Room {
            side: if flipped {
                self.side.opposite()
            } else {
                self.side
            },
            hang: if rehung {
                self.hang.opposite()
            } else {
                self.hang
            },
            width: surface_width,
            height: self.height.min(available),
            measured: true,
            flipped,
            rehung,
        }
    }
}

/// Where a surface goes and how much of it there can be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Room {
    /// The side it actually lands on.
    pub side: Side,
    /// The trigger edge it actually hangs from.
    pub hang: Hang,
    /// How wide it is drawn.
    pub width: f32,
    /// The tallest it may be without leaving the window.
    pub height: f32,
    /// Whether the trigger had been laid out when this was decided. A surface
    /// placed against an unmeasured trigger is provisional: it is bounded by
    /// the window rather than by a side, and the next frame corrects it.
    pub measured: bool,
    /// Whether it had to leave the side it asked for.
    pub flipped: bool,
    /// Whether it had to move to the trigger's other edge to stay in the
    /// window.
    pub rehung: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, point, px, size};

    fn trigger(top: f32, height: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(20.0), px(top)),
            size: size(px(200.0), px(height)),
        }
    }

    fn menu() -> Positioner {
        Positioner::below(300.0).min_width(160.0).spacing(8.0, 6.0)
    }

    #[test]
    fn a_menu_with_room_under_its_trigger_opens_there() {
        let room = menu().resolve_in(1000.0, 800.0, trigger(100.0, 32.0));

        assert_eq!(room.side, Side::Below);
        assert!(!room.flipped);
        assert_eq!(room.height, 300.0, "it got everything it asked for");
    }

    #[test]
    fn a_menu_near_the_bottom_flips_above_its_trigger() {
        let room = menu().resolve_in(1000.0, 400.0, trigger(340.0, 32.0));

        assert_eq!(room.side, Side::Above);
        assert!(room.flipped);
        assert_eq!(
            room.height, 300.0,
            "326 above is more than it asked for, so it asked for all of it"
        );
    }

    #[test]
    fn a_trigger_with_room_on_neither_side_keeps_the_side_it_asked_for() {
        // Halfway down a short window: 84 below, 84 above.
        let room = menu().resolve_in(1000.0, 200.0, trigger(84.0, 32.0));

        assert_eq!(
            room.side,
            Side::Below,
            "moving it would not give it more room, so it is not moved"
        );
        assert_eq!(room.height, 70.0);
    }

    #[test]
    fn an_unmeasured_trigger_is_placed_against_the_window_and_says_so() {
        let unmeasured = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(0.0), px(0.0)),
        };
        let room = menu().resolve_in(1000.0, 800.0, unmeasured);

        assert!(!room.measured, "there is nothing to have decided from");
        assert_eq!(room.side, Side::Below);
        assert_eq!(room.height, 300.0);
        assert_eq!(
            room.width, 160.0,
            "a surface with no trigger to match is at least its own minimum"
        );
    }

    #[test]
    fn a_surface_is_at_least_its_minimum_and_never_wider_than_the_window() {
        let narrow = Bounds {
            origin: point(px(0.0), px(10.0)),
            size: size(px(40.0), px(32.0)),
        };
        assert_eq!(menu().resolve_in(1000.0, 800.0, narrow).width, 160.0);

        let wide = Bounds {
            origin: point(px(0.0), px(10.0)),
            size: size(px(900.0), px(32.0)),
        };
        assert_eq!(
            menu().resolve_in(300.0, 800.0, wide).width,
            284.0,
            "the window less both margins"
        );
    }

    #[test]
    fn a_submenu_flips_to_the_other_edge_of_its_trigger() {
        // 86 points of window after the trigger, 686 before it.
        let against_the_right = Bounds {
            origin: point(px(700.0), px(100.0)),
            size: size(px(200.0), px(32.0)),
        };
        let room = Positioner::below(300.0)
            .side(Side::After)
            .spacing(8.0, 6.0)
            .resolve_in(1000.0, 800.0, against_the_right);

        assert_eq!(room.side, Side::Before);
        assert!(room.flipped);
        assert_eq!(room.height, 300.0);
    }

    #[test]
    fn a_menu_with_room_beside_its_trigger_hangs_from_its_leading_edge() {
        let room = menu().resolve_in(1000.0, 800.0, trigger(100.0, 32.0));

        assert_eq!(room.hang, Hang::Start);
        assert!(!room.rehung);
    }

    #[test]
    fn a_menu_wider_than_the_room_after_its_trigger_hangs_from_the_other_edge() {
        // A small chip against the right of a 400-wide window opening a menu
        // that wants 200: 72 points of room from its leading edge, 372 from
        // its trailing one.
        let chip = Bounds {
            origin: point(px(320.0), px(100.0)),
            size: size(px(60.0), px(32.0)),
        };
        let room = Positioner::below(300.0)
            .min_width(200.0)
            .spacing(8.0, 6.0)
            .resolve_in(400.0, 800.0, chip);

        assert_eq!(room.hang, Hang::End);
        assert!(room.rehung);
        assert_eq!(
            room.side,
            Side::Below,
            "which way it opens is a separate question and this did not change it"
        );
    }

    #[test]
    fn a_menu_that_fits_on_neither_edge_keeps_the_one_it_asked_for() {
        // Centred in a window narrower than the surface: neither edge has room
        // and moving it would not find any, so it is not moved.
        let centred = Bounds {
            origin: point(px(20.0), px(100.0)),
            size: size(px(160.0), px(32.0)),
        };
        let room = Positioner::below(300.0)
            .min_width(400.0)
            .spacing(8.0, 6.0)
            .resolve_in(200.0, 800.0, centred);

        assert_eq!(room.hang, Hang::Start);
        assert!(!room.rehung);
    }

    #[test]
    fn an_unmeasured_trigger_keeps_the_edge_it_asked_for_too() {
        let unmeasured = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(0.0), px(0.0)),
        };
        let room = menu().hang(Hang::End).resolve_in(1000.0, 800.0, unmeasured);

        assert_eq!(room.hang, Hang::End);
        assert!(!room.rehung, "nothing has been decided from anything");
    }

    #[test]
    fn every_hang_has_an_opposite_and_it_is_itself_twice_over() {
        for hang in [Hang::Start, Hang::End] {
            assert_eq!(hang.opposite().opposite(), hang);
            assert_ne!(hang.opposite(), hang);
        }
    }

    #[test]
    fn every_side_has_an_opposite_and_it_is_itself_twice_over() {
        for side in [Side::Below, Side::Above, Side::Before, Side::After] {
            assert_eq!(side.opposite().opposite(), side);
            assert_ne!(side.opposite(), side);
        }
    }
}
