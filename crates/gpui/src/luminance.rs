//! CPU ownership of the fixed-size backdrop luminance readback resources.

use parking_lot::Mutex;

use crate::{MAX_LUMINANCE_PROBES, NO_LUMINANCE_PROBE};

const SLOT_BITS: u32 = 4;
const MAX_GENERATION: u32 = u32::MAX >> SLOT_BITS;
const _: () = assert!(MAX_LUMINANCE_PROBES == 1 << SLOT_BITS);

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
        let Some(index) = luminance_probe_slot(id) else {
            return;
        };
        let slot = &mut self.slots[index];
        if slot.id == Some(id) && frame >= slot.activated_at && frame >= slot.updated_at {
            slot.value = Some(luminance);
            slot.updated_at = frame;
        }
    }

    /// Latest verified value for this exact active lease, or `None` before
    /// its first completion, after replacement, or when not submitted.
    pub fn get(&self, id: u32) -> Option<f32> {
        let slot = &self.slots[luminance_probe_slot(id)?];
        (slot.id == Some(id)).then_some(slot.value).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
