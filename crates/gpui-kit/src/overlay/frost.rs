//! A surface that shows what is behind it, out of focus.
//!
//! [`Frost`] is [`Glass`] with [`GlassPreset::Frosted`]: the
//! pixels underneath are blurred and the surface colour is laid over the blur
//! at `effect.glassAlpha`, and nothing about the backdrop is bent, split into
//! colour or lit. It is the material to reach for when a surface must look the
//! same on every renderer, because a blur is the one backdrop capability they
//! all have.
//!
//! Everything else — the single scene layer that keeps the blur underneath the
//! content, the opaque-theme path that paints no blur at all, and where the
//! backdrop does not exist — is described on [`super::glass`], because this is
//! that module's material with its optics turned off.

use gpui::{App, IntoElement, RenderOnce, Window};
use gpui_kit_theme::{Radius, Surface};

use crate::foundation::Ident;
use crate::overlay::glass::{Glass, GlassPreset};

/// A frosted-glass surface: blurred backdrop, tinted fill, caller's content.
#[derive(IntoElement)]
pub struct Frost {
    inner: Glass,
}

impl std::fmt::Debug for Frost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Frost")
            .field("as", &self.inner)
            .finish()
    }
}

impl Frost {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            inner: Glass::new(ident).preset(GlassPreset::Frosted),
        }
    }

    /// Which surface colour is laid over the blur. The overlay surface is the
    /// default because that is what a floating thing is made of.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.inner = self.inner.surface(surface);
        self
    }

    /// The rounding of the glass. It clips the blur as well as the fill, so a
    /// caller rounding the card inside must say the same thing here or the
    /// blur will show past the corners.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.inner = self.inner.radius(radius);
        self
    }

    /// How far the backdrop is blurred, in pixels, when `effect.glassBlur` is
    /// not what this particular surface wants.
    pub fn blur(mut self, blur: f32) -> Self {
        self.inner = self.inner.blur(blur);
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }
}

impl RenderOnce for Frost {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.inner.render(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::GlassMaterial;
    use gpui_kit_theme::Theme;

    #[test]
    fn frost_is_glass_with_the_optics_off() {
        let theme = Theme::studio_dark();
        assert_eq!(
            Frost::new("surface").inner.material_for_test(&theme),
            GlassMaterial::frosted(),
            "a frosted surface asks the renderer for a blur and nothing else"
        );
    }
}
