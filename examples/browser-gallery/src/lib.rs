#![cfg(target_family = "wasm")]

use std::cell::RefCell;

use gpui::{
    AnyElement, App, Bounds, Context, Entity, IntoElement, Render, SharedString, Subscription,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_kit::prelude::{
    Badge, Button, EmptyKind, EmptyState, HitCount, List, ListItem, ScrollArea, SearchField,
    SearchFieldEvent, Selectable, set_layout_direction, text,
};
use gpui_kit_semantics::{
    DiagnosticArm, NodeSpec, Role, Semantic, SemanticCoordinator, WindowSemanticContext,
};
use gpui_kit_theme::{Theme, ThemeRegistry, TypeScale, activate_theme};
use wasm_bindgen::prelude::*;

const VIEWPORT_WIDTH: f32 = 920.0;
const VIEWPORT_HEIGHT: f32 = 1_000.0;
const DEFAULT_SCENE: &str = "button";
const DEFAULT_THEME: &str = "studio-light";
const DEFAULT_BACKEND: &str = "auto";
const DEFAULT_MODE: &str = "scene";
const NARROW_WIDTH: f32 = 760.0;

thread_local! {
    static APPLICATION: RefCell<Option<gpui::ApplicationHandle>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserMode {
    Scene,
    Playground,
}

impl BrowserMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "scene" => Some(Self::Scene),
            "playground" => Some(Self::Playground),
            _ => None,
        }
    }
}

struct BrowserGallery {
    mode: BrowserMode,
    scene: Option<&'static str>,
    unavailable: Option<String>,
    query: SharedString,
    search: Option<Entity<SearchField>>,
    _subscriptions: Vec<Subscription>,
    _diagnostics: DiagnosticArm,
}

impl BrowserGallery {
    fn new(
        mode: BrowserMode,
        scene: Option<&'static str>,
        unavailable: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut gallery = Self {
            mode,
            scene,
            unavailable,
            query: SharedString::default(),
            search: None,
            _subscriptions: Vec::new(),
            _diagnostics: SemanticCoordinator::global(cx).arm(),
        };

        if mode == BrowserMode::Playground {
            let search = cx.new(|cx| {
                SearchField::new("browser.playground.search", window, cx)
                    .placeholder("Filter scenes")
            });
            let subscription = cx.subscribe(
                &search,
                |gallery, search, event: &SearchFieldEvent, cx| match event {
                    SearchFieldEvent::QueryChanged(query) => {
                        gallery.query = query.clone();
                        let count = search_count(query);
                        cx.defer(move |cx| {
                            search.update(cx, |search, cx| search.set_count(count, cx));
                        });
                        cx.notify();
                    }
                    SearchFieldEvent::Next => gallery.step_scene(1, cx),
                    SearchFieldEvent::Previous => gallery.step_scene(-1, cx),
                    SearchFieldEvent::Cancelled => {
                        gallery.query = SharedString::default();
                        cx.defer(move |cx| {
                            search.update(cx, |search, cx| {
                                search.set_query("", cx);
                                search.set_count(HitCount::Unsearched, cx);
                            });
                        });
                        cx.notify();
                    }
                    _ => {}
                },
            );
            gallery.search = Some(search);
            gallery._subscriptions.push(subscription);
        }
        gallery
    }

    fn select_scene(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(scene) = gpui_kit::scenes::find(name) else {
            return;
        };
        self.scene = Some(scene.name);
        self.unavailable = None;
        set_layout_direction(gpui_kit::scenes::direction(scene.name), cx);
        publish_selection(scene.name, Theme::get(cx).id.as_ref());
        cx.notify();
    }

    fn select_theme(&mut self, id: &str, cx: &mut Context<Self>) {
        if activate_theme(id, cx) {
            publish_selection(self.scene.unwrap_or(DEFAULT_SCENE), id);
            cx.notify();
        }
    }

    fn step_scene(&mut self, delta: isize, cx: &mut Context<Self>) {
        let scenes = matching_scenes(&self.query);
        if scenes.is_empty() {
            return;
        }
        let current = self
            .scene
            .and_then(|selected| scenes.iter().position(|scene| *scene == selected));
        let next = match (current, delta.is_negative()) {
            (Some(0), true) | (None, true) => scenes.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % scenes.len(),
            (None, false) => 0,
        };
        self.select_scene(scenes[next], cx);
    }

