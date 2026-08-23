//! A structured view of a value a host parsed somewhere else.
//!
//! # Why there is a value type here
//!
//! This crate takes no serialization dependency, for the same reason
//! [`SplitLayout`](crate::layout::SplitLayout) converts to plain records
//! instead of deriving `Serialize`: a product-neutral component must not
//! decide which parsing crate an application depends on. [`JsonValue`] is the
//! smallest shape that expresses a JSON document faithfully, and a host
//! converts into it from whatever it already parses with.
//!
//! Two choices in it are deliberate.
//!
//! - A number is carried as the text the document contained. A `f64` cannot
//!   hold every integer a JSON document can write, and it cannot tell `1.10`
//!   from `1.1`; this crate also formats no numbers (`docs/coverage.md`
//!   records that gap), so re-rendering one would be inventing digits.
//! - An object is a `Vec` of pairs rather than a map. JSON documents have an
//!   order and may repeat a key; a map would silently reorder the first and
//!   drop the second, and a viewer that quietly edits its document is the
//!   thing this component exists not to be.
//!
//! # Absent, null, and empty are three facts
//!
//! A key that is not in the document produces no row at all. A key whose value
//! is `null` produces a row reading `null`. A key holding an empty object
//! produces a row reading `{}` that discloses nothing. All three are different
//! statements about the document, and a viewer that renders them alike is
//! lying about one of them.
//!
//! # Withheld is not absent
//!
//! A caller that must not show a subtree replaces it with
//! [`JsonValue::Redacted`], which carries a description of the shape and never
//! the value. The secret therefore never reaches this component, so no
//! rendering path, no snapshot, and no export can leak it — and the row still
//! exists, marked `withheld`, because a secret drawn as an absence tells the
//! reader the document does not contain it.
//!
//! # Virtualization
//!
//! The view lays out only the rows its viewport holds, over the same
//! `uniform_list` primitive [`List`](crate::data::List) and
//! [`DataGrid`](crate::data::DataGrid) use, so the same rule applies: only
//! rendered rows publish semantic nodes, and the container carries how many
//! rows are currently disclosed. It does not build on
//! [`Tree`](crate::data::Tree), which draws one label per node and has no slot
//! for a second column; a key and a typed value drawn as one string would lose
//! the distinction between the name of a thing and what it holds.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    RenderOnce, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Transformation,
    UniformListScrollHandle, Window, div, prelude::FluentBuilder, px, radians, uniform_list,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TextTone, Theme, TypeScale};
use unicode_segmentation::UnicodeSegmentation;

use crate::display::icon::flips;
use crate::foundation::direction::{ActiveDirection, DirectionalExt, LayoutDirection};
use crate::foundation::window_state;
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt, text};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type ToggleHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// A JSON value, plus the one thing JSON cannot say: that a subtree is being
/// withheld on purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    /// The number exactly as the document wrote it.
    Number(SharedString),
    String(SharedString),
    Array(Vec<JsonValue>),
    /// Members in document order. A repeated key is kept, not collapsed.
    Object(Vec<(SharedString, JsonValue)>),
    /// Present, and not shown. Carries a description of the shape and no part
    /// of the value.
    Redacted(SharedString),
}

impl JsonValue {
    pub fn number(text: impl Into<SharedString>) -> Self {
        Self::Number(text.into())
    }

    pub fn string(text: impl Into<SharedString>) -> Self {
        Self::String(text.into())
    }

    pub fn array(items: impl IntoIterator<Item = JsonValue>) -> Self {
        Self::Array(items.into_iter().collect())
    }

