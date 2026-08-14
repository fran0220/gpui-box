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
//! - [`content`] — rendered Markdown and a conversation, the two surfaces that
//!   draw text nobody in this crate wrote. Nothing here executes HTML, opens a
//!   link, or fetches an image.
//! - [`controls`] — actions and editable fields.
//! - [`display`] — status, grouping, and waiting vocabulary.
//! - [`navigation`] — tabs, accordions, trails, rails, and pages.
//! - [`data`] — list, table, and tree.
//! - [`datetime`] — calendar, date field, range, and time, over a
//!   host-supplied [`DateAdapter`](datetime::DateAdapter). This crate owns no
//!   calendar.
//! - [`layout`] — split panes, scroll areas, and toolbars.
//! - [`media`] — an audio player, a video player, a macOS/Windows native
//!   [`PlatformMediaTransport`](media::PlatformMediaTransport), and a bounded
//!   3D model viewer with a glTF reader that refuses more than it accepts.
//! - [`overlay`] — anchored and modal surfaces, menus, notifications.
//! - [`interaction`] — drag and drop, the one gesture that starts in one
//!   component and finishes in another.
//! - [`motion`] — token-driven animation.
//! - [`state`] — the explicit async states a truthful surface distinguishes.
//! - [`strings`] — every word this library shows, and the host's right to
//!   replace any of them without losing the English behind the rest.
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
//! - `docs/content.md` — what a rendered document is not allowed to do, and
//!   what a conversation's five delivery states mean.
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

pub mod agent;
pub mod canvas;
pub mod content;
pub mod controls;
pub mod data;
pub mod datetime;
pub mod display;
pub mod foundation;
pub mod interaction;
pub mod layout;
pub mod media;
pub mod motion;
pub mod navigation;
pub mod overlay;
pub mod scenes;
pub mod state;
pub mod strings;
pub mod structured;

pub use gpui_kit_assets as assets;
pub use gpui_kit_semantics as semantics;
pub use gpui_kit_theme as theme;
pub use gpui_kit_tokens as tokens;

use gpui::App;

