//! Reusable mobile fixture host. Native shells own Activity/scene lifecycle,
//! persistence, system back and keyboard delivery; this entity owns fixture data.
pub mod state;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, ScrollHandle, Styled, Subscription, Window, div, prelude::*, px,
};
use gpui_kit::interaction::{
    refresh::{PullToRefresh, PullToRefreshEvent, RefreshState},
    swipe::{SwipeActions, SwipeActionsEvent, SwipeSide},
};
use gpui_kit::layout::{AppBar, PageLayout};
use gpui_kit::navigation::{BottomNavigation, NavStack, NavigationItem};
use gpui_kit::overlay::{
    popover::PickerPresentation,
    sheet::{BottomSheet, BottomSheetEvent, SheetAction, SheetActionState, SheetDetent},
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticCoordinator};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Surface, TypeScale};
use state::{Checkpoint, FixtureState, Tab};
use std::rc::Rc;

/// Mount after `gpui_kit::install(cx)` on a platform-owned window. The library
/// has no dependency on a desktop platform and can be mounted by mobile shells.
pub fn mount(window: &mut Window, cx: &mut App) -> Entity<ReferenceApp> {
    mount_state(FixtureState::default(), window, cx)
}

/// Restore only validated host data; fresh controls mean overlays stay closed.
pub fn mount_checkpoint(
    saved: Checkpoint,
    window: &mut Window,
    cx: &mut App,
) -> Result<Entity<ReferenceApp>, &'static str> {
    Ok(mount_state(FixtureState::restore(saved)?, window, cx))
}

fn mount_state(state: FixtureState, window: &mut Window, cx: &mut App) -> Entity<ReferenceApp> {
    cx.new(|cx| ReferenceApp::new(state, window, cx))
}

pub struct ReferenceApp {
    pub state: FixtureState,
    name: Entity<TextInput>,
    draft: Entity<TextArea>,
    picker: Entity<Select>,
    sheet: Entity<BottomSheet>,
    refresh: Entity<PullToRefresh>,
    rows: Vec<Entity<SwipeActions>>,
    list_focus: FocusHandle,
    detail_focus: FocusHandle,
    image_fit: FitMode,
    checkpoint: Option<Checkpoint>,
    _subscriptions: Vec<Subscription>,
}

