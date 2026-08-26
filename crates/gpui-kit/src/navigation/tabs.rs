//! A strip of tabs that reports which one was chosen.
//!
//! The selected tab is caller-owned. `Tabs` reports the id that was picked and
//! renders whatever the caller says is current, so a host that refuses a move
//! keeps the tab that still holds underlined.
//!
//! `Tabs` renders the strip only. The body belongs to the caller, which is why
//! no `Role::TabPanel` node is published here.
//!
//! # Document tabs
//!
//! A document tab is this same strip carrying two more facts: whether the
//! thing behind it has changes nobody has written down yet ([`SaveState`]),
//! and whether it can be put away ([`TabItem::closable`]). It is not a second
//! component, because everything a document tab does — reporting a selection
//! it does not apply, refusing a tab, stepping with the arrow keys in reading
//! order, being dragged somewhere else — is what this strip already does. A
//! separate `DocumentTabs` would have to reimplement all of it and would then
//! be a second place for the two of them to disagree about what a tab is.
//!
//! A strip too wide for its frame does one of two things, and the caller
//! says which: it moves the surplus into a menu ([`Tabs::overflow_after`]) or
//! it scrolls ([`Tabs::scrolling`]). Neither hides a tab from the keyboard.

use std::rc::Rc;

use gpui::{
    App, Entity, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    RenderOnce, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div, point,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlMetrics, ControlSize, Space, Theme, TypeScale};

use crate::display::badge::Badge;
use crate::foundation::direction::{ActiveDirection, DirectionalExt, LayoutDirection};
use crate::foundation::stepping::bounded_step;
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt, text};
use crate::interaction::dnd::{
    self, DragItem, DropAxis, DropIntent, DropPosition, MakingWay, RowTarget, SurfaceDrag,
};
use crate::layout::ScrollFade;
use crate::motion::{Flipping, flip, keyed};
use crate::overlay::{Menu, MenuItem};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ReorderHandler = Rc<dyn Fn(&DropIntent, &mut Window, &mut App)>;
type Accepts = Rc<dyn Fn(&DragItem, &DropPosition) -> bool>;

/// The width of the dot a tab wears while it has something unsaved. It occurs
/// once, so it stays next to the component rather than in the token document.
const MARK_SIZE: f32 = 7.0;

/// How wide the fade at a scrolling strip's edge is. Wide enough that a tab
/// passing under it dims over several pixels of travel rather than meeting a
/// line, which is what makes it read as "there is more" instead of as damage.
const FADE_BAND: f32 = 36.0;

/// How far the strip may be from an end before it counts as away from it.
///
/// Measurement lands on fractions, and a strip sitting at rest one hundredth
/// of a pixel from zero must not paint a fade over a tab that is entirely
/// visible.
const AT_END: f32 = 1.0;

/// Whether what a tab holds has been written down, and what happened when
/// somebody tried.
///
/// The three unclean variants are separate presentations, not one "modified"
/// flag with a colour. A save that failed silently and then showed a clean tab
/// would tell the typist their work is safe when it is not, which is the exact
/// failure this library exists to avoid — so [`SaveState::Failed`] carries the
/// host's own reason and the tab publishes itself as invalid.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SaveState {
    /// Everything in this tab is written down.
    #[default]
    Clean,
    /// There are changes nobody has written down yet.
    Dirty,
    /// A save is in flight. Not clean: it has not landed.
    Saving,
    /// A save was attempted and did not land, in the host's own words.
    Failed { reason: SharedString },
}

impl SaveState {
    /// The name the semantic tree publishes, so a test tells the three apart
    /// without reading a colour.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Dirty => "dirty",
            Self::Saving => "saving",
            Self::Failed { .. } => "save-failed",
        }
    }

    pub fn is_clean(&self) -> bool {
        matches!(self, Self::Clean)
    }

    /// What a reader is told. A clean tab is told nothing at all.
    fn wording(&self, cx: &App) -> Option<SharedString> {
        match self {
            // The host's words outrank the catalogue's, so a failure states
            // the reason it was given rather than a generic sentence.
            Self::Failed { reason } => Some(reason.clone()),
            Self::Dirty => Some(cx.strings().text(StringKey::TabDirty)),
            Self::Saving => Some(cx.strings().text(StringKey::TabSaving)),
            Self::Clean => None,
        }
    }

    /// The glyph an overflowed tab carries in the menu, where there is no room
    /// to draw the mark the strip draws.
    fn menu_icon(&self) -> Option<Icon> {
        match self {
            Self::Clean => None,
            Self::Dirty => Some(Icon::Pen),
            Self::Saving => Some(Icon::Refresh),
            Self::Failed { .. } => Some(Icon::Danger),
        }
    }
}

