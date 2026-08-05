use gpui::{
    Anchor, AnyElement, ElementId, IntoElement, Pixels, Point, SharedString, div, prelude::*, px,
};
use gpui_kit_theme::Theme;

use crate::{effects, motion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Enter,
    ModifiedEnter,
    Escape,
    Backspace,
    Other,
}

pub fn classify_key(key: &str, command: bool, control: bool) -> MenuKey {
    match key {
        "up" => MenuKey::Up,
        "down" => MenuKey::Down,
        "enter" if command || control => MenuKey::ModifiedEnter,
        "enter" => MenuKey::Enter,
        "escape" => MenuKey::Escape,
        "backspace" => MenuKey::Backspace,
        _ => MenuKey::Other,
    }
}

pub fn step(active: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count = count as isize;
    Some(match active {
        None if delta >= 0 => 0,
        None => count - 1,
        Some(index) => (index as isize + delta).rem_euclid(count),
    } as usize)
}

pub fn match_rank(query: &str, label: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(1);
    }
    let label = label.to_lowercase();
    if label.starts_with(&query) {
        Some(0)
    } else if label.contains(&query) {
        Some(1)
    } else {
        None
    }
}

pub fn filter_indices<S: AsRef<str>>(query: &str, labels: &[S]) -> Vec<usize> {
    let mut ranked: Vec<_> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| match_rank(query, label.as_ref()).map(|rank| (rank, index)))
        .collect();
    ranked.sort_by_key(|&(rank, index)| (rank, index));
    ranked.into_iter().map(|(_, index)| index).collect()
}

pub fn card(theme: &Theme) -> gpui::Div {
    div()
        .border_1()
        .border_color(theme.colors.hairline_strong)
        .rounded(px(theme.radii.card))
        .shadow_lg()
        .p(px(theme.spacing.xs))
        .overflow_hidden()
        .bg(theme.colors.overlay.opacity(theme.effects.glass_alpha))
        .text_size(px(theme.typography.body.size))
        .text_color(theme.colors.text)
}

pub fn card_flush(theme: &Theme) -> gpui::Div {
    card(theme).p_0()
}

fn pinned(layer: AnyElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_0()
        .child(layer)
        .into_any_element()
}

pub fn anchored_below(id: impl Into<ElementId>, theme: &Theme, content: AnyElement) -> AnyElement {
    let content = effects::frosted(theme, theme.radii.card, content).into_any_element();
    pinned(
        gpui::deferred(
            gpui::anchored()
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(theme.spacing.sm))
                .child(motion::menu_in(
                    id,
                    theme,
                    div()
                        .occlude()
                        .pt(px(theme.spacing.sm - 2.0))
                        .child(content),
                )),
        )
        .priority(1)
        .into_any_element(),
    )
}

pub fn anchored_above(id: impl Into<ElementId>, theme: &Theme, content: AnyElement) -> AnyElement {
    let content = effects::frosted(theme, theme.radii.card, content).into_any_element();
    pinned(
        gpui::deferred(
            gpui::anchored()
                .anchor(Anchor::BottomLeft)
                .snap_to_window_with_margin(px(theme.spacing.sm))
                .child(motion::menu_in(
                    id,
                    theme,
                    div()
                        .occlude()
                        .pb(px(theme.spacing.sm - 2.0))
                        .child(content),
                )),
        )
        .priority(1)
        .into_any_element(),
    )
}

pub fn at(
    id: impl Into<ElementId>,
    theme: &Theme,
    position: Point<Pixels>,
    content: AnyElement,
) -> AnyElement {
    let content = effects::frosted(theme, theme.radii.card, content).into_any_element();
    gpui::deferred(
        gpui::anchored()
            .position(position)
            .anchor(Anchor::TopLeft)
            .snap_to_window_with_margin(px(theme.spacing.sm))
            .child(motion::menu_in(id, theme, div().occlude().child(content))),
    )
    .priority(1)
    .into_any_element()
}

