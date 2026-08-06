//! Product-neutral components and interaction primitives for GPUI.
//!
//! Components are `RenderOnce` builders that read [`gpui_kit_theme::Theme`]
//! from the application context and caller-owned data. They do not know about
//! transports, databases, credentials, or application hosts.
//!
//! ```no_run
//! # use gpui_kit::prelude::*;
//! # fn example() -> impl gpui::IntoElement {
//! Button::new("settings.save")
//!     .label("Save")
//!     .primary()
//!     .on_click(|_window, _cx| {})
//! # }
//! ```

pub mod controls;
pub mod display;
pub mod effects;
pub mod foundation;
pub mod motion;
pub mod overlay;
pub mod state;

pub use gpui_kit_assets as assets;
pub use gpui_kit_semantics as semantics;
pub use gpui_kit_theme as theme;
pub use gpui_kit_tokens as tokens;

use gpui::App;

/// Everything a view needs to build with this library.
pub mod prelude {
    pub use crate::controls::button::{Button, ButtonVariant, IconPosition};
    pub use crate::controls::field::{FieldFrame, SearchFrame};
    pub use crate::display::badge::{Badge, Tone};
    pub use crate::display::card::{Card, ListRow};
    pub use crate::display::loading::{GradientSpinner, PulseLoader, Skeleton};
    pub use crate::display::status::{Callout, StatusDot, StatusLine};
    pub use crate::foundation::{
        ActiveTheme, ControlSize, Density, Disableable, Elevation, Ident, Layer, Selectable,
        Sizable, StyledExt, ThemeRegistry, activate_theme, set_density,
    };
    pub use crate::state::{AsyncStatus, AsyncValue, Loadable};
}

/// Installs fonts, the theme global, and the semantic registry.
pub fn install(cx: &mut App) {
    gpui_kit_assets::register_fonts(cx);
    gpui_kit_theme::Theme::install(cx);
    gpui_kit_semantics::install(cx);
}
