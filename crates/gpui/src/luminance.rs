//! CPU ownership of the fixed-size backdrop luminance readback resources.

use parking_lot::Mutex;

use crate::{MAX_LUMINANCE_PROBES, NO_LUMINANCE_PROBE};

const SLOT_BITS: u32 = 4;
const MAX_GENERATION: u32 = u32::MAX >> SLOT_BITS;
const _: () = assert!(MAX_LUMINANCE_PROBES == 1 << SLOT_BITS);

/// Statistics of the five optical-source texels copied by a glass probe:
/// center and diagonal quarter points, after scattering when enabled and
/// before glass compositing. These are encoded RGB measurements, not linear
/// light, exhaustive backdrop extrema, or exhaustive backdrop variance.
/// Alpha is ignored: stored RGB is neither unpremultiplied nor alpha-weighted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropStatistics {
    /// Arithmetic mean of stored encoded red, green, and blue, in `0..=1`.
    pub mean_rgb: [f32; 3],
    /// Mean encoded luminance; identical to the legacy probe reading.
    pub mean_luminance: f32,
    /// Minimum encoded luminance among the five sampled texels only.
    pub min_luminance: f32,
    /// Maximum encoded luminance among the five sampled texels only.
    pub max_luminance: f32,
    /// Population variance of the five encoded luminances (divisor five).
    pub luminance_variance: f32,
}

impl BackdropStatistics {
    /// Decode five RGBA8 or BGRA8 texels at `stride` byte intervals.
    /// The caller supplies the slice beginning at the probe's first texel;
    /// row padding and other probes are never included in the measurement.
    /// Panics if the slice cannot contain all five texels or stride is below four.
    pub fn from_encoded_texels(data: &[u8], stride: usize, bgra: bool) -> Self {
        assert!(stride >= 4);
        let mut mean_rgb = [0.0; 3];
        let mut luminances = [0.0; crate::LUMINANCE_PROBE_SAMPLES];
        let mut total = 0.0;
        for (index, luminance) in luminances.iter_mut().enumerate() {
            let texel = &data[index * stride..index * stride + 4];
            let rgb = if bgra {
                [texel[2], texel[1], texel[0]]
            } else {
                [texel[0], texel[1], texel[2]]
            }
            .map(|channel| channel as f32 / 255.0);
            for (mean, channel) in mean_rgb.iter_mut().zip(rgb) {
                *mean += channel;
            }
            *luminance = crate::probe_sample_luminance(rgb[0], rgb[1], rgb[2]);
            // Preserve the legacy per-texel arithmetic and accumulation order.
            total += *luminance;
        }
        let count = crate::LUMINANCE_PROBE_SAMPLES as f32;
        let mean_luminance = total / count;
        Self {
            mean_rgb: mean_rgb.map(|total| total / count),
            mean_luminance,
            min_luminance: luminances.into_iter().fold(f32::INFINITY, f32::min),
            max_luminance: luminances.into_iter().fold(f32::NEG_INFINITY, f32::max),
            luminance_variance: luminances
                .into_iter()
                .map(|value| (value - mean_luminance).powi(2))
                .sum::<f32>()
                / count,
        }
    }
}

#[derive(Default)]
struct ProbeAllocator {
    claimed: u32,
    next_generation: [u32; MAX_LUMINANCE_PROBES],
}

impl ProbeAllocator {
    fn acquire(&mut self) -> Option<u32> {
        for slot in 0..MAX_LUMINANCE_PROBES {
            let bit = 1 << slot;
            if self.claimed & bit != 0 {
                continue;
            }
            self.claimed |= bit;
            let generation = self.next_generation[slot];
            // Leave an exhausted slot permanently claimed (retired). Neither
            // wraparound nor the reserved all-ones sentinel may issue an ID.
            if generation > MAX_GENERATION {
                continue;
            }
            let id = (generation << SLOT_BITS) | slot as u32;
            if id == NO_LUMINANCE_PROBE {
                continue;
            }
            self.next_generation[slot] += 1;
            return Some(id);
        }
        None
    }

    fn release(&mut self, id: u32) {
        let slot = luminance_probe_slot(id).expect("a lease owns a valid probe ID");
        self.claimed &= !(1 << slot);
    }
}