/// Everything a view needs to build with this library.
pub mod prelude {
    pub use crate::agent::approval::{
        AlwaysScope, ApprovalDecision, ApprovalEvent, ApprovalPrompt, ApprovalStatus,
    };
    pub use crate::agent::cost::{
        Basis, ContextGauge, CostLine, CostMeter, LastVerified, Limit, Quantity, Reading,
    };
    pub use crate::agent::offering_catalog::{
        OfferingCatalog, OfferingIdentity, OfferingSource, OfferingSourceState, SearchableOffering,
    };
    pub use crate::agent::permission::{
        PermissionAction, PermissionChange, PermissionEntry, PermissionMatrix, PermissionSource,
        PermissionState, PermissionSubject,
    };
    pub use crate::agent::server_list::{
        Catalog, Offering, OfferingKind, ServerEntry, ServerList, ServerState,
    };
    pub use crate::agent::step_list::{RunLength, Step, StepList, StepState};
    pub use crate::agent::thinking::{Reasoning, ThinkingBlock};
    pub use crate::agent::tool_call::{Elapsed, ToolBody, ToolCallCard, ToolCallState, ToolOutput};
    pub use crate::canvas::{
        Diff, EdgeKind, GraphEdge, GraphEndpoint, GraphNode, GraphPort, GraphState, GraphViewport,
        NodeGraph, NodeGraphEvent, NodeMetric, NodeState, Placed, PortDirection, PortSide,
    };
    pub use crate::content::{
        Attachment, BrowserPanel, BufferedRange, CodeBlock, CodeLine, CodeSpan, CodeView,
        DeliveryState, DiffFile, DiffHunk, DiffLine, DiffPresentation, DiffView, DiffViewEvent,
        FitMode, ImageFrame, ImageRequest, ImageSize, ImageState, ImageViewer, ImageViewerEvent,
        LineMark, LogEntry, LogStream, LogStreamState, Markdown, MarkdownEvent, Message,
        MessageBody, MessageList, Reaction, TrackStep, TransportBar, TransportDuration,
        TransportEvent, TransportState, ViewportState,
    };
    pub use crate::controls::auth::{
        OneTimeCodeInput, OneTimeCodeInputEvent, PasswordInput, PasswordInputEvent,
    };
    pub use crate::controls::button::{
        Button, ButtonGroup, ButtonJoin, ButtonVariant, IconButton, IconPosition,
    };
    pub use crate::controls::cascader::{Cascader, CascaderEvent, CascaderOption};
    pub use crate::controls::combobox::{Combobox, ComboboxEvent};
    pub use crate::controls::copy_button::{CopyButton, CopyEvent, CopyState};
    pub use crate::controls::dropzone::{Dropzone, DropzoneState};
    pub use crate::controls::field::{FieldState, field_shell};
    pub use crate::controls::filter_bar::{FilterBar, FilterCondition, ResultCount};
    pub use crate::controls::form_field::FormField;
    pub use crate::controls::inline_edit::InlineEdit;
    pub use crate::controls::input::{TextInput, TextInputEvent};
    pub use crate::controls::keybinding_recorder::{KeybindingRecorder, KeybindingRecorderEvent};
    pub use crate::controls::keymap_editor::{
        KeymapBinding, KeymapCommand, KeymapEditor, KeymapEditorEvent,
    };
    pub use crate::controls::number_input::{NumberInput, NumberInputEvent};
    pub use crate::controls::search::{
        FindReplace, FindReplaceEvent, HitCount, SearchField, SearchFieldEvent,
    };
    pub use crate::controls::segmented::{Segment, SegmentedControl};
    pub use crate::controls::select::{Select, SelectEvent, SelectOption};
    pub use crate::controls::settings_row::{SettingsRow, SettingsSection};
    pub use crate::controls::slider::Slider;
    pub use crate::controls::split_button::SplitButton;
    pub use crate::controls::tag_input::{TagInput, TagInputEvent};
    pub use crate::controls::textarea::{TextArea, TextAreaEvent};
    pub use crate::controls::toggle::{Checkbox, Radio, Switch};
    pub use crate::controls::toggle_button::{Toggle, ToggleGroup, ToggleItem, ToggleSelection};
    pub use crate::controls::upload_list::{OverallProgress, Upload, UploadList, UploadState};
    pub use crate::data::{
        Align, BulkBar, Cell, Column, ColumnWidth, DataGrid, Diagnostic, DiagnosticAction,
        DiagnosticFilter, DiagnosticLocation, DiagnosticSeverity, DiagnosticsList, EditIntent,
        EditOutcome, EditingCell, Expanded, GridColumn, GridLines, GridRow, List, ListItem, Row,
        SelectionChange, SelectionMode, SortDirection, Table, Tree, TreeGrid, TreeGridRow,
        TreeNode,
    };
    pub use crate::datetime::{
        BlockedDay, BlockedReport, Calendar, CalendarEvent, Clock, DateAdapter, DateInput,
        DateInputEvent, Day, DayMark, DayRange, MonthCell, MonthGrid, MonthKey, RangePicker,
        RangePickerEvent, RangeState, Selectability, SharedDateAdapter, TimeInput, TimeInputEvent,
        TimeOfDay, TimeSegment,
    };
    pub use crate::display::animated_number::{AnimatedNumber, grouped};
    pub use crate::display::avatar::Avatar;
    pub use crate::display::badge::{Badge, Tone};
    pub use crate::display::card::{Card, ListRow};
    pub use crate::display::description_list::{
        DescriptionItem, DescriptionList, DescriptionValue,
    };
    pub use crate::display::empty::{Divider, EmptyKind, EmptyState};
    pub use crate::display::failure_panel::FailurePanel;
    pub use crate::display::highlight::HighlightedText;
    pub use crate::display::icon::{Icon, IconTone};
    pub use crate::display::loading::{GradientSpinner, PulseLoader, Skeleton};
    pub use crate::display::progress::ProgressBar;
    pub use crate::display::progress_circle::ProgressCircle;
    pub use crate::display::sparkline::{
        Sparkline, SparklinePoint, SparklineReading, SparklineState,
    };
    pub use crate::display::status::{Callout, StatusDot, StatusLine};
    pub use crate::display::tag::Tag;
    pub use crate::display::timeline::{EntryTime, Timeline, TimelineEntry, TimelineGroup};
    pub use crate::foundation::direction::{
        ActiveDirection, DirectionalExt, LayoutDirection, LogicalSide, PhysicalSide,
        set_layout_direction,
    };
    pub use crate::foundation::{
        ActiveTheme, ControlSize, Density, Disableable, Elevation, FocusRing, HoverLift, Ident,
        Layer, Pressable, Selectable, Sizable, StyledExt, ThemeRegistry, activate_theme,
        set_density, text,
    };
    pub use crate::interaction::dnd::{
        ActiveDrag, DragItem, DropAxis, DropIntent, DropPosition, StagedDrag,
    };
    pub use crate::layout::{
        AspectFit, AspectRatio, Dock, DockEvent, DockPanel, DockRegion, FadeEdges, ScrollArea,
        ScrollAxis, ScrollFade, SplitAxis, SplitChange, SplitKind, SplitLayout, SplitPane,
        SplitPaneSpec, SplitRecord, SplitRecordError, SplitSide, SplitTree, StatusBar, StatusGroup,
        StatusItem, Toolbar, ToolbarItem, scroll_offset,
    };
    pub use crate::media::{
        AudioPlayer, FixtureTransport, MediaAvailability, MediaCommand, MediaEvent, MediaOrigin,
        MediaOutcome, MediaSnapshot, MediaSource, MediaTransport, ModelBounds, ModelDefect,
        ModelError, ModelLimit, ModelMesh, ModelScene, ModelShading, ModelState, ModelViewer,
        ModelViewerEvent, NativeMediaError, NativeMediaEvent, NativeMediaPlayer,
        NativeMediaSubscription, PlatformMediaTransport, VideoPlayer,
    };
    pub use crate::motion::{
        Flip, Flipping, Keyframe, Keyframes, Presence, ScrollLink, Transition, Velocity, flip,
    };
    pub use crate::navigation::{
        Accordion, AccordionSection, Anchor, AnchorList, Breadcrumb, Collapsible, Crumb,
        HistoryEntry, PageTotal, Pagination, SaveState, Sidebar, SidebarItem, SidebarSection,
        StepStatus, TabItem, Tabs, UndoHistory, Wizard, WizardIntent, WizardLayout, WizardStep,
    };
    pub use crate::overlay::{
        Command, CommandPalette, CommandPaletteEvent, ContextMenu, ContextMenuEvent, Dialog,
        DialogEvent, Drawer, DrawerEvent, Edge, FocusTrap, Frost, Glass, GlassGroup, GlassPreset,
        HoverCard, HoverCardEvent, Kbd, Menu, MenuEvent, MenuItem, Menubar, MenubarEvent,
        MenubarMenu, Notification, NotificationCenter, NotificationCenterEvent, Overlay, Placement,
        Popover, PopoverEvent, Toast, ToastCorner, ToastLayer, Tooltip, Tooltipped, UnreadCount,
    };
    pub use crate::state::{AsyncStatus, AsyncValue, Loadable};
    pub use crate::strings::{ActiveStrings, StringKey, Strings, reset_strings, set_strings};
    pub use crate::structured::{
        FieldValue, JsonValue, JsonView, NumberBounds, Schema, SchemaChoice, SchemaField,
        SchemaForm, SchemaFormEvent, SchemaKind, UnrenderableField, ValueKind,
    };
}

/// Installs fonts, the theme global, and the semantic registry.
pub fn install(cx: &mut App) {
    gpui_kit_assets::register_fonts(cx);
    gpui_kit_theme::Theme::install(cx);
    gpui_kit_semantics::install(cx);
    strings::install(cx);
    foundation::direction::install(cx);
    interaction::install(cx);
    controls::input::install(cx);
    controls::textarea::install(cx);
    overlay::toast::install(cx);
}
