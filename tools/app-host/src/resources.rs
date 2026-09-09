//! Closed, owner-scoped immutable resources. This module performs no path or URL IO.
//!
//! The host registers bytes only after its `resources` capability decision. RGBA8
//! is deliberately uncompressed: eight little-endian dimension bytes followed by
//! exactly width * height * 4 pixels. Compressed formats are not a fallback.
//! Registration is synchronous and bounded; there are no concurrent decode jobs.
//! The runtime must reconcile the *approved* EffectOwners on every lifecycle or
//! grant change and issue a new EffectOwner for every generation replacement.
use anyhow::{Result, anyhow, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui::{App, EffectOwner, Global, ImageCacheError, ImageSource, RenderImage, size};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
    sync::Arc,
};

pub const MAX_ENCODED: usize = 128 * 1024;
pub const MAX_BYTES: usize = 96 * 1024;
pub const MAX_OWNER_BYTES: usize = 1024 * 1024;
pub const MAX_OWNER_COUNT: usize = 32;
const RGBA: &str = "image/x.gpui-rgba8";
const BYTES: &str = "application/octet-stream";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResourceRef {
    pub key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub key: String,
    pub mime: String,
    pub data: String,
}

enum Asset {
    Image(Arc<RenderImage>),
    Bytes(Vec<u8>),
}
struct Entry {
    // Identity prevents an old factory from resolving a new registration with
    // the same key after revocation, even if a host mistakenly reuses an owner.
    identity: Rc<()>,
    cost: usize,
    asset: Asset,
}
#[derive(Default)]
struct State {
    owners: HashMap<EffectOwner, HashMap<String, Entry>>,
}

/// App contains only a weak view; the Host owns the lifetime via ResourceStore.
pub struct Resources(Weak<RefCell<State>>);
impl Global for Resources {}

pub struct ResourceStore(Rc<RefCell<State>>);
impl Drop for ResourceStore {
    fn drop(&mut self) {
        self.0.borrow_mut().owners.clear();
    }
}

fn validate_key(key: &str) -> Result<()> {
    ensure!(
        !key.is_empty()
            && key.len() <= 128
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid resource key"
    );
    Ok(())
}

impl ResourceStore {
    /// Supply only active, resource-authorized owners, never descriptor owners.
    pub fn reconcile(&mut self, approved: &HashSet<EffectOwner>, cx: &mut App) {
        let mut state = self.0.borrow_mut();
        state.owners.retain(|owner, entries| {
            if approved.contains(owner) {
                return true;
            }
            evict(entries, cx);
            false
        });
        for owner in approved {
            state.owners.entry(*owner).or_default();
        }
    }

    pub fn revoke(&mut self, owner: EffectOwner, cx: &mut App) {
        if let Some(entries) = self.0.borrow_mut().owners.remove(&owner) {
            evict(&entries, cx);
        }
    }

    /// Call from Host's release observer while App can still evict GPU atlases.
    pub fn clear(&mut self, cx: &mut App) {
        for entries in self.0.borrow().owners.values() {
            evict(entries, cx);
        }
        self.0.borrow_mut().owners.clear();
    }

    /// Atomic registration: rejection consumes no count/byte quota and may be retried.
    pub fn register(
        &mut self,
        owner: EffectOwner,
        registration: Registration,
    ) -> Result<ResourceRef> {
        validate_key(&registration.key)?;
        ensure!(
            matches!(registration.mime.as_str(), RGBA | BYTES),
            "unsupported resource MIME"
        );
        ensure!(
            registration.data.len() <= MAX_ENCODED,
            "encoded resource limit exceeded"
        );
        let mut state = self.0.borrow_mut();
        let entries = state
            .owners
            .get_mut(&owner)
            .ok_or_else(|| anyhow!("resource permission refused"))?;
        ensure!(
            !entries.contains_key(&registration.key),
            "resource key is immutable"
        );
        ensure!(
            entries.len() < MAX_OWNER_COUNT,
            "resource count quota exceeded"
        );
        // Account for canonical padding before allocation. The decoder below
        // rejects malformed padding and non-zero unused trailing bits.
        let padding = if registration.data.ends_with("==") {
            2
        } else {
            usize::from(registration.data.ends_with('='))
        };
        let estimate = (registration.data.len().div_ceil(4) * 3).saturating_sub(padding);
        let used: usize = entries.values().map(|entry| entry.cost).sum();
        ensure!(
            estimate <= MAX_BYTES && used + estimate <= MAX_OWNER_BYTES,
            "resource byte quota exceeded"
        );
        let bytes = STANDARD.decode(&registration.data)?;
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_BYTES,
            "invalid resource byte length"
        );
        let cost = bytes.len();
        let asset = if registration.mime == RGBA {
            ensure!(bytes.len() >= 8, "truncated RGBA header");
            let width = u32::from_le_bytes(bytes[0..4].try_into()?);
            let height = u32::from_le_bytes(bytes[4..8].try_into()?);
            ensure!(
                (1..=1024).contains(&width) && (1..=1024).contains(&height),
                "image dimensions exceed limit"
            );
            let decoded = (width as usize) * (height as usize) * 4;
            ensure!(
                decoded <= MAX_BYTES - 8 && bytes.len() == decoded + 8,
                "invalid decoded image allocation"
            );
            Asset::Image(Arc::new(RenderImage::from_rgba(
                size((width as i32).into(), (height as i32).into()),
                bytes[8..].to_vec(),
            )?))
        } else {
            Asset::Bytes(bytes)
        };
        entries.insert(
            registration.key.clone(),
            Entry {
                identity: Rc::new(()),
                cost,
                asset,
            },
        );
        Ok(ResourceRef {
            key: registration.key,
        })
    }
}

