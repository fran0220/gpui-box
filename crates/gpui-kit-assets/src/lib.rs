//! Embedded Geist fonts and product-neutral SVG icons.
//!
//! See `assets/SOURCE.md` and the repository `THIRD_PARTY_NOTICES`.

use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString, Styled as _, Svg, svg};

macro_rules! icons {
    ($(($variant:ident, $name:literal)),+ $(,)?) => {
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

icons![
    (AddCircle, "add-circle"),
    (AltArrowDown, "alt-arrow-down"),
    (AltArrowLeft, "alt-arrow-left"),
    (AltArrowRight, "alt-arrow-right"),
    (Archive, "archive-minimalistic"),
    (ArchiveUp, "archive-up-minimalistic"),
    (ArrowDown, "arrow-down"),
    (ArrowLeft, "arrow-left"),
    (ArrowRight, "arrow-right"),
    (ArrowUp, "arrow-up"),
    (Chat, "chat-round-line"),
    (Check, "check"),
    (Checklist, "checklist"),
    (Close, "close"),
    (CloseCircle, "close-circle"),
    (Command, "command"),
    (Copy, "copy"),
    (Danger, "danger-triangle"),
    (Document, "document"),
    (DocumentAdd, "document-add"),
    (Folder, "folder"),
    (FolderWithFiles, "folder-with-files"),
    (GitBranch, "git-branch"),
    (Global, "global"),
    (Info, "info-circle"),
    (Key, "key-minimalistic"),
    (Keyboard, "keyboard"),
    (Laptop, "laptop"),
    (List, "list"),
    (Logout, "logout-2"),
    (Magnifier, "magnifer"),
    (Monitor, "monitor"),
    (Paperclip, "paperclip"),
    (Pen, "pen"),
    (PenNew, "pen-new-square"),
    (Plus, "plus"),
    (Refresh, "refresh"),
    (Restart, "restart"),
    (Return, "return"),
    (Settings, "settings-minimalistic"),
    (Sidebar, "sidebar-minimalistic"),
    (SidebarLeft, "sidebar-minimalistic-left"),
    (Smartphone, "smartphone"),
    (SortVertical, "sort-vertical"),
    (Stop, "stop"),
    (Terminal, "terminal"),
    (Trash, "trash-bin-minimalistic"),
    (Tuning, "tuning"),
    (Widget, "widget"),
];

pub fn icon(icon: Icon) -> Svg {
    svg().path(icon.path()).flex_none()
}

static FONT_GEIST: &[u8] = include_bytes!("../assets/fonts/Geist.ttf");
static FONT_GEIST_MONO: &[u8] = include_bytes!("../assets/fonts/GeistMono.ttf");
static FONT_GEIST_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Geist-Medium.ttf");
static FONT_GEIST_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Geist-SemiBold.ttf");
static FONT_GEIST_BOLD: &[u8] = include_bytes!("../assets/fonts/Geist-Bold.ttf");

pub fn font_bytes() -> [&'static [u8]; 5] {
    [
        FONT_GEIST,
        FONT_GEIST_MONO,
        FONT_GEIST_MEDIUM,
        FONT_GEIST_SEMIBOLD,
        FONT_GEIST_BOLD,
    ]
}

pub fn register_fonts(cx: &App) {
    let fonts = font_bytes().into_iter().map(Cow::Borrowed).collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        tracing::warn!(%error, "gpui-kit could not register embedded fonts");
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
    fn bundled_fonts_are_not_placeholders() {
        for bytes in font_bytes() {
            assert!(bytes.len() > 1024);
        }
    }
}
