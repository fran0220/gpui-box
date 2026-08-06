use gpui::{ElementId, SharedString};

/// One identity used for both the GPUI element and the semantic tree.
///
/// Ids come from business identity, never list position, so an assertion that
/// targets `settings.provider.anthropic` keeps working when the row moves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident(SharedString);

impl Ident {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self(id.into())
    }

    /// Derives a child identity, for example the clear button inside a field.
    pub fn child(&self, suffix: impl AsRef<str>) -> Self {
        Self(SharedString::from(format!(
            "{}.{}",
            self.0,
            suffix.as_ref()
        )))
    }

    pub fn element_id(&self) -> ElementId {
        ElementId::Name(self.0.clone())
    }

    /// An element id for one repeated part, such as a loader cell.
    ///
    /// Repeated visual parts have no business identity, so they get an index
    /// here and never appear as semantic assertion targets.
    pub fn indexed_element_id(&self, index: usize) -> ElementId {
        ElementId::named_usize(self.0.clone(), index)
    }

    pub fn semantic_id(&self) -> SharedString {
        self.0.clone()
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
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
        formatter.write_str(self.0.as_ref())
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
    fn element_and_semantic_ids_share_one_string() {
        let ident = Ident::new("gallery.primary");
        assert_eq!(
            ident.element_id(),
            ElementId::Name("gallery.primary".into())
        );
        assert_eq!(ident.semantic_id().as_ref(), "gallery.primary");
    }
}
