//! A responsive, selectable collection of image tiles.
//!
//! The image bytes and loading state belong to the caller. This component only
//! provides stable tile identity, layout, labels, and selection intents.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::foundation::{Disableable, Hoverable, Ident, SelectedFill, StyledExt, text};
use crate::layout::{Breakpoint, GridColumns, measure};
use crate::strings::ActiveNumbers;

/// One tile in an [`ImageList`].
pub struct ImageListItem {
    pub id: SharedString,
    pub label: SharedString,
    pub image: AnyElement,
    pub disabled: bool,
}

impl std::fmt::Debug for ImageListItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageListItem")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ImageListItem {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        image: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            image: image.into_any_element(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// A token-spaced image tile collection.
#[derive(IntoElement)]
pub struct ImageList {
    ident: Ident,
    items: Vec<ImageListItem>,
    columns: GridColumns,
    gap: Space,
    selected: Option<SharedString>,
    disabled: bool,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for ImageList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageList")
            .field("ident", &self.ident)
            .field("items", &self.items.len())
            .field("columns", &self.columns)
            .field("selected", &self.selected)
            .finish()
    }
}

impl ImageList {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            items: Vec::new(),
            columns: GridColumns::new(4),
            gap: Space::Sm,
            selected: None,
            disabled: false,
            on_select: None,
        }
    }

    pub fn items(mut self, items: impl IntoIterator<Item = ImageListItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = GridColumns::new(columns as u16);
        self
    }

    /// Sets the tile count at a theme-defined measured-container breakpoint.
    pub fn columns_at(mut self, breakpoint: Breakpoint, columns: usize) -> Self {
        self.columns = self.columns.at(breakpoint, columns as u16);
        self
    }

    pub fn gap(mut self, gap: Space) -> Self {
        self.gap = gap;
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
}

impl Disableable for ImageList {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for ImageList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let item_count = self.items.len();
        let measured = measure::cell(&self.ident.semantic_id(), window, cx);
        let width = {
            let width = f32::from(measured.get().size.width);
            (width > 0.0).then_some(width)
        };
        let columns = self.columns.resolve(width, &theme);
        let list_id = self.ident.child("items");
        let mut grid = div()
            .id(list_id.element_id())
            .grid()
            .grid_cols(columns)
            .gap(px(theme.space(self.gap)));
        for item in self.items {
            let item_id = item.id.clone();
            let ident = list_id.child(item.id.as_ref());
            let selected = self.selected.as_ref() == Some(&item.id);
            let disabled = self.disabled || item.disabled;
            let mut tile = div()
                .id(ident.element_id())
                .col_span(1)
                .column()
                .gap(px(theme.space(Space::Xs)))
                .p(px(theme.space(Space::Xs)))
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .selected_fill(&theme, selected)
                .when(disabled, |element| element.opacity(theme.opacity.disabled))
                .when(!disabled && self.on_select.is_some(), |element| {
                    // A tile that answers a click answers the pointer over it.
                    // The selected wash already fills the tile, so only an
                    // unselected one takes the hover step.
                    element
                        .cursor_pointer()
                        .when(!selected, |element| element.hover_row(&theme))
                })
                .child(
                    div()
                        .aspect_ratio(1.0)
                        .w_full()
                        .overflow_hidden()
                        .radius(&theme, Radius::Control)
                        .child(item.image),
                )
                .child(
                    text(&theme, TypeScale::Caption, item.label.clone()).text_color(if disabled {
                        theme.colors.text_disabled
                    } else {
                        theme.colors.text
                    }),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Button)
                        .parent(list_id.semantic_id())
                        .text(item.label.clone())
                        .selected(selected)
                        .disabled(disabled),
                );
            if !disabled && let Some(handler) = self.on_select.clone() {
                tile = tile.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    handler(item_id.clone(), window, cx)
                });
            }
            grid = grid.child(tile);
        }
        div()
            .on_children_prepainted({
                let measured = measured.clone();
                move |bounds, window, _| {
                    if let Some(bounds) = bounds.first() {
                        measure::record(&measured, *bounds, window);
                    }
                }
            })
            .id(self.ident.element_id())
            .w_full()
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(
                grid.semantic_in(
                    cx,
                    NodeSpec::new(list_id.semantic_id(), Role::List)
                        .parent(self.ident.semantic_id())
                        .value(cx.numbers().count(item_count)),
                ),
            )
            .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Group))
    }
}
