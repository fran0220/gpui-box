//! Opt-in fonts for native Unicode capture review, not application defaults.
//!
//! Register before the first layout on the capture's isolated text system.
//! The pinned full font and its OFL license live in `fixtures/fonts/`.

use std::borrow::Cow;

/// Add the pinned Noto Color Emoji review fixture to a caller-owned text system.
///
/// This does not enable system font discovery or change default fallback policy.
/// Call once per fresh capture text system, before constructing the headless app.
pub fn register_emoji_review_font(text_system: &dyn gpui::PlatformTextSystem) -> gpui::Result<()> {
    text_system.add_fonts(vec![Cow::Borrowed(include_bytes!(
        "../../../fixtures/fonts/noto-color-emoji/NotoColorEmoji.ttf"
    ))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{FontRun, RenderGlyphParams, font, point, px};

    #[test]
    fn explicit_review_font_shapes_and_rasterizes_color_emoji() -> gpui::Result<()> {
        let text_system = gpui_platform::test_text_system("Geist");
        assert!(text_system.all_font_names().is_empty());
        text_system.add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../../../crates/gpui-kit-assets/assets/fonts/Geist.ttf"
        ))])?;
        let font_id = text_system.font_id(&font("Geist"))?;
        let missing = text_system.layout_line("🙂", px(26.0), &[FontRun { len: 4, font_id }]);
        assert!(
            missing
                .runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .all(|glyph| glyph.id.0 == 0)
        );
        register_emoji_review_font(text_system.as_ref())?;
        let line = text_system.layout_line("🙂", px(26.0), &[FontRun { len: 4, font_id }]);
        assert_eq!(line.runs.len(), 1);
        assert_eq!(line.runs[0].glyphs.len(), 1);
        let glyph = &line.runs[0].glyphs[0];
        assert_ne!(glyph.id.0, 0);
        assert!(glyph.is_emoji);
        let params = RenderGlyphParams {
            font_id: line.runs[0].font_id,
            glyph_id: glyph.id,
            font_size: px(26.0),
            subpixel_variant: point(0, 0),
            scale_factor: 2.0,
            is_emoji: glyph.is_emoji,
            subpixel_rendering: false,
            dilation: 0,
        };
        let bounds = text_system.glyph_raster_bounds(&params)?;
        let (_, bytes) = text_system.rasterize_glyph(&params, bounds)?;
        assert!(
            bytes
                .chunks_exact(4)
                .any(|bgra| { bgra[3] != 0 && (bgra[0] != bgra[1] || bgra[1] != bgra[2]) })
        );
        Ok(())
    }
}
