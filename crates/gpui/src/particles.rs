//! Deterministic CPU particle sampling for atlas-backed sprite batches.
//!
//! Emitters describe bounded, product-neutral motion. Sampling is a pure
//! function of the emitter seed and absolute elapsed time, so dropped frames
//! do not change trajectories and replay/headless rendering can choose an
//! exact instant. The renderer still receives one ordinary sprite batch; this
//! module does not create one element or texture upload per particle.

use anyhow::{Result, anyhow, ensure};
use std::time::Duration;

use crate::{
    Bounds, Corners, DevicePixels, Hsla, Pixels, Point, Radians, Size, SpriteBlendMode,
    SpriteColorMode, SpriteInstance, SpriteTransform, bounds, point, px, size, white,
};

/// Hard ceiling for one sampled particle batch.
///
/// Hosts that need more concurrent work should split it across the semantic
/// effect budget rather than allocating an unbounded temporary instance list.
pub const MAX_PARTICLES_PER_BATCH: u32 = 4_096;

/// One deterministic emitter sampled into atlas-backed sprite instances.
///
/// Coordinates are logical window pixels. Speed is pixels per second and
/// acceleration is pixels per second squared. All particles share one source
/// rectangle and paint mode while seed-derived values vary spawn position,
/// direction, speed, size, and initial rotation.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleEmitter {
    seed: u64,
    source: Bounds<DevicePixels>,
    origin: Point<Pixels>,
    spawn_area: Size<Pixels>,
    count: u32,
    emission_span: Duration,
    lifetime: Duration,
    direction: Radians,
    spread: Radians,
    speed_min: f32,
    speed_max: f32,
    acceleration: Point<f32>,
    start_size: Size<Pixels>,
    end_size: Size<Pixels>,
    size_variation: f32,
    initial_rotation: Radians,
    rotation_spread: Radians,
    angular_velocity: Radians,
    fade_in: f32,
    fade_out: f32,
    opacity: f32,
    corner_radii: Corners<Pixels>,
    color_mode: SpriteColorMode,
    blend_mode: SpriteBlendMode,
    tint: Hsla,
}

impl ParticleEmitter {
    /// Creates a bounded burst from `origin` using one image source rectangle.
    ///
    /// The default motion emits in all directions for 800 ms with normal color
    /// and source-over compositing. Builders change geometry and paint policy;
    /// invalid or oversized emitters are reported atomically by
    /// [`crate::Window::paint_particle_batch`].
    pub fn new(seed: u64, source: Bounds<DevicePixels>, origin: Point<Pixels>, count: u32) -> Self {
        Self {
            seed,
            source,
            origin,
            spawn_area: Size::default(),
            count,
            emission_span: Duration::ZERO,
            lifetime: Duration::from_millis(800),
            direction: Radians::default(),
            spread: Radians(std::f32::consts::TAU),
            speed_min: 24.0,
            speed_max: 72.0,
            acceleration: Point::default(),
            start_size: size(px(8.0), px(8.0)),
            end_size: size(px(2.0), px(2.0)),
            size_variation: 0.25,
            initial_rotation: Radians::default(),
            rotation_spread: Radians(std::f32::consts::TAU),
            angular_velocity: Radians::default(),
            fade_in: 0.0,
            fade_out: 0.3,
            opacity: 1.0,
            corner_radii: Corners::default(),
            color_mode: SpriteColorMode::Color,
            blend_mode: SpriteBlendMode::Normal,
            tint: white(),
        }
    }

    /// Spreads spawn points uniformly across a rectangle centered on `origin`.
    pub fn spawn_area(mut self, area: Size<Pixels>) -> Self {
        self.spawn_area = area;
        self
    }

    /// Distributes births over `span`; zero makes every particle a burst.
    pub fn emission_span(mut self, span: Duration) -> Self {
        self.emission_span = span;
        self
    }

