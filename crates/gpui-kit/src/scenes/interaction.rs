//! Mobile list interactions, with caller-owned async state and retained rows.

use super::support::*;
use crate::interaction::{
    refresh::{PullToRefresh, RefreshState},
    swipe::{SwipeActions, SwipeSide},
};
use crate::overlay::sheet::{SheetAction, SheetActionState};

struct SceneRefresh(Vec<Entity<PullToRefresh>>);
impl Global for SceneRefresh {}

pub(super) fn pull_to_refresh(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneRefresh>() {
        let states = [
            ("ready", RefreshState::Ready),
            (
                "pending",
                RefreshState::Pending("Waiting for caller".into()),
            ),
            (
                "loading",
                RefreshState::Loading("Refreshing fixture".into()),
            ),
            (
                "error",
                RefreshState::Error("Refresh failed · retained rows".into()),
            ),
        ];
        let mut views = Vec::new();
        for (id, state) in states {
            let scroll = gpui::ScrollHandle::new();
            let view = cx.new(|_| {
                PullToRefresh::new(
                    format!("scene.refresh.{id}"),
                    scroll.clone(),
                    "Refresh",
                    "Release to refresh",
                )
            });
            view.update(cx, |refresh, cx| {
                refresh.set_state(state, cx);
                refresh.set_content(
                    Some(Rc::new(move |_, cx| {
                        let theme = cx.theme().clone();
                        div()
                            .id(SharedString::from(format!("scene.refresh.{id}.scroll")))
                            .column()
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .children(
                                [
                                    ("notes", "Notes · verified fixture"),
                                    ("draft", "Draft · verified fixture"),
                                    ("archive", "Archive · verified fixture"),
                                    ("saved", "Saved · verified fixture"),
                                ]
                                .into_iter()
                                .map(|(key, label)| {
                                    div()
                                        .h(px(64.0))
                                        .flex_none()
                                        .p_token(&theme, Space::Sm)
                                        .child(crate::foundation::text(
                                            &theme,
                                            TypeScale::Body,
                                            label,
                                        ))
                                        .semantic_in(
                                            cx,
                                            NodeSpec::new(
                                                format!("scene.refresh.{id}.{key}"),
                                                Role::Group,
                                            )
                                            .text(label),
                                        )
                                }),
                            )
                            .into_any_element()
                    })),
                    cx,
                );
            });
            views.push(view);
        }
        cx.set_global(SceneRefresh(views));
    }
    let theme = cx.theme().clone();
    stack(&theme)
        .w_full()
        .child(caption(
            &theme,
            "Fixture states · refreshing never replaces the verified rows",
        ))
        .child(
            row(&theme).items_start().children(
                cx.global::<SceneRefresh>()
                    .0
                    .iter()
                    .map(|view| div().w(px(250.0)).h(px(370.0)).child(view.clone())),
            ),
        )
        .into_any_element()
}

struct SceneSwipe(Vec<Entity<SwipeActions>>);
impl Global for SceneSwipe {}

pub(super) fn swipe_actions(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneSwipe>() {
        let mut views = Vec::new();
        for (id, side, state) in [
            ("closed", None, SheetActionState::Ready),
            ("left", Some(SwipeSide::Left), SheetActionState::Ready),
            (
                "right",
                Some(SwipeSide::Right),
                SheetActionState::Error("Deletion failed; row retained".into()),
            ),
            (
                "pending",
                Some(SwipeSide::Right),
                SheetActionState::Pending("Waiting for caller".into()),
            ),
        ] {
            let view = cx.new(|_| SwipeActions::new(format!("scene.swipe.{id}"), "Actions"));
            view.update(cx, |swipe, cx| {
                let mut remove = SheetAction::new("remove", "Delete");
                remove.destructive = true;
                swipe.set_actions(SwipeSide::Left, vec![SheetAction::new("pin", "Pin")], cx);
                swipe.set_actions(SwipeSide::Right, vec![remove], cx);
                swipe.set_content(
                    Some(Rc::new(move |_, cx| {
                        let theme = cx.theme().clone();
                        div()
                            .h(px(64.0))
                            .column()
                            .justify_center()
                            .child(crate::foundation::text(
                                &theme,
                                TypeScale::Label,
                                "Project notes · fixture",
                            ))
                            .child(crate::foundation::text(
                                &theme,
                                TypeScale::Body,
                                "Last verified content remains visible",
                            ))
                            .semantic_in(
                                cx,
                                NodeSpec::new(format!("scene.swipe.{id}.content"), Role::Group)
                                    .text("Project notes · fixture"),
                            )
                            .into_any_element()
                    })),
                    cx,
                );
                swipe.reveal(side, cx);
                swipe.set_state(state, cx);
            });
            views.push(view);
        }
        cx.set_global(SceneSwipe(views));
    }
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(580.0))
        .child(caption(
            &theme,
            "Reveal is not activation · physical left and right trays",
        ))
        .children(cx.global::<SceneSwipe>().0.iter().cloned())
        .into_any_element()
}
