//! Dynamic recording demonstration; no real window or baseline acceptance.
use anyhow::Result;
use gpui::{
    AnyWindowHandle, App, AvailableSpace, Bounds, ContentMask, Corners, HeadlessAppContext,
    PaintRecording, RenderImage, Window, canvas, div, point, prelude::*, px, rgb, size,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticCoordinator};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

struct Host {
    live: bool,
    opacity: f32,
    scale: f32,
    capture_clip: bool,
    recording: Rc<RefCell<Option<PaintRecording>>>,
    image: Arc<RenderImage>,
    paints: Rc<Cell<usize>>,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        if !self.live && cx.reduce_motion() {
            self.recording.borrow_mut().take();
        }
        let slot = self.recording.clone();
        let image = self.image.clone();
        let paints = self.paints.clone();
        let live = self.live;
        let scale = self.scale;
        let capture_clip = self.capture_clip;
        div().size_full().bg(rgb(0x101820)).child(
            div().size_full().opacity(self.opacity).child(
                canvas(
                    move |bounds, window, cx| {
                        if !live {
                            return None;
                        }
                        let child = div()
                            .w(px(236.))
                            .h(px(112.))
                            .p(px(15.))
                            .rounded(px(13.))
                            .overflow_hidden()
                            .bg(rgb(0x23685d))
                            .text_color(rgb(0xffffff))
                            .text_size(px(21.))
                            .child("Frozen Ag7 • 图像")
                            .child(
                                div()
                                    .mt(px(7.))
                                    .w(px(90.))
                                    .h(px(29.))
                                    .bg(rgb(0xe0af42))
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new("retirement.action", Role::Button),
                                    )
                                    .on_click(|_, _, _| panic!("retired action must not run")),
                            )
                            .child(
                                canvas(
                                    |bounds, _, _| bounds,
                                    move |_, bounds, window, _| {
                                        paints.set(paints.get() + 1);
                                        let rect = Bounds::new(
                                            bounds.origin + point(px(137.), px(-33.)),
                                            size(px(58.), px(38.)),
                                        );
                                        window
                                            .paint_image(
                                                rect,
                                                rect,
                                                Corners::all(px(5.)),
                                                image,
                                                0,
                                                false,
                                            )
                                            .unwrap();
                                    },
                                )
                                .size(px(1.)),
                            );
                        let mut child = child.into_any_element();
                        capture_scope(window, scale, capture_clip, |window| {
                            child.prepaint_as_root(
                                bounds.origin + point(px(27.), px(21.)),
                                bounds.size.map(AvailableSpace::Definite),
                                window,
                                cx,
                            )
                        });
                        Some(child)
                    },
                    move |_, child, window, cx| {
                        if let Some(mut child) = child {
                            capture_scope(window, scale, capture_clip, |window| {
                                window.paint_layer(
                                    Bounds::new(point(px(0.), px(0.)), size(px(320.), px(200.))),
                                    |window| {
                                        let mark = window.paint_mark();
                                        child.paint(window, cx);
                                        *slot.borrow_mut() =
                                            Some(window.record_paint_since(mark).unwrap());
                                    },
                                );
                            });
                        } else if let Some(recording) = slot.borrow().as_ref() {
                            window
                                .with_visual_scale(scale, point(px(89.), px(61.)), |window| {
                                    window.paint_recording(recording)
                                })
                                .unwrap();
                        }
                    },
                )
                .size_full(),
            ),
        )
    }
}

fn capture_scope<R>(
    window: &mut Window,
    scale: f32,
    clipped: bool,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    window.with_visual_scale(scale, point(px(89.), px(61.)), |window| {
        window.with_content_mask(
            clipped.then(|| ContentMask {
                bounds: Bounds::new(point(px(0.), px(0.)), size(px(220.), px(180.))),
            }),
            f,
        )
    })
}