/// One tab, identified by business identity rather than by position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    pub id: SharedString,
    pub label: SharedString,
    pub icon: Option<Icon>,
    pub badge: Option<SharedString>,
    pub disabled: bool,
    pub save_state: SaveState,
    pub closable: bool,
}

impl TabItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            disabled: false,
            save_state: SaveState::Clean,
            closable: false,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A count shown next to the label, such as how many items the tab holds.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Whether what this tab holds has been written down.
    pub fn save_state(mut self, state: SaveState) -> Self {
        self.save_state = state;
        self
    }

    /// There are changes nobody has written down yet.
    pub fn dirty(self) -> Self {
        self.save_state(SaveState::Dirty)
    }

    /// A save is in flight. Distinct from clean, because it has not landed.
    pub fn saving(self) -> Self {
        self.save_state(SaveState::Saving)
    }

    /// A save was attempted and did not land, in the host's own words.
    pub fn save_failed(self, reason: impl Into<SharedString>) -> Self {
        self.save_state(SaveState::Failed {
            reason: reason.into(),
        })
    }

    /// Whether this tab carries a close affordance.
    ///
    /// Closing is reported through [`Tabs::on_close`]; a strip with no such
    /// handler draws no close control however many tabs claim to be closable.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// The row this tab becomes when it does not fit in the strip.
    ///
    /// A hidden tab that is the current one still has to read as the current
    /// one, and a menu row says so with a checkmark. That takes the glyph
    /// slot, so the current tab shows no save mark in the menu; every other
    /// row carries one, and the strip is where a current tab's mark is read.
    fn menu_row(&self, selected: bool) -> MenuItem {
        if selected {
            return MenuItem::check(self.id.clone(), self.label.clone(), true)
                .disabled(self.disabled);
        }
        let mut row =
            MenuItem::command(self.id.clone(), self.label.clone()).disabled(self.disabled);
        if let Some(glyph) = self.save_state.menu_icon().or(self.icon) {
            row = row.icon(glyph);
        }
        row
    }
}

/// What a strip does with the tabs it has no room for.
///
/// The two answers are not interchangeable, and which one is right depends on
/// what a tab stands for. A menu is right when the tabs are documents: they
/// have names, the names are how the reader thinks of them, and a list of
/// names is a better index than a strip you have to travel along. Scrolling is
/// right when a tab is a *place* the reader has arranged — a workspace, a
/// project — because then its position in the row is part of what identifies
/// it, and moving it into a menu takes that away.
///
/// Either way every tab keeps its keyboard: arrow, home and end step over all
/// of them, and a step that lands on a tab which is not on screen brings it
/// there.
/// Private because the two builders are the whole of the choice: a caller
/// names the behaviour it wants, never this. It also keeps `Overflow` from
/// colliding with GPUI's own, which a host importing both preludes would hit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Overflow {
    /// The strip wraps onto another row. Nothing is hidden and nothing moves,
    /// which is right for a handful of tabs in a container that can grow.
    #[default]
    Wraps,
    /// Tabs past the cut are moved into a menu. Needs [`Tabs::overflow_menu`];
    /// without one there is nowhere to move them, so nothing is cut.
    Menu(usize),
    /// The strip scrolls sideways, fading at whichever edge has more behind
    /// it.
    Scrolls,
}

