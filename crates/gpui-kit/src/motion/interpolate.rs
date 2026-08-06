//! Values that can be sampled part way between two states.

use gpui::{Hsla, Pixels, Point, Rems, Size, px, rems};

/// A value an animation can move through.
///
/// `t` outside 0..1 is meaningful: overshoot curves and underdamped springs
/// deliberately pass their target, so implementations extrapolate rather than
/// clamp.
pub trait Interpolate: Copy {
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl Interpolate for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Interpolate for Pixels {
    fn lerp(self, other: Self, t: f32) -> Self {
        px(f32::from(self).lerp(f32::from(other), t))
    }
}

impl Interpolate for Rems {
    fn lerp(self, other: Self, t: f32) -> Self {
        rems(self.0.lerp(other.0, t))
    }
}

impl Interpolate for Hsla {
    /// Interpolates hue the short way around the wheel, so a red-to-magenta
    /// transition does not sweep through the entire spectrum.
    fn lerp(self, other: Self, t: f32) -> Self {
        let mut delta = other.h - self.h;
        if delta > 0.5 {
            delta -= 1.0;
        } else if delta < -0.5 {
            delta += 1.0;
        }
        Hsla {
            h: (self.h + delta * t).rem_euclid(1.0),
            s: self.s.lerp(other.s, t).clamp(0.0, 1.0),
            l: self.l.lerp(other.l, t).clamp(0.0, 1.0),
            a: self.a.lerp(other.a, t).clamp(0.0, 1.0),
        }
    }
}

impl<T: Interpolate + Clone + std::fmt::Debug + Default + PartialEq> Interpolate for Point<T> {
    fn lerp(self, other: Self, t: f32) -> Self {
        Point {
            x: self.x.lerp(other.x, t),
            y: self.y.lerp(other.y, t),
        }
    }
}

impl<T: Interpolate + Clone + std::fmt::Debug + Default + PartialEq> Interpolate for Size<T> {
    fn lerp(self, other: Self, t: f32) -> Self {
        Size {
            width: self.width.lerp(other.width, t),
            height: self.height.lerp(other.height, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{hsla, point, size};

    #[test]
    fn endpoints_are_exact() {
        assert_eq!(2.0f32.lerp(10.0, 0.0), 2.0);
        assert_eq!(2.0f32.lerp(10.0, 1.0), 10.0);
        assert_eq!(px(0.0).lerp(px(8.0), 0.5), px(4.0));
    }

    #[test]
    fn overshoot_extrapolates_instead_of_clamping() {
        assert_eq!(0.0f32.lerp(10.0, 1.2), 12.0);
    }

    #[test]
    fn hue_takes_the_short_way_around_the_wheel() {
        let magenta = hsla(0.9, 1.0, 0.5, 1.0);
        let red = hsla(0.05, 1.0, 0.5, 1.0);
        let middle = magenta.lerp(red, 0.5);
        // The short path wraps past 1.0 rather than sweeping back through green.
        assert!(
            middle.h > 0.9 || middle.h < 0.05,
            "hue took the long way: {middle:?}"
        );
    }

    #[test]
    fn color_channels_stay_in_range_under_overshoot() {
        let from = hsla(0.0, 0.2, 0.2, 0.4);
        let to = hsla(0.1, 0.9, 0.9, 1.0);
        let past = from.lerp(to, 1.4);
        assert!((0.0..=1.0).contains(&past.s));
        assert!((0.0..=1.0).contains(&past.l));
        assert!((0.0..=1.0).contains(&past.a));
    }

    #[test]
    fn compound_values_interpolate_component_wise() {
        let moved = point(px(0.0), px(10.0)).lerp(point(px(10.0), px(0.0)), 0.5);
        assert_eq!(moved, point(px(5.0), px(5.0)));
        let grown = size(px(0.0), px(0.0)).lerp(size(px(4.0), px(8.0)), 0.5);
        assert_eq!(grown, size(px(2.0), px(4.0)));
    }
}
