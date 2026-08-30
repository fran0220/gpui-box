//! Maps GPUI-independent tokens into the paint and typography types views use.

use std::sync::Arc;

use gpui::{App, BorrowAppContext, BoxShadow, Global, Hsla, Rgba, SharedString, point, px};
use gpui_kit_tokens::{
    BorderWeight, Color, DensityScale, InteractiveColor, OpacityRole, TokenDocument, TokenError,
    bundled, contrast_ratio, over, presets,
};

pub use gpui_kit_tokens::{
    AgentColor, Appearance, ControlSize, Density, Elevation, Layer, LoaderColor, MotionDuration,
    MotionEasing, NodeColor, Palette, Radius, SEQUENCE_LENGTH, SemanticColor, Space, SpringPreset,
    SpringTokens, Surface, SyntaxColor, TextTone, TypeScale,
};

/// Reads the active theme from any context that dereferences to [`App`].
///
/// Components take `&mut App` during render and pull the theme themselves, so
/// callers never thread a `&Theme` through builder arguments.
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        Theme::get(self)
    }
}

/// The presentation tiers a coloured component can take, loudest first.
///
/// One vocabulary for every coloured surface, resolved by
/// [`Theme::variant_colors`] so components cannot each invent a private
/// meaning for "light". `Default` is the neutral tier: it reads the theme's
/// own surface colours and ignores the colour choice entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// A solid block of the colour with whichever theme text pole stays
    /// readable across its resting and pressed paints.
    Filled,
    /// A wash of the colour under text in the colour.
    Light,
    /// A hairline of the colour around text in the colour.
    Outline,
    /// Text in the colour that gains a wash only under the pointer.
    Subtle,
    /// The neutral control surface; the colour choice is not consulted.
    #[default]
    Default,
    /// Text in the colour with no surface of its own.
    Transparent,
    /// A white surface with text in the colour, for tinted backdrops.
    White,
}

/// Strengths of a semantic colour used as a background wash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticWash {
    Faint,
    Standard,
    Strong,
}

/// Strengths of a semantic outline, from a report boundary through an active
/// drop or canvas target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticBorder {
    Report,
    Selected,
    Target,
}

/// How much primary text is mixed into decorative colour to keep it visible
/// in both appearances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastTint {
    Soft,
    Standard,
}

impl Variant {
    /// The stable name a semantic node publishes for the tier.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Filled => "filled",
            Self::Light => "light",
            Self::Outline => "outline",
            Self::Subtle => "subtle",
            Self::Default => "default",
            Self::Transparent => "transparent",
            Self::White => "white",
        }
    }
}

/// The colour a variant is resolved against.
///
/// A palette group keeps the choice in the token document, so a retint or a
/// theme switch re-resolves it; a semantic role follows the theme's meaning;
/// an explicit paint is the caller's own and travels unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum ColorChoice {
    /// A palette group by name, such as `"indigo"`; steps are chosen per
    /// appearance by the resolver.
    Palette(SharedString),
    /// One of the theme's semantic roles.
    Semantic(SemanticColor),
    /// An explicit caller-owned paint.
    Custom(Hsla),
}

impl From<SemanticColor> for ColorChoice {
    fn from(role: SemanticColor) -> Self {
        Self::Semantic(role)
    }
}

impl From<Hsla> for ColorChoice {
    fn from(paint: Hsla) -> Self {
        Self::Custom(paint)
    }
}

impl From<&'static str> for ColorChoice {
    fn from(group: &'static str) -> Self {
        Self::Palette(group.into())
    }
}

impl From<SharedString> for ColorChoice {
    fn from(group: SharedString) -> Self {
        Self::Palette(group)
    }
}

/// The resolved paints of one variant tier in one colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariantColors {
    pub background: Hsla,
    pub background_hover: Hsla,
    pub background_active: Hsla,
    pub text: Hsla,
    /// Only the Outline tier draws one.
    pub border: Option<Hsla>,
}

#[derive(Debug, Clone)]
struct PaletteSteps {
    filled: [String; 3],
    hover: [String; 3],
    active: [String; 3],
    readable_dark: [String; 3],
    readable_light: [String; 3],
}

fn token_color(paint: Hsla) -> Color {
    let paint = Rgba::from(paint);
    Color {
        red: paint.r,
        green: paint.g,
        blue: paint.b,
        alpha: paint.a,
    }
}

/// The weakest contrast `foreground` keeps over every interactive paint.
///
/// Filled palette and semantic paints are opaque. A custom paint may not be,
/// so it is composited over the resolver's neutral control surface before it
/// is compared; that is the same surface a default tier occupies.
fn weakest_contrast(foreground: Hsla, backgrounds: [Hsla; 3], substrate: Hsla) -> f32 {
    let substrate = token_color(substrate);
    backgrounds
        .into_iter()
        .map(|background| {
            let background = over(token_color(background), substrate);
            contrast_ratio(token_color(foreground), background)
        })
        .fold(f32::INFINITY, f32::min)
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub id: SharedString,
    pub name: SharedString,
    pub appearance: Appearance,
    pub density: Density,
    pub colors: Colors,
    pub typography: Typography,
    pub spacing: Spacing,
    pub measures: Measures,
    pub radii: Radii,
    pub control: Control,
    pub borders: Borders,
    pub opacity: Opacity,
    pub motion: Motion,
    pub elevation: Elevations,
    pub z_index: ZIndices,
    pub effects: Effects,
    /// The active document's palette, carried so that paint the shared role
    /// vocabulary has no slot for still travels with the theme rather than
    /// being read from a second document the registry does not know about.
    ///
    /// Read it through [`Theme::palette_color`]. Shared behind an `Arc`
    /// because a theme is cloned on every render that reads it.
    pub palette: Arc<Palette>,
    palette_steps: PaletteSteps,
}

#[derive(Debug, Clone)]
pub struct Colors {
    pub backdrop: Hsla,
    pub canvas: Hsla,
    pub sunken: Hsla,
    pub panel: Hsla,
    pub raised: Hsla,
    pub overlay: Hsla,
    /// The modal veil, drawn over the page at `opacity.scrim`. Dark themes
    /// declare a cast the page does not have, because more black over a
    /// near-black backdrop is invisible.
    pub scrim: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_faint: Hsla,
    pub text_placeholder: Hsla,
    pub text_disabled: Hsla,
    pub text_on_accent: Hsla,
    /// The one filled control on a surface, and the label it carries.
    ///
    /// Its own role rather than `text` reused as a fill. A primary button
    /// painted in the theme's darkest ink is a decision a theme should be able
    /// to make or decline; borrowing the prose colour made it the only
    /// decision available. See `gpui_kit_tokens::InteractiveColor::PrimaryFill`.
    pub primary_fill: Hsla,
    pub white_fill: Hsla,
    pub white_fill_hover: Hsla,
    pub white_fill_active: Hsla,
    pub text_on_primary_fill: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub selected: Hsla,
    pub hairline: Hsla,
    pub hairline_strong: Hsla,
    pub track: Hsla,
    pub divider: Hsla,
    pub focus: Hsla,
    pub accent: Hsla,
    pub accent_strong: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,
    pub success: Hsla,
    pub info: Hsla,
    /// The vocabulary of work in progress: the moving mark, the groove it
    /// travels, the shape of absent content, and the highlight that crosses
    /// it. The mark carries the theme's accent so the moving part is legible
    /// at any stroke; the quiet roles stay grey. Both are the token
    /// document's to change. See `gpui_kit_tokens::LoaderColors`.
    pub loader_mark: Hsla,
    pub loader_track: Hsla,
    pub loader_placeholder: Hsla,
    pub loader_sheen: Hsla,
    /// The categorical series scale, in order. Indexed by a chart and cycled
    /// past its end. See `gpui_kit_tokens::SequenceColors`.
    pub sequence: SequenceScale,
    /// The node canvas vocabulary: ports, edges, the label chip and the
    /// grid. See `gpui_kit_tokens::NodeColors`.
    pub node: NodePalette,
    /// Quiet tool-family tints and the wash behind expanded transcript
    /// evidence. See `gpui_kit_tokens::AgentColors`.
    pub agent: AgentPalette,
    /// The vocabulary code is painted in, wherever this library draws code.
    /// See `gpui_kit_tokens::SyntaxColors`.
    pub syntax: SyntaxPalette,
    /// The plane a terminal grid paints on, and the achromatic wash over its
    /// selected cells. See `gpui_kit_tokens::TerminalColors`.
    pub terminal_background: Hsla,
    pub terminal_selection: Hsla,
    /// ANSI slots 0-7 normal, 8-15 bright. Anything above 15 is the 6x6x6
    /// cube and the grey ramp, which are arithmetic rather than tokens.
    pub terminal_ansi: [Hsla; 16],
}

/// The resolved paint classes of code.
///
/// A struct rather than a lookup by name, so a renderer that asks for a class
/// this library does not have fails to compile instead of falling back to a
/// colour that happens to be there.
#[derive(Debug, Clone, Copy)]
pub struct SyntaxPalette {
    pub keyword: Hsla,
    pub string: Hsla,
    pub comment: Hsla,
    pub number: Hsla,
    pub inline: Hsla,
    pub inline_wash: Hsla,
    pub added: Hsla,
    pub added_wash: Hsla,
    pub removed: Hsla,
    pub removed_wash: Hsla,
}

/// The resolved categorical series scale.
///
/// A list rather than named fields, because a series is chosen by position:
/// the third slice is the third colour, and a caller with more series than
/// the scale has colours reads through [`Self::get`], which wraps.
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceScale {
    pub categorical: Vec<Hsla>,
}