/// What a scrolling strip has to remember between two frames.
///
/// It is keyed on the strip's own identity rather than owned by the caller,
/// because both halves are the strip's business and neither is a decision the
/// host makes: where the reader left the strip, and which tab it has already
/// brought into view. Asking every caller to hold a `ScrollHandle` would put a
/// field in every host for something no host reads.
#[derive(Default)]
struct Strip {
    scroll: ScrollHandle,
    /// The selection this strip has already scrolled to. Scrolling on the
    /// *change* rather than on the state is the whole of not fighting the
    /// reader: once a tab has been shown, scrolling away from it is allowed to
    /// stick.
    showed: Option<SharedString>,
    /// Whether the tab that was last brought into view still has to be moved
    /// clear of the fade. It cannot be done on the frame that asks for the
    /// scroll, because the tab has no bounds until it has been laid out once.
    clearing: bool,
}

/// A row of tabs. The strip publishes one [`Role::Tab`] node per tab.
#[derive(IntoElement)]
pub struct Tabs {
    ident: Ident,
    tabs: Vec<TabItem>,
    selected: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    on_select: Option<SelectHandler>,
    on_close: Option<CloseHandler>,
    reorderable: bool,
    accepts: Option<Accepts>,
    on_reorder: Option<ReorderHandler>,
    overflow: Overflow,
    overflow_menu: Option<Entity<Menu>>,
}

impl std::fmt::Debug for Tabs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Tabs")
            .field("ident", &self.ident)
            .field("tabs", &self.tabs.len())
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .field("closable", &self.on_close.is_some())
            .field("overflow", &self.overflow)
            .finish()
    }
}