fn evict(entries: &HashMap<String, Entry>, cx: &mut App) {
    for entry in entries.values() {
        if let Asset::Image(image) = &entry.asset {
            cx.drop_image(image.clone(), None);
            // A window currently updating is temporarily absent from App.windows.
            // Once that update returns, evict its atlas too. This is one bounded
            // turn of retention, not a resource cache or an asynchronous load.
            let image = image.clone();
            cx.defer(move |cx| cx.drop_image(image, None));
        }
    }
}

/// A revocable lease, not ownership of resource bytes. Consumers must parse
/// synchronously within with_bytes and enforce format-specific work limits.
/// In particular, authorizing these bytes does not authorize external references.
pub struct ResourceBytes(Lease);
struct Lease {
    state: Weak<RefCell<State>>,
    owner: EffectOwner,
    key: String,
    identity: Weak<()>,
}
impl Lease {
    fn with_entry<R>(
        &self,
        owner: Option<EffectOwner>,
        use_entry: impl FnOnce(&Entry) -> Result<R>,
    ) -> Result<R> {
        ensure!(owner == Some(self.owner), "cross-owner resource refused");
        let state = self
            .state
            .upgrade()
            .ok_or_else(|| anyhow!("resource host disposed"))?;
        let state = state.borrow();
        let entry = state
            .owners
            .get(&self.owner)
            .and_then(|entries| entries.get(&self.key))
            .ok_or_else(|| anyhow!("resource revoked or unknown"))?;
        ensure!(
            self.identity.ptr_eq(&Rc::downgrade(&entry.identity)),
            "stale resource registration"
        );
        use_entry(entry)
    }
}
impl ResourceBytes {
    pub fn with_bytes<R>(&self, cx: &App, use_bytes: impl FnOnce(&[u8]) -> Result<R>) -> Result<R> {
        self.0
            .with_entry(cx.current_effect_owner(), |entry| match &entry.asset {
                Asset::Bytes(bytes) => use_bytes(bytes),
                Asset::Image(_) => Err(anyhow!("resource is not opaque bytes")),
            })
    }
}

impl Resources {
    pub fn install(cx: &mut App) -> ResourceStore {
        let store = ResourceStore(Rc::new(RefCell::new(State::default())));
        cx.set_global(Self(Rc::downgrade(&store.0)));
        store
    }

    fn lease(reference: &ResourceRef, cx: &App) -> Result<Lease> {
        validate_key(&reference.key)?;
        let owner = cx
            .current_effect_owner()
            .ok_or_else(|| anyhow!("resource owner missing"))?;
        let view = cx
            .try_global::<Self>()
            .ok_or_else(|| anyhow!("resources unavailable"))?;
        let state = view
            .0
            .upgrade()
            .ok_or_else(|| anyhow!("resource host disposed"))?;
        let state = state.borrow();
        let entry = state
            .owners
            .get(&owner)
            .and_then(|entries| entries.get(&reference.key))
            .ok_or_else(|| anyhow!("resource refused or unknown"))?;
        Ok(Lease {
            state: view.0.clone(),
            owner,
            key: reference.key.clone(),
            identity: Rc::downgrade(&entry.identity),
        })
    }

    pub fn image(reference: &ResourceRef, cx: &App) -> Result<ImageSource> {
        let lease = Self::lease(reference, cx)?;
        lease.with_entry(cx.current_effect_owner(), |entry| {
            ensure!(
                matches!(entry.asset, Asset::Image(_)),
                "resource is not an image"
            );
            Ok(())
        })?;
        Ok(ImageSource::from(
            move |_: &mut gpui::Window, cx: &mut App| {
                Some(
                    lease
                        .with_entry(cx.current_effect_owner(), |entry| match &entry.asset {
                            Asset::Image(image) => Ok(image.clone()),
                            Asset::Bytes(_) => Err(anyhow!("resource is not an image")),
                        })
                        .map_err(|error| ImageCacheError::Other(Arc::new(error))),
                )
            },
        ))
    }

    pub fn bytes(reference: &ResourceRef, cx: &App) -> Result<ResourceBytes> {
        let lease = Self::lease(reference, cx)?;
        lease.with_entry(cx.current_effect_owner(), |entry| {
            ensure!(
                matches!(entry.asset, Asset::Bytes(_)),
                "resource is not opaque bytes"
            );
            Ok(())
        })?;
        Ok(ResourceBytes(lease))
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;

#[cfg(all(test, feature = "capture"))]
#[path = "resources_visual_tests.rs"]
mod visual_tests;
