//! Product-neutral components and interaction primitives for GPUI.
//!
//! Components consume [`gpui_kit_theme::Theme`] and caller-owned data. They do
//! not know about transports, databases, credentials, or application hosts.

pub mod badge;
pub mod button;
pub mod card;
pub mod effects;
pub mod input;
pub mod loaders;
pub mod motion;
pub mod popover;
pub mod settings;
pub mod state;
pub mod status;

pub use gpui_kit_assets as assets;
pub use gpui_kit_theme as theme;
pub use gpui_kit_tokens as tokens;

use gpui::App;

pub fn install(cx: &mut App) {
    gpui_kit_assets::register_fonts(cx);
    gpui_kit_theme::Theme::install(cx);
}
