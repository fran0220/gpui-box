//! Values that can be sampled part way between two states.

use gpui::{Hsla, Pixels, Point, Rems, Size, px, rems};

/// A value an animation can move through.
///
/// `t` outside 0..1 is meaningful: overshoot curves and underdamped springs
/// deliberately pass their target, so implementations extrapolate rather than
/// clamp.
pub trait Interpolate: Copy {
    fn lerp(self, other: Self, t: f32) -> Self;

    /// How far apart two values are, in one number, so a retarget can rescale
    /// a velocity it is carrying into the new distance.
    fn distance(self, other: Self) -> f32;
}

impl Interpolate for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }

    fn distance(self, other: Self) -> f32 {
        (other - self).abs()
    }
}

impl Interpolate for Pixels {
    fn lerp(self, other: Self, t: f32) -> Self {
        px(f32::from(self).lerp(f32::from(other), t))
    }

    fn distance(self, other: Self) -> f32 {
        f32::from(self).distance(f32::from(other))
    }
}

impl Interpolate for Rems {
    fn lerp(self, other: Self, t: f32) -> Self {
        rems(self.0.lerp(other.0, t))
    }

    fn distance(self, other: Self) -> f32 {
        self.0.distance(other.0)
    }
}

impl Interpolate for Hsla {
    /// Interpolates hue the short way around the wheel, so a red-to-magenta
    /// transition does not sweep through the entire spectrum.
    fn lerp(self, other: Self, t: f32) -> Self {
        Hsla {
            h: (self.h + hue_delta(self.h, other.h) * t).rem_euclid(1.0),
            s: self.s.lerp(other.s, t).clamp(0.0, 1.0),
            l: self.l.lerp(other.l, t).clamp(0.0, 1.0),
            a: self.a.lerp(other.a, t).clamp(0.0, 1.0),
        }
    }

    /// Measured over the same short way around the wheel that `lerp` travels,
    /// so the distance is the distance actually covered.
    fn distance(self, other: Self) -> f32 {
        let hue = hue_delta(self.h, other.h);
        let saturation = other.s - self.s;
        let lightness = other.l - self.l;
        let alpha = other.a - self.a;
        (hue * hue + saturation * saturation + lightness * lightness + alpha * alpha).sqrt()
    }
}

/// The signed hue step from `from` to `to` the short way around the wheel.
fn hue_delta(from: f32, to: f32) -> f32 {
    let delta = to - from;
    if delta > 0.5 {
        delta - 1.0
    } else if delta < -0.5 {
        delta + 1.0
    } else {
        delta
    }
}

impl<T: Interpolate + Clone + std::fmt::Debug + Default + PartialEq> Interpolate for Point<T> {
    fn lerp(self, other: Self, t: f32) -> Self {
        Point {
            x: self.x.lerp(other.x, t),
            y: self.y.lerp(other.y, t),
        }
    }

    fn distance(self, other: Self) -> f32 {
        let x = self.x.distance(other.x);
        let y = self.y.distance(other.y);
        (x * x + y * y).sqrt()
    }
}

impl<T: Interpolate + Clone + std::fmt::Debug + Default + PartialEq> Interpolate for Size<T> {
    fn lerp(self, other: Self, t: f32) -> Self {
        Size {
            width: self.width.lerp(other.width, t),
            height: self.height.lerp(other.height, t),
        }
    }

    fn distance(self, other: Self) -> f32 {
        let width = self.width.distance(other.width);
        let height = self.height.distance(other.height);
        (width * width + height * height).sqrt()
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
    fn distance_is_how_far_a_value_has_to_travel() {
        assert_eq!(2.0f32.distance(10.0), 8.0);
        assert_eq!(10.0f32.distance(2.0), 8.0);
        assert_eq!(px(1.0).distance(px(4.0)), 3.0);
        assert_eq!(rems(1.0).distance(rems(2.5)), 1.5);
        assert_eq!(
            point(px(0.0), px(0.0)).distance(point(px(3.0), px(4.0))),
            5.0
        );
        assert_eq!(
            size(px(0.0), px(0.0)).distance(size(px(6.0), px(8.0))),
            10.0
        );
    }

    #[test]
    fn hue_distance_takes_the_short_way_around_the_wheel() {
        let magenta = hsla(0.9, 0.5, 0.5, 1.0);
        let red = hsla(0.05, 0.5, 0.5, 1.0);
        assert!(
            (magenta.distance(red) - 0.15).abs() < 1e-5,
            "hue took the long way: {}",
            magenta.distance(red)
        );
        assert_eq!(magenta.distance(red), red.distance(magenta));
    }

    #[test]
    fn compound_values_interpolate_component_wise() {
        let moved = point(px(0.0), px(10.0)).lerp(point(px(10.0), px(0.0)), 0.5);
        assert_eq!(moved, point(px(5.0), px(5.0)));
        let grown = size(px(0.0), px(0.0)).lerp(size(px(4.0), px(8.0)), 0.5);
        assert_eq!(grown, size(px(2.0), px(4.0)));
    }
}
