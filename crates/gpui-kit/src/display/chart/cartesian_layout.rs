//! One logical-to-screen mapping shared by paint, bounds and controlled input.
use super::ChartOrientation;
use gpui::{Bounds, Pixels, Point, point, px};

impl ChartOrientation {
    pub(super) fn screen(self, x: f64, y: f64) -> [f64; 2] {
        match self {
            Self::Vertical => [x, 1. - y],
            Self::Horizontal => [y, x],
        }
    }
    pub(super) fn at(self, bounds: Bounds<Pixels>, x: f64, y: f64) -> Point<Pixels> {
        let [x, y] = self.screen(x, y);
        point(
            bounds.origin.x + px((x * f64::from(f32::from(bounds.size.width))) as f32),
            bounds.origin.y + px((y * f64::from(f32::from(bounds.size.height))) as f32),
        )
    }
    pub(super) fn fraction(self, bounds: Bounds<Pixels>, position: Point<Pixels>) -> f64 {
        match self {
            Self::Vertical => {
                f64::from(f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width))
            }
            Self::Horizontal => {
                f64::from(f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height))
            }
        }
    }
    pub(super) fn dimensions(self, width: f32, height: f32) -> [f32; 2] {
        match self {
            Self::Vertical => [width, height],
            Self::Horizontal => [height, width],
        }
    }
    pub(super) fn rect(self, [left, low, right, high]: [f64; 4]) -> [f64; 4] {
        match self {
            Self::Vertical => [left, 1. - high, right, 1. - low],
            Self::Horizontal => [low, left, high, right],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asymmetric_orientation_uses_same_coordinates_for_bounds_and_input() {
        let bounds = Bounds::new(point(px(17.), px(43.)), gpui::size(px(270.), px(130.)));
        for orientation in [ChartOrientation::Vertical, ChartOrientation::Horizontal] {
            let p = orientation.at(bounds, 0.23, 0.81);
            assert!((orientation.fraction(bounds, p) - 0.23).abs() < 1e-6);
        }
        assert_eq!(
            ChartOrientation::Horizontal.rect([0.13, 0.27, 0.61, 0.89]),
            [0.27, 0.13, 0.89, 0.61]
        );
        assert_eq!(
            ChartOrientation::Horizontal.dimensions(270., 130.),
            [130., 270.]
        );
        assert_eq!(
            ChartOrientation::Horizontal.at(bounds, 0., 1.),
            point(px(287.), px(43.))
        );
    }
}
