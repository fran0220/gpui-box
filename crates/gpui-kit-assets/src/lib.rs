//! Embedded Geist fonts and product-neutral SVG icons.
//!
//! See `assets/SOURCE.md` and the repository `THIRD_PARTY_NOTICES`.

use std::borrow::Cow;
use std::sync::OnceLock;

use gpui::{App, AssetSource, FontFallbacks, Result, SharedString, Styled as _, Svg, svg};

/// Whether a glyph's meaning is carried by the direction it reads in.
///
/// A chevron that points at the next item points the other way once the
/// interface reads right to left; a checkmark, a gear, and a globe do not.
/// The property belongs to the drawing, not to the component that places it,
/// which is why it lives beside the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mirroring {
    /// Flipped horizontally when the interface reads right to left.
    Directional,
    /// Drawn the same way in both reading directions.
    Fixed,
}

macro_rules! icons {
    ($(($variant:ident, $name:literal, $mirroring:ident)),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Icon {
            $($variant),+
        }

        impl Icon {
            pub const ALL: &'static [Icon] = &[$(Icon::$variant),+];

            pub const fn path(self) -> &'static str {
                match self {
                    $(Icon::$variant => concat!("icons/", $name, ".svg")),+,
                }
            }

            /// How this glyph behaves when the reading direction reverses.
            pub const fn mirroring(self) -> Mirroring {
                match self {
                    $(Icon::$variant => Mirroring::$mirroring),+,
                }
            }

            /// Whether a right-to-left interface draws this glyph flipped.
            pub const fn mirrors_in_rtl(self) -> bool {
                matches!(self.mirroring(), Mirroring::Directional)
            }
        }

        #[derive(Debug)]
        pub struct Assets;

        impl AssetSource for Assets {
            fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
                Ok(match path {
                    $(
                        concat!("icons/", $name, ".svg") => Some(Cow::Borrowed(
                            include_bytes!(concat!("../assets/icons/", $name, ".svg")).as_slice(),
                        )),
                    )+
                    _ => None,
                })
            }

            fn list(&self, prefix: &str) -> Result<Vec<SharedString>> {
                Ok(Icon::ALL
                    .iter()
                    .map(|icon| icon.path())
                    .filter(|path| path.starts_with(prefix))
                    .map(SharedString::from)
                    .collect())
            }
        }
    };
}

// The third column is the reading-direction decision for that drawing. It is
// a judgement about the artwork itself: a glyph is Directional when flipping
// it preserves the meaning and leaving it alone reverses it. Radially or
// bilaterally symmetric glyphs, glyphs whose only axis is vertical, and
// conventional marks that are read as symbols rather than as pictures — a
// checkmark, a rotation, a keyboard — stay Fixed.
icons![
    (AddCircle, "add-circle", Fixed),
    (AltArrowDown, "alt-arrow-down", Fixed),
    (AltArrowLeft, "alt-arrow-left", Directional),
    (AltArrowRight, "alt-arrow-right", Directional),
    (Archive, "archive-minimalistic", Fixed),
    (ArchiveUp, "archive-up-minimalistic", Fixed),
    (ArrowDown, "arrow-down", Fixed),
    (ArrowLeft, "arrow-left", Directional),
    (ArrowRight, "arrow-right", Directional),
    (ArrowUp, "arrow-up", Fixed),
    (Chat, "chat-round-line", Directional),
    (Check, "check", Fixed),
    (Checklist, "checklist", Directional),
    (Close, "close", Fixed),
    (CloseCircle, "close-circle", Fixed),
    (Command, "command", Fixed),
    (Copy, "copy", Directional),
    (Danger, "danger-triangle", Fixed),
    (Document, "document", Directional),
    (DocumentAdd, "document-add", Directional),
    (Folder, "folder", Directional),
    (FolderWithFiles, "folder-with-files", Directional),
    (GitBranch, "git-branch", Directional),
    (Global, "global", Fixed),
    (Info, "info-circle", Fixed),
    (Key, "key-minimalistic", Directional),
    (Keyboard, "keyboard", Fixed),
    (Laptop, "laptop", Fixed),
    (List, "list", Directional),
    (Logout, "logout-2", Directional),
    (Magnifier, "magnifer", Directional),
    (Monitor, "monitor", Fixed),
    (Paperclip, "paperclip", Directional),
    (Pen, "pen", Directional),
    (PenNew, "pen-new-square", Directional),
    (Plus, "plus", Fixed),
    (Refresh, "refresh", Fixed),
    (Restart, "restart", Fixed),
    (Return, "return", Directional),
    (Settings, "settings-minimalistic", Fixed),
    (Sidebar, "sidebar-minimalistic", Directional),
    (SidebarLeft, "sidebar-minimalistic-left", Directional),
    (Smartphone, "smartphone", Fixed),
    (SortVertical, "sort-vertical", Fixed),
    (Stop, "stop", Fixed),
    (Terminal, "terminal", Directional),
    (Trash, "trash-bin-minimalistic", Fixed),
    (Tuning, "tuning", Fixed),
    (Widget, "widget", Fixed),
];

