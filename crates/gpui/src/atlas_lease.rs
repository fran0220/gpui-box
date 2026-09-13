//! Renderer-side ownership of atlas allocations used by frozen paint.
use crate::{AtlasKey, AtlasTextureId, AtlasTile};
use collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Native submission callbacks may release resources on another thread. Wasm
/// GPU resources are thread-confined, so their leases must remain local too.
#[doc(hidden)]
#[cfg(not(target_family = "wasm"))]
pub trait AtlasLeaseThreadBound: Send + Sync {}
#[cfg(not(target_family = "wasm"))]
impl<T: Send + Sync> AtlasLeaseThreadBound for T {}

/// Wasm counterpart of the native submission callback bound.
#[doc(hidden)]
#[cfg(target_family = "wasm")]
pub trait AtlasLeaseThreadBound {}
#[cfg(target_family = "wasm")]
impl<T> AtlasLeaseThreadBound for T {}

#[cfg(not(target_family = "wasm"))]
type ReleaseCallback = dyn FnOnce() + Send + Sync;
#[cfg(target_family = "wasm")]
type ReleaseCallback = dyn FnOnce();

/// Keeps atlas allocations alive. A reset invalidates the lease permanently.
/// Clones share ownership; the final drop releases only the leased allocations.
/// Native leases are Send + Sync; Wasm leases retain thread-local GPU resources.
#[derive(Clone)]
pub struct AtlasLease(Arc<Lease>);

struct Lease {
    valid: Arc<AtomicBool>,
    release: Option<Box<ReleaseCallback>>,
}

impl AtlasLease {
    /// False after renderer resource reset, even if tile coordinates were reused.
    pub fn is_valid(&self) -> bool {
        self.0.valid.load(Ordering::Acquire)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            release();
        }
    }
}

/// Internal renderer bookkeeping. Call under the atlas allocation lock.
#[doc(hidden)]
pub struct AtlasLeaseRegistry {
    epoch: u64,
    revision: u64,
    valid: Arc<AtomicBool>,
    pins: HashMap<AtlasKey, (usize, bool)>,
    tiles: HashMap<(AtlasTextureId, u32), (AtlasTile, AtlasKey)>,
    #[cfg(test)]
    tile_lookups: usize,
}

impl Default for AtlasLeaseRegistry {
    fn default() -> Self {
        Self {
            epoch: 0,
            revision: 0,
            valid: Arc::new(AtomicBool::new(true)),
            pins: HashMap::default(),
            tiles: HashMap::default(),
            #[cfg(test)]
            tile_lookups: 0,
        }
    }
}

impl AtlasLeaseRegistry {
    /// Register an allocation under the same lock as the atlas key map.
    pub fn insert_tile(&mut self, key: AtlasKey, tile: AtlasTile) {
        self.tiles
            .insert((tile.texture_id, tile.tile_id.0), (tile, key));
    }

    /// Forget an allocation only when its actual eviction occurs.
    pub fn remove_tile(&mut self, tile: AtlasTile) {
        self.tiles.remove(&(tile.texture_id, tile.tile_id.0));
    }

    /// Retain tiles with one expected-constant-time lookup per reference,
    /// independent of unrelated atlas population. Complete identity is checked.
    pub fn pin_tiles(
        &mut self,
        tiles: &[AtlasTile],
        release: impl FnOnce(u64, Vec<AtlasKey>) + AtlasLeaseThreadBound + 'static,
    ) -> Option<AtlasLease> {
        let keys = tiles
            .iter()
            .map(|tile| {
                #[cfg(test)]
                {
                    self.tile_lookups += 1;
                }
                self.tiles
                    .get(&(tile.texture_id, tile.tile_id.0))
                    .filter(|(registered, _)| registered == tile)
                    .map(|(_, key)| key.clone())
            })
            .collect::<Option<Vec<_>>>()?;
        Some(self.pin(keys, release))
    }

    /// Changes before any unleased coordinates can be recycled. Paint marks
    /// use this to detect eviction/reset during the live capture interval.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The caller must first verify every key still names its requested tile.
    pub fn pin(
        &mut self,
        keys: Vec<AtlasKey>,
        release: impl FnOnce(u64, Vec<AtlasKey>) + AtlasLeaseThreadBound + 'static,
    ) -> AtlasLease {
        for key in &keys {
            self.pins.entry(key.clone()).or_default().0 += 1;
        }
        let epoch = self.epoch;
        AtlasLease(Arc::new(Lease {
            valid: self.valid.clone(),
            release: Some(Box::new(move || release(epoch, keys))),
        }))
    }