impl Tabs {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            tabs: Vec::new(),
            selected: None,
            size: ControlSize::Md,
            disabled: false,
            on_select: None,
            on_close: None,
            reorderable: false,
            accepts: None,
            on_reorder: None,
            overflow: Overflow::default(),
            overflow_menu: None,
        }
    }

    pub fn tab(mut self, tab: TabItem) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn tabs(mut self, tabs: impl IntoIterator<Item = TabItem>) -> Self {
        self.tabs.extend(tabs);
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Reports the tab that should be put away. The strip closes nothing.
    ///
    /// Only a tab marked [`TabItem::closable`] gets a control, and the control
    /// is a target of its own: it swallows the click that reaches it, so the
    /// gesture that means "switch to this tab" cannot land on it by accident.
    /// A middle click anywhere on a closable tab reports the same thing, which
    /// is the platform convention wherever a pointer has three buttons.
    pub fn on_close(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// Keeps the first `count` tabs in the strip and moves the rest into the
    /// overflow menu.
    ///
    /// The cut is declared rather than measured, for the reason
    /// [`Toolbar::overflow_after`](crate::layout::Toolbar::overflow_after)
    /// states: GPUI measures after the element tree exists, so a strip cannot
    /// find out what fits and then still move a tab somewhere else.
    ///
    /// A menu keeps every hidden tab named, listed, and carrying its own save
    /// state, which is what makes it the right answer for document tabs: the
    /// name is how the reader thinks of the thing, so a list of names is a
    /// better index than a row you travel along. Where position is part of
    /// what identifies a tab, [`Tabs::scrolling`] is the other answer.
    ///
    /// Either way the keyboard reaches a hidden tab directly: arrow, home and
    /// end step over **every** tab the caller declared, hidden or not, and
    /// report the one they land on.
    ///
    /// This and [`Tabs::scrolling`] are the same setting, so the later call
    /// replaces the earlier one rather than the two combining into a strip
    /// that both scrolls and hides.
    pub fn overflow_after(mut self, count: usize) -> Self {
        self.overflow = Overflow::Menu(count);
        self
    }

    /// Lets the strip scroll sideways instead of wrapping or hiding.
    ///
    /// Scrolling costs something a menu does not: a tab that is off screen has
    /// no position the reader can point at. The strip pays that back two ways.
    /// The keyboard still steps over every tab, and a step that lands on one
    /// which is not on screen scrolls it into view — so nothing is reachable
    /// only by dragging a strip about. And the edge with more behind it fades,
    /// so "there are further tabs this way" is on screen rather than something
    /// you find by trying.
    ///
    /// Bringing the selection into view happens when the selection *changes*,
    /// not while it stays put. A strip that re-centred the current tab every
    /// frame would haul itself back the instant the reader scrolled off to
    /// look at something else, which is the failure that makes people stop
    /// scrolling these strips at all.
    ///
    /// This and [`Tabs::overflow_after`] are the same setting, so the later
    /// call replaces the earlier one.
    pub fn scrolling(mut self) -> Self {
        self.overflow = Overflow::Scrolls;
        self
    }

    /// The menu the overflowed tabs are moved into.
    ///
    /// Caller-owned, because whether it is open outlives a frame. The menu
    /// reports the tab that was taken as [`MenuEvent::Invoked`](crate::overlay::MenuEvent),
    /// carrying the same id the strip would have reported.
    pub fn overflow_menu(mut self, menu: Entity<Menu>) -> Self {
        self.overflow_menu = Some(menu);
        self
    }

    /// Where the cut falls, which is nowhere when there is no menu to move
    /// anything into.
    fn cut(&self) -> usize {
        match (self.overflow, self.overflow_menu.is_some()) {
            (Overflow::Menu(cut), true) => cut,
            _ => usize::MAX,
        }
    }

    /// Lets a tab be dragged to another place in the strip.
    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    /// Whether this strip takes a payload, and where. Without one, it takes
    /// its own tabs and nothing else.
    pub fn accepts(
        mut self,
        predicate: impl Fn(&DragItem, &DropPosition) -> bool + 'static,
    ) -> Self {
        self.accepts = Some(Rc::new(predicate));
        self
    }

    /// Reports where a dropped tab should go. The strip does not move it.
    pub fn on_reorder(
        mut self,
        handler: impl Fn(&DropIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reorder = Some(Rc::new(handler));
        self
    }

    fn reorder(&self, window: &mut Window, cx: &mut App) -> Option<Reorder> {
        if self.disabled || !self.reorderable {
            return None;
        }
        let on_drop = self.on_reorder.clone()?;
        let surface = self.ident.semantic_id();
        let accepts = self.accepts.clone().unwrap_or_else(|| {
            let own = surface.clone();
            Rc::new(move |item: &DragItem, _: &DropPosition| item.source == own)
        });
        Some(Reorder {
            drag: dnd::surface_drag(&surface, window, cx),
            surface,
            accepts,
            on_drop,
        })
    }

    /// Whether this tab can be put away right now: the caller said it can,
    /// the strip has somewhere to report it, and nothing is refused.
    fn closes(&self, tab: &TabItem) -> bool {
        tab.closable && !self.disabled && !tab.disabled && self.on_close.is_some()
    }

    /// The mark a tab wears while what it holds is not written down.
    ///
    /// A clean tab draws nothing and publishes nothing, which is what makes
    /// the mark's presence the whole signal.
    fn save_mark(
        &self,
        tab: &TabItem,
        ident: &Ident,
        theme: &Theme,
        cx: &App,
    ) -> Option<gpui::AnyElement> {
        let wording = tab.save_state.wording(cx)?;
        let (color, filled) = match tab.save_state {
            SaveState::Dirty => (theme.colors.accent, true),
            // A save in flight is drawn as an outline of the dirty dot: the
            // work is still not written down, and the ring says something is
            // happening to it without claiming it landed.
            SaveState::Saving => (theme.colors.text_muted, false),
            SaveState::Failed { .. } => (theme.colors.danger, true),
            SaveState::Clean => return None,
        };
        Some(
            div()
                .flex_none()
                .size(px(MARK_SIZE))
                .rounded_full()
                .when(filled, |element| element.bg(color))
                .when(!filled, |element| {
                    element.border(px(theme.borders.thick)).border_color(color)
                })
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("save").semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .text(wording)
                        .value(tab.save_state.name())
                        .busy(matches!(tab.save_state, SaveState::Saving))
                        .invalid(matches!(tab.save_state, SaveState::Failed { .. })),
                )
                .into_any_element(),
        )
    }

    /// The control that puts a tab away.
    ///
    /// It is a hit target of its own with its own identity, and it stops the
    /// click travelling, so the gesture that means "switch to this tab" cannot
    /// land on it by accident.
    fn close_control(
        &self,
        tab: &TabItem,
        ident: &Ident,
        theme: &Theme,
        metrics: ControlMetrics,
        cx: &mut App,
    ) -> Option<gpui::AnyElement> {
        let handler = self.on_close.clone().filter(|_| self.closes(tab))?;
        let close_ident = ident.child("close");
        let name = cx
            .strings()
            .format(StringKey::TabClose, &[tab.label.as_ref()]);
        let id = tab.id.clone();
        let keyed_id = id.clone();
        let keyed = Rc::clone(&handler);

        Some(
            div()
                .id(close_ident.element_id())
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(metrics.icon_size + 4.0))
                .rounded_full()
                .cursor_pointer()
                .tab_index(0)
                .hover(|style| style.bg(theme.colors.hover))
                .focus_ring(theme)
                .child(
                    icon(Icon::Close)
                        .size(px(metrics.icon_size - 3.0))
                        .text_color(theme.colors.text_muted),
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    handler(id.clone(), window, cx);
                })
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        keyed(keyed_id.clone(), window, cx);
                    }
                })
                .semantic_in(
                    cx,
                    NodeSpec::new(close_ident.semantic_id(), Role::Button)
                        .parent(ident.semantic_id())
                        .text(name),
                )
                .into_any_element(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn tab_element(
        &self,
        tab: &TabItem,
        index: usize,
        theme: &Theme,
        metrics: ControlMetrics,
        reorder: Option<&Reorder>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyElement {
        let selected = self.selected.as_ref() == Some(&tab.id);
        let disabled = self.disabled || tab.disabled;
        let actionable = !disabled && self.on_select.is_some();
        let draggable = reorder.filter(|_| !disabled);
        let drag = draggable.and_then(|reorder| reorder.drag.as_ref());
        let carried = drag.is_some_and(|drag| drag.carries(&tab.id));
        let landing = drag.and_then(|drag| drag.indicator_for(&tab.id));
        let ident = self.ident.child(tab.id.as_ref());
        let hover_group = ident.child("hover").semantic_id();
        let color = if disabled {
            theme.colors.text_disabled
        } else if selected {
            theme.colors.text
        } else {
            theme.colors.text_muted
        };

        let mut element = div()
            .id(ident.element_id())
            .group(hover_group.clone())
            .flex_none()
            .column()
            .child(
                div()
                    .row()
                    .h(px(metrics.height))
                    .px(px(metrics.padding_x))
                    .gap(px(metrics.gap))
                    .children(
                        tab.icon
                            .map(|glyph| icon(glyph).size(px(metrics.icon_size)).text_color(color)),
                    )
                    .child(
                        text(theme, TypeScale::Label, tab.label.clone())
                            .text_size(px(metrics.font_size))
                            .text_color(color)
                            .when(actionable, |element| {
                                element.group_hover(hover_group, |style| {
                                    style.text_color(theme.colors.text)
                                })
                            }),
                    )
                    .children(tab.badge.clone().map(|badge| Badge::new(badge).neutral()))
                    .children(self.save_mark(tab, &ident, theme, cx))
                    .children(self.close_control(tab, &ident, theme, metrics, cx)),
            )
            // The underline is a sibling rather than a border so an unselected
            // tab reserves the same height and nothing shifts when it is
            // chosen. The accent bar inside it is one element for the whole
            // strip, so choosing another tab moves it instead of putting a
            // second one somewhere else.
            .child(
                div()
                    .relative()
                    .h(px(theme.effects.selection_rail_width))
                    .children(selected.then(|| {
                        let indicator =
                            flip(self.ident.child("indicator").semantic_id(), window, cx);
                        // Inset from the tab's own padding and rounded at the
                        // top, so the mark sits under the label rather than
                        // under the gap between two labels.
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(px(metrics.padding_x))
                            .right(px(metrics.padding_x))
                            .rounded_t(px(theme.effects.selection_rail_width / 2.0))
                            .bg(theme.colors.accent)
                            .flip(&indicator, window, cx)
                    })),
            )
            .children(landing.map(|(position, accepted)| {
                dnd::indicator(&position, accepted, DropAxis::Horizontal, cx)
            }))
            .when(carried, |element| element.opacity(theme.opacity.muted))
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .focus_ring(theme)
            });

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let id = tab.id.clone();
            let click = Rc::clone(&handler);
            let clicked = id.clone();
            element = element
                .on_click(move |_, window, cx| click(clicked.clone(), window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(id.clone(), window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        // A middle click is the platform's own "put this away" on every
        // pointer that has three buttons; a pointer that has two never sends
        // it, so nothing is lost where the convention does not exist. It sits
        // on mouse-down rather than on click because the middle button has no
        // click gesture in GPUI.
        if let (true, Some(handler)) = (self.closes(tab), self.on_close.clone()) {
            let id = tab.id.clone();
            element = element.on_mouse_down(
                MouseButton::Middle,
                move |_: &MouseDownEvent, window, cx| {
                    handler(id.clone(), window, cx);
                    cx.stop_propagation();
                },
            );
        }

        if let Some(reorder) = draggable {
            let mut item =
                DragItem::new(reorder.surface.clone(), tab.id.clone(), tab.label.clone());
            if let Some(glyph) = tab.icon {
                item = item.icon(glyph);
            }
            element = dnd::draggable(element, item);
            element = dnd::drop_target(
                element,
                RowTarget {
                    surface: reorder.surface.clone(),
                    id: tab.id.clone(),
                    index,
                    allow_into: false,
                    axis: DropAxis::Horizontal,
                    accepts: Rc::clone(&reorder.accepts),
                    on_drop: Rc::clone(&reorder.on_drop),
                },
            );
        }

        let element = element.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Tab)
                .parent(self.ident.semantic_id())
                .checked(selected)
                .disabled(disabled)
                .text(tab.label.clone())
                // The state is published by name, so a test tells dirty from
                // saving from a save that failed without reading a colour.
                .value(tab.save_state.name())
                .busy(matches!(tab.save_state, SaveState::Saving))
                .invalid(matches!(tab.save_state, SaveState::Failed { .. })),
        );

        match draggable {
            Some(reorder) => {
                let shift = reorder
                    .drag
                    .as_ref()
                    .filter(|drag| drag.makes_way(index))
                    .map_or(px(0.0), |_| dnd::make_way_gap(cx, DropAxis::Horizontal));
                element
                    .make_way(ident.semantic_id(), point(shift, px(0.0)), window, cx)
                    .into_any_element()
            }
            None => element.into_any_element(),
        }
    }
}