    fn scene_content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(reason) = self.unavailable.clone() {
            return EmptyState::new("browser.gallery.unavailable", "Browser gallery unavailable")
                .kind(EmptyKind::Unavailable)
                .detail(reason)
                .into_any_element();
        }
        let scene = gpui_kit::scenes::find(self.scene.expect("available host has a scene"))
            .expect("browser scene is registered");
        (scene.build)(window, cx)
    }

    fn playground(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::get(cx).clone();
        let narrow = f32::from(window.viewport_size().width) < NARROW_WIDTH;
        let selected = self.scene.unwrap_or(DEFAULT_SCENE);
        let scenes = matching_scenes(&self.query);
        let rows = scenes.clone();
        let gallery = cx.entity().downgrade();
        let scene_list = List::new(
            "browser.playground.scenes",
            rows.len(),
            move |index, _, _| {
                let name = SharedString::from(rows[index]);
                ListItem::new(name.clone(), name.clone()).text(name)
            },
        )
        .selected(selected)
        .visible_rows(if narrow { 4 } else { 23 })
        .on_select(move |id, _, cx| {
            gallery
                .update(cx, |gallery, cx| gallery.select_scene(id.as_ref(), cx))
                .ok();
        });

        let navigation = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(theme.spacing.sm))
            .p(px(theme.spacing.sm))
            .when(narrow, |element| element.w_full())
            .when(!narrow, |element| element.w(px(256.0)).h_full())
            .children(self.search.clone())
            .child(scene_list);

        let preview_name = self.scene.unwrap_or("unavailable");
        let preview = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .gap(px(theme.spacing.sm))
            .p(px(theme.spacing.sm))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme.spacing.sm))
                    .child(
                        text(&theme, TypeScale::Label, "Live scene").semantic_in(
                            cx,
                            NodeSpec::new("browser.playground.preview.title", Role::Heading)
                                .level(2)
                                .text("Live scene"),
                        ),
                    )
                    .child(Badge::new(preview_name).neutral()),
            )
            .child(
                div().flex_1().min_h_0().min_w_0().child(
                    ScrollArea::new("browser.playground.preview")
                        .label("Live scene preview")
                        .both()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w_full()
                                .min_h(px(VIEWPORT_HEIGHT))
                                .child(self.scene_content(window, cx)),
                        ),
                ),
            );

        let theme_buttons = ThemeRegistry::global(cx)
            .ids()
            .into_iter()
            .map(|id| {
                let active = id == theme.id;
                let target = id.clone();
                let gallery = cx.entity().downgrade();
                Button::new(format!("browser.playground.theme.{id}"))
                    .label(id)
                    .secondary()
                    .selected(active)
                    .on_click(move |_, cx| {
                        gallery
                            .update(cx, |gallery, cx| {
                                gallery.select_theme(target.as_ref(), cx);
                            })
                            .ok();
                    })
            })
            .collect::<Vec<_>>();

        let header = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(theme.spacing.sm))
            .px(px(theme.spacing.lg))
            .py(px(theme.spacing.sm))
            .bg(theme.colors.panel)
            .child(
                text(&theme, TypeScale::Title, "GPUI Box compose")
                    .mr(px(theme.spacing.sm))
                    .semantic_in(
                        cx,
                        NodeSpec::new("browser.playground.title", Role::Heading)
                            .level(1)
                            .text("GPUI Box compose"),
                    ),
            )
            .children(theme_buttons)
            .child(
                Button::new("browser.playground.docs")
                    .label("Documentation")
                    .ghost()
                    .on_click(|_, cx| cx.open_url("/docs/")),
            );

        div()
            .id("browser.playground.root")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.colors.canvas)
            .font_family(theme.typography.sans.clone())
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .when(narrow, |element| element.flex_col())
                    .when(!narrow, |element| element.flex_row())
                    .child(navigation)
                    .child(preview),
            )
            .semantic_in(
                cx,
                NodeSpec::new("browser.playground.root", Role::Window).text("GPUI Box compose"),
            )
            .into_any_element()
    }
}

impl Render for BrowserGallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let publish = SemanticCoordinator::global(cx).begin_frame(window);
        let theme = Theme::get(cx).clone();
        let content = if self.mode == BrowserMode::Playground {
            self.playground(window, cx)
        } else {
            self.scene_content(window, cx)
        };

        div()
            .on_children_prepainted(move |_, _, _| publish_snapshot(&publish))
            .id("browser.gallery.root")
            .size_full()
            .overflow_hidden()
            .bg(theme.colors.canvas)
            .semantic_in(
                cx,
                NodeSpec::new("browser.gallery.root", Role::Window)
                    .text("GPUI Box browser gallery"),
            )
            .child(content)
    }
}

fn matching_scenes(query: &str) -> Vec<&'static str> {
    let words = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    gpui_kit::scenes::catalog()
        .into_iter()
        .filter(|scene| words.iter().all(|word| scene.name.contains(word)))
        .map(|scene| scene.name)
        .collect()
}

fn search_count(query: &str) -> HitCount {
    if query.trim().is_empty() {
        return HitCount::Unsearched;
    }
    match matching_scenes(query).len() {
        0 => HitCount::None,
        total => HitCount::Known {
            total,
            current: None,
        },
    }
}

