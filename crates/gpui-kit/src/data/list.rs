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
    AnyElement, App, Global, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    RenderOnce, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled,
    UniformListScrollHandle, Window, div, point, prelude::FluentBuilder, px, uniform_list,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Theme};

use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};
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

        let rows = {
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
                                row_height,
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
        }
        .track_scroll(&scroll)
        .w_full()
        .with_sizing_behavior(if self.visible_rows.is_some() {
            ListSizingBehavior::Auto
        } else {
            ListSizingBehavior::Infer
        })
        .when_some(self.visible_rows, |element, rows| {
            element.h(px(row_height * rows as f32))
        });

        let mut container = div().id(ident.element_id()).column().w_full().child(rows);

        // A drag that reaches the edge of the viewport has run out of list to
        // aim at, so the list brings the next row to the pointer rather than
        // asking the pointer to go somewhere it cannot.
        if reorder.is_some() {
            let rendered = Rc::clone(&rendered);
            let scroll = scroll.clone();
            container = container.on_drag_move::<DragItem>(move |event, window, _| {
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
                scroll.scroll_to_item(next, ScrollStrategy::Nearest);
                window.refresh();
            });
        }

        if let Some(handler) = handler {
            let selected = self.selected.clone();
            let rendered = Rc::clone(&rendered);
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
                scroll.scroll_to_item(index, ScrollStrategy::Nearest);
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
    height: f32,
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
        .h(px(height))
        .px(px(theme.space(Space::Sm)))
        .gap(px(theme.space(Space::Sm)))
        .when(selected, |element| element.bg(theme.colors.selected))
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
                .when(!selected, |element| {
                    element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
                })
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

#[derive(Default)]
struct ScrollHandles(RefCell<HashMap<SharedString, UniformListScrollHandle>>);

impl Global for ScrollHandles {}

/// Brings row `index` of the list with this identity into view.
///
/// Scroll position belongs to the list, not to whoever draws over it, so a
/// surface built on a list — a conversation that follows its newest message —
/// moves it by naming the list rather than by owning a GPUI handle of its own.
pub fn scroll_to_row(ident: &Ident, index: usize, cx: &mut App) {
    scroll_handle(ident, cx).scroll_to_item(index, ScrollStrategy::Bottom);
}

/// The scroll position of the list with this identity.
///
/// Where a list is scrolled is transient view state, but a `RenderOnce`
/// builder is rebuilt every frame and cannot carry it. Keying the handle by
/// identity keeps the position across rebuilds without making every caller own
/// a GPUI handle.
fn scroll_handle(ident: &Ident, cx: &mut App) -> UniformListScrollHandle {
    if !cx.has_global::<ScrollHandles>() {
        cx.set_global(ScrollHandles::default());
    }
    let mut handles = cx.global::<ScrollHandles>().0.borrow_mut();
    handles.entry(ident.semantic_id()).or_default().clone()
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