impl SequenceScale {
    /// The colour of series `index`, cycling past the end of the scale.
    pub fn get(&self, index: usize) -> Hsla {
        self.categorical[index % self.categorical.len()]
    }

    pub fn len(&self) -> usize {
        self.categorical.len()
    }

    pub fn is_empty(&self) -> bool {
        self.categorical.is_empty()
    }
}

/// The resolved node canvas vocabulary.
#[derive(Debug, Clone, Copy)]
pub struct NodePalette {
    pub header_wash: Hsla,
    pub port_idle: Hsla,
    pub port_hover: Hsla,
    pub port_connected: Hsla,
    pub edge: Hsla,
    pub edge_active: Hsla,
    pub edge_feedback: Hsla,
    pub edge_feedback_active: Hsla,
    pub label_wash: Hsla,
    pub grid: Hsla,
    pub grid_strong: Hsla,
    pub grid_axis: Hsla,
}

impl NodePalette {
    pub fn get(&self, role: NodeColor) -> Hsla {
        match role {
            NodeColor::HeaderWash => self.header_wash,
            NodeColor::PortIdle => self.port_idle,
            NodeColor::PortHover => self.port_hover,
            NodeColor::PortConnected => self.port_connected,
            NodeColor::Edge => self.edge,
            NodeColor::EdgeActive => self.edge_active,
            NodeColor::EdgeFeedback => self.edge_feedback,
            NodeColor::EdgeFeedbackActive => self.edge_feedback_active,
            NodeColor::LabelWash => self.label_wash,
            NodeColor::Grid => self.grid,
            NodeColor::GridStrong => self.grid_strong,
            NodeColor::GridAxis => self.grid_axis,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AgentPalette {
    pub read: Hsla,
    pub network: Hsla,
    pub shell: Hsla,
    pub edit: Hsla,
    pub external: Hsla,
    pub evidence_wash: Hsla,
}

impl AgentPalette {
    pub fn get(&self, role: AgentColor) -> Hsla {
        match role {
            AgentColor::Read => self.read,
            AgentColor::Network => self.network,
            AgentColor::Shell => self.shell,
            AgentColor::Edit => self.edit,
            AgentColor::External => self.external,
            AgentColor::EvidenceWash => self.evidence_wash,
        }
    }
}

impl SyntaxPalette {
    pub fn get(&self, class: SyntaxColor) -> Hsla {
        match class {
            SyntaxColor::Keyword => self.keyword,
            SyntaxColor::StringLiteral => self.string,
            SyntaxColor::Comment => self.comment,
            SyntaxColor::Number => self.number,
            SyntaxColor::Inline => self.inline,
            SyntaxColor::InlineWash => self.inline_wash,
            SyntaxColor::Added => self.added,
            SyntaxColor::AddedWash => self.added_wash,
            SyntaxColor::Removed => self.removed,
            SyntaxColor::RemovedWash => self.removed_wash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Typography {
    pub sans: SharedString,
    pub sans_fallback: SharedString,
    pub mono: SharedString,
    pub mono_fallback: SharedString,
    pub readout_scale: f32,
    pub caption: TypeStyle,
    pub label: TypeStyle,
    pub body: TypeStyle,
    pub strong: TypeStyle,
    pub subtitle: TypeStyle,
    pub title: TypeStyle,
    pub code: TypeStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeStyle {
    pub size: f32,
    pub line_height: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub xxs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measures {
    pub readable_width: f32,
    pub dialog_width: f32,
    pub menu_min_width: f32,
    pub compact_menu_min_width: f32,
    pub menu_max_height: f32,
    pub compact_menu_max_height: f32,
    pub standalone_icon: f32,
    pub scrollbar_track: f32,
    pub scrollbar_thumb: f32,
    pub scrollbar_min_thumb: f32,
    pub caret_width: f32,
    pub text_decoration_width: f32,
    pub progress_track_height: f32,
    pub slider_track_height: f32,
    pub slider_vertical_height: f32,
    pub container_small: f32,
    pub container_medium: f32,
    pub container_large: f32,
    pub container_extra_large: f32,
    pub compact_overlay_width: f32,
    pub media_viewer_height: f32,
    pub timeline_rail_width: f32,
    pub status_mark: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radii {
    pub small: f32,
    pub control: f32,
    pub card: f32,
    pub dialog: f32,
    pub bubble: f32,
    pub pill: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Control {
    pub xs: ControlMetrics,
    pub sm: ControlMetrics,
    pub md: ControlMetrics,
    pub lg: ControlMetrics,
}

impl Control {
    pub fn get(&self, size: ControlSize) -> ControlMetrics {
        match size {
            ControlSize::Xs => self.xs,
            ControlSize::Sm => self.sm,
            ControlSize::Md => self.md,
            ControlSize::Lg => self.lg,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMetrics {
    pub height: f32,
    pub padding_x: f32,
    pub gap: f32,
    pub font_size: f32,
    pub icon_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Borders {
    pub hairline: f32,
    pub thick: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opacity {
    pub disabled: f32,
    pub muted: f32,
    pub scrim: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub instant_ms: u64,
    pub quick_ms: u64,
    pub menu_ms: u64,
    pub dialog_ms: u64,
    pub resize_ms: u64,
    pub entrance_ms: u64,
    pub spin_ms: u64,
    pub slow_ms: u64,
    /// The gap between one member of a staggered group and the next.
    pub stagger_step_ms: u64,
    pub stagger_max_items: usize,
    pub micro_bounce_ms: u64,
    pub micro_wobble_ms: u64,
    pub micro_pop_ms: u64,
    pub pulse_ms: u64,
    pub shimmer_ms: u64,
    pub toast_ms: u64,
    pub hover_card_open_ms: u64,
    pub hover_card_grace_ms: u64,
    pub feedback_ms: u64,
    pub celebration_ms: u64,
    pub confirmation_ms: u64,
    pub linear: [f32; 4],
    pub standard: [f32; 4],
    pub ease_in: [f32; 4],
    pub ease_out: [f32; 4],
    pub ease_in_out: [f32; 4],
    pub emphasized: [f32; 4],
    pub overshoot: [f32; 4],
    pub exit: [f32; 4],
    pub settle: [f32; 4],
    pub snappy: SpringTokens,
    pub smooth: SpringTokens,
    pub bouncy: SpringTokens,
    pub grab: SpringTokens,
    /// How far a pressed control sinks, in pixels.
    pub press_offset: f32,
    /// How far a hovered control rises, in pixels.
    pub hover_lift: f32,
    /// The speed past which a released gesture is a flick, in pixels a second.
    pub flick_velocity: f32,
    /// How much of an overscroll is shown at the boundary.
    pub rubber_band_tension: f32,
}

/// Shadows for each elevation step. Flat is intentionally empty rather than a
/// transparent shadow, so a flat surface allocates no shadow work at all.
#[derive(Debug, Clone, PartialEq)]
pub struct Elevations {
    pub flat: Vec<BoxShadow>,
    pub raised: Vec<BoxShadow>,
    pub overlay: Vec<BoxShadow>,
    pub modal: Vec<BoxShadow>,
}

impl Elevations {
    pub fn get(&self, level: Elevation) -> &[BoxShadow] {
        match level {
            Elevation::Flat => &self.flat,
            Elevation::Raised => &self.raised,
            Elevation::Overlay => &self.overlay,
            Elevation::Modal => &self.modal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZIndices {
    pub content: i32,
    pub sticky: i32,
    pub dock: i32,
    pub popover: i32,
    pub tooltip: i32,
    pub modal: i32,
    pub toast: i32,
}

impl ZIndices {
    pub fn get(&self, layer: Layer) -> i32 {
        match layer {
            Layer::Content => self.content,
            Layer::Sticky => self.sticky,
            Layer::Dock => self.dock,
            Layer::Popover => self.popover,
            Layer::Tooltip => self.tooltip,
            Layer::Modal => self.modal,
            Layer::Toast => self.toast,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effects {
    pub edge_fade_band: f32,
    pub selected_ring_alpha: f32,
    pub selection_rail_width: f32,
    pub focus_ring_width: f32,
    pub focus_ring_alpha: f32,
    pub glow_alpha: f32,
    pub glow_blur: f32,
    /// The bloom budget, negative: how far a state glow is pulled in before
    /// it is blurred, so it stays out of its neighbours' pixels.
    pub glow_spread: f32,
    pub glass_alpha: f32,
    pub glass_frost_blur: f32,
    pub glass_bevel_ratio: f32,
    pub glass_bevel_min: f32,
    pub glass_bevel_max: f32,
    pub glass_refraction: f32,
    pub glass_dispersion: f32,
    pub glass_specular: f32,
    pub glass_transmission_gain: f32,
    pub glass_optical_lift: f32,
    pub glass_hairline: f32,
    pub glass_specular_sharpness: f32,
    pub glass_light_angle: f32,
    pub glass_merge_distance: f32,
    pub glass_contrast_flip_low: f32,
    pub glass_contrast_flip_high: f32,
    pub glass_press_depth: f32,
    /// How strongly a raised surface catches light along its top edge. The
    /// gradient itself is composed by the component, the way [`Theme::glow`]
    /// composes a bloom from a colour and an alpha.
    pub sheen_alpha: f32,
    /// The alpha an area fill starts at under a chart line, fading to nothing
    /// at the baseline.
    pub area_wash_alpha: f32,
    /// How strongly a node header band takes its category colour.
    pub header_tint_alpha: f32,
    pub node_active_wash_alpha: f32,
    pub node_active_stroke_alpha: f32,
    pub node_traffic_alpha: f32,
    pub node_preview_alpha: f32,
    pub node_minimap_alpha: f32,
    /// How wide an identity rail is, in pixels: a node's category stripe, a
    /// callout's edge. Distinct from `selection_rail_width`, which reports a
    /// transient state rather than what a thing is.
    pub rail_width: f32,
    pub semantic_wash_faint_alpha: f32,
    pub semantic_wash_alpha: f32,
    pub semantic_wash_strong_alpha: f32,
    pub semantic_border_alpha: f32,
    pub accent_border_alpha: f32,
    pub accent_border_strong_alpha: f32,
    pub subtle_hover_alpha: f32,
    pub soft_contrast_alpha: f32,
    pub contrast_tint_alpha: f32,
    pub track_resting_alpha: f32,
    pub content_veil_alpha: f32,
    pub critical_fill_alpha: f32,
    pub critical_inactive_alpha: f32,
    pub variant_light_alpha: f32,
    pub variant_light_hover_alpha: f32,
    pub variant_light_active_alpha: f32,
    pub variant_outline_hover_alpha: f32,
    pub variant_outline_active_alpha: f32,
    pub variant_subtle_hover_alpha: f32,
    pub variant_subtle_active_alpha: f32,
    pub primary_hover_opacity: f32,
    pub custom_color_readable_dark_floor: f32,
    pub custom_color_readable_light_ceiling: f32,
    pub custom_color_hover_lightness_delta: f32,
    pub custom_color_active_lightness_delta: f32,
}

impl Theme {
    pub fn studio_dark() -> Self {
        Self::from_tokens(gpui_kit_tokens::studio_dark(), Density::default())
    }

    pub fn studio_light() -> Self {
        Self::from_tokens(gpui_kit_tokens::studio_light(), Density::default())
    }

    /// Builds a theme from any validated token document at one density.
    ///
    /// Density scales spacing, control geometry and type independently, and
    /// rounds to whole pixels so compact layouts stay on the pixel grid.
    pub fn from_tokens(tokens: &TokenDocument, density: Density) -> Self {
        let scale = tokens.density(density);
        let style = |step| {
            let step = tokens.type_step(step);
            TypeStyle {
                size: scale_font(step.size, scale),
                line_height: scale_font(step.line_height, scale),
                weight: step.weight,
            }
        };
        Self {
            id: tokens.meta.id.clone().into(),
            name: tokens.meta.name.clone().into(),
            appearance: tokens.meta.appearance,
            density,
            colors: Colors {
                backdrop: color(tokens.surface(Surface::Backdrop)),
                canvas: color(tokens.surface(Surface::Canvas)),
                sunken: color(tokens.surface(Surface::Sunken)),
                panel: color(tokens.surface(Surface::Panel)),
                raised: color(tokens.surface(Surface::Raised)),
                overlay: color(tokens.surface(Surface::Overlay)),
                scrim: color(tokens.scrim()),
                text: color(tokens.text(TextTone::Primary)),
                text_muted: color(tokens.text(TextTone::Muted)),
                text_faint: color(tokens.text(TextTone::Faint)),
                text_placeholder: color(tokens.text(TextTone::Placeholder)),
                text_disabled: color(tokens.text(TextTone::Disabled)),
                text_on_accent: color(tokens.text(TextTone::OnAccent)),
                primary_fill: color(tokens.interactive(InteractiveColor::PrimaryFill)),
                white_fill: color(tokens.interactive(InteractiveColor::WhiteFill)),
                white_fill_hover: color(tokens.interactive(InteractiveColor::WhiteFillHover)),
                white_fill_active: color(tokens.interactive(InteractiveColor::WhiteFillActive)),
                text_on_primary_fill: color(tokens.text(TextTone::OnPrimaryFill)),
                hover: color(tokens.interactive(InteractiveColor::Hover)),
                active: color(tokens.interactive(InteractiveColor::Active)),
                selected: color(tokens.interactive(InteractiveColor::Selected)),
                hairline: color(tokens.interactive(InteractiveColor::Hairline)),
                hairline_strong: color(tokens.interactive(InteractiveColor::HairlineStrong)),
                track: color(tokens.interactive(InteractiveColor::Track)),
                divider: color(tokens.interactive(InteractiveColor::Divider)),
                focus: color(tokens.interactive(InteractiveColor::Focus)),
                accent: color(tokens.semantic(SemanticColor::Accent)),
                accent_strong: color(tokens.semantic(SemanticColor::AccentStrong)),
                danger: color(tokens.semantic(SemanticColor::Danger)),
                warning: color(tokens.semantic(SemanticColor::Warning)),
                success: color(tokens.semantic(SemanticColor::Success)),
                info: color(tokens.semantic(SemanticColor::Info)),
                loader_mark: color(tokens.loader(LoaderColor::Mark)),
                loader_track: color(tokens.loader(LoaderColor::Track)),
                loader_placeholder: color(tokens.loader(LoaderColor::Placeholder)),
                loader_sheen: color(tokens.loader(LoaderColor::Sheen)),
                sequence: SequenceScale {
                    categorical: tokens.sequence().into_iter().map(color).collect(),
                },
                node: NodePalette {
                    header_wash: color(tokens.node(NodeColor::HeaderWash)),
                    port_idle: color(tokens.node(NodeColor::PortIdle)),
                    port_hover: color(tokens.node(NodeColor::PortHover)),
                    port_connected: color(tokens.node(NodeColor::PortConnected)),
                    edge: color(tokens.node(NodeColor::Edge)),
                    edge_active: color(tokens.node(NodeColor::EdgeActive)),
                    edge_feedback: color(tokens.node(NodeColor::EdgeFeedback)),
                    edge_feedback_active: color(tokens.node(NodeColor::EdgeFeedbackActive)),
                    label_wash: color(tokens.node(NodeColor::LabelWash)),
                    grid: color(tokens.node(NodeColor::Grid)),
                    grid_strong: color(tokens.node(NodeColor::GridStrong)),
                    grid_axis: color(tokens.node(NodeColor::GridAxis)),
                },
                agent: AgentPalette {
                    read: color(tokens.agent(AgentColor::Read)),
                    network: color(tokens.agent(AgentColor::Network)),
                    shell: color(tokens.agent(AgentColor::Shell)),
                    edit: color(tokens.agent(AgentColor::Edit)),
                    external: color(tokens.agent(AgentColor::External)),
                    evidence_wash: color(tokens.agent(AgentColor::EvidenceWash)),
                },
                syntax: SyntaxPalette {
                    keyword: color(tokens.syntax(SyntaxColor::Keyword)),
                    string: color(tokens.syntax(SyntaxColor::StringLiteral)),
                    comment: color(tokens.syntax(SyntaxColor::Comment)),
                    number: color(tokens.syntax(SyntaxColor::Number)),
                    inline: color(tokens.syntax(SyntaxColor::Inline)),
                    inline_wash: color(tokens.syntax(SyntaxColor::InlineWash)),
                    added: color(tokens.syntax(SyntaxColor::Added)),
                    added_wash: color(tokens.syntax(SyntaxColor::AddedWash)),
                    removed: color(tokens.syntax(SyntaxColor::Removed)),
                    removed_wash: color(tokens.syntax(SyntaxColor::RemovedWash)),
                },
                terminal_background: color(tokens.terminal_background()),
                terminal_selection: color(tokens.terminal_selection()),
                terminal_ansi: tokens.terminal_ansi().map(color),
            },
            typography: Typography {
                sans: tokens.typography.sans.family.clone().into(),
                sans_fallback: tokens
                    .typography
                    .sans
                    .platform_fallback()
                    .to_string()
                    .into(),
                mono: tokens.typography.mono.family.clone().into(),
                mono_fallback: tokens
                    .typography
                    .mono
                    .platform_fallback()
                    .to_string()
                    .into(),
                readout_scale: tokens.typography.readout_scale,
                caption: style(TypeScale::Caption),
                label: style(TypeScale::Label),
                body: style(TypeScale::Body),
                strong: style(TypeScale::Strong),
                subtitle: style(TypeScale::Subtitle),
                title: style(TypeScale::Title),
                code: style(TypeScale::Code),
            },
            spacing: Spacing {
                xxs: scale_space(tokens.spacing(Space::Xxs), scale),
                xs: scale_space(tokens.spacing(Space::Xs), scale),
                sm: scale_space(tokens.spacing(Space::Sm), scale),
                md: scale_space(tokens.spacing(Space::Md), scale),
                lg: scale_space(tokens.spacing(Space::Lg), scale),
                xl: scale_space(tokens.spacing(Space::Xl), scale),
                xxl: scale_space(tokens.spacing(Space::Xxl), scale),
            },
            measures: Measures {
                readable_width: tokens.measure.readable_width,
                dialog_width: tokens.measure.dialog_width,
                menu_min_width: tokens.measure.menu_min_width,
                compact_menu_min_width: tokens.measure.compact_menu_min_width,
                menu_max_height: tokens.measure.menu_max_height,
                compact_menu_max_height: tokens.measure.compact_menu_max_height,
                standalone_icon: tokens.measure.standalone_icon,
                scrollbar_track: tokens.measure.scrollbar_track,
                scrollbar_thumb: tokens.measure.scrollbar_thumb,
                scrollbar_min_thumb: tokens.measure.scrollbar_min_thumb,
                caret_width: tokens.measure.caret_width,
                text_decoration_width: tokens.measure.text_decoration_width,
                progress_track_height: tokens.measure.progress_track_height,
                slider_track_height: tokens.measure.slider_track_height,
                slider_vertical_height: tokens.measure.slider_vertical_height,
                container_small: tokens.measure.container_small,
                container_medium: tokens.measure.container_medium,
                container_large: tokens.measure.container_large,
                container_extra_large: tokens.measure.container_extra_large,
                compact_overlay_width: tokens.measure.compact_overlay_width,
                media_viewer_height: tokens.measure.media_viewer_height,
                timeline_rail_width: tokens.measure.timeline_rail_width,
                status_mark: tokens.measure.status_mark,
            },
            radii: Radii {
                small: tokens.radius(Radius::Small),
                control: tokens.radius(Radius::Control),
                card: tokens.radius(Radius::Card),
                dialog: tokens.radius(Radius::Dialog),
                bubble: tokens.radius(Radius::Bubble),
                pill: tokens.radius(Radius::Pill),
            },
            control: {
                let metrics = |size| {
                    let step = tokens.control(size);
                    ControlMetrics {
                        height: scale_control(step.height, scale),
                        padding_x: scale_control(step.padding_x, scale),
                        gap: scale_control(step.gap, scale),
                        font_size: scale_font(step.font_size, scale),
                        icon_size: scale_control(step.icon_size, scale),
                    }
                };
                Control {
                    xs: metrics(ControlSize::Xs),
                    sm: metrics(ControlSize::Sm),
                    md: metrics(ControlSize::Md),
                    lg: metrics(ControlSize::Lg),
                }
            },
            borders: Borders {
                hairline: tokens.border_width(BorderWeight::Hairline),
                thick: tokens.border_width(BorderWeight::Thick),
            },
            opacity: Opacity {
                disabled: tokens.opacity(OpacityRole::Disabled),
                muted: tokens.opacity(OpacityRole::Muted),
                scrim: tokens.opacity(OpacityRole::Scrim),
            },
            motion: Motion {
                instant_ms: millis(tokens, MotionDuration::Instant),
                quick_ms: millis(tokens, MotionDuration::Quick),
                menu_ms: millis(tokens, MotionDuration::Menu),
                dialog_ms: millis(tokens, MotionDuration::Dialog),
                resize_ms: millis(tokens, MotionDuration::Resize),
                entrance_ms: millis(tokens, MotionDuration::Entrance),
                spin_ms: millis(tokens, MotionDuration::Spin),
                slow_ms: millis(tokens, MotionDuration::Slow),
                stagger_step_ms: millis(tokens, MotionDuration::StaggerStep),
                stagger_max_items: tokens.motion.stagger_max_items,
                micro_bounce_ms: millis(tokens, MotionDuration::MicroBounce),
                micro_wobble_ms: millis(tokens, MotionDuration::MicroWobble),
                micro_pop_ms: millis(tokens, MotionDuration::MicroPop),
                pulse_ms: millis(tokens, MotionDuration::Pulse),
                shimmer_ms: millis(tokens, MotionDuration::Shimmer),
                toast_ms: millis(tokens, MotionDuration::Toast),
                hover_card_open_ms: millis(tokens, MotionDuration::HoverCardOpen),
                hover_card_grace_ms: millis(tokens, MotionDuration::HoverCardGrace),
                feedback_ms: millis(tokens, MotionDuration::Feedback),
                celebration_ms: millis(tokens, MotionDuration::Celebration),
                confirmation_ms: millis(tokens, MotionDuration::Confirmation),
                linear: tokens.easing(MotionEasing::Linear),
                standard: tokens.easing(MotionEasing::Standard),
                ease_in: tokens.easing(MotionEasing::EaseIn),
                ease_out: tokens.easing(MotionEasing::EaseOut),
                ease_in_out: tokens.easing(MotionEasing::EaseInOut),
                emphasized: tokens.easing(MotionEasing::Emphasized),
                overshoot: tokens.easing(MotionEasing::Overshoot),
                exit: tokens.easing(MotionEasing::Exit),
                settle: tokens.easing(MotionEasing::Settle),
                snappy: tokens.spring(SpringPreset::Snappy),
                smooth: tokens.spring(SpringPreset::Smooth),
                bouncy: tokens.spring(SpringPreset::Bouncy),
                grab: tokens.spring(SpringPreset::Grab),
                press_offset: tokens.press_offset(),
                hover_lift: tokens.hover_lift(),
                flick_velocity: tokens.flick_velocity(),
                rubber_band_tension: tokens.rubber_band_tension(),
            },
            elevation: Elevations {
                flat: shadow(tokens, Elevation::Flat),
                raised: shadow(tokens, Elevation::Raised),
                overlay: shadow(tokens, Elevation::Overlay),
                modal: shadow(tokens, Elevation::Modal),
            },
            z_index: ZIndices {
                content: tokens.z_index(Layer::Content),
                sticky: tokens.z_index(Layer::Sticky),
                dock: tokens.z_index(Layer::Dock),
                popover: tokens.z_index(Layer::Popover),
                tooltip: tokens.z_index(Layer::Tooltip),
                modal: tokens.z_index(Layer::Modal),
                toast: tokens.z_index(Layer::Toast),
            },
            effects: Effects {
                edge_fade_band: tokens.effect.edge_fade_band,
                selected_ring_alpha: tokens.effect.selected_ring_alpha,
                selection_rail_width: tokens.effect.selection_rail_width,
                focus_ring_width: tokens.effect.focus_ring_width,
                focus_ring_alpha: tokens.effect.focus_ring_alpha,
                glow_alpha: tokens.effect.glow_alpha,
                glow_blur: tokens.effect.glow_blur,
                glow_spread: tokens.effect.glow_spread,
                glass_alpha: tokens.effect.glass_alpha,
                glass_frost_blur: tokens.effect.glass_frost_blur,
                glass_bevel_ratio: tokens.effect.glass_bevel_ratio,
                glass_bevel_min: tokens.effect.glass_bevel_min,
                glass_bevel_max: tokens.effect.glass_bevel_max,
                glass_refraction: tokens.effect.glass_refraction,
                glass_dispersion: tokens.effect.glass_dispersion,
                glass_specular: tokens.effect.glass_specular,
                glass_transmission_gain: tokens.effect.glass_transmission_gain,
                glass_optical_lift: tokens.effect.glass_optical_lift,
                glass_hairline: tokens.effect.glass_hairline,
                glass_specular_sharpness: tokens.effect.glass_specular_sharpness,
                glass_light_angle: tokens.effect.glass_light_angle,
                glass_merge_distance: tokens.effect.glass_merge_distance,
                glass_contrast_flip_low: tokens.effect.glass_contrast_flip_low,
                glass_contrast_flip_high: tokens.effect.glass_contrast_flip_high,
                glass_press_depth: tokens.effect.glass_press_depth,
                sheen_alpha: tokens.effect.sheen_alpha,
                area_wash_alpha: tokens.effect.area_wash_alpha,
                header_tint_alpha: tokens.effect.header_tint_alpha,
                node_active_wash_alpha: tokens.effect.node_active_wash_alpha,
                node_active_stroke_alpha: tokens.effect.node_active_stroke_alpha,
                node_traffic_alpha: tokens.effect.node_traffic_alpha,
                node_preview_alpha: tokens.effect.node_preview_alpha,
                node_minimap_alpha: tokens.effect.node_minimap_alpha,
                rail_width: tokens.effect.rail_width,
                semantic_wash_faint_alpha: tokens.effect.semantic_wash_faint_alpha,
                semantic_wash_alpha: tokens.effect.semantic_wash_alpha,
                semantic_wash_strong_alpha: tokens.effect.semantic_wash_strong_alpha,
                semantic_border_alpha: tokens.effect.semantic_border_alpha,
                accent_border_alpha: tokens.effect.accent_border_alpha,
                accent_border_strong_alpha: tokens.effect.accent_border_strong_alpha,
                subtle_hover_alpha: tokens.effect.subtle_hover_alpha,
                soft_contrast_alpha: tokens.effect.soft_contrast_alpha,
                contrast_tint_alpha: tokens.effect.contrast_tint_alpha,
                track_resting_alpha: tokens.effect.track_resting_alpha,
                content_veil_alpha: tokens.effect.content_veil_alpha,
                critical_fill_alpha: tokens.effect.critical_fill_alpha,
                critical_inactive_alpha: tokens.effect.critical_inactive_alpha,
                variant_light_alpha: tokens.effect.variant_light_alpha,
                variant_light_hover_alpha: tokens.effect.variant_light_hover_alpha,
                variant_light_active_alpha: tokens.effect.variant_light_active_alpha,
                variant_outline_hover_alpha: tokens.effect.variant_outline_hover_alpha,
                variant_outline_active_alpha: tokens.effect.variant_outline_active_alpha,
                variant_subtle_hover_alpha: tokens.effect.variant_subtle_hover_alpha,
                variant_subtle_active_alpha: tokens.effect.variant_subtle_active_alpha,
                primary_hover_opacity: tokens.effect.primary_hover_opacity,
                custom_color_readable_dark_floor: tokens.effect.custom_color_readable_dark_floor,
                custom_color_readable_light_ceiling: tokens
                    .effect
                    .custom_color_readable_light_ceiling,
                custom_color_hover_lightness_delta: tokens
                    .effect
                    .custom_color_hover_lightness_delta,
                custom_color_active_lightness_delta: tokens
                    .effect
                    .custom_color_active_lightness_delta,
            },
            palette: Arc::new(tokens.color.palette.clone()),
            palette_steps: PaletteSteps {
                filled: tokens.color.palette_steps.filled.clone(),
                hover: tokens.color.palette_steps.hover.clone(),
                active: tokens.color.palette_steps.active.clone(),
                readable_dark: tokens.color.palette_steps.readable_dark.clone(),
                readable_light: tokens.color.palette_steps.readable_light.clone(),
            },
        }
    }

    /// A palette entry, addressed as `"group.step"`.
    ///
    /// The typed roles are the vocabulary components paint from, and a
    /// component never reaches past them. This is for the application that
    /// has paint the shared vocabulary does not model — a colour per person,
    /// a syntax class, a diff sign — and wants it to live in the same token
    /// document, validated by the same parse and retinted by the same
    /// registry, instead of as literals in views.
    ///
    /// An entry the active document does not declare is `None`, never a
    /// guessed colour: a theme that has not named a scale has not agreed to
    /// paint it.
    pub fn palette_color(&self, path: &str) -> Option<Hsla> {
        let (group, step) = path.split_once('.')?;
        let value = self.palette.get(group)?.get(step)?;
        Color::resolve(path, value, &self.palette).ok().map(color)
    }

    pub fn surface(&self, surface: Surface) -> Hsla {
        match surface {
            Surface::Backdrop => self.colors.backdrop,
            Surface::Canvas => self.colors.canvas,
            Surface::Sunken => self.colors.sunken,
            Surface::Panel => self.colors.panel,
            Surface::Raised => self.colors.raised,
            Surface::Overlay => self.colors.overlay,
        }
    }

    pub fn text_color(&self, tone: TextTone) -> Hsla {
        match tone {
            TextTone::Primary => self.colors.text,
            TextTone::Muted => self.colors.text_muted,
            TextTone::Faint => self.colors.text_faint,
            TextTone::Placeholder => self.colors.text_placeholder,
            TextTone::Disabled => self.colors.text_disabled,
            TextTone::OnAccent => self.colors.text_on_accent,
            TextTone::OnPrimaryFill => self.colors.text_on_primary_fill,
        }
    }

    pub fn semantic_color(&self, color: SemanticColor) -> Hsla {
        match color {
            SemanticColor::Accent => self.colors.accent,
            SemanticColor::AccentStrong => self.colors.accent_strong,
            SemanticColor::Danger => self.colors.danger,
            SemanticColor::Warning => self.colors.warning,
            SemanticColor::Success => self.colors.success,
            SemanticColor::Info => self.colors.info,
        }
    }

    /// A semantic colour used as a background without becoming a solid fill.
    pub fn semantic_wash(&self, color: SemanticColor, strength: SemanticWash) -> Hsla {
        self.color_wash(self.semantic_color(color), strength)
    }

    /// A caller-owned colour used as a background without becoming a solid
    /// fill. The strength remains theme-owned even when the hue is not.
    pub fn color_wash(&self, color: Hsla, strength: SemanticWash) -> Hsla {
        let alpha = match strength {
            SemanticWash::Faint => self.effects.semantic_wash_faint_alpha,
            SemanticWash::Standard => self.effects.semantic_wash_alpha,
            SemanticWash::Strong => self.effects.semantic_wash_strong_alpha,
        };
        color.opacity(alpha)
    }

    /// A semantic boundary, with stronger tiers reserved for selected and
    /// actively targeted entities.
    pub fn semantic_border(&self, color: SemanticColor, strength: SemanticBorder) -> Hsla {
        self.color_border(self.semantic_color(color), strength)
    }

    /// A theme-owned boundary strength applied to a caller-owned colour.
    pub fn color_border(&self, color: Hsla, strength: SemanticBorder) -> Hsla {
        let alpha = match strength {
            SemanticBorder::Report => self.effects.semantic_border_alpha,
            SemanticBorder::Selected => self.effects.accent_border_alpha,
            SemanticBorder::Target => self.effects.accent_border_strong_alpha,
        };
        color.opacity(alpha)
    }

    pub fn subtle_hover(&self) -> Hsla {
        self.colors.hover.opacity(self.effects.subtle_hover_alpha)
    }

    pub fn contrast_tint(&self, color: Hsla, strength: ContrastTint) -> Hsla {
        let alpha = match strength {
            ContrastTint::Soft => self.effects.soft_contrast_alpha,
            ContrastTint::Standard => self.effects.contrast_tint_alpha,
        };
        color.blend(self.colors.text.opacity(alpha))
    }

    pub fn resting_track(&self) -> Hsla {
        self.colors.track.opacity(self.effects.track_resting_alpha)
    }

    pub fn content_veil(&self) -> Hsla {
        self.colors.canvas.opacity(self.effects.content_veil_alpha)
    }

    pub fn critical_window_fill(&self, active: bool) -> Hsla {
        if active {
            self.colors.danger.opacity(self.effects.critical_fill_alpha)
        } else {
            self.colors
                .text
                .opacity(self.effects.critical_inactive_alpha)
        }
    }

    pub fn space(&self, step: Space) -> f32 {
        match step {
            Space::Xxs => self.spacing.xxs,
            Space::Xs => self.spacing.xs,
            Space::Sm => self.spacing.sm,
            Space::Md => self.spacing.md,
            Space::Lg => self.spacing.lg,
            Space::Xl => self.spacing.xl,
            Space::Xxl => self.spacing.xxl,
        }
    }

    pub fn radius(&self, step: Radius) -> f32 {
        match step {
            Radius::Small => self.radii.small,
            Radius::Control => self.radii.control,
            Radius::Card => self.radii.card,
            Radius::Dialog => self.radii.dialog,
            Radius::Bubble => self.radii.bubble,
            Radius::Pill => self.radii.pill,
        }
    }

    pub fn type_style(&self, scale: TypeScale) -> TypeStyle {
        match scale {
            TypeScale::Caption => self.typography.caption,
            TypeScale::Label => self.typography.label,
            TypeScale::Body => self.typography.body,
            TypeScale::Strong => self.typography.strong,
            TypeScale::Subtitle => self.typography.subtitle,
            TypeScale::Title => self.typography.title,
            TypeScale::Code => self.typography.code,
        }
    }

    pub fn easing(&self, easing: MotionEasing) -> [f32; 4] {
        match easing {
            MotionEasing::Linear => self.motion.linear,
            MotionEasing::Standard => self.motion.standard,
            MotionEasing::EaseIn => self.motion.ease_in,
            MotionEasing::EaseOut => self.motion.ease_out,
            MotionEasing::EaseInOut => self.motion.ease_in_out,
            MotionEasing::Emphasized => self.motion.emphasized,
            MotionEasing::Overshoot => self.motion.overshoot,
            MotionEasing::Exit => self.motion.exit,
            MotionEasing::Settle => self.motion.settle,
        }
    }

    pub fn spring(&self, preset: SpringPreset) -> SpringTokens {
        match preset {
            SpringPreset::Snappy => self.motion.snappy,
            SpringPreset::Smooth => self.motion.smooth,
            SpringPreset::Bouncy => self.motion.bouncy,
            SpringPreset::Grab => self.motion.grab,
        }
    }

    pub fn shadow(&self, level: Elevation) -> &[BoxShadow] {
        self.elevation.get(level)
    }

    pub fn layer(&self, layer: Layer) -> i32 {
        self.z_index.get(layer)
    }

    /// Installs the bundled theme registry. Idempotent.
    pub fn install(cx: &mut App) {
        if !cx.has_global::<ThemeRegistry>() {
            cx.set_global(ThemeRegistry::new());
        }
    }

    /// The theme in force here, which is the innermost override if a subtree
    /// installed one and the registry's active theme otherwise.
    pub fn get(cx: &App) -> &Self {
        if let Some(overridden) = cx
            .try_global::<ThemeOverrides>()
            .and_then(|overrides| overrides.0.last())
        {
            return overridden;
        }
        cx.global::<ThemeRegistry>().active()
    }

    /// The ring drawn around whichever control currently has the keyboard.
    ///
    /// It spreads outward in the focus colour, so it reads differently from
    /// [`Self::selected_ring`]: focus says where the next keystroke goes,
    /// selection says which answer is current, and a reader that cannot tell
    /// the two apart cannot tell what pressing a key would do.
    pub fn focus_ring(&self) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.colors.focus.opacity(self.effects.focus_ring_alpha),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(self.effects.focus_ring_width),
            inset: false,
        }]
    }

    pub fn selected_ring(&self) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.colors.text.opacity(self.effects.selected_ring_alpha),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(self.borders.hairline),
            inset: true,
        }]
    }

    /// Resolves the shared presentation tiers for one colour.
    ///
    /// Every coloured component reads the same seven tiers through this one
    /// resolver, so "a light indigo chip" and "a light indigo button" agree
    /// on what light and indigo mean. The tier decides how loud the colour
    /// is; the colour itself is the caller's, named as a palette group, a
    /// semantic role, or an explicit paint.
    pub fn variant_colors(&self, variant: Variant, color: &ColorChoice) -> VariantColors {
        let transparent = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        };
        // Default is the neutral tier and carries no colour at all, so it is
        // resolved before the colour is: a colour on a Default control would
        // be a colour the control then refuses to show.
        if variant == Variant::Default {
            return VariantColors {
                background: self.colors.raised,
                background_hover: self.colors.active,
                background_active: self.colors.active,
                text: self.colors.text,
                border: None,
            };
        }

        let base = self.resolve_color_choice(color);
        let readable = self.readable_shade(color, base);
        let (hover, active) = self.pressed_shades(color, base);
        match variant {
            Variant::Default => unreachable!("resolved above"),
            Variant::Filled => {
                let backgrounds = [base, hover, active];
                let on_accent =
                    weakest_contrast(self.colors.text_on_accent, backgrounds, self.colors.raised);
                let prose = weakest_contrast(self.colors.text, backgrounds, self.colors.raised);
                VariantColors {
                    background: base,
                    background_hover: hover,
                    background_active: active,
                    text: if on_accent >= prose {
                        self.colors.text_on_accent
                    } else {
                        self.colors.text
                    },
                    border: None,
                }
            }
            Variant::Light => VariantColors {
                background: base.opacity(self.effects.variant_light_alpha),
                background_hover: base.opacity(self.effects.variant_light_hover_alpha),
                background_active: base.opacity(self.effects.variant_light_active_alpha),
                text: readable,
                border: None,
            },
            Variant::Outline => VariantColors {
                background: transparent,
                background_hover: base.opacity(self.effects.variant_outline_hover_alpha),
                background_active: base.opacity(self.effects.variant_outline_active_alpha),
                text: readable,
                border: Some(readable),
            },
            Variant::Subtle => VariantColors {
                background: transparent,
                background_hover: base.opacity(self.effects.variant_subtle_hover_alpha),
                background_active: base.opacity(self.effects.variant_subtle_active_alpha),
                text: readable,
                border: None,
            },
            Variant::Transparent => VariantColors {
                background: transparent,
                background_hover: transparent,
                background_active: transparent,
                text: readable,
                border: None,
            },
            Variant::White => VariantColors {
                background: self.colors.white_fill,
                background_hover: self.colors.white_fill_hover,
                background_active: self.colors.white_fill_active,
                text: base,
                border: None,
            },
        }
    }

    /// The single paint a colour choice stands for in this theme.
    ///
    /// A palette group resolves to its filled step; a group the active
    /// document does not carry falls back to the accent rather than to a
    /// guessed colour, so an unrecognised name is visible as "the theme's
    /// own colour" instead of as an arbitrary one.
    fn resolve_color_choice(&self, color: &ColorChoice) -> Hsla {
        match color {
            ColorChoice::Semantic(role) => self.semantic_color(*role),
            ColorChoice::Custom(paint) => *paint,
            ColorChoice::Palette(group) => self
                .ramp_step(group, &self.palette_steps.filled)
                .unwrap_or(self.colors.accent),
        }
    }

    /// The shade of the colour that reads as text on this theme's surfaces.
    fn readable_shade(&self, color: &ColorChoice, base: Hsla) -> Hsla {
        if let ColorChoice::Palette(group) = color {
            let steps = match self.appearance {
                Appearance::Dark => &self.palette_steps.readable_dark,
                Appearance::Light => &self.palette_steps.readable_light,
            };
            if let Some(shade) = self.ramp_step(group, steps) {
                return shade;
            }
        }
        match self.appearance {
            Appearance::Dark => Hsla {
                l: base.l.max(self.effects.custom_color_readable_dark_floor),
                ..base
            },
            Appearance::Light => Hsla {
                l: base.l.min(self.effects.custom_color_readable_light_ceiling),
                ..base
            },
        }
    }

    /// The hover and pressed shades of a filled colour.
    fn pressed_shades(&self, color: &ColorChoice, base: Hsla) -> (Hsla, Hsla) {
        if let ColorChoice::Palette(group) = color
            && let (Some(hover), Some(active)) = (
                self.ramp_step(group, &self.palette_steps.hover),
                self.ramp_step(group, &self.palette_steps.active),
            )
        {
            return (hover, active);
        }
        (
            Hsla {
                l: (base.l - self.effects.custom_color_hover_lightness_delta).max(0.0),
                ..base
            },
            Hsla {
                l: (base.l - self.effects.custom_color_active_lightness_delta).max(0.0),
                ..base
            },
        )
    }

    /// The first step of `preferred` the active palette carries for `group`.
    fn ramp_step(&self, group: &str, preferred: &[String]) -> Option<Hsla> {
        let steps = self.palette.get(group)?;
        preferred
            .iter()
            .find_map(|step| steps.get(step).map(|value| (step.as_str(), value.as_str())))
            .and_then(|(step, value)| {
                Color::resolve(&format!("{group}.{step}"), value, &self.palette)
                    .ok()
                    .map(color)
            })
    }

    /// The colour a surface in a named state bleeds into the pixels around it.
    ///
    /// It is the state itself made visible at the edge, which is what lets a
    /// surface report "running" or "failed" without a border drawn round it.
    /// Blurred and unoffset, so nothing about it reads as a line.
    ///
    /// Pulled in by the bloom budget before it is blurred: an unpulled glow
    /// puts its full alpha on the surface's own edge and reaches its whole
    /// blur past it, which is how a failed panel came to tint the panel
    /// beside it.
    pub fn glow(&self, color: Hsla) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: color.opacity(self.effects.glow_alpha),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(self.effects.glow_blur),
            spread_radius: px(self.effects.glow_spread),
            inset: false,
        }]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::studio_dark()
    }
}

/// The stack of subtree theme overrides, innermost last.
///
/// A component reads [`Theme::get`] and cannot tell whether the theme it got
/// came from the registry or from an ancestor that overrode it, which is what
/// makes the escape hatch work without every component learning about it.
#[derive(Default)]
pub struct ThemeOverrides(Vec<Theme>);

impl Global for ThemeOverrides {}

/// Installs `theme` for everything rendered until the matching [`pop_theme`].
///
/// This is the low half of the subtree escape hatch. Callers use
/// `gpui_kit::ThemeOverlay`, which pairs the two around one child and cannot
/// leak an override into a sibling.
pub fn push_theme(cx: &mut App, theme: Theme) {
    if !cx.has_global::<ThemeOverrides>() {
        cx.set_global(ThemeOverrides::default());
    }
    cx.update_global::<ThemeOverrides, ()>(|overrides, _| overrides.0.push(theme));
}

/// Removes the innermost override installed by [`push_theme`].
pub fn pop_theme(cx: &mut App) {
    if cx.has_global::<ThemeOverrides>() {
        cx.update_global::<ThemeOverrides, ()>(|overrides, _| {
            overrides.0.pop();
        });
    }
}

/// The set of themes an application can switch between at runtime.
#[derive(Debug)]
pub struct ThemeRegistry {
    tokens: Vec<Arc<TokenDocument>>,
    active: usize,
    density: Density,
    theme: Theme,
}

impl Global for ThemeRegistry {}

impl ThemeRegistry {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn new() -> Self {
        let tokens: Vec<Arc<TokenDocument>> = bundled()
            .into_iter()
            .chain(presets())
            .map(|document| Arc::new(document.clone()))
            .collect();
        let theme = Theme::from_tokens(&tokens[0], Density::default());
        Self {
            tokens,
            active: 0,
            density: Density::default(),
            theme,
        }
    }

    /// Adds or replaces a theme. A registered id replaces the earlier document
    /// so an application can override a bundled theme without shadowing it.
    pub fn register(&mut self, tokens: TokenDocument) {
        let id = tokens.meta.id.clone();
        match self.tokens.iter().position(|other| other.meta.id == id) {
            Some(index) => self.tokens[index] = Arc::new(tokens),
            None => self.tokens.push(Arc::new(tokens)),
        }
        self.rebuild();
    }

    pub fn register_json(&mut self, json: &str) -> Result<(), TokenError> {
        self.register(TokenDocument::parse(json)?);
        Ok(())
    }

    pub fn ids(&self) -> Vec<SharedString> {
        self.tokens
            .iter()
            .map(|tokens| SharedString::from(tokens.meta.id.clone()))
            .collect()
    }

    pub fn active(&self) -> &Theme {
        &self.theme
    }

    pub fn density(&self) -> Density {
        self.density
    }

    /// Returns false when the id is not registered, leaving the active theme
    /// untouched rather than falling back to a default the caller did not ask
    /// for.
    pub fn activate(&mut self, id: &str) -> bool {
        let Some(index) = self.tokens.iter().position(|tokens| tokens.meta.id == id) else {
            return false;
        };
        self.active = index;
        self.rebuild();
        true
    }

    pub fn set_density(&mut self, density: Density) {
        self.density = density;
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.theme = Theme::from_tokens(&self.tokens[self.active], self.density);
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Switches the active theme and repaints every window.
pub fn activate_theme(id: &str, cx: &mut App) -> bool {
    let switched = cx.update_global::<ThemeRegistry, bool>(|registry, _| registry.activate(id));
    if switched {
        cx.refresh_windows();
    }
    switched
}

/// Changes the density axis and repaints every window.
pub fn set_density(density: Density, cx: &mut App) {
    cx.update_global::<ThemeRegistry, ()>(|registry, _| registry.set_density(density));
    cx.refresh_windows();
}

fn scale_space(value: f32, scale: DensityScale) -> f32 {
    (value * scale.space).round().max(1.0)
}

fn scale_control(value: f32, scale: DensityScale) -> f32 {
    (value * scale.control).round().max(1.0)
}

fn scale_font(value: f32, scale: DensityScale) -> f32 {
    ((value * scale.font) * 2.0).round() / 2.0
}

fn shadow(tokens: &TokenDocument, level: Elevation) -> Vec<BoxShadow> {
    tokens
        .elevation(level)
        .layers
        .into_iter()
        .filter(|layer| layer.color.alpha != 0.0)
        .map(|layer| BoxShadow {
            color: color(layer.color),
            offset: point(px(0.0), px(layer.y)),
            blur_radius: px(layer.blur),
            spread_radius: px(layer.spread),
            inset: false,
        })
        .collect()
}

fn millis(tokens: &gpui_kit_tokens::TokenDocument, step: MotionDuration) -> u64 {
    tokens.motion_duration(step).as_millis() as u64
}

fn color(value: Color) -> Hsla {
    Hsla::from(Rgba {
        r: value.red,
        g: value.green,
        b: value.blue,
        a: value.alpha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The library groups content with colour rather than with a line drawn
    /// round it, so a step between two surfaces has to be visible on its own.
    /// Below this the step stops reading and a border is doing the work
    /// instead, which is the arrangement this threshold exists to prevent
    /// anyone drifting back into.
    const MIN_SURFACE_STEP: f32 = 0.02;

    /// Every theme the library ships, not only the pair it designs against.
    ///
    /// A preset is not decoration once a component stops drawing a line to
    /// help its surfaces apart: a field in Nord is carried by the same step a
    /// field in studio-dark is, so a preset that does not clear the floor
    /// ships a control nobody can find.
    fn shipped_themes() -> Vec<Theme> {
        gpui_kit_tokens::all()
            .into_iter()
            .map(|tokens| Theme::from_tokens(tokens, Density::Comfortable))
            .collect()
    }

    #[test]
    fn a_surface_step_is_visible_without_a_border_to_help_it() {
        for theme in shipped_themes() {
            let steps = [
                (
                    "backdrop to canvas",
                    theme.colors.backdrop,
                    theme.colors.canvas,
                ),
                // The step a field stands on. `StyledExt::well` holds its
                // border transparent, so this is the whole resting statement
                // that an editable control is one.
                ("canvas to sunken", theme.colors.sunken, theme.colors.canvas),
                ("sunken to panel", theme.colors.sunken, theme.colors.panel),
                ("canvas to panel", theme.colors.canvas, theme.colors.panel),
            ];
            for (name, lower, upper) in steps {
                assert!(
                    (upper.l - lower.l).abs() >= MIN_SURFACE_STEP,
                    "{} in {}: {} is too close to {} to group anything",
                    name,
                    theme.id,
                    lower.l,
                    upper.l
                );
            }
        }
    }

    #[test]
    fn every_theme_separates_surfaces_and_text_emphasis() {
        for theme in shipped_themes() {
            assert!(theme.colors.backdrop.l < theme.colors.canvas.l);
            assert!(theme.colors.sunken.l < theme.colors.panel.l);
            assert!(theme.colors.canvas.l < theme.colors.panel.l);
            assert!(theme.colors.panel.l <= theme.colors.raised.l);

            // Emphasis is distance from the canvas, which inverts with
            // appearance, so compare magnitudes rather than raw lightness.
            let emphasis = |tone: Hsla| (tone.l - theme.colors.canvas.l).abs();
            assert!(emphasis(theme.colors.text) > emphasis(theme.colors.text_muted));
            assert!(emphasis(theme.colors.text_muted) > emphasis(theme.colors.text_faint));
        }
    }

    #[test]
    fn compact_density_shrinks_geometry_without_touching_color() {
        let comfortable = Theme::from_tokens(gpui_kit_tokens::studio_dark(), Density::Comfortable);
        let compact = Theme::from_tokens(gpui_kit_tokens::studio_dark(), Density::Compact);
        assert!(compact.spacing.lg < comfortable.spacing.lg);
        assert!(
            compact.control.get(ControlSize::Md).height
                < comfortable.control.get(ControlSize::Md).height
        );
        assert!(compact.typography.body.size < comfortable.typography.body.size);
        assert_eq!(compact.colors.accent, comfortable.colors.accent);
        assert_eq!(compact.radii.card, comfortable.radii.card);
    }

    #[test]
    fn density_keeps_geometry_on_the_pixel_grid() {
        let compact = Theme::from_tokens(gpui_kit_tokens::studio_dark(), Density::Compact);
        for size in ControlSize::ALL {
            let metrics = compact.control.get(size);
            assert_eq!(metrics.height.fract(), 0.0);
            assert_eq!(metrics.padding_x.fract(), 0.0);
        }
        assert_eq!(compact.spacing.md.fract(), 0.0);
    }

    #[test]
    fn variant_tiers_share_one_resolver() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let indigo: ColorChoice = "indigo".into();
            let filled = theme.variant_colors(Variant::Filled, &indigo);
            let light = theme.variant_colors(Variant::Light, &indigo);
            let outline = theme.variant_colors(Variant::Outline, &indigo);
            let subtle = theme.variant_colors(Variant::Subtle, &indigo);
            let transparent = theme.variant_colors(Variant::Transparent, &indigo);

            // Filled is the loud tier: a solid block whose text pole keeps
            // the stronger weakest contrast across rest, hover, and active.
            assert_eq!(filled.background.a, 1.0);
            let backgrounds = [
                filled.background,
                filled.background_hover,
                filled.background_active,
            ];
            let chosen = weakest_contrast(filled.text, backgrounds, theme.colors.raised);
            let other = if filled.text == theme.colors.text_on_accent {
                theme.colors.text
            } else {
                assert_eq!(filled.text, theme.colors.text);
                theme.colors.text_on_accent
            };
            assert!(chosen >= weakest_contrast(other, backgrounds, theme.colors.raised));
            assert_eq!(
                filled.background,
                theme.palette_color("indigo.600").expect("full ramp")
            );

            // Light is a wash of the same colour, never a solid.
            assert!(light.background.a < 0.5);
            assert!(light.background.a > 0.0);

            // Only Outline carries a border.
            assert!(outline.border.is_some());
            for resolved in [filled, light, subtle, transparent] {
                assert!(resolved.border.is_none());
            }

            // Subtle and Transparent rest without a surface; only Subtle
            // gains one under the pointer.
            assert_eq!(subtle.background.a, 0.0);
            assert!(subtle.background_hover.a > 0.0);
            assert_eq!(transparent.background_hover.a, 0.0);
        }
    }

    #[test]
    fn the_default_tier_ignores_the_color_choice() {
        let theme = Theme::studio_dark();
        let indigo = theme.variant_colors(Variant::Default, &"indigo".into());
        let red = theme.variant_colors(Variant::Default, &"red".into());
        assert_eq!(indigo, red);
        assert_eq!(indigo.background, theme.colors.raised);
        assert_eq!(indigo.text, theme.colors.text);
    }

    #[test]
    fn readable_text_follows_the_appearance() {
        let dark = Theme::studio_dark();
        let light = Theme::studio_light();
        let choice: ColorChoice = "teal".into();
        let on_dark = dark.variant_colors(Variant::Subtle, &choice).text;
        let on_light = light.variant_colors(Variant::Subtle, &choice).text;
        // A tinted label must move toward the text pole of its own page.
        assert!(on_dark.l > 0.5, "dark themes read pale shades");
        assert!(on_light.l < 0.5, "light themes read deep shades");
    }

    #[test]
    fn an_unknown_palette_group_falls_back_to_the_accent() {
        let theme = Theme::studio_dark();
        let resolved = theme.variant_colors(Variant::Filled, &"mauve".into());
        assert_eq!(resolved.background, theme.colors.accent);
    }

    #[test]
    fn custom_and_semantic_choices_resolve_without_a_ramp() {
        let theme = Theme::studio_dark();
        let paint = Hsla {
            h: 0.6,
            s: 0.8,
            l: 0.5,
            a: 1.0,
        };
        let custom = theme.variant_colors(Variant::Filled, &paint.into());
        assert_eq!(custom.background, paint);
        assert!(custom.background_hover.l < paint.l);

        let danger = theme.variant_colors(Variant::Filled, &SemanticColor::Danger.into());
        assert_eq!(danger.background, theme.colors.danger);
    }

    #[test]
    fn filled_tiers_choose_text_for_the_whole_interactive_paint_set() {
        let theme = Theme::studio_light();
        let cyan: Hsla = Rgba {
            r: 0.082,
            g: 0.667,
            b: 0.749,
            a: 1.0,
        }
        .into();
        let resolved = theme.variant_colors(Variant::Filled, &cyan.into());
        let backgrounds = [
            resolved.background,
            resolved.background_hover,
            resolved.background_active,
        ];

        assert_eq!(resolved.text, theme.colors.text);
        assert!(
            weakest_contrast(resolved.text, backgrounds, theme.colors.raised)
                > weakest_contrast(
                    theme.colors.text_on_accent,
                    backgrounds,
                    theme.colors.raised
                ),
            "the shared filled resolver left lower-contrast lettering on cyan"
        );
    }

    #[test]
    fn the_registry_switches_themes_and_refuses_unknown_ids() {
        let mut registry = ThemeRegistry::new();
        assert_eq!(registry.active().id, "studio-dark");
        assert!(registry.activate("studio-light"));
        assert_eq!(registry.active().appearance, Appearance::Light);
        assert!(!registry.activate("studio-solarized"));
        assert_eq!(registry.active().id, "studio-light");
    }

    #[test]
    fn every_shipped_theme_is_registered_and_the_studio_dark_one_is_active() {
        let registry = ThemeRegistry::new();
        assert_eq!(registry.active().id, "studio-dark");
        let ids: Vec<String> = registry.ids().iter().map(|id| id.to_string()).collect();
        assert_eq!(
            ids,
            [
                "studio-dark",
                "studio-light",
                "catppuccin-mocha",
                "catppuccin-latte",
                "nord",
                "tokyo-night",
                "gruvbox-dark",
                "dracula",
                "solarized-dark",
                "solarized-light",
            ]
        );
    }

    #[test]
    fn every_preset_can_be_activated() {
        let mut registry = ThemeRegistry::new();
        for tokens in presets() {
            assert!(registry.activate(&tokens.meta.id), "{}", tokens.meta.id);
            assert_eq!(registry.active().id, tokens.meta.id);
            assert_eq!(registry.active().appearance, tokens.meta.appearance);
        }
    }

    #[test]
    fn a_registered_theme_replaces_the_bundled_one_with_the_same_id() {
        let mut registry = ThemeRegistry::new();
        let before = registry.ids().len();
        registry
            .register_json(gpui_kit_tokens::studio_dark_json())
            .expect("bundled json is valid");
        assert_eq!(registry.ids().len(), before);
    }

    #[test]
    fn invalid_contrast_is_reported_before_the_registry_changes() {
        let mut registry = ThemeRegistry::new();
        let before = registry.ids();
        let mut value: serde_json::Value =
            serde_json::from_str(gpui_kit_tokens::studio_dark_json()).expect("bundled JSON");
        value["meta"]["id"] = serde_json::json!("low-contrast");
        value["color"]["text"]["primary"] = value["color"]["surface"]["canvas"].clone();

        let error = registry
            .register_json(&value.to_string())
            .expect_err("invisible text must not register");
        assert!(
            error
                .to_string()
                .contains("color.text.primary on color.surface.canvas")
        );
        assert_eq!(registry.ids(), before);
    }

    #[test]
    fn density_survives_a_theme_switch() {
        let mut registry = ThemeRegistry::new();
        registry.set_density(Density::Compact);
        registry.activate("studio-light");
        assert_eq!(registry.active().density, Density::Compact);
    }

    #[test]
    fn flat_elevation_costs_nothing_and_deeper_layers_cast_more() {
        let theme = Theme::studio_dark();
        assert!(theme.shadow(Elevation::Flat).is_empty());
        assert!(
            theme.shadow(Elevation::Modal)[0].blur_radius
                > theme.shadow(Elevation::Raised)[0].blur_radius
        );
        assert!(theme.layer(Layer::Toast) > theme.layer(Layer::Popover));
    }

    #[test]
    fn the_series_scale_arrives_resolved_and_cycles() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            assert_eq!(theme.colors.sequence.len(), SEQUENCE_LENGTH);
            assert_eq!(theme.colors.sequence.get(0), theme.colors.sequence.get(8));
            assert_eq!(theme.colors.sequence.get(11), theme.colors.sequence.get(3));
            // A series scale that resolved to one paint would draw a chart
            // nobody can take apart, and every entry is opaque because a
            // series is a fill rather than a wash.
            assert_ne!(theme.colors.sequence.get(0), theme.colors.sequence.get(1));
            for index in 0..SEQUENCE_LENGTH {
                assert_eq!(theme.colors.sequence.get(index).a, 1.0);
            }
        }
    }

    #[test]
    fn the_canvas_vocabulary_says_which_ports_and_edges_are_live() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let node = theme.colors.node;
            assert_ne!(node.port_idle, node.port_connected);
            assert_ne!(node.port_idle, node.port_hover);
            assert_ne!(node.edge, node.edge_active);
            // A return path is a fact about control flow, so it is neither
            // the flow it returns from nor the failure paint.
            assert_ne!(node.edge, node.edge_feedback);
            assert_ne!(node.edge_feedback_active, theme.colors.danger);
            assert_eq!(node.get(NodeColor::PortIdle), node.port_idle);
            assert_eq!(node.get(NodeColor::GridAxis), node.grid_axis);
            // The chip behind an edge label has to cover the line under it.
            assert!(node.label_wash.a > 0.9);
            // The grid is barely there, its major interval is louder, and the
            // origin rules are louder still.
            assert!(node.grid.a < node.grid_strong.a);
            assert!(node.grid_strong.a < node.grid_axis.a);
        }
    }

    /// Depth is a contact shadow plus a soft key, in that order.
    #[test]
    fn a_raised_surface_casts_a_contact_shadow_under_its_key() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            for level in [Elevation::Raised, Elevation::Overlay, Elevation::Modal] {
                let shadows = theme.shadow(level);
                assert_eq!(shadows.len(), 2, "{level:?}");
                assert!(shadows[0].blur_radius < shadows[1].blur_radius);
                assert!(shadows[0].offset.y < shadows[1].offset.y);
            }
        }
    }

    #[test]
    fn backdrop_is_the_substrate_below_the_page() {
        let theme = Theme::studio_dark();
        assert_eq!(theme.surface(Surface::Backdrop), theme.colors.backdrop);
        assert_ne!(theme.colors.backdrop, theme.colors.canvas);
    }

    #[test]
    fn a_step_with_two_layers_keeps_their_order() {
        let mut value: serde_json::Value =
            serde_json::from_str(gpui_kit_tokens::studio_dark_json()).expect("bundled JSON");
        value["elevation"]["raised"] = serde_json::json!([
            { "y": 1, "blur": 2, "spread": 0, "color": "{neutral.0}/3d" },
            { "y": 2, "blur": 6, "spread": -1, "color": "{neutral.0}/59" }
        ]);
        let tokens = TokenDocument::parse(&value.to_string()).expect("two-layer raised");
        let theme = Theme::from_tokens(&tokens, Density::default());
        let shadows = theme.shadow(Elevation::Raised);
        assert_eq!(shadows.len(), 2);
        assert_eq!(shadows[0].offset.y, px(1.0));
        assert_eq!(shadows[0].blur_radius, px(2.0));
        assert_eq!(shadows[0].spread_radius, px(0.0));
        assert_eq!(shadows[1].offset.y, px(2.0));
        assert_eq!(shadows[1].blur_radius, px(6.0));
        assert_eq!(shadows[1].spread_radius, px(-1.0));
    }

    #[test]
    fn repeated_semantic_metrics_are_token_backed() {
        let theme = Theme::studio_dark();
        assert_eq!(theme.spacing.lg, 16.0);
        assert_eq!(theme.radii.card, 12.0);
        assert_eq!(theme.radii.dialog, 16.0);
        assert_eq!(theme.motion.menu_ms, 140);
    }

    #[test]
    fn control_metrics_grow_with_size() {
        let theme = Theme::studio_dark();
        let heights: Vec<f32> = ControlSize::ALL
            .iter()
            .map(|size| theme.control.get(*size).height)
            .collect();
        assert!(heights.windows(2).all(|window| window[0] < window[1]));
        assert_eq!(theme.borders.hairline, 1.0);
        assert!(theme.opacity.disabled < 1.0);
    }

    #[test]
    fn focus_and_selection_do_not_look_alike() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let focus = theme.focus_ring();
            let selected = theme.selected_ring();
            assert_eq!(focus.len(), 1);
            assert_ne!(focus[0].color, selected[0].color);
            assert!(!focus[0].inset && selected[0].inset);
            // A ring that reserved space would move the layout the moment the
            // keyboard arrived on a control.
            assert_eq!(focus[0].offset, point(px(0.0), px(0.0)));
            assert!(focus[0].spread_radius > px(0.0));
        }
    }

    #[test]
    fn selected_ring_does_not_change_layout() {
        let theme = Theme::studio_dark();
        let ring = theme.selected_ring();
        assert_eq!(ring.len(), 1);
        assert!(ring[0].inset);
        assert_eq!(ring[0].spread_radius, px(1.0));
    }

    #[test]
    fn a_palette_entry_resolves_to_the_same_paint_a_role_would_have() {
        let theme = Theme::studio_dark();
        // `indigo.400` is what `color.semantic.accent` references, so reading
        // the scale directly must land on the paint the role produced rather
        // than on a second interpretation of the same hex.
        assert_eq!(theme.palette_color("indigo.400"), Some(theme.colors.accent));
        assert_eq!(
            theme.palette_color("indigo.600"),
            Some(theme.colors.accent_strong)
        );
        // A scale no role happens to reference is readable all the same; that
        // is the point of reaching the palette rather than the roles.
        assert_eq!(
            theme.palette_color("neutral.500"),
            Some(color(Color::parse("test", "#565656").expect("literal")))
        );
    }

    #[test]
    fn an_undeclared_palette_entry_is_none_rather_than_a_guess() {
        let theme = Theme::studio_dark();
        assert_eq!(theme.palette_color("indigo.999"), None);
        assert_eq!(theme.palette_color("nosuchgroup.400"), None);
        // A path with no step names a group, not a colour.
        assert_eq!(theme.palette_color("indigo"), None);
    }

    #[test]
    fn the_palette_travels_with_the_document_the_registry_activated() {
        let mut registry = ThemeRegistry::new();
        let retinted = serde_json::to_string(&{
            let mut document: serde_json::Value = serde_json::from_str(include_str!(
                "../../gpui-kit-tokens/tokens/studio-dark.json"
            ))
            .expect("bundled dark document");
            document["meta"]["id"] = serde_json::Value::String("palette-probe".into());
            document["color"]["palette"]["identity"] = serde_json::json!({ "violet": "#8b5cf6" });
            document
        })
        .expect("retinted document");

        // Before registration the scale does not exist, and afterwards the
        // active theme paints it — which is the whole claim: an application
        // scale follows the registry instead of a document held on the side.
        assert_eq!(registry.active().palette_color("identity.violet"), None);
        registry.register_json(&retinted).expect("register");
        assert!(registry.activate("palette-probe"));
        assert_eq!(
            registry.active().palette_color("identity.violet"),
            Some(color(Color::parse("test", "#8b5cf6").expect("literal")))
        );
    }
}
