#![cfg(target_family = "wasm")]

use std::cell::RefCell;

use gpui::{
    App, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};
use gpui_kit::prelude::{EmptyKind, EmptyState, set_layout_direction};
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticRegistry};
use gpui_kit_theme::{Theme, ThemeRegistry, activate_theme};
use wasm_bindgen::prelude::*;

const VIEWPORT_WIDTH: f32 = 920.0;
const VIEWPORT_HEIGHT: f32 = 1_000.0;
const DEFAULT_SCENE: &str = "button";
const DEFAULT_THEME: &str = "studio-light";
const DEFAULT_BACKEND: &str = "auto";

thread_local! {
    static APPLICATION: RefCell<Option<gpui::ApplicationHandle>> = const { RefCell::new(None) };
}

struct BrowserGallery {
    scene: Option<&'static str>,
    unavailable: Option<String>,
}

impl Render for BrowserGallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = SemanticRegistry::global(cx);
        registry.begin_frame();
        let publish = registry.clone();
        let theme = Theme::get(cx).clone();
        let content = if let Some(reason) = self.unavailable.clone() {
            EmptyState::new("browser.gallery.unavailable", "Browser gallery unavailable")
                .kind(EmptyKind::Unavailable)
                .detail(reason)
                .into_any_element()
        } else {
            let scene = gpui_kit::scenes::find(self.scene.expect("available host has a scene"))
                .expect("browser scene is registered");
            (scene.build)(window, cx)
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
                    .text("gpui-kit browser gallery"),
            )
            .child(content)
    }
}

fn config(key: &str, default: &'static str) -> String {
    let global = js_sys::global();
    js_sys::Reflect::get(&global, &JsValue::from_str("gpuiKitConfig"))
        .ok()
        .and_then(|config| js_sys::Reflect::get(&config, &JsValue::from_str(key)).ok())
        .and_then(|value| value.as_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn publish_snapshot(registry: &SemanticRegistry) {
    let snapshot = registry.snapshot();
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

fn run() {
    let requested_scene = config("scene", DEFAULT_SCENE);
    let scene = gpui_kit::scenes::find(&requested_scene);
    let requested_theme = config("theme", DEFAULT_THEME);
    let requested_backend = config("backend", DEFAULT_BACKEND);
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
        if let Some(error) = backend_error {
            unavailable.push(error);
        }
        let unavailable = (!unavailable.is_empty()).then(|| unavailable.join(" "));
        let scene_name = scene.as_ref().map(|scene| scene.name);
        set_layout_direction(
            gpui_kit::scenes::direction(scene_name.unwrap_or(DEFAULT_SCENE)),
            cx,
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
            |_, cx| {
                cx.new(|_| BrowserGallery {
                    scene: scene_name,
                    unavailable,
                })
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
