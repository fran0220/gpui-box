//! A virtualized list over a caller-owned data set.
//!
//! The list never holds the rows. It asks the caller to render one index at a
//! time, and the caller stamps each row with the business identity that row
//! already has, so a semantic id never encodes where the row happens to sit.
//!
//! # Only rendered rows are published
//!
//! Virtualization means the window holds a viewport, not a data set. A row
//! outside the viewport is not laid out, has no bounds, and publishes no
//! semantic node, so a test can only assert what is on screen. The list node
//! itself carries the total in `value`, which is how a test states the honest
//! version: a thousand items exist, twelve of them are rendered.
//!
//! Virtualization needs a bounded viewport. With [`List::visible_rows`] the
//! list renders only the rows that fit; without it the list sizes itself to
//! its content and every row is laid out.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ListAlignment, ListSizingBehavior,
    ParentElement, RenderOnce, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled,
    Window, div, point, prelude::FluentBuilder, px, uniform_list,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Theme};

use crate::data::flow::Flow;
use crate::data::viewport::scroll_handle;
pub use crate::data::viewport::{glide_to_row, reveal_row, scroll_to_row};
use crate::foundation::direction::ActiveDirection;
use crate::foundation::{
    Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedRow, Sizable, StyledExt,
};
use crate::interaction::dnd::{
    self, DragItem, DropAxis, DropIntent, DropPosition, MakingWay, RowTarget, SurfaceDrag,
};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type RenderRow = Rc<dyn Fn(usize, &mut Window, &mut App) -> ListItem>;
type ReorderHandler = Rc<dyn Fn(&DropIntent, &mut Window, &mut App)>;
type Accepts = Rc<dyn Fn(&DragItem, &DropPosition) -> bool>;

/// One row, named by the caller.
pub struct ListItem {
    id: SharedString,
    text: Option<SharedString>,
    disabled: bool,
    content: AnyElement,
}

impl std::fmt::Debug for ListItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ListItem")
            .field("id", &self.id)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ListItem {
    /// `id` is the row's business identity, not its index.
    pub fn new(id: impl Into<SharedString>, content: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            text: None,
            disabled: false,
            content: content.into_any_element(),
        }
    }

    /// The name the row publishes, for a test or a screen reader that has only
    /// the tree to go on.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A list that renders only the rows its viewport holds.
#[derive(IntoElement)]
pub struct List {
    ident: Ident,
    count: usize,
    render_row: RenderRow,
    selected: Option<SharedString>,
    row_height: Option<f32>,
    /// Whether a row is as tall as its content rather than as tall as a slot.
    flowing: bool,
    /// The rows in order, when the caller named them. See [`List::keys`].
    keys: Option<Vec<SharedString>>,
    /// Where a flowing list rests when its content does not fill it.
    alignment: ListAlignment,
    visible_rows: Option<usize>,
    size: ControlSize,
    disabled: bool,
    on_select: Option<SelectHandler>,
    reorderable: bool,
    accepts: Option<Accepts>,
    on_reorder: Option<ReorderHandler>,
}