    pub fn object(members: impl IntoIterator<Item = (impl Into<SharedString>, JsonValue)>) -> Self {
        Self::Object(
            members
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    /// Withholds a subtree, described by a shape the caller wrote.
    pub fn redacted(shape: impl Into<SharedString>) -> Self {
        Self::Redacted(shape.into())
    }

    /// Measures a value and keeps only the measurement.
    ///
    /// The value is consumed here and never stored, which is what makes this
    /// safe to call with the secret itself: what comes back holds a sentence
    /// about the shape and nothing of the content.
    pub fn redacted_from(value: &JsonValue, cx: &App) -> Self {
        let strings = cx.strings();
        let shape = match value {
            JsonValue::String(text) => {
                let count = text.graphemes(true).count();
                strings.format_plural(
                    StringKey::DescriptionCharacterOne,
                    StringKey::DescriptionCharacters,
                    cx.numbers().plural(count),
                    &[cx.numbers().count(count).as_ref()],
                )
            }
            JsonValue::Object(members) => strings.format_plural(
                StringKey::JsonShapeEntryOne,
                StringKey::JsonShapeEntries,
                cx.numbers().plural(members.len()),
                &[cx.numbers().count(members.len()).as_ref()],
            ),
            JsonValue::Array(items) => strings.format_plural(
                StringKey::JsonShapeItemOne,
                StringKey::JsonShapeItems,
                cx.numbers().plural(items.len()),
                &[cx.numbers().count(items.len()).as_ref()],
            ),
            _ => strings.text(StringKey::JsonShapeValue),
        };
        Self::Redacted(shape)
    }

    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Null => ValueKind::Null,
            Self::Bool(_) => ValueKind::Bool,
            Self::Number(_) => ValueKind::Number,
            Self::String(_) => ValueKind::String,
            Self::Array(_) => ValueKind::Array,
            Self::Object(_) => ValueKind::Object,
            Self::Redacted(_) => ValueKind::Redacted,
        }
    }

    /// How many rows this value discloses when it is opened. A scalar
    /// discloses nothing; so does an empty container, which is why an empty
    /// object never grows a chevron that would open onto nothing.
    fn member_count(&self) -> usize {
        match self {
            Self::Array(items) => items.len(),
            Self::Object(members) => members.len(),
            _ => 0,
        }
    }
}

/// What kind of thing a row holds, which is the one fact its node publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
    Redacted,
}

impl ValueKind {
    /// The published name. `null` and `withheld` are states rather than
    /// values, and an empty container says so, because "object" and "an object
    /// with nothing in it" are different answers to the same question.
    fn published(self, value: &JsonValue) -> SharedString {
        match value {
            JsonValue::Null => SharedString::new_static("null"),
            JsonValue::Bool(true) => SharedString::new_static("true"),
            JsonValue::Bool(false) => SharedString::new_static("false"),
            JsonValue::Number(text) => text.clone(),
            JsonValue::String(text) => text.clone(),
            JsonValue::Array(items) if items.is_empty() => SharedString::new_static("empty array"),
            JsonValue::Array(_) => SharedString::new_static("array"),
            JsonValue::Object(members) if members.is_empty() => {
                SharedString::new_static("empty object")
            }
            JsonValue::Object(_) => SharedString::new_static("object"),
            // The shape is not published: a snapshot carries that the value
            // was withheld and nothing that describes it.
            JsonValue::Redacted(_) => SharedString::new_static("withheld"),
        }
    }
}

/// One row as it is drawn: what the keyboard can reach this frame.
#[derive(Debug, Clone)]
struct Line {
    /// The row's identity within the document: a slash-joined path of keys
    /// and array indices, escaped the way a JSON pointer token is. It is
    /// business identity, not list position: the path of a member does not
    /// change when a sibling above it is removed.
    path: SharedString,
    /// The key, or the index within an array.
    label: SharedString,
    kind: ValueKind,
    /// What is drawn to the right of the key.
    shown: SharedString,
    /// A redacted row's shape, drawn beside the mark and published nowhere.
    shape: Option<SharedString>,
    published: SharedString,
    level: u32,
    open: bool,
    has_members: bool,
    parent: Option<SharedString>,
    first_member: Option<SharedString>,
}

/// Escapes one path token the way RFC 6901 does, so a key containing a slash
/// cannot be read back as two levels of nesting.
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn join(parent: &str, token: &str) -> SharedString {
    if parent.is_empty() {
        SharedString::from(escape(token))
    } else {
        SharedString::from(format!("{parent}/{}", escape(token)))
    }
}