/// What a tab needs to take part in a reorder.
#[derive(Clone)]
struct Reorder {
    surface: SharedString,
    drag: Option<SurfaceDrag>,
    accepts: Accepts,
    on_drop: ReorderHandler,
}

impl Disableable for Tabs {
    /// Refuses the whole strip. A disabled strip installs no handler at all,
    /// including the keyboard one.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Tabs {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let reorder = self.reorder(window, cx);

        let direction = cx.layout_direction();
        let scrolls = self.overflow == Overflow::Scrolls;
        // Where the reader left the strip, and what it has already shown them.
        let state = scrolls.then(|| {
            keyed::slot::<Strip>(
                &self.ident.semantic_id(),
                window.window_handle().window_id(),
                cx,
            )
        });
        let mut fades = (false, false);
        let mut scroll = None;
        if let Some(state) = &state {
            let mut state = state.borrow_mut();
            let showing = self
                .selected
                .as_ref()
                .and_then(|id| self.tabs.iter().position(|tab| &tab.id == id));
            if state.showed != self.selected {
                if let Some(index) = showing {
                    // Applied in the next prepaint, once the tabs have bounds
                    // to be scrolled to, and by the minimum that puts the tab
                    // fully on screen.
                    state.scroll.scroll_to_item(index);
                }
                state.showed = self.selected.clone();
                state.clearing = showing.is_some();
            }
            // The minimum that puts a tab on screen leaves it flush against
            // the end it came from, which is exactly where the fade is: the
            // strip would dim the label it had just gone to fetch. So a tab
            // the strip brought into view is moved a band clear of the end,
            // on the frame that first has bounds to measure it by.
            if let (true, Some(index)) = (state.clearing, showing)
                && clear_the_fade(&state.scroll, index, FADE_BAND)
            {
                state.clearing = false;
            }
            // Read as a distance from the start, because which sign means
            // "scrolled onward" is a detail of the platform's scroll
            // convention and not something worth depending on.
            let travelled = f32::from(state.scroll.offset().x).abs();
            let total = f32::from(state.scroll.max_offset().x).abs();
            let (from_start, from_end) = (travelled > AT_END, travelled < total - AT_END);
            fades = match direction {
                LayoutDirection::LeftToRight => (from_start, from_end),
                LayoutDirection::RightToLeft => (from_end, from_start),
            };
            scroll = Some(state.scroll.clone());
        }