impl std::fmt::Debug for List {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("List")
            .field("ident", &self.ident)
            .field("count", &self.count)
            .field("selected", &self.selected)
            .field("visible_rows", &self.visible_rows)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl List {
    pub fn new(
        ident: impl Into<Ident>,
        count: usize,
        render_row: impl Fn(usize, &mut Window, &mut App) -> ListItem + 'static,
    ) -> Self {
        Self {
            ident: ident.into(),
            count,
            render_row: Rc::new(render_row),
            selected: None,
            row_height: None,
            flowing: false,
            keys: None,
            alignment: ListAlignment::Top,
            visible_rows: None,
            size: ControlSize::Md,
            disabled: false,
            on_select: None,
            reorderable: false,
            accepts: None,
            on_reorder: None,
        }
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Every row is this tall. Uniform height is what makes the list cheap;
    /// the default comes from the control scale for the list's size.
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Lets each row be as tall as what it holds.
    ///
    /// A uniform list costs the same for ten thousand rows as for ten because
    /// it knows where every row starts without laying one out. That is also
    /// its price: a row is a slot, and content longer than the slot has to say
    /// how much it left out. A flowing list measures a row when it first comes
    /// into view and keeps that measurement, so the row grows to fit — and in
    /// exchange it can only estimate how far a set it has not seen reaches, so
    /// a scrollbar over one settles as the reader moves through it.
    ///
    /// [`List::row_height`] stops being the height of a row and becomes the
    /// estimate used for rows nobody has laid out yet.
    pub fn flowing(mut self) -> Self {
        self.flowing = true;
        self
    }

    /// Rests a flowing list against its end rather than its start, for a
    /// conversation or a log whose newest row is the one worth seeing.
    pub fn anchored_to_end(mut self) -> Self {
        self.alignment = ListAlignment::Bottom;
        self
    }

    /// Names the rows, in the order they are drawn.
    ///
    /// A flowing list only learns a row's height by laying it out, and what it
    /// has learned is addressed by index. So when the set changes, the
    /// question is which indices still mean what they meant, and a count
    /// cannot answer it: a row inserted at the top moves every row below it,
    /// and a list told only that the count went up by one would keep every
    /// height against the wrong row.
    ///
    /// Naming the rows answers it exactly. The list splices the range that
    /// actually changed and keeps every measurement either side, so a
    /// conversation whose newest block grew re-measures that block rather than
    /// the conversation, and a reader's scroll position does not lurch.
    ///
    /// The names are the rows' own identities, the same ones
    /// [`ListItem::new`] takes, and are ignored by a uniform list, which has
    /// nothing measured to keep.
    pub fn keys(mut self, keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.keys = Some(keys.into_iter().map(Into::into).collect());
        self
    }

    /// Bounds the viewport to `rows` rows, which is what lets the list skip
    /// the rows it does not show.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Lets a row be picked up and put down somewhere else in the list.
    ///
    /// The whole row is the handle. A list row's ordinary action is a click,
    /// and GPUI only calls a press a drag once it has travelled two pixels, so
    /// both fit on the same row without a grip column the caller would have to
    /// render into every row it owns.
    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    /// Whether this list takes a payload, and where.
    ///
    /// Without one, a reorderable list takes its own rows and nothing else.
    /// Anything wider than that is policy, and policy is the caller's.
    pub fn accepts(
        mut self,
        predicate: impl Fn(&DragItem, &DropPosition) -> bool + 'static,
    ) -> Self {
        self.accepts = Some(Rc::new(predicate));
        self
    }

    /// Reports where a dropped row should go. The list does not move it.
    pub fn on_reorder(
        mut self,
        handler: impl Fn(&DropIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reorder = Some(Rc::new(handler));
        self
    }
}

impl Disableable for List {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for List {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for List {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let row_height = self.row_height.unwrap_or(metrics.height);
        let ident = self.ident.clone();
        let count = self.count;
        let handler = self
            .on_select
            .clone()
            .filter(|_| !self.disabled)
            .filter(|_| count > 0);
        let render_row = Rc::clone(&self.render_row);
        let scroll = scroll_handle(&ident, cx);
        let reorder = self.reorder(window, cx);

        // Which index published which id on the last frame. The rows fill it
        // during prepaint, so the keyboard handler, which runs later, can name
        // the row it is moving away from without consulting the data set.
        let rendered: Rendered = Rc::new(RefCell::new(HashMap::new()));

        let sizing = if self.visible_rows.is_some() {
            ListSizingBehavior::Auto
        } else {
            ListSizingBehavior::Infer
        };
        let flowing = self.flowing;
        let rows: AnyElement = if flowing {
            let ident = ident.clone();
            let theme = theme.clone();
            let selected = self.selected.clone();
            let handler = handler.clone();
            let render_row = Rc::clone(&render_row);
            let rendered = Rc::clone(&rendered);
            let reorder = reorder.clone();
            let row_ident = ident.clone();
            // A list whose rows are as tall as their content is a flow whose
            // rows happen to be list rows.
            let mut flow = Flow::new(ident, count, move |index, window, cx| {
                let item = render_row(index, window, cx);
                rendered.borrow_mut().insert(index, item.id.clone());
                // A flowing row states no height: it is as tall as what it
                // holds, which is the whole point of asking for one.
                row_element(
                    &row_ident,
                    &theme,
                    None,
                    item,
                    index,
                    selected.as_ref(),
                    handler.as_ref(),
                    reorder.as_ref(),
                    window,
                    cx,
                )
            })
            .estimate(row_height);
            if self.alignment == ListAlignment::Bottom {
                flow = flow.anchored_to_end();
            }
            if let Some(keys) = self.keys.clone() {
                flow = flow.keys(keys);
            }
            if let Some(rows) = self.visible_rows {
                flow = flow.visible_rows(rows);
            }
            flow.into_any_element()
        } else {
            let ident = ident.clone();
            let theme = theme.clone();
            let selected = self.selected.clone();
            let handler = handler.clone();
            let render_row = Rc::clone(&render_row);
            let rendered = Rc::clone(&rendered);
            let reorder = reorder.clone();
            uniform_list(
                ident.child("rows").element_id(),
                count,
                move |range: Range<usize>, window, cx| {
                    rendered
                        .borrow_mut()
                        .retain(|index, _| range.contains(index));
                    range
                        .map(|index| {
                            let item = render_row(index, window, cx);
                            rendered.borrow_mut().insert(index, item.id.clone());
                            row_element(
                                &ident,
                                &theme,
                                Some(row_height),
                                item,
                                index,
                                selected.as_ref(),
                                handler.as_ref(),
                                reorder.as_ref(),
                                window,
                                cx,
                            )
                        })
                        .collect::<Vec<_>>()
                },
            )
            .track_scroll(&scroll)
            .w_full()
            .with_sizing_behavior(sizing)
            .when_some(self.visible_rows, |element, rows| {
                element.h(px(row_height * rows as f32))
            })
            .into_any_element()
        };

        let mut container = div().id(ident.element_id()).column().w_full().child(rows);

        // A drag that reaches the edge of the viewport has run out of list to
        // aim at, so the list brings the next row to the pointer rather than
        // asking the pointer to go somewhere it cannot.
        if reorder.is_some() {
            let rendered = Rc::clone(&rendered);
            let scroll = scroll.clone();
            let reveal_ident = ident.clone();
            container = container.on_drag_move::<DragItem>(move |event, window, cx| {
                let pointer = event.event.position;
                if !event.bounds.contains(&pointer) {
                    return;
                }
                let band = px(row_height);
                let rendered = rendered.borrow();
                let next = if pointer.y < event.bounds.top() + band {
                    rendered.keys().min().and_then(|first| first.checked_sub(1))
                } else if pointer.y > event.bounds.bottom() - band {
                    rendered
                        .keys()
                        .max()
                        .map(|last| last + 1)
                        .filter(|next| *next < count)
                } else {
                    None
                };
                let Some(next) = next else {
                    return;
                };
                if flowing {
                    reveal_row(&reveal_ident, next, cx);
                } else {
                    scroll.scroll_to_item(next, ScrollStrategy::Nearest);
                }
                window.refresh();
            });
        }

        if let Some(handler) = handler {
            let selected = self.selected.clone();
            let rendered = Rc::clone(&rendered);
            let keyboard_ident = ident.clone();
            container = container.on_key_down(move |event, window, cx| {
                let from = current_index(&rendered, selected.as_ref());
                let Some(target) = target_index(event.keystroke.key.as_str(), from, count) else {
                    return;
                };
                let delta = if from.is_some_and(|from| target < from) {
                    -1
                } else {
                    1
                };
                let Some((index, id)) = selectable(&render_row, target, delta, count, window, cx)
                else {
                    return;
                };
                // The list scrolls to what it reported, so the row the caller
                // is being told about is one the typist can see. Scrolling is
                // the list's own state, so it asks for the frame that applies
                // it rather than waiting for the caller to notice.
                if flowing {
                    reveal_row(&keyboard_ident, index, cx);
                } else {
                    scroll.scroll_to_item(index, ScrollStrategy::Nearest);
                }
                window.refresh();
                if Some(&id) == selected.as_ref() {
                    return;
                }
                handler(id, window, cx);
                cx.stop_propagation();
            });
        }

        container.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::List).value(count.to_string()),
        )
    }
}

type Rendered = Rc<RefCell<HashMap<usize, SharedString>>>;

/// What a row needs to take part in a reorder.
#[derive(Clone)]
struct Reorder {
    surface: SharedString,
    drag: Option<SurfaceDrag>,
    accepts: Accepts,
    on_drop: ReorderHandler,
}

impl List {
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
}

#[allow(clippy::too_many_arguments)]
fn row_element(
    list: &Ident,
    theme: &Theme,
    height: Option<f32>,
    item: ListItem,
    index: usize,
    selected: Option<&SharedString>,
    handler: Option<&SelectHandler>,
    reorder: Option<&Reorder>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ident = list.child(item.id.as_ref());
    let selected = selected == Some(&item.id);
    let actionable = !item.disabled && handler.is_some();
    let draggable = reorder.filter(|_| !item.disabled);
    let drag = draggable.and_then(|reorder| reorder.drag.as_ref());
    let carried = drag.is_some_and(|drag| drag.carries(&item.id));
    let landing = drag.and_then(|drag| drag.indicator_for(&item.id));
    let label = item.text.clone().unwrap_or_else(|| item.id.clone());

    let mut row = div()
        .id(ident.element_id())
        .row()
        .w_full()
        .when_some(height, |element, height| element.h(px(height)))
        .px(px(theme.space(Space::Sm)))
        .gap(px(theme.space(Space::Sm)))
        .selected_row(theme, cx.layout_direction(), selected)
        .when(item.disabled, |element| {
            element.opacity(theme.opacity.disabled)
        })
        // The row the pointer is carrying stays where the data still says it
        // is, and says so by receding rather than by leaving a hole.
        .when(carried, |element| element.opacity(theme.opacity.muted))
        .when(actionable, |element| {
            element
                .cursor_pointer()
                .tab_index(0)
                .pressable(cx)
                .when(!selected, |element| element.hover_row(theme))
                .focus_ring(theme)
        })
        .child(div().flex_1().overflow_hidden().child(item.content))
        .children(landing.map(|(position, accepted)| {
            dnd::indicator(&position, accepted, DropAxis::Vertical, cx)
        }));

    if let (true, Some(handler)) = (actionable, handler) {
        let id = item.id.clone();
        let handler = Rc::clone(handler);
        row = row.on_click(move |_, window, cx| handler(id.clone(), window, cx));
    }

    if let Some(reorder) = draggable {
        row = dnd::draggable(
            row,
            DragItem::new(reorder.surface.clone(), item.id.clone(), label),
        );
        row = dnd::drop_target(
            row,
            RowTarget {
                surface: reorder.surface.clone(),
                id: item.id.clone(),
                index,
                allow_into: false,
                axis: DropAxis::Vertical,
                accepts: Rc::clone(&reorder.accepts),
                on_drop: Rc::clone(&reorder.on_drop),
            },
        );
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Row)
        .parent(list.semantic_id())
        .selected(selected)
        .disabled(item.disabled);
    if let Some(text) = item.text {
        spec = spec.text(text);
    }
    let row = row.semantic_in(cx, spec);

    match draggable {
        Some(reorder) => {
            let shift = reorder
                .drag
                .as_ref()
                .filter(|drag| drag.makes_way(index))
                .map_or(px(0.0), |_| dnd::make_way_gap(cx, DropAxis::Vertical));
            row.make_way(ident.semantic_id(), point(px(0.0), shift), window, cx)
                .into_any_element()
        }
        None => row.into_any_element(),
    }
}

/// Where the reported selection sits among the rows that were rendered.
///
/// A selection scrolled out of the viewport has no known index, so a move
/// starts from the top of what is visible rather than from nowhere.
fn current_index(rendered: &Rendered, selected: Option<&SharedString>) -> Option<usize> {
    let rendered = rendered.borrow();
    selected
        .and_then(|id| {
            rendered
                .iter()
                .find(|(_, row)| *row == id)
                .map(|(index, _)| *index)
        })
        .or_else(|| rendered.keys().min().copied())
}

fn target_index(key: &str, from: Option<usize>, count: usize) -> Option<usize> {
    match key {
        "up" => from?.checked_sub(1),
        "down" => match from {
            Some(from) => Some(from + 1).filter(|next| *next < count),
            None => Some(0),
        },
        "home" => Some(0),
        "end" => count.checked_sub(1),
        _ => None,
    }
}

/// The first row from `target` in `delta`'s direction that accepts selection.
///
/// Naming a row that was never rendered means asking the caller to build it,
/// which is the only way a list that does not hold the data can report a row
/// the typist cannot yet see.
fn selectable(
    render_row: &RenderRow,
    target: usize,
    delta: isize,
    count: usize,
    window: &mut Window,
    cx: &mut App,
) -> Option<(usize, SharedString)> {
    let mut index = target as isize;
    while index >= 0 && (index as usize) < count {
        let item = render_row(index as usize, window, cx);
        if !item.disabled {
            return Some((index as usize, item.id));
        }
        index += delta;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_stops_at_the_ends_instead_of_wrapping() {
        assert_eq!(target_index("up", Some(0), 10), None);
        assert_eq!(target_index("down", Some(9), 10), None);
        assert_eq!(target_index("down", Some(3), 10), Some(4));
        assert_eq!(target_index("home", Some(3), 10), Some(0));
        assert_eq!(target_index("end", Some(3), 10), Some(9));
    }

    #[test]
    fn a_move_without_a_selection_enters_at_the_top() {
        assert_eq!(target_index("down", None, 10), Some(0));
        assert_eq!(target_index("end", None, 0), None);
    }
}