static PROBE_ALLOCATOR: Mutex<ProbeAllocator> = Mutex::new(ProbeAllocator {
    claimed: 0,
    next_generation: [0; MAX_LUMINANCE_PROBES],
});

/// A lazy, exclusive claim on one backdrop luminance readback slot.
///
/// Pass [`Self::id`] to [`crate::GlassMaterial::probe`] and
/// [`crate::Window::backdrop_luminance`], retaining the lease while using it.
/// IDs are opaque: their low **4 bits** address a physical GPU slot and their
/// high **28 bits** are a per-slot generation. Reacquisition advances that
/// generation, so a new owner cannot read an old owner's cached sample.
///
/// Generations never wrap. At exhaustion the slot is permanently retired;
/// the all-ones [`NO_LUMINANCE_PROBE`] ID is never issued. Exhaustion therefore
/// makes probes unavailable instead of aliasing an old ID. Allocation remains
/// process-wide and conservative across windows, as each renderer has its own
/// cache. The ID is valid only while its lease is alive.
#[derive(Debug, Default)]
pub struct LuminanceProbeLease(Option<u32>);

impl LuminanceProbeLease {
    /// Return this lease's opaque ID, claiming a free slot on first use.
    /// Return `None` when all physical slots are leased or retired; a later
    /// call retries if the lease has not acquired a slot yet.
    pub fn id(&mut self) -> Option<u32> {
        if self.0.is_none() {
            self.0 = PROBE_ALLOCATOR.lock().acquire();
        }
        self.0
    }
}

impl Drop for LuminanceProbeLease {
    fn drop(&mut self) {
        if let Some(id) = self.0 {
            PROBE_ALLOCATOR.lock().release(id);
        }
    }
}

/// Decode only the physical readback index, never the cache's ownership key.
/// The no-probe sentinel has no slot. GPU texture/buffer sizes remain fixed.
pub fn luminance_probe_slot(id: u32) -> Option<usize> {
    (id != NO_LUMINANCE_PROBE).then_some((id & ((1 << SLOT_BITS) - 1)) as usize)
}

#[derive(Clone, Copy, Default)]
struct CachedProbe {
    id: Option<u32>,
    value: Option<f32>,
    statistics: Option<BackdropStatistics>,
    activated_at: u64,
    updated_at: u64,
}

/// Generation- and submission-aware CPU cache shared by all glass renderers.
///
/// Each frame registers only probes actually encoded for admitted surfaces.
/// Missing probes immediately read `None`, including surfaces painted through
/// an opaque budget fallback. Completion callbacks carry the full lease ID
/// and submission sequence, not merely the physical GPU slot.
#[derive(Clone, Copy, Default)]
pub struct LuminanceProbeCache {
    frame: u64,
    slots: [CachedProbe; MAX_LUMINANCE_PROBES],
}

impl LuminanceProbeCache {
    /// Register a frame's encoded probe IDs, including an empty frame, and
    /// return the sequence to capture with its GPU completion. Continuous
    /// owners keep their latest completed value without waiting for the GPU.
    pub fn begin_frame(&mut self, probes: impl IntoIterator<Item = u32>) -> u64 {
        self.frame += 1;
        let mut active = [None; MAX_LUMINANCE_PROBES];
        for id in probes {
            if let Some(slot) = luminance_probe_slot(id) {
                active[slot] = Some(id);
            }
        }
        for (slot, id) in self.slots.iter_mut().zip(active) {
            if slot.id != id {
                *slot = CachedProbe {
                    id,
                    activated_at: self.frame,
                    ..CachedProbe::default()
                };
            }
        }
        self.frame
    }

    /// Publish a completed sample only for its still-active owner and
    /// activation. Late older submissions cannot overwrite a newer sample;
    /// an older frame of the same continuously active owner remains usable.
    pub fn publish(&mut self, frame: u64, id: u32, luminance: f32) {
        self.publish_value(frame, id, luminance, None);
    }

    /// Publish statistics atomically with their legacy mean, using the same
    /// owner, activation, and completion-order checks as scalar readings.
    pub fn publish_statistics(&mut self, frame: u64, id: u32, statistics: BackdropStatistics) {
        self.publish_value(frame, id, statistics.mean_luminance, Some(statistics));
    }