        let mut strip = div()
            .id(self.ident.element_id())
            .row_reading(direction)
            .items_end()
            .gap(px(theme.space(Space::Xs)))
            // Wrapping and scrolling are contradictory answers to the same
            // question: a strip that wraps never has a second screenful to
            // scroll to.
            .when(!scrolls, |element| element.flex_wrap())
            .when_some(scroll.as_ref(), |element, scroll| {
                element.min_w_0().overflow_x_scroll().track_scroll(scroll)
            });

        if let (false, Some(handler)) = (self.disabled, self.on_select.clone()) {
            let tabs = self.tabs.clone();
            let selected = self.selected.clone();
            // A strip of tabs runs in reading order, so the arrow that means
            // "the previous tab" is the one pointing back the way the strip
            // was laid out, not the one pointing left.
            strip = strip.on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                let next = match direction.arrow_step(key) {
                    Some(step_by) => step(&tabs, selected.as_ref(), step_by as isize),
                    None => match key {
                        "home" => edge(&tabs, -1),
                        "end" => edge(&tabs, 1),
                        _ => return,
                    },
                };
                // A move that lands nowhere, or back on the tab that is
                // already current, is not a choice and is not reported.
                let Some(next) = next.filter(|next| Some(next) != selected.as_ref()) else {
                    return;
                };
                handler(next, window, cx);
                cx.stop_propagation();
            });
        }

        let cut = self.cut();
        let mut hidden: Vec<MenuItem> = Vec::new();
        for (index, tab) in self.tabs.iter().enumerate() {
            if index >= cut {
                hidden.push(tab.menu_row(self.selected.as_ref() == Some(&tab.id)));
                continue;
            }
            strip = strip.child(self.tab_element(
                tab,
                index,
                &theme,
                metrics,
                reorder.as_ref(),
                window,
                cx,
            ));
        }

        let hidden_count = hidden.len();
        let overflow = self
            .overflow_menu
            .clone()
            .filter(|_| hidden_count > 0)
            .map(|menu| {
                if menu.read(cx).offered() != hidden.as_slice() {
                    menu.update(cx, |menu, cx| menu.set_items(hidden, cx));
                }
                let overflow_ident = self.ident.child("overflow");
                div()
                    .flex_none()
                    .column()
                    // The trigger stands where a tab would, so it is built
                    // like one: a row of the tab height with the underline
                    // lane beneath it. Without the lane the strip's own
                    // bottom alignment hangs the trigger a rail's width
                    // below the labels and breaks the line they sit on.
                    .child(div().row().h(px(metrics.height)).items_center().child(menu))
                    .child(div().h(px(theme.effects.selection_rail_width)))
                    // The trigger says how many tabs moved here, so a snapshot
                    // shows that they were relocated and not dropped.
                    .semantic_in(
                        cx,
                        NodeSpec::new(overflow_ident.semantic_id(), Role::Group)
                            .parent(self.ident.semantic_id())
                            .text(cx.strings().text(StringKey::TabMoreTabs))
                            .value(cx.numbers().count(hidden_count)),
                    )
            });

        let strip = strip.children(overflow);
        // The strip holds every tab the caller declared, drawn or overflowed,
        // because the keyboard reaches all of them.
        let published = NodeSpec::new(self.ident.semantic_id(), Role::List)
            .value(cx.numbers().count(self.tabs.len()));

        match scrolls {
            // A scrolling element's own bounds travel with its content, so the
            // strip publishes the frame around it instead. Otherwise the one
            // node that says where the strip is would report a rectangle
            // somewhere off the left of the window as soon as the reader
            // scrolled, and nothing would be able to say which tabs are on
            // screen.
            true => div()
                .child(
                    ScrollFade::new(self.ident.child("fade"))
                        .band(FADE_BAND)
                        .left(fades.0)
                        .right(fades.1)
                        // The strip is as tall as one tab; a fade sized to its
                        // container would paint over whatever sits beneath it.
                        .fit_height()
                        .child(strip),
                )
                .semantic_in(cx, published)
                .into_any_element(),
            false => strip.semantic_in(cx, published).into_any_element(),
        }
    }
}

