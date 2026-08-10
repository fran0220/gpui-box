#![cfg(target_family = "wasm")]

use gpui::{
    App, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};
use gpui_kit::prelude::{EmptyKind, EmptyState, set_layout_direction};
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticRegistry};
use gpui_kit_theme::{Theme, activate_theme};
use wasm_bindgen::prelude::*;

const VIEWPORT_WIDTH: f32 = 920.0;
const VIEWPORT_HEIGHT: f32 = 1_000.0;
const DEFAULT_SCENE: &str = "button";
const DEFAULT_THEME: &str = "studio-light";

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

fn run() {
    let requested_scene = config("scene", DEFAULT_SCENE);
    let scene = gpui_kit::scenes::find(&requested_scene);
    let requested_theme = config("theme", DEFAULT_THEME);
    let app = gpui_platform::single_threaded_web().with_assets(gpui_kit::assets::Assets);

    app.run(move |cx: &mut App| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(true);
        let theme_available = activate_theme(&requested_theme, cx);
        if !theme_available {
            activate_theme(DEFAULT_THEME, cx);
        }
        let unavailable = match (scene.as_ref(), theme_available) {
            (None, false) => Some(format!(
                "Unknown scene `{requested_scene}` and theme `{requested_theme}`."
            )),
            (None, true) => Some(format!("Unknown scene `{requested_scene}`.")),
            (Some(_), false) => Some(format!("Unknown theme `{requested_theme}`.")),
            (Some(_), true) => None,
        };
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
}

#[wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run();
}