/// What is drawn to the right of a key.
///
/// The container marks and the three JSON literals are syntax rather than
/// prose: a reader pastes them back into a document, so they are not in the
/// string catalogue and are not translated.
fn shown_text(value: &JsonValue) -> SharedString {
    match value {
        JsonValue::Null => SharedString::new_static("null"),
        JsonValue::Bool(true) => SharedString::new_static("true"),
        JsonValue::Bool(false) => SharedString::new_static("false"),
        JsonValue::Number(text) => text.clone(),
        JsonValue::String(text) => SharedString::from(format!("\"{text}\"")),
        JsonValue::Array(items) if items.is_empty() => SharedString::new_static("[]"),
        JsonValue::Array(_) => SharedString::new_static("[…]"),
        JsonValue::Object(members) if members.is_empty() => SharedString::new_static("{}"),
        JsonValue::Object(_) => SharedString::new_static("{…}"),
        JsonValue::Redacted(_) => SharedString::new_static("••••••••"),
    }
}

fn flatten(
    value: &JsonValue,
    path: SharedString,
    label: SharedString,
    level: u32,
    parent: Option<&SharedString>,
    expanded: &[SharedString],
    out: &mut Vec<Line>,
) {
    let has_members = value.member_count() > 0;
    let open = has_members && expanded.contains(&path);
    let first_member = match value {
        JsonValue::Object(members) => members
            .first()
            .map(|(key, _)| join(path.as_ref(), key.as_ref())),
        JsonValue::Array(items) if !items.is_empty() => Some(join(path.as_ref(), "0")),
        _ => None,
    };
    out.push(Line {
        path: path.clone(),
        label,
        kind: value.kind(),
        shown: shown_text(value),
        shape: match value {
            JsonValue::Redacted(shape) => Some(shape.clone()),
            _ => None,
        },
        published: value.kind().published(value),
        level,
        open,
        has_members,
        parent: parent.cloned(),
        first_member,
    });
    if !open {
        return;
    }
    match value {
        JsonValue::Object(members) => {
            for (key, member) in members {
                flatten(
                    member,
                    join(path.as_ref(), key.as_ref()),
                    key.clone(),
                    level + 1,
                    Some(&path),
                    expanded,
                    out,
                );
            }
        }
        JsonValue::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let token = index.to_string();
                flatten(
                    item,
                    join(path.as_ref(), &token),
                    SharedString::from(token),
                    level + 1,
                    Some(&path),
                    expanded,
                    out,
                );
            }
        }
        _ => {}
    }
}