/// Moves the strip so the tab at `index` sits at least `band` from both ends
/// of its viewport, and reports whether there was anything measured to do it
/// with.
///
/// The nudge is bounded by the strip's own travel, so a tab at either end of
/// the row stays exactly where it is — and no fade is painted over it there
/// either, because there is nothing behind that end to fade towards.
fn clear_the_fade(scroll: &ScrollHandle, index: usize, band: f32) -> bool {
    let Some(tab) = scroll.bounds_for_item(index) else {
        return false;
    };
    let viewport = scroll.bounds();
    if viewport.size.width <= px(0.0) {
        return false;
    }
    let offset = scroll.offset();
    // A tab's bounds are where layout put it, before the strip was scrolled,
    // which is the same frame of reference the scroll offset is stated in.
    let start = f32::from(tab.left() + offset.x) - f32::from(viewport.left());
    let end = f32::from(viewport.right()) - f32::from(tab.right() + offset.x);
    let nudge = fade_nudge(start, end, band);
    if nudge != 0.0 {
        // A strip's offset runs from zero back to minus its travel, so a
        // nudge that would take it past either end is a tab already at that
        // end, where there is nothing behind the edge to fade towards.
        let travel = f32::from(scroll.max_offset().x).abs();
        let travelled = (f32::from(offset.x) - nudge).clamp(-travel, 0.0);
        scroll.set_offset(point(px(travelled), offset.y));
    }
    true
}