    /// Returns true when eviction must wait for the final lease release.
    pub fn defer_remove(&mut self, key: &AtlasKey) -> bool {
        if let Some((_, pending)) = self.pins.get_mut(key) {
            *pending = true;
            true
        } else {
            self.revision = self
                .revision
                .checked_add(1)
                .expect("atlas revision exhausted");
            false
        }
    }

    /// Returns deferred keys to remove while still holding the allocation lock,
    /// so reset/reallocation cannot occur between release and removal.
    pub fn release(&mut self, epoch: u64, keys: Vec<AtlasKey>) -> Vec<AtlasKey> {
        if epoch != self.epoch {
            return Vec::new();
        }
        let mut remove = Vec::new();
        for key in keys {
            let (count, pending) = self.pins.get_mut(&key).expect("lease owns its pin");
            *count -= 1;
            if *count == 0 {
                if *pending {
                    remove.push(key.clone());
                }
                self.pins.remove(&key);
            }
        }
        remove
    }

    /// Must precede recycling resources on device loss or atlas clear.
    pub fn invalidate(&mut self) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("atlas revision exhausted");
        self.valid.store(false, Ordering::Release);
        self.valid = Arc::new(AtomicBool::new(true));
        self.epoch = self.epoch.checked_add(1).expect("atlas epoch exhausted");
        self.pins.clear();
        self.tiles.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AtlasTextureKind, Bounds, DevicePixels, ImageId, RenderImageParams, TileId, point, size,
    };

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn native_submission_can_release_the_final_lease_on_another_thread() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<AtlasLease>();
        let released = Arc::new(AtomicBool::new(false));
        let report = released.clone();
        let lease = AtlasLeaseRegistry::default().pin(Vec::new(), move |epoch, keys| {
            assert_eq!(epoch, 0);
            assert!(keys.is_empty());
            report.store(true, Ordering::Release);
        });
        let submitted = lease.clone();
        drop(lease);
        assert!(!released.load(Ordering::Acquire));
        std::thread::spawn(move || drop(submitted))
            .join()
            .expect("submission release");
        assert!(released.load(Ordering::Acquire));
    }

    #[test]
    fn multi_card_retention_looks_up_only_referenced_tiles_and_forgets_evictions() {
        for population in [64, 8192] {
            let mut registry = AtlasLeaseRegistry::default();
            let mut tiles = Vec::new();
            for i in 0..population {
                let tile = AtlasTile {
                    texture_id: AtlasTextureId {
                        index: 0,
                        kind: AtlasTextureKind::Polychrome,
                    },
                    tile_id: TileId(i),
                    padding: 0,
                    bounds: Bounds::new(
                        point(DevicePixels(i as i32), DevicePixels(0)),
                        size(DevicePixels(1), DevicePixels(1)),
                    ),
                };
                registry.insert_tile(
                    AtlasKey::Image(RenderImageParams {
                        image_id: ImageId(i as usize),
                        frame_index: 0,
                    }),
                    tile,
                );
                tiles.push(tile);
            }
            let mut leases = Vec::new();
            for card in 0..1024 {
                let tile = tiles[card % tiles.len()];
                leases.push(
                    registry
                        .pin_tiles(&[tile, tile, tiles[0]], |_, _| {})
                        .expect("three sprite references"),
                );
            }
            assert_eq!(
                registry.tile_lookups, 3072,
                "atlas population must not multiply per-card lookup work"
            );
            let mut forged = tiles[0];
            forged.padding = 1;
            assert!(registry.pin_tiles(&[forged], |_, _| {}).is_none());
            registry.remove_tile(tiles[1]);
            assert!(registry.pin_tiles(&[tiles[1]], |_, _| {}).is_none());
            registry.invalidate();
            assert!(leases.iter().all(|lease| !lease.is_valid()));
            assert!(registry.pin_tiles(&[tiles[0]], |_, _| {}).is_none());
            assert!(registry.tiles.is_empty());
        }
    }
}