pub fn icon(icon: Icon) -> Svg {
    svg().path(icon.path()).flex_none()
}

static FONT_GEIST: &[u8] = include_bytes!("../assets/fonts/Geist.ttf");
static FONT_GEIST_MONO: &[u8] = include_bytes!("../assets/fonts/GeistMono.ttf");
static FONT_GEIST_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Geist-Medium.ttf");
static FONT_GEIST_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Geist-SemiBold.ttf");
static FONT_GEIST_BOLD: &[u8] = include_bytes!("../assets/fonts/Geist-Bold.ttf");
static FONT_NOTO_SANS_ARABIC: &[u8] = include_bytes!("../assets/fonts/NotoSansArabic.ttf");
static FONT_NOTO_SANS_HEBREW: &[u8] = include_bytes!("../assets/fonts/NotoSansHebrew.ttf");
/// The seven keyboard symbols `Kbd` can emit that no Geist face draws.
///
/// Without it those glyphs render only where the platform happens to own a
/// font that covers them, which made the library's own output depend on what
/// the host machine had installed. See `assets/SOURCE.md`.
static FONT_KEY_SYMBOLS: &[u8] = include_bytes!("../assets/fonts/KeySymbols.ttf");

pub fn font_bytes() -> [&'static [u8]; 8] {
    [
        FONT_GEIST,
        FONT_GEIST_MONO,
        FONT_GEIST_MEDIUM,
        FONT_GEIST_SEMIBOLD,
        FONT_GEIST_BOLD,
        FONT_NOTO_SANS_ARABIC,
        FONT_NOTO_SANS_HEBREW,
        FONT_KEY_SYMBOLS,
    ]
}

/// The deterministic script fallbacks carried by every Kit text style.
pub fn text_fallbacks() -> FontFallbacks {
    static FALLBACKS: OnceLock<FontFallbacks> = OnceLock::new();
    FALLBACKS
        .get_or_init(|| {
            FontFallbacks::from_fonts(vec![
                "Noto Sans Arabic".to_owned(),
                "Noto Sans Hebrew".to_owned(),
            ])
        })
        .clone()
}

pub fn register_fonts(cx: &App) {
    let fonts = font_bytes().into_iter().map(Cow::Borrowed).collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        tracing::warn!(%error, "GPUI Box could not register embedded fonts");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_icon_is_embedded_svg() {
        let assets = Assets;
        for icon in Icon::ALL {
            let bytes = assets
                .load(icon.path())
                .expect("asset lookup")
                .expect("registered icon");
            let text = std::str::from_utf8(&bytes).expect("UTF-8 SVG");
            assert!(text.contains("<svg"));
            assert!(text.contains("viewBox"));
        }
    }

    #[test]
    fn brand_icons_are_not_part_of_the_generic_catalog() {
        let assets = Assets;
        for path in [
            "icons/comet-logo.svg",
            "icons/claude-mark.svg",
            "icons/openai-mark.svg",
            "icons/cursor-mark.svg",
        ] {
            assert!(assets.load(path).expect("lookup").is_none());
        }
    }

    #[test]
    fn a_direction_bearing_glyph_mirrors_and_a_symbol_does_not() {
        assert!(Icon::AltArrowRight.mirrors_in_rtl());
        assert!(Icon::ArrowLeft.mirrors_in_rtl());
        assert!(Icon::Magnifier.mirrors_in_rtl());
        assert!(!Icon::Check.mirrors_in_rtl());
        assert!(!Icon::Settings.mirrors_in_rtl());
        assert!(!Icon::Global.mirrors_in_rtl());
        // A vertical axis is not a reading axis.
        assert!(!Icon::AltArrowDown.mirrors_in_rtl());
        assert!(!Icon::SortVertical.mirrors_in_rtl());
    }

    #[test]
    fn every_icon_carries_a_mirroring_decision() {
        // The macro makes this total, so the check that matters is that the
        // catalog was actually thought about rather than answered one way.
        let directional = Icon::ALL
            .iter()
            .filter(|icon| icon.mirrors_in_rtl())
            .count();
        assert_eq!(Icon::ALL.len(), 49);
        assert_eq!(directional, 23);
    }

    #[test]
    fn bundled_fonts_are_not_placeholders() {
        for bytes in font_bytes() {
            assert!(bytes.len() > 1024);
        }
    }

    #[test]
    fn script_fallbacks_are_stable_and_ordered() {
        assert_eq!(
            text_fallbacks().fallback_list(),
            ["Noto Sans Arabic", "Noto Sans Hebrew"]
        );
    }
}