fn frame(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> Result<image::RgbaImage> {
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
    cx.capture_screenshot(window)
}

#[test]
fn frozen_paint_dynamic_text_image_semantics_and_reduced_motion() -> Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            "Geist",
        )),
        Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(false);
    });
    let coordinator = cx.update(|cx| SemanticCoordinator::global(cx));
    let _diagnostics = coordinator.arm();
    let paints = Rc::new(Cell::new(0));
    let slot = Rc::new(RefCell::new(None));
    let image = Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_fn(12, 8, |x, y| {
            if x < 4 || y > 5 {
                image::Rgba([40, 90, 240, 255])
            } else {
                image::Rgba([240, 180, 30, 255])
            }
        }),
    )]));
    let window = cx.open_window(size(px(320.), px(180.)), |_, cx: &mut App| {
        cx.new(|_| Host {
            live: true,
            opacity: 0.7,
            scale: 1.12,
            capture_clip: false,
            recording: slot.clone(),
            image: image.clone(),
            paints: paints.clone(),
        })
    })?;
    let live = frame(&mut cx, window.into())?;
    assert!(
        coordinator
            .snapshot(window.window_id())
            .unwrap()
            .find("retirement.action")
            .is_some()
    );
    let before = paints.get();
    window.update(&mut cx, |host, window, cx| {
        host.live = false;
        window.drop_image(host.image.clone()).unwrap();
        cx.notify();
    })?;
    let frozen = frame(&mut cx, window.into())?;
    assert_eq!(
        live.as_raw(),
        frozen.as_raw(),
        "last-live glyphs, clipping and evicted image must remain pixel-identical"
    );
    assert_eq!(
        paints.get(),
        before,
        "frozen replay cannot run child callbacks"
    );
    assert!(
        coordinator
            .snapshot(window.window_id())
            .unwrap()
            .find("retirement.action")
            .is_none()
    );
    window.update(&mut cx, |host, _, cx| {
        host.opacity = 0.45;
        host.scale = 0.83;
        cx.notify();
    })?;
    let fading = frame(&mut cx, window.into())?;
    assert_ne!(fading.as_raw(), frozen.as_raw());
    let x = fading.width() * 60 / 320;
    let y = fading.height() * 110 / 180;
    assert!(
        fading.get_pixel(x, y)[1] < frozen.get_pixel(x, y)[1],
        "the same green interior must fade, not merely move"
    );
    assert_eq!(paints.get(), before);
    window.update(&mut cx, |host, _, cx| {
        host.live = true;
        host.opacity = 0.7;
        host.scale = 1.12;
        host.recording.borrow_mut().take();
        cx.notify();
    })?;
    let reinserted = frame(&mut cx, window.into())?;
    assert_eq!(live.as_raw(), reinserted.as_raw());
    assert!(paints.get() > before);
    window.update(&mut cx, |host, _, cx| {
        host.live = false;
        cx.set_reduce_motion(true);
        cx.notify();
    })?;
    let dropped = frame(&mut cx, window.into())?;
    assert!(slot.borrow().is_none());
    assert!(
        coordinator
            .snapshot(window.window_id())
            .unwrap()
            .find("retirement.action")
            .is_none()
    );
    assert_ne!(dropped.as_raw(), live.as_raw());
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/frozen-paint-review");
    std::fs::create_dir_all(&output)?;
    for (name, image) in [
        ("live", live),
        ("retired", frozen),
        ("fading", fading),
        ("reinserted", reinserted),
        ("reduced-motion", dropped),
    ] {
        image.save(output.join(format!("{name}.png")))?;
    }
    cx.update(|cx| cx.set_reduce_motion(false));
    let clipped_window = cx.open_window(size(px(320.), px(180.)), |_, cx: &mut App| {
        cx.new(|_| Host {
            live: true,
            opacity: 0.7,
            scale: 1.12,
            capture_clip: true,
            recording: Rc::new(RefCell::new(None)),
            image: image.clone(),
            paints: Rc::new(Cell::new(0)),
        })
    })?;
    let clipped_live = frame(&mut cx, clipped_window.into())?;
    clipped_window.update(&mut cx, |host, _, cx| {
        host.live = false;
        cx.notify();
    })?;
    let clipped_retired = frame(&mut cx, clipped_window.into())?;
    assert_eq!(
        clipped_live.as_raw(),
        clipped_retired.as_raw(),
        "removing the live ancestor clip must not reveal clipped/culled content"
    );
    clipped_live.save(output.join("clipped-live.png"))?;
    clipped_retired.save(output.join("clipped-retired.png"))?;
    Ok(())
}
