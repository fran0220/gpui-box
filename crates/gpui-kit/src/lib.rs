//! Product-neutral components and interaction primitives for GPUI.
//!
//! Components read [`gpui_kit_theme::Theme`] from the application context and
//! caller-owned data. They do not know about transports, databases,
//! credentials, or application hosts. Anything that has to survive a frame —
//! a text field, a menu, a dialog — is a `Render` view; everything else is a
//! `RenderOnce` builder.
//!
//! # Modules
//!
//! - [`foundation`] — the contracts every component implements:
//!   [`Ident`](foundation::Ident), [`Disableable`](foundation::Disableable),
//!   [`Sizable`](foundation::Sizable), [`Selectable`](foundation::Selectable),
//!   and the one focus ring, [`FocusRing`](foundation::FocusRing).
//! - [`controls`] — actions and editable fields.
//! - [`display`] — status, grouping, and waiting vocabulary.
//! - [`navigation`] — tabs, accordions, trails, rails, and pages.
//! - [`data`] — list, table, and tree.
//! - [`layout`] — split panes, scroll areas, and toolbars.
//! - [`overlay`] — anchored and modal surfaces, menus, notifications.
//! - [`interaction`] — drag and drop, the one gesture that starts in one
//!   component and finishes in another.
//! - [`motion`] and [`effects`] — token-driven animation and paint.
//! - [`state`] — the explicit async states a truthful surface distinguishes.
//! - [`scenes`] — one canonical rendering per component, shared by the gallery,
//!   the capture task, and the headless audit.
//!
//! # Documentation
//!
//! - `docs/components.md` — every component and the rules it keeps.
//! - `docs/coverage.md` — what is provided, and what is deliberately not.
//! - `docs/truthful-ui.md` — why a refusal is never rendered as an absence.
//! - `docs/semantic-automation.md` — the semantic tree and what a node reports.
//! - `docs/token-model.md` — where visible values come from.
//! - `docs/interaction.md` — the drag contract: what a drop reports, what a
//!   drag publishes, and what the host has to do with it.
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
pub mod interaction;
pub mod layout;
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
    pub use crate::controls::dropzone::{Dropzone, DropzoneState};
    pub use crate::controls::field::{FieldState, field_shell};
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
        Align, BulkBar, Cell, Column, ColumnWidth, DataGrid, EditIntent, EditOutcome, EditingCell,
        Expanded, GridColumn, GridRow, List, ListItem, Row, SelectionChange, SelectionMode,
        SortDirection, Table, Tree, TreeNode,
    };
    pub use crate::display::animated_number::{AnimatedNumber, grouped};
    pub use crate::display::avatar::Avatar;
    pub use crate::display::badge::{Badge, Tone};
    pub use crate::display::card::{Card, ListRow};
    pub use crate::display::empty::{Divider, EmptyKind, EmptyState};
    pub use crate::display::loading::{GradientSpinner, PulseLoader, Skeleton};
    pub use crate::display::progress::ProgressBar;
    pub use crate::display::status::{Callout, StatusDot, StatusLine};
    pub use crate::display::tag::Tag;
    pub use crate::foundation::{
        ActiveTheme, ControlSize, Density, Disableable, Elevation, FocusRing, HoverLift, Ident,
        Layer, Pressable, Selectable, Sizable, StyledExt, ThemeRegistry, activate_theme,
        set_density,
    };
    pub use crate::interaction::dnd::{
        ActiveDrag, DragItem, DropAxis, DropIntent, DropPosition, StagedDrag,
    };
    pub use crate::layout::{
        ScrollArea, ScrollAxis, SplitAxis, SplitPane, SplitSide, Toolbar, ToolbarItem,
    };
    pub use crate::motion::{Flip, Flipping, Presence, Transition, flip};
    pub use crate::navigation::{
        Accordion, AccordionSection, Breadcrumb, Crumb, PageTotal, Pagination, Sidebar,
        SidebarItem, SidebarSection, TabItem, Tabs,
    };
    pub use crate::overlay::{
        Command, CommandPalette, CommandPaletteEvent, ContextMenu, ContextMenuEvent, Dialog,
        DialogEvent, Drawer, DrawerEvent, Edge, FocusTrap, Kbd, Menu, MenuEvent, MenuItem, Overlay,
        Placement, Popover, PopoverEvent, Toast, ToastCorner, ToastLayer, Tooltip, Tooltipped,
    };
    pub use crate::state::{AsyncStatus, AsyncValue, Loadable};
}

/// Installs fonts, the theme global, and the semantic registry.
pub fn install(cx: &mut App) {
    gpui_kit_assets::register_fonts(cx);
    gpui_kit_theme::Theme::install(cx);
    gpui_kit_semantics::install(cx);
    interaction::install(cx);
    controls::input::install(cx);
    controls::textarea::install(cx);
    overlay::toast::install(cx);
}