    /// Sets how long each particle remains alive after its own birth.
    pub fn lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }

    /// Sets the central direction and total angular spread in radians.
    pub fn direction(mut self, direction: Radians, spread: Radians) -> Self {
        self.direction = direction;
        self.spread = spread;
        self
    }

    /// Sets the inclusive initial speed range in logical pixels per second.
    pub fn speed(mut self, min: f32, max: f32) -> Self {
        self.speed_min = min;
        self.speed_max = max;
        self
    }

    /// Sets constant acceleration in logical pixels per second squared.
    pub fn acceleration(mut self, acceleration: Point<f32>) -> Self {
        self.acceleration = acceleration;
        self
    }

    /// Sets the destination size at birth and at the end of the lifetime.
    pub fn size(mut self, start: Size<Pixels>, end: Size<Pixels>) -> Self {
        self.start_size = start;
        self.end_size = end;
        self
    }

    /// Varies both dimensions by up to `fraction` around their sampled size.
    ///
    /// Valid fractions are in `0.0..=1.0`.
    pub fn size_variation(mut self, fraction: f32) -> Self {
        self.size_variation = fraction;
        self
    }

    /// Sets initial rotation, initial random spread, and radians per second.
    pub fn rotation(
        mut self,
        initial: Radians,
        spread: Radians,
        angular_velocity: Radians,
    ) -> Self {
        self.initial_rotation = initial;
        self.rotation_spread = spread;
        self.angular_velocity = angular_velocity;
        self
    }

    /// Sets fade-in and fade-out shares of each particle lifetime.
    ///
    /// Each share is in `0.0..=1.0`; overlapping fades multiply.
    pub fn fade(mut self, fade_in: f32, fade_out: f32) -> Self {
        self.fade_in = fade_in;
        self.fade_out = fade_out;
        self
    }

    /// Sets additional emitter opacity, clamped while painting.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Sets the rounded mask applied to each sampled destination.
    pub fn corner_radii(mut self, corner_radii: Corners<Pixels>) -> Self {
        self.corner_radii = corner_radii;
        self
    }

    /// Sets sample-to-color conversion and the alpha-mask tint.
    pub fn color_mode(mut self, color_mode: SpriteColorMode, tint: Hsla) -> Self {
        self.color_mode = color_mode;
        self.tint = tint;
        self
    }

    /// Sets the fixed-function compositing equation for this emitter.
    pub fn blend_mode(mut self, blend_mode: SpriteBlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    /// Returns the number of deterministic particle slots in this emitter.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Returns the time from the first birth until the final particle expires.
    pub fn total_duration(&self) -> Duration {
        self.emission_span.saturating_add(self.lifetime)
    }

    pub(crate) fn source(&self) -> Bounds<DevicePixels> {
        self.source
    }

    fn validate(&self, index: usize) -> Result<()> {
        let finite_pixels = |value: Pixels| value.0.is_finite();
        ensure!(
            self.origin.x.0.is_finite() && self.origin.y.0.is_finite(),
            "particle emitter {index} origin must be finite"
        );
        ensure!(
            finite_pixels(self.spawn_area.width)
                && finite_pixels(self.spawn_area.height)
                && self.spawn_area.width >= Pixels::ZERO
                && self.spawn_area.height >= Pixels::ZERO,
            "particle emitter {index} spawn area must be finite and non-negative"
        );
        ensure!(
            !self.lifetime.is_zero(),
            "particle emitter {index} lifetime must be positive"
        );
        ensure!(
            self.direction.0.is_finite() && self.spread.0.is_finite() && self.spread.0 >= 0.0,
            "particle emitter {index} direction and spread must be finite with non-negative spread"
        );
        ensure!(
            self.speed_min.is_finite()
                && self.speed_max.is_finite()
                && self.speed_min >= 0.0
                && self.speed_max >= self.speed_min,
            "particle emitter {index} speed range must be finite, ordered, and non-negative"
        );
        ensure!(
            self.acceleration.x.is_finite() && self.acceleration.y.is_finite(),
            "particle emitter {index} acceleration must be finite"
        );
        ensure!(
            [
                self.start_size.width,
                self.start_size.height,
                self.end_size.width,
                self.end_size.height,
            ]
            .into_iter()
            .all(|value| finite_pixels(value) && value > Pixels::ZERO),
            "particle emitter {index} sizes must be finite and positive"
        );
        ensure!(
            self.size_variation.is_finite() && (0.0..=1.0).contains(&self.size_variation),
            "particle emitter {index} size variation must be in 0..=1"
        );
        ensure!(
            self.initial_rotation.0.is_finite()
                && self.rotation_spread.0.is_finite()
                && self.rotation_spread.0 >= 0.0
                && self.angular_velocity.0.is_finite(),
            "particle emitter {index} rotation values must be finite with non-negative spread"
        );
        ensure!(
            self.fade_in.is_finite()
                && self.fade_out.is_finite()
                && (0.0..=1.0).contains(&self.fade_in)
                && (0.0..=1.0).contains(&self.fade_out),
            "particle emitter {index} fade shares must be in 0..=1"
        );
        ensure!(
            self.opacity.is_finite(),
            "particle emitter {index} opacity must be finite"
        );
        ensure!(
            [
                self.corner_radii.top_left,
                self.corner_radii.top_right,
                self.corner_radii.bottom_right,
                self.corner_radii.bottom_left,
            ]
            .into_iter()
            .all(|radius| finite_pixels(radius) && radius >= Pixels::ZERO),
            "particle emitter {index} corner radii must be finite and non-negative"
        );
        ensure!(
            [self.tint.h, self.tint.s, self.tint.l, self.tint.a]
                .into_iter()
                .all(f32::is_finite),
            "particle emitter {index} tint must be finite"
        );
        Ok(())
    }
}

