//! Real native renderer evidence, including stale custom loader and atlas eviction.
use super::*;
use gpui::{
    AppContext as _, Context, HeadlessAppContext, IntoElement, ParentElement, Render, Styled,
    StyledImage, Window, div, effect_owner, img, px, rgb,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticCoordinator};

struct ImageReview {
    owner: EffectOwner,
    reference: ResourceRef,
    source: ImageSource,
}
impl Render for ImageReview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let label = cx.with_effect_owner(Some(self.owner), |cx| {
            if Resources::image(&self.reference, cx).is_ok() {
                "Ready immutable image"
            } else {
                "Resource refused"
            }
        });
        div()
            .child(effect_owner(
                self.owner,
                img(self.source.clone())
                    .w(px(200.))
                    .h(px(100.))
                    .with_fallback(|| div().size_full().bg(rgb(0xff00ff)).into_any_element()),
            ))
            .semantic_in(cx, NodeSpec::new("resource-image", Role::Image).text(label))
    }
}

#[test]
fn native_pixels_stale_mount_factory_and_released_image() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::test_text_system("Geist"),
        Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    let owner = EffectOwner::new();
    let other = EffectOwner::new();
    let principal = EffectOwner::new();
    let peer = EffectOwner::new();
    let (mut store, reference, source, image) = cx.update(|cx| {
        gpui_kit::install(cx);
        let mut store = Resources::install(cx);
        store.reconcile(&HashMap::from([(owner, principal), (peer, principal)]), cx);
        let reference = store
            .register(
                principal,
                Registration {
                    key: "pixels".into(),
                    mime: RGBA.into(),
                    data: STANDARD.encode(
                        [
                            32u32.to_le_bytes().as_slice(),
                            16u32.to_le_bytes().as_slice(),
                            &(0..512)
                                .flat_map(|pixel| {
                                    if pixel % 32 < 16 {
                                        [255, 0, 0, 255]
                                    } else {
                                        [0, 0, 255, 255]
                                    }
                                })
                                .collect::<Vec<_>>(),
                        ]
                        .concat(),
                    ),
                },
            )
            .expect("authorized image registration");
        let source = cx.with_effect_owner(Some(owner), |cx| {
            Resources::image(&reference, cx).expect("authorized image source")
        });
        let image = {
            let state = store.0.borrow();
            let Asset::Image(image) = &state.owners[&principal]["pixels"].asset else {
                panic!("image")
            };
            image.clone()
        };
        (store, reference, source, image)
    });
    let _armed = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    let loader = source.clone();
    let handle = cx
        .open_window(size(px(200.), px(100.)), |_, cx| {
            cx.new(|_| ImageReview {
                owner,
                reference,
                source,
            })
        })
        .expect("native headless window");
    let window = handle.into();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        let ImageSource::Custom(loader) = &loader else {
            panic!("must be custom, never a resource path")
        };
        assert!(cx.with_effect_owner(Some(other), |cx| {
            loader(window, cx).expect("settled refusal").is_err()
        }));
        assert!(loader(window, cx).expect("settled refusal").is_err());
        window.draw(cx).clear(cx);
    })
    .expect("native draw");
    let ready = cx.capture_screenshot(window).expect("ready capture");
    assert_eq!(
        ready.get_pixel(ready.width() / 4, ready.height() / 2).0,
        [255, 0, 0, 255]
    );
    assert_eq!(
        ready.get_pixel(ready.width() * 3 / 4, ready.height() / 2).0,
        [0, 0, 255, 255]
    );
    cx.update(|cx| {
        let snapshot = SemanticCoordinator::global(cx)
            .snapshot(window.window_id())
            .expect("ready semantics");
        assert!(
            serde_json::to_string(&snapshot)
                .expect("serializable semantics")
                .contains("Ready immutable image")
        );
    });
    // Revoke during a window update: the active window is absent from App.windows.
    cx.update_window(window, |_, window, cx| {
        store.reconcile(&HashMap::from([(peer, principal)]), cx);
        let ImageSource::Custom(loader) = &loader else {
            unreachable!()
        };
        assert!(cx.with_effect_owner(Some(owner), |cx| {
            loader(window, cx).expect("settled revocation").is_err()
        }));
        cx.with_effect_owner(Some(peer), |cx| {
            let source = Resources::image(
                &ResourceRef {
                    key: "pixels".into(),
                },
                cx,
            )
            .expect("same-generation peer image");
            let ImageSource::Custom(load) = source else {
                panic!("custom source")
            };
            assert_eq!(
                load(window, cx)
                    .expect("settled peer")
                    .expect("authorized peer")
                    .id,
                image.id
            );
        });
    })
    .expect("revocation during window update");
    cx.run_until_parked();
    cx.update(|cx| {
        handle
            .update(cx, |_, _, cx| cx.notify())
            .expect("request redraw")
    });
    cx.update_window(window, |_, window, cx| {
        window.draw(cx).clear(cx);
    })
    .expect("refused draw");
    let refused = cx.capture_screenshot(window).expect("refused capture");
    assert_eq!(refused.get_pixel(20, 50).0, [255, 0, 255, 255]);
    cx.update_window(window, |_, _, cx| store.revoke(principal, cx))
        .expect("revoke principal during update");
    cx.run_until_parked();
    assert_eq!(
        Arc::strong_count(&image),
        1,
        "revoked image must not remain in native loader state"
    );
    cx.update(|cx| {
        let snapshot = SemanticCoordinator::global(cx)
            .snapshot(window.window_id())
            .expect("refused semantics");
        assert!(
            serde_json::to_string(&snapshot)
                .expect("serializable refused semantics")
                .contains("Resource refused")
        );
        store.clear(cx);
    });
    if let Ok(directory) = std::env::var("GPUI_RESOURCE_ARTIFACTS") {
        std::fs::create_dir_all(&directory).expect("artifact directory");
        ready
            .save(format!("{directory}/resources-ready.png"))
            .expect("save ready image");
        refused
            .save(format!("{directory}/resources-refused.png"))
            .expect("save refused image");
    }
}
