use gpui::{App, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    gpui_platform::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx: &mut App| {
            gpui_kit::install(cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(390.0), px(844.0)),
                            cx,
                        ))),
                        ..Default::default()
                    },
                    gpui_box_mobile_reference::mount,
                )
                .expect("open reference window");
            window
                .update(cx, |_, window, _| window.activate_window())
                .expect("activate reference window");
        });
}