impl ReferenceApp {
    fn new(state: FixtureState, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| {
            TextInput::new("reference.form.name", window, cx)
                .name("姓名 / Name")
                .text(state.name.clone())
                .control_size(ControlSize::Touch)
                .input_options(gpui::TextInputOptions {
                    action: gpui::TextInputAction::Next,
                    ..Default::default()
                })
        });
        let draft = cx.new(|cx| {
            TextArea::new("reference.form.draft", window, cx)
                .placeholder("多行草稿 / Multiline draft")
                .text(state.draft.clone())
                .rows(3)
                .control_size(ControlSize::Touch)
        });
        let picker = cx.new(|cx| {
            Select::new("reference.form.category", window, cx)
                .name("分类 / Category")
                .control_size(ControlSize::Touch)
                .presentation(PickerPresentation::Bottom)
                .options([
                    SelectOption::new("personal", "个人 / Personal"),
                    SelectOption::new("work", "工作 / Work"),
                ])
                .selected(state.category.clone())
        });
        let sheet = cx.new(|cx| BottomSheet::new("reference.sheet", window, cx));
        let sheet_input = cx.new(|cx| {
            TextInput::new("reference.sheet.query", window, cx)
                .name("Sheet search")
                .placeholder("输入中文 / Type here")
                .control_size(ControlSize::Touch)
        });
        let sheet_scroll = ScrollHandle::new();
        sheet.update(cx, |sheet, cx| {
            sheet.set_title("Keyboard-aware Kit sheet", cx);
            sheet.set_detents(vec![SheetDetent::new("compact", 300.0), SheetDetent::new("expanded", 520.0)], "compact", cx);
            sheet.set_scroll_handle(Some(sheet_scroll.clone()), cx);
            sheet.set_content(Some(Rc::new(move |_, cx| {
                let theme = cx.theme().clone();
                div().id("reference.sheet.body").flex().flex_col().size_full().overflow_y_scroll().track_scroll(&sheet_scroll)
                    .gap(px(theme.space(Space::Md))).child(sheet_input.clone())
                    .child("Real window insets are consumed by the Kit sheet. No keyboard height is invented here.")
                    .into_any_element()
            })), cx);
        });
        let scroll = ScrollHandle::new();
        let refresh = cx.new(|_| {
            PullToRefresh::new(
                "reference.refresh",
                scroll.clone(),
                "Refresh fixture",
                "Release to refresh fixture",
            )
        });
        let mut subscriptions = vec![
            cx.subscribe_in(
                &name,
                window,
                |this, _, action: &gpui::TextInputAction, window, cx| {
                    if *action == gpui::TextInputAction::Next {
                        this.draft.read(cx).focus_handle(cx).focus(window, cx);
                    }
                },
            ),
            cx.subscribe(&name, |this, _, event: &TextInputEvent, cx| {
                if let TextInputEvent::Change(value) = event {
                    this.state.name = value.to_string();
                    this.state.dirty = true;
                    cx.notify();
                }
            }),
            cx.subscribe(&draft, |this, _, event: &TextAreaEvent, cx| {
                if let TextAreaEvent::Change(value) = event {
                    this.state.draft = value.text().to_string();
                    this.state.dirty = true;
                    cx.notify();
                }
            }),
            cx.subscribe(&picker, |this, _, event: &SelectEvent, cx| {
                if let SelectEvent::Selected(id) = event {
                    this.state.category = id.to_string();
                    this.state.dirty = true;
                    this.picker
                        .update(cx, |picker, cx| picker.set_selected(Some(id.clone()), cx));
                    cx.notify();
                }
            }),
            cx.subscribe(&sheet, |this, _, event, cx| {
                if let BottomSheetEvent::DetentRequested(id) = event {
                    this.sheet.update(cx, |sheet, cx| {
                        sheet.set_detent(id.clone(), cx);
                    });
                }
            }),
            cx.subscribe(&refresh, |this, _, _: &PullToRefreshEvent, cx| {
                this.state.fail_refresh();
                this.refresh.update(cx, |refresh, cx| {
                    refresh.set_state(
                        RefreshState::Error(
                            "Fixture request failed; verified rows retained".into(),
                        ),
                        cx,
                    )
                });
                cx.notify();
            }),
        ];
        let mut rows = Vec::new();
        for note in &state.notes {
            let id = note.id.clone();
            let row = cx.new(|_| SwipeActions::new(format!("reference.row.{id}"), "Actions"));
            let open_host = cx.entity().downgrade();
            let title = note.title.clone();
            let open_id = id.clone();
            row.update(cx, |row, cx| {
                row.set_actions(SwipeSide::Left, vec![SheetAction::new("pin", "Pin")], cx);
                let mut remove = SheetAction::new("remove", "Remove");
                remove.destructive = true;
                let mut unavailable = SheetAction::new("share", "Share");
                unavailable.disabled = true;
                row.set_actions(SwipeSide::Right, vec![remove, unavailable], cx);
                row.set_content(
                    Some(Rc::new(move |_, _| {
                        let host = open_host.clone();
                        let id = open_id.clone();
                        Button::new(format!("reference.open.{id}"))
                            .label(title.clone())
                            .secondary()
                            .control_size(ControlSize::Touch)
                            .full_width(true)
                            .on_click(move |_, cx| {
                                host.update(cx, |this, cx| {
                                    this.state.open_note(&id);
                                    cx.notify();
                                })
                                .ok();
                            })
                            .into_any_element()
                    })),
                    cx,
                );
            });
            subscriptions.push(cx.subscribe(&row, move |this, row, event, cx| {
                if let SwipeActionsEvent::ActionRequested(action) = event {
                    this.state.row_action(&id, action.as_ref());
                    row.update(cx, |row, cx| {
                        if action.as_ref() == "remove" {
                            row.set_state(
                                SheetActionState::Error("Host refused removal".into()),
                                cx,
                            );
                        } else {
                            row.reveal(None, cx);
                        }
                    });
                    cx.notify();
                }
            }));
            rows.push(row);
        }
        let content_rows = rows.clone();
        refresh.update(cx, |refresh, cx| {
            refresh.set_content(
                Some(Rc::new(move |_, cx| {
                    let theme = cx.theme().clone();
                    div()
                        .id("reference.list.scroll")
                        .flex()
                        .flex_col()
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&scroll)
                        .gap(px(theme.space(Space::Md)))
                        .children(content_rows.iter().cloned())
                        .child(
                            "Verified fixture rows · reveal actions, then explicitly activate one.",
                        )
                        .into_any_element()
                })),
                cx,
            )
        });
        Self {
            state,
            name,
            draft,
            picker,
            sheet,
            refresh,
            rows,
            list_focus: cx.focus_handle(),
            detail_focus: cx.focus_handle(),
            image_fit: FitMode::Contain,
            checkpoint: None,
            _subscriptions: subscriptions,
        }
    }

    /// Native wrappers call this on a real back request. Open overlays consume
    /// it before routing. False means refused or root, never permission to exit.
    pub fn request_back(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.sheet.read(cx).is_open(cx) {
            self.sheet.update(cx, |sheet, cx| sheet.close(window, cx));
            return true;
        }
        if self.picker.read(cx).is_open() {
            self.picker.update(cx, |picker, cx| picker.close(cx));
            return true;
        }
        let accepted = self.state.request_back();
        cx.notify();
        accepted
    }

    /// Native wrappers call before backgrounding and persist the returned data
    /// through their own storage. Nothing is silently written by this example.
    pub fn prepare_background(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Checkpoint {
        self.sheet.update(cx, |sheet, cx| sheet.close(window, cx));
        self.picker.update(cx, |picker, cx| picker.close(cx));
        for row in &self.rows {
            row.update(cx, |row, cx| {
                row.reveal(None, cx);
            });
        }
        self.state.checkpoint()
    }

    fn action(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &Context<Self>,
        handler: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Button {
        let host = cx.entity().downgrade();
        Button::new(id)
            .label(label)
            .secondary()
            .control_size(ControlSize::Touch)
            .on_click(move |window, cx| {
                host.update(cx, |this, cx| {
                    handler(this, window, cx);
                    cx.notify();
                })
                .ok();
            })
    }
}

impl Render for ReferenceApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let theme = cx.theme().clone();
        let content: AnyElement = match self.state.tab {
            Tab::Library => {
                let detail = self.state.current_note();
                let body = if let Some(note) = detail {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(theme.space(Space::Md)))
                        .child(note.title.clone())
                        .child(if note.pinned {
                            "Pinned fixture"
                        } else {
                            "Unpinned fixture"
                        })
                        .child("Caller-owned detail visit. Back retains the forward history.")
                        .semantic_in(
                            cx,
                            NodeSpec::new("reference.detail", Role::Region)
                                .text(note.title.clone()),
                        )
                        .into_any_element()
                } else {
                    div()
                        .h_full()
                        .child(self.refresh.clone())
                        .into_any_element()
                };
                let history = self.state.history.clone();
                let focus = if detail.is_some() {
                    self.detail_focus.clone()
                } else {
                    self.list_focus.clone()
                };
                // NavStack keeps intrinsic document-height semantics. Resolve
                // this bounded workspace's height from its actual body slot.
                gpui_kit::layout::Responsive::new(
                    "reference.library.viewport",
                    move |size, _, _| {
                        NavStack::new(
                            "reference.history",
                            &history,
                            "Fixture library",
                            focus,
                            div()
                                .w_full()
                                .h(px(size.height().unwrap_or(0.0)))
                                .child(body),
                        )
                        .into_any_element()
                    },
                )
                .fill()
                .into_any_element()
            }
            Tab::Form => div()
                .id("reference.form.scroll")
                .flex()
                .flex_col()
                .size_full()
                .overflow_y_scroll()
                .gap(px(theme.space(Space::Sm)))
                .child("姓名 / Name")
                .child(self.name.clone())
                .child("多行草稿 / Multiline draft")
                .child(self.draft.clone())
                .child("分类 / Bottom picker")
                .child(self.picker.clone())
                .child(self.action(
                    "reference.form.accept",
                    "Accept fixture draft",
                    cx,
                    |this, _, _| {
                        this.state.dirty = false;
                        this.state.notice =
                            "Fixture draft accepted in memory · not a server save".into();
                    },
                ))
                .into_any_element(),
            Tab::Image => ImageViewer::new(
                "reference.image",
                [ImageFrame::new("fixture", "Fixture illustration")
                    .source("fixture:illustration")
                    .natural(600, 400)],
            )
            .height(260.0)
            .control_size(ControlSize::Touch)
            .fit(self.image_fit)
            .on_event({
                let host = cx.entity().downgrade();
                move |event, _, cx| {
                    if let ImageViewerEvent::FitChanged(fit) = event {
                        host.update(cx, |this, cx| {
                            this.image_fit = *fit;
                            cx.notify();
                        })
                        .ok();
                    }
                }
            })
            .image(|_, _, cx| {
                let theme = cx.theme().clone();
                Some(
                    div()
                        .size_full()
                        .surface(&theme, Surface::Panel)
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .child("GPUI Box")
                        .child("Fixture illustration · 600 × 400")
                        .into_any_element(),
                )
            })
            .into_any_element(),
        };
        let selected = match self.state.tab {
            Tab::Library => "library",
            Tab::Form => "form",
            Tab::Image => "image",
        };
        let host = cx.entity().downgrade();
        let bottom = BottomNavigation::new("reference.nav")
            .items([
                NavigationItem::new("library", "Library"),
                NavigationItem::new("form", "Form"),
                NavigationItem::new("image", "Image"),
            ])
            .selected(selected)
            .on_select(move |id, _, cx| {
                host.update(cx, |this, cx| {
                    this.state.select_tab(match id.as_ref() {
                        "form" => Tab::Form,
                        "image" => Tab::Image,
                        _ => Tab::Library,
                    });
                    cx.notify();
                })
                .ok();
            });
        div()
            .size_full()
            .surface(&theme, Surface::Panel)
            .text_color(theme.colors.text)
            .font_family(theme.typography.sans.clone())
            .child(
                PageLayout::new(
                    "reference.page",
                    div()
                        .size_full()
                        .p(px(theme.space(Space::Sm)))
                        .child(content),
                )
                .header(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            AppBar::new("reference.app-bar", "Mobile fixture")
                                .leading(self.action(
                                    "reference.back",
                                    "Back",
                                    cx,
                                    |this, window, cx| {
                                        this.request_back(window, cx);
                                    },
                                ))
                                .trailing(self.action(
                                    "reference.sheet.open",
                                    "Sheet",
                                    cx,
                                    |this, window, cx| {
                                        this.sheet.update(cx, |sheet, cx| sheet.open(window, cx));
                                    },
                                )),
                        )
                        .child(
                            div()
                                .p(px(theme.space(Space::Sm)))
                                .type_scale(&theme, TypeScale::Caption)
                                .child(self.state.notice.clone())
                                .semantic_in(
                                    cx,
                                    NodeSpec::new("reference.notice", Role::Text)
                                        .text(self.state.notice.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .px(px(theme.space(Space::Sm)))
                                .gap(px(theme.space(Space::Xs)))
                                .child(self.action(
                                    "reference.checkpoint",
                                    "Checkpoint",
                                    cx,
                                    |this, window, cx| {
                                        this.checkpoint = Some(this.prepare_background(window, cx));
                                        this.state.notice =
                                            "Fixture checkpoint held in memory".into();
                                    },
                                ))
                                .child(
                                    self.action(
                                        "reference.restore",
                                        "Restore",
                                        cx,
                                        |this, window, cx| {
                                            if let Some(saved) = this.checkpoint.clone() {
                                                match FixtureState::restore(saved) {
                                                    Ok(state) => {
                                                        let checkpoint = this.checkpoint.clone();
                                                        *this = Self::new(state, window, cx);
                                                        this.checkpoint = checkpoint;
                                                    }
                                                    Err(error) => this.state.notice = error.into(),
                                                }
                                            }
                                        },
                                    )
                                    .disabled(self.checkpoint.is_none()),
                                ),
                        ),
                )
                .footer(bottom),
            )
            .child(self.sheet.clone())
    }
}
