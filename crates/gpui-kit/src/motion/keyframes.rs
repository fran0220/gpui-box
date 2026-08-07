//! A value that travels through named stops instead of straight from one end
//! to the other.

use gpui_kit_theme::Theme;

use super::easing::{CubicBezier, Easing};
use super::interpolate::Interpolate;
use super::spec::MotionSpec;

/// One stop on the path: the value, where along the run it is reached, and
/// optionally the curve it is reached on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe<T> {
    offset: f32,
    value: T,
    easing: Option<Easing>,
}

impl<T> Keyframe<T> {
    /// `offset` is a fraction of the run and is clamped into 0..=1.
    pub fn new(offset: f32, value: T) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            value,
            easing: None,
        }
    }

    /// The curve the value travels on to reach this stop, rather than the
    /// specification's.
    pub fn eased(mut self, easing: impl Into<Easing>) -> Self {
        self.easing = Some(easing.into());
        self
    }
}

/// A stop with its curve already resolved against the theme, because
/// [`Keyframes::sample`] has no theme to resolve one against.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stop<T> {
    offset: f32,
    value: T,
    curve: CubicBezier,
}

/// A value that passes through named stops rather than travelling straight
/// from one end to the other.
///
/// The stops are the whole path: sampling outside 0..=1 clamps, because an
/// author who wrote where a value goes did not write what is past the last
/// place they put it.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes<T: Interpolate> {
    stops: Vec<Stop<T>>,
}

impl<T: Interpolate> Keyframes<T> {
    /// Builds the path from stops given in any order.
    ///
    /// `None` for an empty list: a path through no stops has no value to
    /// report, and every other answer would be invented.
    pub fn new(
        theme: &Theme,
        spec: MotionSpec,
        stops: impl IntoIterator<Item = Keyframe<T>>,
    ) -> Option<Self> {
        let mut stops: Vec<Stop<T>> = stops
            .into_iter()
            .map(|stop| Stop {
                offset: stop.offset,
                value: stop.value,
                curve: match stop.easing {
                    Some(easing) => easing.curve(theme),
                    None => spec.curve,
                },
            })
            .collect();
        if stops.is_empty() {
            return None;
        }
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        Some(Self { stops })
    }

    /// The value at `progress`.
    ///
    /// A path that does not start at 0 or reach 1 holds its nearest stop out
    /// to that end, so an author can describe only the part that moves.
    pub fn sample(&self, progress: f32) -> T {
        let progress = progress.clamp(0.0, 1.0);
        let first = self.stops.first().expect("a keyframe list is never empty");
        if progress <= first.offset {
            return first.value;
        }
        let last = self.stops.last().expect("a keyframe list is never empty");
        if progress >= last.offset {
            return last.value;
        }
        let reached = self
            .stops
            .iter()
            .position(|stop| stop.offset >= progress)
            .unwrap_or(self.stops.len() - 1)
            .max(1);
        let from = &self.stops[reached - 1];
        let to = &self.stops[reached];
        let span = to.offset - from.offset;
        if span <= 0.0 {
            return to.value;
        }
        from.value
            .lerp(to.value, to.curve.eval((progress - from.offset) / span))
    }

    /// Where the stops sit along the run, in order.
    pub fn offsets(&self) -> Vec<f32> {
        self.stops.iter().map(|stop| stop.offset).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> MotionSpec {
        MotionSpec::new(200, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
    }

    fn keyframes(stops: Vec<Keyframe<f32>>) -> Keyframes<f32> {
        Keyframes::new(&Theme::studio_dark(), spec(), stops).expect("stops were given")
    }

    #[test]
    fn stops_are_ordered_however_they_were_given() {
        let path = keyframes(vec![
            Keyframe::new(1.0, 10.0),
            Keyframe::new(0.0, 0.0),
            Keyframe::new(0.5, 4.0),
        ]);
        assert_eq!(path.offsets(), vec![0.0, 0.5, 1.0]);
        assert!((path.sample(0.5) - 4.0).abs() < 1e-4);
        assert!((path.sample(0.25) - 2.0).abs() < 1e-3);
    }

    #[test]
    fn an_empty_path_is_not_a_path() {
        assert!(Keyframes::<f32>::new(&Theme::studio_dark(), spec(), []).is_none());
    }

    #[test]
    fn the_ends_extend_to_the_stops_that_were_written() {
        let path = keyframes(vec![Keyframe::new(0.25, 2.0), Keyframe::new(0.75, 6.0)]);
        assert_eq!(path.sample(0.0), 2.0);
        assert_eq!(path.sample(0.1), 2.0);
        assert_eq!(path.sample(1.0), 6.0);
    }

    #[test]
    fn sampling_outside_the_run_clamps_to_the_ends() {
        let path = keyframes(vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)]);
        assert_eq!(path.sample(-2.0), 0.0);
        assert_eq!(path.sample(4.0), 10.0);
    }

    #[test]
    fn a_stop_travels_on_its_own_curve() {
        let stops = |easing: Option<Easing>| {
            let reached = Keyframe::new(1.0, 10.0);
            vec![
                Keyframe::new(0.0, 0.0),
                match easing {
                    Some(easing) => reached.eased(easing),
                    None => reached,
                },
            ]
        };
        let linear = keyframes(stops(None));
        let eased = keyframes(stops(Some(Easing::Custom(CubicBezier::new(
            0.9, 0.0, 1.0, 0.4,
        )))));
        assert!(
            eased.sample(0.5) < linear.sample(0.5),
            "a slow-starting curve must lag the specification's, {} against {}",
            eased.sample(0.5),
            linear.sample(0.5)
        );
    }

    #[test]
    fn a_single_stop_holds_for_the_whole_run() {
        let path = keyframes(vec![Keyframe::new(0.5, 3.0)]);
        assert_eq!(path.sample(0.0), 3.0);
        assert_eq!(path.sample(1.0), 3.0);
    }
}