/// How far the strip has to travel onward for a tab `start` from one end and
/// `end` from the other to clear a fade `band` wide at both.
///
/// A tab wider than the room between the two fades cannot clear both, so the
/// end it is closer to wins and the other stays dimmed: moving it off one
/// fade and onto the other would say the tab is at an edge it is not at.
fn fade_nudge(start: f32, end: f32, band: f32) -> f32 {
    if end < band && end <= start {
        band - end
    } else if start < band && start < end {
        -(band - start)
    } else {
        0.0
    }
}

/// The next tab that can be chosen in `delta`'s direction, skipping refusals.
///
/// Movement stops at the ends instead of wrapping, so arrowing past the last
/// tab reports nothing rather than jumping back to the first.
fn step(tabs: &[TabItem], selected: Option<&SharedString>, delta: isize) -> Option<SharedString> {
    let from = selected.and_then(|id| tabs.iter().position(|tab| &tab.id == id));
    bounded_step(tabs.len(), from, delta, |index| tabs[index].disabled)
        .map(|index| tabs[index].id.clone())
}

/// The first tab from the left when `delta` is negative, from the right when
/// it is positive.
fn edge(tabs: &[TabItem], delta: isize) -> Option<SharedString> {
    step(tabs, None, -delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_already_clear_of_both_fades_is_not_moved() {
        assert_eq!(fade_nudge(40.0, 40.0, FADE_BAND), 0.0);
        assert_eq!(fade_nudge(FADE_BAND, FADE_BAND, FADE_BAND), 0.0);
    }

    #[test]
    fn a_tab_flush_against_an_end_is_moved_a_whole_band_clear_of_it() {
        // Flush against the far end: the strip travels onward.
        assert_eq!(fade_nudge(200.0, 0.0, FADE_BAND), FADE_BAND);
        // Flush against the near end: the strip travels back.
        assert_eq!(fade_nudge(0.0, 200.0, FADE_BAND), -FADE_BAND);
        // Part of the way under a fade is moved by what is left of it.
        assert_eq!(fade_nudge(200.0, 10.0, FADE_BAND), FADE_BAND - 10.0);
    }

    #[test]
    fn a_tab_too_wide_to_clear_both_fades_clears_the_end_it_is_nearer() {
        let nudge = fade_nudge(4.0, 8.0, FADE_BAND);
        assert!(
            nudge < 0.0,
            "the nearer end wins rather than the tab being shuffled between two fades"
        );
        assert_eq!(nudge, -(FADE_BAND - 4.0));
    }
}