fn config(key: &str, default: &'static str) -> String {
    let global = js_sys::global();
    js_sys::Reflect::get(&global, &JsValue::from_str("gpuiBoxConfig"))
        .ok()
        .and_then(|config| js_sys::Reflect::get(&config, &JsValue::from_str(key)).ok())
        .and_then(|value| value.as_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn publish_snapshot(context: &WindowSemanticContext) {
    let snapshot = context.snapshot();
    if !snapshot.contains("browser.gallery.root") {
        return;
    }
    let Ok(json) = serde_json::to_string(&snapshot) else {
        return;
    };
    let global = js_sys::global();
    if js_sys::Reflect::set(
        &global,
        &JsValue::from_str("gpuiKitSemanticSnapshot"),
        &JsValue::from_str(&json),
    )
    .is_err()
    {
        return;
    }
    let _ = js_sys::Reflect::set(
        &global,
        &JsValue::from_str("gpuiKitGalleryReady"),
        &JsValue::TRUE,
    );
}

fn publish_catalog(cx: &App) {
    let scenes = gpui_kit::scenes::catalog()
        .into_iter()
        .map(|scene| scene.name)
        .collect::<Vec<_>>();
    let themes = ThemeRegistry::global(cx)
        .ids()
        .into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let Ok(json) = serde_json::to_string(&serde_json::json!({
        "scenes": scenes,
        "themes": themes,
    })) else {
        return;
    };
    let _ = js_sys::Reflect::set(
        &js_sys::global(),
        &JsValue::from_str("gpuiKitCatalog"),
        &JsValue::from_str(&json),
    );
}

fn publish_selection(scene: &str, theme: &str) {
    let global = js_sys::global();
    let Ok(json) = serde_json::to_string(&serde_json::json!({
        "scene": scene,
        "theme": theme,
    })) else {
        return;
    };
    let _ = js_sys::Reflect::set(
        &global,
        &JsValue::from_str("gpuiBoxSelection"),
        &JsValue::from_str(&json),
    );
    let Ok(callback) = js_sys::Reflect::get(&global, &JsValue::from_str("gpuiBoxSelectionChanged"))
    else {
        return;
    };
    let Some(callback) = callback.dyn_ref::<js_sys::Function>() else {
        return;
    };
    let _ = callback.call2(
        &JsValue::NULL,
        &JsValue::from_str(scene),
        &JsValue::from_str(theme),
    );
}

fn run() {
    let requested_scene = config("scene", DEFAULT_SCENE);
    let scene = gpui_kit::scenes::find(&requested_scene);
    let requested_theme = config("theme", DEFAULT_THEME);
    let requested_backend = config("backend", DEFAULT_BACKEND);
    let requested_mode = config("mode", DEFAULT_MODE);
    let mode = BrowserMode::parse(&requested_mode).unwrap_or(BrowserMode::Scene);
    let (backend, backend_error) = match requested_backend.as_str() {
        "auto" => (gpui_platform::WebBackendPreference::Auto, None),
        "webgpu" => (gpui_platform::WebBackendPreference::WebGpu, None),
        "webgl" => (gpui_platform::WebBackendPreference::WebGl, None),
        _ => (
            gpui_platform::WebBackendPreference::Auto,
            Some(format!("Unknown renderer backend `{requested_backend}`.")),
        ),
    };
    let app =
        gpui_platform::application_with_web_backend(backend).with_assets(gpui_kit::assets::Assets);

    let handle = app.run_embedded(move |cx: &mut App| {
        gpui_kit::install(cx);
        publish_catalog(cx);
        cx.set_reduce_motion(true);
        let theme_available = activate_theme(&requested_theme, cx);
        if !theme_available {
            activate_theme(DEFAULT_THEME, cx);
        }
        let mut unavailable = Vec::new();
        if scene.is_none() {
            unavailable.push(format!("Unknown scene `{requested_scene}`."));
        }
        if !theme_available {
            unavailable.push(format!("Unknown theme `{requested_theme}`."));
        }
        if BrowserMode::parse(&requested_mode).is_none() {
            unavailable.push(format!("Unknown browser mode `{requested_mode}`."));
        }
        if let Some(error) = backend_error {
            unavailable.push(error);
        }
        let unavailable = (!unavailable.is_empty()).then(|| unavailable.join(" "));
        let scene_name = scene
            .as_ref()
            .map(|scene| scene.name)
            .or((mode == BrowserMode::Playground).then_some(DEFAULT_SCENE));
        set_layout_direction(
            gpui_kit::scenes::direction(scene_name.unwrap_or(DEFAULT_SCENE)),
            cx,
        );
        publish_selection(
            scene_name.unwrap_or(DEFAULT_SCENE),
            if theme_available {
                &requested_theme
            } else {
                DEFAULT_THEME
            },
        );

        let bounds = Bounds::new(
            gpui::point(px(0.0), px(0.0)),
            size(px(VIEWPORT_WIDTH), px(VIEWPORT_HEIGHT)),
        );
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                is_resizable: false,
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| BrowserGallery::new(mode, scene_name, unavailable, window, cx))
            },
        )
        .expect("open browser gallery window");
        cx.activate(true);
    });
    APPLICATION.with(|application| application.replace(Some(handle)));
}

#[wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run();
}