    fn publish_value(
        &mut self,
        frame: u64,
        id: u32,
        luminance: f32,
        statistics: Option<BackdropStatistics>,
    ) {
        let Some(index) = luminance_probe_slot(id) else {
            return;
        };
        let slot = &mut self.slots[index];
        if slot.id == Some(id) && frame >= slot.activated_at && frame >= slot.updated_at {
            slot.value = Some(luminance);
            slot.statistics = statistics;
            slot.updated_at = frame;
        }
    }

    /// Latest verified value for this exact active lease, or `None` before
    /// its first completion, after replacement, or when not submitted.
    pub fn get(&self, id: u32) -> Option<f32> {
        let slot = &self.slots[luminance_probe_slot(id)?];
        (slot.id == Some(id)).then_some(slot.value).flatten()
    }

    /// Latest statistics for the exact active lease, with the same freshness
    /// rules as [`Self::get`]. A scalar-only publication has no statistics.
    pub fn statistics(&self, id: u32) -> Option<BackdropStatistics> {
        let slot = &self.slots[luminance_probe_slot(id)?];
        (slot.id == Some(id)).then_some(slot.statistics).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_statistics_respect_channels_padding_and_alpha() {
        let rgb = [
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [255, 255, 0],
            [0, 0, 0],
        ];
        for stride in [4, 256] {
            for bgra in [false, true] {
                // Start at a nonzero slot/row offset; poison all padding.
                let offset = 3 * stride * 5;
                let mut bytes = vec![199; offset + 4 * stride + 4];
                for (index, [red, green, blue]) in rgb.into_iter().enumerate() {
                    let start = offset + index * stride;
                    bytes[start..start + 4].copy_from_slice(&if bgra {
                        [blue, green, red, index as u8 * 51]
                    } else {
                        [red, green, blue, index as u8 * 51]
                    });
                }
                let value = BackdropStatistics::from_encoded_texels(&bytes[offset..], stride, bgra);
                assert_eq!(value.mean_rgb, [0.4, 0.4, 0.2]);
                assert!((value.mean_luminance - 0.38556).abs() < 1e-7);
                assert_eq!(value.min_luminance, 0.0);
                assert!((value.max_luminance - 0.9278).abs() < 1e-7);
                assert!((f64::from(value.luminance_variance) - 0.1358905824).abs() < 1e-7);
                // Exact historical arithmetic, including summation order.
                let mut legacy = 0.0;
                for [r, g, b] in rgb {
                    legacy += crate::probe_sample_luminance(
                        r as f32 / 255.0,
                        g as f32 / 255.0,
                        b as f32 / 255.0,
                    );
                }
                assert_eq!(value.mean_luminance.to_bits(), (legacy / 5.0).to_bits());
            }
        }
    }

    #[test]
    fn statistics_follow_owner_activation_and_completion_freshness() {
        let red = BackdropStatistics::from_encoded_texels(&[255, 0, 0, 255].repeat(5), 4, false);
        let blue = BackdropStatistics::from_encoded_texels(&[0, 0, 255, 255].repeat(5), 4, false);
        let mut allocator = ProbeAllocator::default();
        let first = allocator.acquire().expect("initial slot is free");
        let mut cache = LuminanceProbeCache::default();
        assert_eq!(cache.statistics(first), None);
        assert_eq!(cache.statistics(NO_LUMINANCE_PROBE), None);
        let old = cache.begin_frame([first]);
        assert_eq!(cache.statistics(first), None);
        cache.publish_statistics(old, first, red);
        assert_eq!(cache.statistics(first), Some(red));
        cache.begin_frame([]);
        cache.publish_statistics(old, first, red);
        assert_eq!(cache.statistics(first), None);
        let current = cache.begin_frame([first]);
        cache.publish_statistics(old, first, red);
        assert_eq!(cache.statistics(first), None);
        let latest = cache.begin_frame([first]);
        cache.publish_statistics(current, first, red);
        assert_eq!(cache.statistics(first), Some(red));
        cache.publish_statistics(latest, first, blue);
        cache.publish_statistics(current, first, red);
        assert_eq!(cache.statistics(first), Some(blue));
        assert_eq!(cache.get(first), Some(blue.mean_luminance));
        allocator.release(first);
        let second = allocator.acquire().expect("released slot is free");
        assert_eq!(cache.statistics(second), None);
        let frame = cache.begin_frame([second]);
        cache.publish_statistics(latest, first, red);
        assert_eq!(cache.statistics(second), None);
        assert_eq!(cache.statistics(first), None);
        cache.publish_statistics(frame, second, blue);
        assert_eq!(cache.statistics(second), Some(blue));
        cache.publish(frame, second, 0.5);
        assert_eq!(cache.statistics(second), None);
        assert_eq!(cache.get(second), Some(0.5));
    }

    #[test]
    fn a_reacquired_slot_cannot_read_or_publish_the_previous_owners_value() {
        let mut allocator = ProbeAllocator::default();
        let mut cache = LuminanceProbeCache::default();
        let first = allocator.acquire().expect("initial slot is free");
        let first_frame = cache.begin_frame([first]);
        cache.publish(first_frame, first, 0.9);
        assert_eq!(cache.get(first), Some(0.9));
        allocator.release(first);
        let second = allocator.acquire().expect("released slot is free");
        assert_eq!(luminance_probe_slot(first), luminance_probe_slot(second));
        assert_ne!(first, second);
        assert_eq!(cache.get(second), None);
        let second_frame = cache.begin_frame([second]);
        cache.publish(first_frame, first, 0.9);
        assert_eq!(cache.get(second), None);
        cache.publish(second_frame, second, 0.2);
        cache.publish(first_frame, first, 0.9);
        assert_eq!(cache.get(second), Some(0.2));
    }

    #[test]
    fn an_unsubmitted_fallback_probe_has_no_reading() {
        let mut cache = LuminanceProbeCache::default();
        let frame = cache.begin_frame([0]);
        cache.publish(frame, 0, 0.9);
        assert_eq!(cache.get(0), Some(0.9));
        cache.begin_frame([]);
        cache.publish(frame, 0, 0.9);
        assert_eq!(cache.get(0), None);
    }

    #[test]
    fn late_old_activation_callbacks_cannot_restore_or_overwrite_readings() {
        let mut cache = LuminanceProbeCache::default();
        let old = cache.begin_frame([0]);
        cache.begin_frame([]);
        let current = cache.begin_frame([0]);
        cache.publish(old, 0, 0.9);
        assert_eq!(cache.get(0), None);
        // A continuously active owner need not wait for the latest frame.
        let latest = cache.begin_frame([0]);
        cache.publish(current, 0, 0.2);
        assert_eq!(cache.get(0), Some(0.2));
        cache.publish(latest, 0, 0.3);
        cache.publish(current, 0, 0.2);
        cache.publish(old, 0, 0.9);
        assert_eq!(cache.get(0), Some(0.3));
    }

    #[test]
    fn exhausted_generations_retire_slots_instead_of_wrapping() {
        let mut allocator = ProbeAllocator::default();
        allocator.next_generation[0] = MAX_GENERATION;
        let last = allocator
            .acquire()
            .expect("last generation remains available");
        assert_eq!(last, MAX_GENERATION << SLOT_BITS);
        allocator.release(last);
        let next = allocator.acquire().expect("another slot remains available");
        assert_eq!(luminance_probe_slot(next), Some(1));
        assert_ne!(next, last);
        assert_ne!(next, 0, "the ancient generation-zero ID is not reissued");

        let mut exhausted = ProbeAllocator {
            next_generation: [MAX_GENERATION + 1; MAX_LUMINANCE_PROBES],
            ..ProbeAllocator::default()
        };
        assert_eq!(exhausted.acquire(), None);
        // Even the last physical slot cannot issue the all-ones sentinel.
        exhausted.claimed = (1 << (MAX_LUMINANCE_PROBES - 1)) - 1;
        exhausted.next_generation[MAX_LUMINANCE_PROBES - 1] = MAX_GENERATION;
        assert_eq!(exhausted.acquire(), None);
        assert_eq!(luminance_probe_slot(NO_LUMINANCE_PROBE), None);
    }
}
