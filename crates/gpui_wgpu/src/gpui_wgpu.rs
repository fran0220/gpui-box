mod cosmic_text_system;
mod wgpu_atlas;
mod wgpu_context;
mod wgpu_renderer;

pub use cosmic_text_system::*;
pub use wgpu;
pub use wgpu_atlas::*;
pub use wgpu_context::*;
#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
pub use wgpu_renderer::WgpuHeadlessRenderer;
pub use wgpu_renderer::{GpuContext, WgpuRenderer, WgpuSurfaceConfig};

// These pixel tests exercise the deterministic software-adapter contract:
// llvmpipe on Linux and WARP on Windows. macOS headless rendering uses the
// separate Metal renderer and has no wgpu fallback adapter to request.
#[cfg(all(test, any(target_os = "linux", target_os = "windows")))]
mod tests {
    use super::*;
    use gpui::{
        App, Bounds, ContentMask, Context, Corners, EdgeFade, HeadlessAppContext, IntoElement,
        Render, Window, canvas, div, fill, point, prelude::*, px, rgb, size,
    };
    use image::RgbaImage;
    use std::{path::PathBuf, sync::Arc};

    struct HeadlessTestView;

    impl Render for HeadlessTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(320.0))
                        .h(px(176.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(0x172033))
                        .font_family("DejaVu Sans")
                        .child(
                            div()
                                .w(px(280.0))
                                .h(px(112.0))
                                .p_5()
                                .rounded_lg()
                                .shadow_lg()
                                .bg(rgb(0xf4f7fb))
                                .text_color(rgb(0x182230))
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .mb_4()
                                        .child(div().size(px(20.0)).rounded_sm().bg(rgb(0xef4444)))
                                        .child(div().size(px(20.0)).rounded_sm().bg(rgb(0x22c55e)))
                                        .child(div().size(px(20.0)).rounded_sm().bg(rgb(0x3b82f6))),
                                )
                                .child(
                                    div()
                                        .text_size(px(16.0))
                                        .child("Deterministic wgpu rendering"),
                                ),
                        ),
                )
        }
    }

    struct RuntimePrimitiveTestView;

    impl Render for RuntimePrimitiveTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(rgb(0x172033)).child(
                canvas(
                    |_, _, _| (),
                    |bounds, (), window, _| {
                        window.paint_backdrop_blur(bounds, Corners::all(px(12.0)), px(16.0));
                        window.with_edge_fade(
                            Some(EdgeFade {
                                bounds: Bounds::new(bounds.origin, bounds.size),
                                band: px(32.0),
                                top: true,
                                bottom: false,
                                left: false,
                                right: false,
                            }),
                            |window| window.paint_quad(fill(bounds, rgb(0xef4444))),
                        );
                    },
                )
                .size_full(),
            )
        }
    }

    struct BackdropBlurTestView {
        blur_count: usize,
    }

    impl Render for BackdropBlurTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let blur_count = self.blur_count;
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    for stripe in 0..40 {
                        let color = if stripe % 2 == 0 {
                            rgb(0xf4f4f4)
                        } else {
                            rgb(0x101820)
                        };
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left() + px(stripe as f32 * 4.0), bounds.top()),
                                size(px(4.0), bounds.size.height),
                            ),
                            color,
                        ));
                    }

                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(72.0), bounds.top() + px(32.0)),
                            size(px(16.0), px(32.0)),
                        ),
                        rgb(0x2563eb),
                    ));

                    let first_blur = Bounds::new(
                        point(bounds.left() + px(40.0), bounds.top() + px(20.0)),
                        size(px(80.0), px(56.0)),
                    );
                    let content_mask = ContentMask {
                        bounds: Bounds::new(
                            point(bounds.left() + px(48.0), bounds.top() + px(16.0)),
                            size(px(64.0), px(64.0)),
                        ),
                    };
                    if blur_count > 0 {
                        window.with_content_mask(Some(content_mask), |window| {
                            window.paint_backdrop_blur(
                                first_blur,
                                Corners::all(px(12.0)),
                                px(12.0),
                            );
                        });
                    }

                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(84.0), bounds.top() + px(36.0)),
                            size(px(12.0), px(24.0)),
                        ),
                        rgb(0x16a34a),
                    ));

                    if blur_count > 1 {
                        window.paint_backdrop_blur(
                            Bounds::new(
                                point(bounds.left() + px(64.0), bounds.top() + px(28.0)),
                                size(px(48.0), px(40.0)),
                            ),
                            Corners::all(px(8.0)),
                            px(6.0),
                        );
                    }

                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left() + px(76.0), bounds.top() + px(44.0)),
                            size(px(8.0), px(12.0)),
                        ),
                        rgb(0xdc2626),
                    ));
                },
            )
            .size_full()
        }
    }

    fn capture_backdrop_blur_test_image(blur_count: usize) -> anyhow::Result<RgbaImage> {
        let text_system = Arc::new(CosmicTextSystem::new("DejaVu Sans"));
        let mut cx = HeadlessAppContext::with_platform(text_system, Arc::new(()), || {
            Some(Box::new(
                WgpuHeadlessRenderer::new().expect("failed to create wgpu headless renderer"),
            ))
        });
        let window = cx.open_window(size(px(160.0), px(96.0)), move |_, cx: &mut App| {
            cx.new(|_| BackdropBlurTestView { blur_count })
        })?;
        let window = window.into();
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })?;
        cx.capture_screenshot(window)
    }

    #[test]
    fn test_headless_rendering_is_deterministic() -> anyhow::Result<()> {
        if let Err(error) = env_logger::builder()
            .is_test(true)
            .filter_module("gpui_wgpu", log::LevelFilter::Info)
            .try_init()
        {
            log::debug!("Test logger was already initialized: {error}");
        }

        let text_system = Arc::new(CosmicTextSystem::new("DejaVu Sans"));
        let mut cx = HeadlessAppContext::with_platform(text_system, Arc::new(()), || {
            Some(Box::new(
                WgpuHeadlessRenderer::new().expect("failed to create wgpu headless renderer"),
            ))
        });
        let window = cx.open_window(size(px(324.0), px(180.0)), |_, cx: &mut App| {
            cx.new(|_| HeadlessTestView)
        })?;
        let window = window.into();
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })?;

        let first = cx.capture_screenshot(window)?;
        let second = cx.capture_screenshot(window)?;

        assert_eq!(first.as_raw(), second.as_raw());
        assert_eq!(first.dimensions(), (648, 360));
        assert_eq!(first.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(first.get_pixel(10, 10).0, [0x17, 0x20, 0x33, 255]);
        assert_eq!(first.get_pixel(104, 128).0, [0xef, 0x44, 0x44, 255]);
        assert_eq!(first.get_pixel(160, 128).0, [0x22, 0xc5, 0x5e, 255]);
        assert_eq!(first.get_pixel(216, 128).0, [0x3b, 0x82, 0xf6, 255]);

        let text_ink_pixels = {
            let image = &first;
            (180..250)
                .flat_map(|y| (90..590).map(move |x| image.get_pixel(x, y).0))
                .filter(|pixel| pixel[0] < 100 && pixel[1] < 100 && pixel[2] < 100)
                .count()
        };
        assert!(text_ink_pixels > 100, "rendered text was missing");

        let background = [0x17, 0x20, 0x33, 255];
        let shadow_pixels = {
            let image = &first;
            (294..320)
                .flat_map(|y| (44..604).map(move |x| image.get_pixel(x, y).0))
                .filter(|pixel| *pixel != background)
                .count()
        };
        assert!(shadow_pixels > 100, "rendered shadow was missing");

        if let Some(path) = std::env::var_os("GPUI_HEADLESS_EVIDENCE_PATH") {
            let path = PathBuf::from(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            first.save(path)?;
        }

        Ok(())
    }

    #[test]
    fn test_runtime_primitives_render_headlessly() -> anyhow::Result<()> {
        let text_system = Arc::new(CosmicTextSystem::new("DejaVu Sans"));
        let mut cx = HeadlessAppContext::with_platform(text_system, Arc::new(()), || {
            Some(Box::new(
                WgpuHeadlessRenderer::new().expect("failed to create wgpu headless renderer"),
            ))
        });
        let window = cx.open_window(size(px(160.0), px(96.0)), |_, cx: &mut App| {
            cx.new(|_| RuntimePrimitiveTestView)
        })?;
        let window = window.into();
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })?;

        let image = cx.capture_screenshot(window)?;
        let faded_edge = image.get_pixel(160, 4).0;
        let solid_center = image.get_pixel(160, 100).0;

        assert_eq!(image.dimensions(), (320, 192));
        assert!(
            solid_center[0] > 200 && solid_center[1] < 100 && solid_center[2] < 100,
            "edge-faded quad did not reach its solid color: {solid_center:?}"
        );
        assert!(
            faded_edge[0] + 100 < solid_center[0],
            "edge-faded quad did not fade at the top edge: edge={faded_edge:?}, center={solid_center:?}"
        );

        Ok(())
    }

    #[test]
    fn test_backdrop_blur_pixels_clipping_order_and_determinism() -> anyhow::Result<()> {
        let unblurred = capture_backdrop_blur_test_image(0)?;
        let once_blurred = capture_backdrop_blur_test_image(1)?;
        let twice_blurred = capture_backdrop_blur_test_image(2)?;
        let repeated = capture_backdrop_blur_test_image(2)?;

        assert_eq!(twice_blurred.as_raw(), repeated.as_raw());
        assert_eq!(unblurred.dimensions(), (320, 192));

        let changed_interior_pixels = (48 * 2..112 * 2)
            .flat_map(|x| (20 * 2..76 * 2).map(move |y| (x, y)))
            .filter(|&(x, y)| unblurred.get_pixel(x, y) != once_blurred.get_pixel(x, y))
            .count();
        assert!(
            changed_interior_pixels > 1_000,
            "blur did not change enough high-frequency interior pixels: {changed_interior_pixels}"
        );

        for y in 0..192 {
            for x in 0..320 {
                let inside_content_mask = (96..224).contains(&x) && (32..160).contains(&y);
                let inside_blur_bounds = (80..240).contains(&x) && (40..152).contains(&y);
                if !inside_content_mask || !inside_blur_bounds {
                    assert_eq!(
                        unblurred.get_pixel(x, y),
                        once_blurred.get_pixel(x, y),
                        "blur changed a pixel outside its clipped bounds at ({x}, {y})"
                    );
                }
            }
        }

        for (x, y) in [(96, 40), (97, 40), (222, 40), (223, 40)] {
            assert_eq!(
                unblurred.get_pixel(x, y),
                once_blurred.get_pixel(x, y),
                "rounded clipping changed corner pixel ({x}, {y})"
            );
        }

        assert_ne!(
            unblurred.get_pixel(148, 80),
            once_blurred.get_pixel(148, 80),
            "content painted before the blur remained sharp"
        );
        assert_eq!(once_blurred.get_pixel(158, 100).0, [0xdc, 0x26, 0x26, 0xff]);
        assert_eq!(
            once_blurred.get_pixel(180, 96).0,
            [0x16, 0xa3, 0x4a, 0xff],
            "content painted after the first blur was not preserved"
        );
        assert_ne!(
            once_blurred.get_pixel(180, 96),
            twice_blurred.get_pixel(180, 96),
            "the second blur did not consume content painted after the first blur"
        );
        assert_eq!(
            twice_blurred.get_pixel(158, 100).0,
            [0xdc, 0x26, 0x26, 0xff]
        );

        Ok(())
    }
}