/// A collapsible view of a structured value.
#[derive(IntoElement)]
pub struct JsonView {
    ident: Ident,
    value: JsonValue,
    root_label: Option<SharedString>,
    expanded: Vec<SharedString>,
    selected: Option<SharedString>,
    visible_rows: Option<usize>,
    row_height: Option<f32>,
    size: ControlSize,
    disabled: bool,
    on_toggle: Option<ToggleHandler>,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for JsonView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JsonView")
            .field("ident", &self.ident)
            .field("kind", &self.value.kind())
            .field("expanded", &self.expanded)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl JsonView {
    pub fn new(ident: impl Into<Ident>, value: JsonValue) -> Self {
        Self {
            ident: ident.into(),
            value,
            root_label: None,
            expanded: Vec::new(),
            selected: None,
            visible_rows: None,
            row_height: None,
            size: ControlSize::Md,
            disabled: false,
            on_toggle: None,
            on_select: None,
        }
    }

    /// What the single row of a document that is one scalar is called. A
    /// document that is an object or an array names its own members and never
    /// uses this.
    pub fn root_label(mut self, label: impl Into<SharedString>) -> Self {
        self.root_label = Some(label.into());
        self
    }

    /// The paths whose members are disclosed. Everything else is shut, and
    /// nothing under a shut path is laid out or published.
    pub fn expanded(mut self, paths: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded = paths.into_iter().collect();
        self
    }

    pub fn expanded_paths<S: AsRef<str>>(mut self, paths: &[S]) -> Self {
        self.expanded = paths
            .iter()
            .map(|path| SharedString::from(path.as_ref().to_string()))
            .collect();
        self
    }

    pub fn selected(mut self, path: impl Into<SharedString>) -> Self {
        self.selected = Some(path.into());
        self
    }

    /// Bounds the viewport, which is what lets the view skip the rows it does
    /// not show. Without it the view sizes itself to its content and every
    /// disclosed row is laid out.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Reports a path and the disclosure state it should take. The view opens
    /// nothing itself.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// The rows this view would disclose, as paths, in the order the keyboard
    /// walks them. A caller uses it to expand everything without knowing the
    /// shape of the document.
    pub fn disclosed_paths(&self, cx: &App) -> Vec<SharedString> {
        self.lines(cx)
            .into_iter()
            .map(|line| line.path)
            .collect::<Vec<_>>()
    }

    fn lines(&self, cx: &App) -> Vec<Line> {
        let mut lines = Vec::new();
        match &self.value {
            JsonValue::Object(members) => {
                for (key, member) in members {
                    flatten(
                        member,
                        join("", key.as_ref()),
                        key.clone(),
                        1,
                        None,
                        &self.expanded,
                        &mut lines,
                    );
                }
            }
            JsonValue::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    let token = index.to_string();
                    flatten(
                        item,
                        join("", &token),
                        SharedString::from(token),
                        1,
                        None,
                        &self.expanded,
                        &mut lines,
                    );
                }
            }
            scalar => {
                let label = self
                    .root_label
                    .clone()
                    .unwrap_or_else(|| cx.strings().text(StringKey::JsonRootValue));
                flatten(
                    scalar,
                    SharedString::default(),
                    label,
                    1,
                    None,
                    &self.expanded,
                    &mut lines,
                );
            }
        }
        lines
    }

    /// The semantic id of one row.
    ///
    /// A document that is one scalar has no path, so its single row is named
    /// `value`. No key can collide with it: a document with keys renders its
    /// members instead and never produces that row.
    fn row_ident(&self, path: &SharedString) -> Ident {
        if path.is_empty() {
            self.ident.child("value")
        } else {
            self.ident.child(path.as_ref())
        }
    }
}

impl Disableable for JsonView {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for JsonView {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

/// What a keystroke asks for.
enum Move {
    Select(SharedString),
    Toggle(SharedString, bool),
}

/// The same movement a tree has, over the rows a frame disclosed. The arrow
/// toward children opens or descends; its logical opposite shuts or climbs.
fn keystroke_move(
    key: &str,
    direction: LayoutDirection,
    lines: &[Line],
    selected: Option<&SharedString>,
) -> Option<Move> {
    let at = lines
        .iter()
        .position(|line| Some(&line.path) == selected)
        .filter(|_| selected.is_some());
    let key = match direction.arrow_step(key) {
        Some(1) => "toward-children",
        Some(_) => "toward-parent",
        None => key,
    };
    match key {
        "up" | "down" => {
            let next = match (key, at) {
                ("down", Some(at)) => at + 1,
                ("down", None) => 0,
                ("up", Some(at)) => at.checked_sub(1)?,
                _ => lines.len().checked_sub(1)?,
            };
            lines.get(next).map(|line| Move::Select(line.path.clone()))
        }
        "home" => lines.first().map(|line| Move::Select(line.path.clone())),
        "end" => lines.last().map(|line| Move::Select(line.path.clone())),
        "toward-children" => {
            let line = lines.get(at?)?;
            if line.has_members && !line.open {
                Some(Move::Toggle(line.path.clone(), true))
            } else {
                line.first_member
                    .clone()
                    .filter(|_| line.open)
                    .map(Move::Select)
            }
        }
        "toward-parent" => {
            let line = lines.get(at?)?;
            if line.has_members && line.open {
                Some(Move::Toggle(line.path.clone(), false))
            } else {
                line.parent.clone().map(Move::Select)
            }
        }
        _ => None,
    }
}

impl RenderOnce for JsonView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let row_height = self.row_height.unwrap_or(metrics.height);
        let lines = Rc::new(self.lines(cx));
        let count = lines.len();
        let scroll = scroll_handle(&self.ident, window, cx);
        let view = Rc::new(self);