pub(crate) fn sample_particle_emitters(
    emitters: &[ParticleEmitter],
    elapsed: Duration,
) -> Result<Vec<SpriteInstance>> {
    let total = emitters.iter().try_fold(0u32, |total, emitter| {
        total
            .checked_add(emitter.count)
            .ok_or_else(|| anyhow!("particle batch count overflow"))
    })?;
    ensure!(
        total <= MAX_PARTICLES_PER_BATCH,
        "particle batch requests {total} slots, maximum is {MAX_PARTICLES_PER_BATCH}"
    );
    for (index, emitter) in emitters.iter().enumerate() {
        emitter.validate(index)?;
    }

    let elapsed = elapsed.as_secs_f32();
    let mut instances = Vec::with_capacity(total as usize);
    for emitter in emitters {
        let lifetime = emitter.lifetime.as_secs_f32();
        let span = emitter.emission_span.as_secs_f32();
        for particle in 0..emitter.count {
            let birth = if particle == 0 || span == 0.0 {
                0.0
            } else {
                let slot = particle as f32 / emitter.count as f32;
                let jitter =
                    (random_unit(emitter.seed, particle, 0) - 0.5) * (0.5 / emitter.count as f32);
                span * (slot + jitter).clamp(0.0, 1.0)
            };
            let age = elapsed - birth;
            if age < 0.0 || age >= lifetime {
                continue;
            }
            let progress = (age / lifetime).clamp(0.0, 1.0);
            let spawn_x = emitter.origin.x.0
                + (random_unit(emitter.seed, particle, 1) - 0.5) * emitter.spawn_area.width.0;
            let spawn_y = emitter.origin.y.0
                + (random_unit(emitter.seed, particle, 2) - 0.5) * emitter.spawn_area.height.0;
            let angle = emitter.direction.0
                + (random_unit(emitter.seed, particle, 3) - 0.5) * emitter.spread.0;
            let speed = mix(
                emitter.speed_min,
                emitter.speed_max,
                random_unit(emitter.seed, particle, 4),
            );
            let position = point(
                px(spawn_x + angle.cos() * speed * age + 0.5 * emitter.acceleration.x * age * age),
                px(spawn_y + angle.sin() * speed * age + 0.5 * emitter.acceleration.y * age * age),
            );
            let variation =
                1.0 + (random_unit(emitter.seed, particle, 5) - 0.5) * 2.0 * emitter.size_variation;
            let sampled_size = size(
                px(mix(
                    emitter.start_size.width.0,
                    emitter.end_size.width.0,
                    progress,
                ) * variation),
                px(mix(
                    emitter.start_size.height.0,
                    emitter.end_size.height.0,
                    progress,
                ) * variation),
            );
            let fade_in = if emitter.fade_in > 0.0 {
                (progress / emitter.fade_in).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let fade_out = if emitter.fade_out > 0.0 {
                ((1.0 - progress) / emitter.fade_out).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let opacity = emitter.opacity.clamp(0.0, 1.0) * fade_in * fade_out;
            if opacity <= 0.0 {
                continue;
            }
            let rotation = emitter.initial_rotation.0
                + (random_unit(emitter.seed, particle, 6) - 0.5) * emitter.rotation_spread.0
                + emitter.angular_velocity.0 * age;
            instances.push(
                SpriteInstance::new(
                    bounds(
                        point(
                            position.x - sampled_size.width / 2.0,
                            position.y - sampled_size.height / 2.0,
                        ),
                        sampled_size,
                    ),
                    emitter.source,
                )
                .transform(SpriteTransform::identity().rotate(Radians(rotation)))
                .corner_radii(emitter.corner_radii)
                .opacity(opacity)
                .color_mode(emitter.color_mode, emitter.tint)
                .blend_mode(emitter.blend_mode),
            );
        }
    }
    Ok(instances)
}

fn random_unit(seed: u64, particle: u32, lane: u64) -> f32 {
    let mut value = seed
        ^ u64::from(particle).wrapping_mul(0x9e3779b97f4a7c15)
        ^ lane.wrapping_mul(0xbf58476d1ce4e5b9);
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 31;
    ((value >> 40) as u32) as f32 / 16_777_216.0
}

fn mix(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Bounds<DevicePixels> {
        bounds(
            point(DevicePixels(0), DevicePixels(0)),
            size(DevicePixels(16), DevicePixels(16)),
        )
    }

    fn emitter(seed: u64) -> ParticleEmitter {
        ParticleEmitter::new(seed, source(), point(px(100.0), px(80.0)), 24)
            .speed(18.0, 72.0)
            .acceleration(point(0.0, 60.0))
            .size(size(px(12.0), px(8.0)), size(px(3.0), px(2.0)))
            .rotation(Radians(0.2), Radians(1.4), Radians(0.8))
            .fade(0.1, 0.35)
    }

    #[test]
    fn a_seed_and_absolute_time_produce_the_same_instances() {
        let emitter = emitter(7);
        let elapsed = Duration::from_millis(320);
        let first = sample_particle_emitters(std::slice::from_ref(&emitter), elapsed)
            .expect("valid emitter samples");
        let second = sample_particle_emitters(&[emitter], elapsed).expect("same emitter samples");
        assert_eq!(first, second);
        assert_eq!(first.len(), 24);
    }

    #[test]
    fn different_seeds_change_geometry_without_changing_the_slot_count() {
        let elapsed = Duration::from_millis(320);
        let first = sample_particle_emitters(&[emitter(7)], elapsed)
            .expect("the first bounded emitter samples");
        let second = sample_particle_emitters(&[emitter(8)], elapsed)
            .expect("the second bounded emitter samples");
        assert_eq!(first.len(), second.len());
        assert_ne!(first, second);
    }

    #[test]
    fn streaming_births_and_expiry_follow_absolute_time() {
        let emitter = ParticleEmitter::new(9, source(), point(px(0.0), px(0.0)), 8)
            .emission_span(Duration::from_millis(800))
            .lifetime(Duration::from_millis(400));
        let first =
            sample_particle_emitters(std::slice::from_ref(&emitter), Duration::from_millis(10))
                .expect("the initial stream sample is valid");
        let middle =
            sample_particle_emitters(std::slice::from_ref(&emitter), Duration::from_millis(600))
                .expect("the middle stream sample is valid");
        let finished = sample_particle_emitters(&[emitter], Duration::from_millis(1_200))
            .expect("the finished stream sample is valid");
        assert_eq!(first.len(), 1);
        assert!(!middle.is_empty());
        assert!(finished.is_empty());
    }

    #[test]
    fn invalid_and_unbounded_emitters_are_reported_before_sampling() {
        let invalid = emitter(1).speed(8.0, 2.0);
        assert!(sample_particle_emitters(&[invalid], Duration::ZERO).is_err());

        let excessive = ParticleEmitter::new(
            1,
            source(),
            point(px(0.0), px(0.0)),
            MAX_PARTICLES_PER_BATCH + 1,
        );
        assert!(sample_particle_emitters(&[excessive], Duration::ZERO).is_err());
    }

    #[test]
    fn the_integer_rng_is_stable_and_stays_below_one() {
        assert_eq!(random_unit(7, 3, 2).to_bits(), 1_037_935_008);
        for particle in 0..128 {
            let value = random_unit(99, particle, 6);
            assert!((0.0..1.0).contains(&value));
        }
    }
}