pub fn modal(
    id: impl Into<ElementId>,
    theme: &Theme,
    viewport: gpui::Size<Pixels>,
    content: AnyElement,
) -> AnyElement {
    let content = effects::frosted(theme, theme.radii.dialog, content).into_any_element();
    gpui::deferred(
        gpui::anchored()
            .position(gpui::point(px(0.0), px(0.0)))
            .child(
                div()
                    .occlude()
                    .w(viewport.width)
                    .h(viewport.height)
                    .bg(gpui::black().opacity(0.6))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(motion::dialog_in(id, theme, div().child(content))),
            ),
    )
    .priority(2)
    .into_any_element()
}

pub fn menu_row(theme: &Theme, selected: bool, highlighted: bool) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .px(px(theme.spacing.sm))
        .py(px(6.0))
        .rounded(px(theme.radii.control))
        .text_size(px(theme.typography.body.size))
        .text_color(if selected || highlighted {
            theme.colors.text
        } else {
            theme.colors.text_muted
        })
        .when(selected, |element| {
            element
                .bg(theme.colors.selected)
                .shadow(theme.selected_ring())
        })
        .when(!selected && highlighted, |element| {
            element.bg(theme.colors.hover)
        })
        .when(!selected && !highlighted, |element| {
            element.hover(|style| style.bg(theme.colors.hover).text_color(theme.colors.text))
        })
}

pub fn heading(theme: &Theme, label: &str) -> gpui::Div {
    div()
        .px(px(theme.spacing.sm))
        .pb(px(theme.spacing.xs))
        .pt(px(6.0))
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.colors.text_muted.opacity(0.6))
        .child(SharedString::from(tracked_upper(label)))
}

pub fn separator(theme: &Theme) -> gpui::Div {
    div()
        .h(px(1.0))
        .mx(px(-theme.spacing.xs))
        .my(px(theme.spacing.xs))
        .bg(theme.colors.hairline)
}

pub fn key_cap(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .h(px(22.0))
        .px(px(5.0))
        .rounded(px(theme.radii.small))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.colors.hover.opacity(0.38))
        .font_family(theme.typography.mono.clone())
        .text_size(px(theme.typography.caption.size))
        .text_color(theme.colors.text_muted)
        .child(label.into())
}

pub fn dialog_card(theme: &Theme) -> gpui::Div {
    div()
        .w(px(360.0))
        .p(px(theme.spacing.xl - theme.spacing.xs))
        .rounded(px(theme.radii.dialog))
        .bg(theme.colors.overlay)
        .border_1()
        .border_color(theme.colors.hairline_strong)
        .shadow_lg()
        .flex()
        .flex_col()
        .text_color(theme.colors.text)
}

pub fn dialog_title(theme: &Theme, title: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(15.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.colors.text)
        .child(title.into())
}

pub fn dialog_body(theme: &Theme, body: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(px(theme.spacing.sm))
        .text_size(px(theme.typography.body.size))
        .line_height(px(theme.typography.body.line_height))
        .text_color(theme.colors.text_muted)
        .child(body.into())
}

pub fn tracked_upper(label: &str) -> String {
    let mut output = String::with_capacity(label.len() * 2);
    for (index, character) in label.to_uppercase().chars().enumerate() {
        if index > 0 {
            output.push('\u{200A}');
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_wraps_and_handles_empty_lists() {
        assert_eq!(step(None, 0, 1), None);
        assert_eq!(step(None, 3, 1), Some(0));
        assert_eq!(step(None, 3, -1), Some(2));
        assert_eq!(step(Some(2), 3, 1), Some(0));
        assert_eq!(step(Some(0), 3, -1), Some(2));
    }

    #[test]
    fn filtering_prefers_prefixes_and_is_stable() {
        let labels = ["main", "feature/main-sync", "master", "dev"];
        assert_eq!(filter_indices("ma", &labels), vec![0, 2, 1]);
        assert_eq!(filter_indices("", &labels), vec![0, 1, 2, 3]);
    }

    #[test]
    fn key_classification_keeps_modified_enter_distinct() {
        assert_eq!(classify_key("enter", false, false), MenuKey::Enter);
        assert_eq!(classify_key("enter", true, false), MenuKey::ModifiedEnter);
        assert_eq!(classify_key("escape", false, false), MenuKey::Escape);
    }
}