        let owner = Rc::clone(&view);
        let source = Rc::clone(&lines);
        let row_theme = theme.clone();
        let rows = uniform_list(
            view.ident.child("rows").element_id(),
            count,
            move |range: Range<usize>, window, cx| {
                range
                    .filter_map(|index| source.get(index).cloned())
                    .map(|line| {
                        row_element(
                            &owner,
                            &row_theme,
                            row_height,
                            metrics.icon_size,
                            &line,
                            window,
                            cx,
                        )
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&scroll)
        .w_full()
        .with_sizing_behavior(if view.visible_rows.is_some() {
            ListSizingBehavior::Auto
        } else {
            ListSizingBehavior::Infer
        })
        .when_some(view.visible_rows, |element, rows| {
            element.h(px(row_height * rows as f32))
        });

        let mut container = div()
            .id(view.ident.element_id())
            .column()
            .w_full()
            .font_family(theme.typography.mono.clone())
            .child(rows);

        if !view.disabled && (view.on_select.is_some() || view.on_toggle.is_some()) {
            let direction = cx.layout_direction();
            let selected = view.selected.clone();
            let select = view.on_select.clone();
            let toggle = view.on_toggle.clone();
            let lines = Rc::clone(&lines);
            let scroll = scroll.clone();
            container = container.on_key_down(move |event, window, cx| {
                let Some(next) = keystroke_move(
                    event.keystroke.key.as_str(),
                    direction,
                    &lines,
                    selected.as_ref(),
                ) else {
                    return;
                };
                match next {
                    Move::Select(path) => {
                        if Some(&path) == selected.as_ref() {
                            return;
                        }
                        // The view scrolls to what it reported, so a row the
                        // caller is told about is one the reader can see.
                        if let Some(index) = lines.iter().position(|line| line.path == path) {
                            scroll.scroll_to_item(index, ScrollStrategy::Nearest);
                            window.refresh();
                        }
                        let Some(handler) = select.as_ref() else {
                            return;
                        };
                        handler(path, window, cx);
                    }
                    Move::Toggle(path, open) => {
                        let Some(handler) = toggle.as_ref() else {
                            return;
                        };
                        handler(path, open, window, cx);
                    }
                }
                cx.stop_propagation();
            });
        }

        container.semantic_in(
            cx,
            NodeSpec::new(view.ident.semantic_id(), Role::Tree).value(cx.numbers().count(count)),
        )
    }
}

fn row_element(
    view: &JsonView,
    theme: &Theme,
    height: f32,
    icon_size: f32,
    line: &Line,
    _window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let direction = cx.layout_direction();
    let ident = view.row_ident(&line.path);
    let selected = view.selected.as_ref() == Some(&line.path);
    let selectable = !view.disabled && view.on_select.is_some();
    let toggleable = !view.disabled && line.has_members && view.on_toggle.is_some();
    let value_color = match line.kind {
        // A withheld or missing value recedes; anything the document actually
        // states is drawn at full strength. Neither invents a status colour.
        ValueKind::Null | ValueKind::Redacted => theme.colors.text_faint,
        _ => theme.colors.text,
    };

    let chevron = line.has_members.then(|| {
        let toggle = ident.child("toggle");
        let mut glyph = div()
            .id(toggle.element_id())
            .row()
            .flex_none()
            .size(px(icon_size))
            .child(
                icon(Icon::AltArrowRight)
                    .size(px(icon_size))
                    .text_color(theme.colors.text_muted)
                    // Open, it points down, which is not directional.
                    .when(
                        !line.open && flips(Icon::AltArrowRight, direction),
                        |glyph| {
                            glyph.with_transformation(Transformation::scale(gpui::size(-1.0, 1.0)))
                        },
                    )
                    .when(line.open, |glyph| {
                        glyph.with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                    }),
            )
            .when(toggleable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .focus_ring(theme)
            });

        if let (true, Some(handler)) = (toggleable, view.on_toggle.clone()) {
            let path = line.path.clone();
            let open = line.open;
            glyph = glyph.on_click(move |_, window, cx| {
                handler(path.clone(), !open, window, cx);
                // A disclosure is not a selection, so the row beneath must not
                // also report one.
                cx.stop_propagation();
            });
        }

        glyph.semantic_in(
            cx,
            NodeSpec::new(toggle.semantic_id(), Role::Button)
                .parent(ident.semantic_id())
                .text(line.label.clone())
                .expanded(line.open)
                .disabled(!toggleable),
        )
    });

    let mut row = div()
        .id(ident.element_id())
        .row_reading(direction)
        .w_full()
        .h(px(height))
        .pe(direction, px(theme.space(Space::Sm)))
        .ps(
            direction,
            px(theme.space(Space::Sm)
                + line.level.saturating_sub(1) as f32 * theme.space(Space::Md)),
        )
        .gap(px(theme.space(Space::Xs)))
        .when(selected, |element| element.bg(theme.colors.selected))
        .when(view.disabled, |element| {
            element.opacity(theme.opacity.disabled)
        })
        .when(selectable, |element| {
            element
                .cursor_pointer()
                .tab_index(0)
                .pressable(cx)
                .when(!selected, |element| {
                    element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
                })
                .focus_ring(theme)
        })
        .children(chevron)
        // A row with nothing to disclose still lines up with its siblings, so
        // the indent reads as depth rather than as decoration.
        .when(!line.has_members, |element| {
            element.child(div().flex_none().size(px(icon_size)))
        })
        .child(
            text(theme, TypeScale::Code, line.label.clone())
                .flex_none()
                .text_start(direction)
                .text_tone(theme, TextTone::Muted),
        )
        .child(
            text(theme, TypeScale::Code, line.shown.clone())
                .flex_1()
                .overflow_hidden()
                .text_start(direction)
                .text_color(value_color),
        );

    // The mark says the value was kept back; the shape says how much was kept
    // back. Neither is the value, and neither reaches the semantic tree.
    if let Some(shape) = line.shape.clone() {
        row = row.child(
            div()
                .flex_none()
                .row_reading(direction)
                .gap(px(theme.space(Space::Xs)))
                .child(
                    text(
                        theme,
                        TypeScale::Caption,
                        cx.strings().text(StringKey::JsonWithheld),
                    )
                    .text_tone(theme, TextTone::Muted),
                )
                .child(text(theme, TypeScale::Code, shape).text_tone(theme, TextTone::Faint)),
        );
    }

    if let (true, Some(handler)) = (selectable, view.on_select.clone()) {
        let path = line.path.clone();
        row = row.on_click(move |_, window, cx| handler(path.clone(), window, cx));
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::TreeItem)
        .parent(match &line.parent {
            Some(parent) => view.row_ident(parent).semantic_id(),
            None => view.ident.semantic_id(),
        })
        .text(line.label.clone())
        .value(line.published.clone())
        .selected(selected)
        .disabled(view.disabled)
        .level(line.level);
    // Only a row with something under it claims a disclosure state. An empty
    // object reporting `expanded: false` would read as one that is merely shut.
    if line.has_members {
        spec = spec.expanded(line.open);
    }

    row.semantic_in(cx, spec).into_any_element()
}

type ScrollHandles = HashMap<SharedString, UniformListScrollHandle>;

/// Where a view is scrolled, kept across the frames a `RenderOnce` builder is
/// rebuilt in, keyed by the identity the caller gave it.
fn scroll_handle(ident: &Ident, window: &Window, cx: &mut App) -> UniformListScrollHandle {
    window_state::with(
        window.window_handle().window_id(),
        cx,
        |handles: &mut ScrollHandles| handles.entry(ident.semantic_id()).or_default().clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> JsonValue {
        JsonValue::object([
            ("name", JsonValue::string("run")),
            ("retries", JsonValue::number("3")),
            ("cursor", JsonValue::Null),
            ("labels", JsonValue::object(Vec::<(&str, JsonValue)>::new())),
            (
                "steps",
                JsonValue::array([JsonValue::string("plan"), JsonValue::string("apply")]),
            ),
        ])
    }

    fn lines(expanded: &[&str]) -> Vec<Line> {
        let expanded: Vec<SharedString> = expanded
            .iter()
            .map(|path| SharedString::from(path.to_string()))
            .collect();
        let mut out = Vec::new();
        let JsonValue::Object(members) = document() else {
            unreachable!()
        };
        for (key, member) in &members {
            flatten(
                member,
                join("", key.as_ref()),
                key.clone(),
                1,
                None,
                &expanded,
                &mut out,
            );
        }
        out
    }

    #[test]
    fn a_shut_value_discloses_nothing() {
        let shut = lines(&[]);
        let paths: Vec<&str> = shut.iter().map(|line| line.path.as_ref()).collect();
        assert_eq!(
            paths,
            vec!["name", "retries", "cursor", "labels", "steps"],
            "a shut array must not lay out its items"
        );
    }

    #[test]
    fn an_empty_object_offers_no_disclosure() {
        let labels = lines(&[]);
        let empty = labels
            .iter()
            .find(|line| line.path.as_ref() == "labels")
            .expect("present");
        assert!(!empty.has_members);
        assert_eq!(empty.published.as_ref(), "empty object");
    }

    #[test]
    fn a_key_containing_a_slash_stays_one_level() {
        let value = JsonValue::object([("a/b", JsonValue::object([("c", JsonValue::Bool(true))]))]);
        let JsonValue::Object(members) = &value else {
            unreachable!()
        };
        let mut out = Vec::new();
        flatten(
            &members[0].1,
            join("", members[0].0.as_ref()),
            members[0].0.clone(),
            1,
            None,
            &[SharedString::from("a~1b")],
            &mut out,
        );
        assert_eq!(out[0].path.as_ref(), "a~1b");
        assert_eq!(out[1].path.as_ref(), "a~1b/c");
    }

    #[test]
    fn right_opens_a_shut_value_and_then_descends() {
        let shut = lines(&[]);
        let steps = SharedString::from("steps");
        match keystroke_move("right", LayoutDirection::LeftToRight, &shut, Some(&steps)) {
            Some(Move::Toggle(path, next)) => {
                assert_eq!(path.as_ref(), "steps");
                assert!(next);
            }
            _ => panic!("right must open a shut value"),
        }
        let open = lines(&["steps"]);
        match keystroke_move("right", LayoutDirection::LeftToRight, &open, Some(&steps)) {
            Some(Move::Select(path)) => assert_eq!(path.as_ref(), "steps/0"),
            _ => panic!("right must descend into an open value"),
        }
    }

    #[test]
    fn left_opens_a_shut_value_in_right_to_left_layout() {
        let shut = lines(&[]);
        let steps = SharedString::from("steps");
        match keystroke_move("left", LayoutDirection::RightToLeft, &shut, Some(&steps)) {
            Some(Move::Toggle(path, next)) => {
                assert_eq!(path.as_ref(), "steps");
                assert!(next);
            }
            _ => panic!("left must open a shut value in RTL"),
        }
    }

    #[test]
    fn a_move_stops_at_the_ends() {
        let shut = lines(&[]);
        let last = SharedString::from("steps");
        assert!(keystroke_move("down", LayoutDirection::LeftToRight, &shut, Some(&last)).is_none());
        let first = SharedString::from("name");
        assert!(keystroke_move("up", LayoutDirection::LeftToRight, &shut, Some(&first)).is_none());
    }
}
