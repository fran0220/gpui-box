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
pub mod data;
pub mod display;
pub mod effects;
pub mod foundation;
pub mod motion;
pub mod navigation;
pub mod overlay;
pub mod scenes;
pub mod state;

pub use gpui_kit_assets as assets;
pub use gpui_kit_semantics as semantics;
pub use gpui_kit_theme as theme;
pub use gpui_kit_tokens as tokens;

use gpui::App;

/// Everything a view needs to build with this library.
pub mod prelude {
    pub use crate::controls::button::{
        Button, ButtonGroup, ButtonJoin, ButtonVariant, IconButton, IconPosition,
    };
    pub use crate::controls::combobox::{Combobox, ComboboxEvent};
    pub use crate::controls::field::{FieldFrame, FieldState, SearchFrame, field_shell};
    pub use crate::controls::form_field::FormField;
    pub use crate::controls::input::{TextInput, TextInputEvent};
    pub use crate::controls::number_input::{NumberInput, NumberInputEvent};
    pub use crate::controls::segmented::{Segment, SegmentedControl};
    pub use crate::controls::select::{Select, SelectEvent, SelectOption};
    pub use crate::controls::slider::Slider;
    pub use crate::controls::split_button::SplitButton;
    pub use crate::controls::tag_input::{TagInput, TagInputEvent};
    pub use crate::controls::textarea::{TextArea, TextAreaEvent};
    pub use crate::controls::toggle::{Checkbox, Radio, Switch};
    pub use crate::data::{
        Align, Cell, Column, ColumnWidth, List, ListItem, Row, SortDirection, Table, Tree, TreeNode,
    };
    pub use crate::display::avatar::Avatar;
    pub use crate::display::badge::{Badge, Tone};
    pub use crate::display::card::{Card, ListRow};
    pub use crate::display::empty::{Divider, EmptyKind, EmptyState};
    pub use crate::display::loading::{GradientSpinner, PulseLoader, Skeleton};
    pub use crate::display::progress::ProgressBar;
    pub use crate::display::status::{Callout, StatusDot, StatusLine};
    pub use crate::display::tag::Tag;
    pub use crate::foundation::{
        ActiveTheme, ControlSize, Density, Disableable, Elevation, Ident, Layer, Selectable,
        Sizable, StyledExt, ThemeRegistry, activate_theme, set_density,
    };
    pub use crate::motion::{Presence, Transition};
    pub use crate::navigation::{Accordion, AccordionSection, Breadcrumb, Crumb, TabItem, Tabs};
    pub use crate::overlay::{
        Command, CommandPalette, CommandPaletteEvent, ContextMenu, ContextMenuEvent, Dialog,
        DialogEvent, FocusTrap, Kbd, Menu, MenuEvent, MenuItem, Overlay, Placement, Popover,
        PopoverEvent, Toast, ToastCorner, ToastLayer, Tooltip, Tooltipped,
    };
    pub use crate::state::{AsyncStatus, AsyncValue, Loadable};
}

/// Installs fonts, the theme global, and the semantic registry.
pub fn install(cx: &mut App) {
    gpui_kit_assets::register_fonts(cx);
    gpui_kit_theme::Theme::install(cx);
    gpui_kit_semantics::install(cx);
    controls::input::install(cx);
    controls::textarea::install(cx);
    overlay::toast::install(cx);
}
