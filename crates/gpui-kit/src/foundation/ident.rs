use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use gpui::{ElementId, SharedString};
use smallvec::{SmallVec, smallvec};

/// One identity used for both the GPUI element and the semantic tree.
///
/// Ids come from business identity, never list position, so an assertion that
/// targets `settings.provider.anthropic` keeps working when the row moves.
pub struct Ident {
    segments: SmallVec<[SharedString; 3]>,
    resolved: OnceLock<SharedString>,
}

impl Ident {
    pub fn new(id: impl Into<SharedString>) -> Self {
        let id = id.into();
        let resolved = OnceLock::new();
        resolved.set(id.clone()).expect("a new identity is empty");
        Self {
            segments: smallvec![id],
            resolved,
        }
    }

    /// Derives a lazily joined child identity, for example the clear button
    /// inside a field.
    ///
    /// Segments stay separate until an element or semantic id needs the full
    /// string. Chained builders therefore materialize once rather than once
    /// per intermediate child, and the inline segment storage dies with the
    /// value instead of retaining an interning table.
    pub fn child(&self, suffix: impl AsRef<str>) -> Self {
        let mut segments = self.segments.clone();
        segments.push(suffix.as_ref().into());
        Self {
            segments,
            resolved: OnceLock::new(),
        }
    }

    pub fn element_id(&self) -> ElementId {
        ElementId::Name(self.value().clone())
    }

    /// An element id for one repeated part, such as a loader cell.
    ///
    /// Repeated visual parts have no business identity, so they get an index
    /// here and never appear as semantic assertion targets.
    pub fn indexed_element_id(&self, index: usize) -> ElementId {
        ElementId::named_usize(self.value().clone(), index)
    }

    pub fn semantic_id(&self) -> SharedString {
        self.value().clone()
    }

    pub fn as_str(&self) -> &str {
        self.value().as_ref()
    }

    fn value(&self) -> &SharedString {
        self.resolved
            .get_or_init(|| join_segments(self.segments.as_slice()))
    }
}

impl Clone for Ident {
    fn clone(&self) -> Self {
        let resolved = OnceLock::new();
        if let Some(value) = self.resolved.get() {
            resolved
                .set(value.clone())
                .expect("a cloned identity is empty");
        }
        Self {
            segments: self.segments.clone(),
            resolved,
        }
    }
}

impl std::fmt::Debug for Ident {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("Ident")
            .field(&self.as_str())
            .finish()
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Ident {}

impl Hash for Ident {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

/// Joins common ids without a temporary heap string. Longer identities make
/// exactly the allocation retained by `SharedString`.
fn join_segments(segments: &[SharedString]) -> SharedString {
    const INLINE_BUILD: usize = 256;

    let len = segments.iter().map(|segment| segment.len()).sum::<usize>()
        + segments.len().saturating_sub(1);
    if len <= INLINE_BUILD {
        let mut bytes = [0; INLINE_BUILD];
        let mut end = 0;
        for (index, segment) in segments.iter().enumerate() {
            if index > 0 {
                bytes[end] = b'.';
                end += 1;
            }
            bytes[end..end + segment.len()].copy_from_slice(segment.as_bytes());
            end += segment.len();
        }
        return std::str::from_utf8(&bytes[..end])
            .expect("identity segments are valid UTF-8")
            .into();
    }

    let mut joined = String::with_capacity(len);
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            joined.push('.');
        }
        joined.push_str(segment);
    }
    joined.into()
}

impl From<&'static str> for Ident {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Ident {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<SharedString> for Ident {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_ids_are_prefixed_by_their_owner() {
        let field = Ident::new("settings.token");
        assert_eq!(field.child("clear").as_str(), "settings.token.clear");
    }

    #[test]
    fn chained_children_join_only_when_the_value_is_requested() {
        let ident = Ident::new("file")
            .child("main.rs")
            .child("line")
            .child("changed");
        assert!(ident.resolved.get().is_none());
        assert_eq!(ident.as_str(), "file.main.rs.line.changed");
        assert!(ident.resolved.get().is_some());
    }

    #[test]
    fn element_and_semantic_ids_share_one_string() {
        let ident = Ident::new("gallery.primary");
        assert_eq!(
            ident.element_id(),
            ElementId::Name("gallery.primary".into())
        );
        assert_eq!(ident.semantic_id().as_ref(), "gallery.primary");
    }
}
