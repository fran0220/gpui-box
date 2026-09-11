//! Fixture-only native Android verification scene. No provider or product data.
#[cfg(any(target_os = "android", feature = "host-check"))]
mod reference;

#[cfg(any(target_os = "android", feature = "host-check"))]
mod demo {
    use gpui::{prelude::*, *};
    use gpui_kit::prelude::{Button, TextInput};
    use jni::{JNIEnv, objects::JObject};

    struct Demo {
        count: usize,
        text: Entity<TextInput>,
    }

    impl Render for Demo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let insets = window.insets().effective();
            window.set_back_enabled(self.count > 0);
            let entity = cx.entity().downgrade();
            let mut app = cx.to_async();
            window.set_back_handler(move || {
                let _ = entity.update(&mut app, |this, cx| {
                    this.count = 0;
                    cx.notify();
                });
            });
            div()
                .id("android-demo")
                .size_full()
                .bg(rgb(0xf3f4f6))
                .text_color(rgb(0x111827))
                .font_family("IBM Plex Sans")
                .flex().flex_col().gap_4()
                .pt(insets.top + px(24.)).pb(insets.bottom + px(24.))
                .pl(insets.left + px(24.)).pr(insets.right + px(24.))
                .child("GPUI Box · Android native fixture")
                .child("Touch, IME, insets and restoration verification")
                .child(div().id("android-count").child(format!("Count: {}", self.count)))
                .child(Button::new("android-increment").label("Increment")
                    .on_click({ let entity = cx.entity().downgrade(); move |_, cx| { let _ = entity.update(cx, |this, cx| { this.count += 1; cx.notify(); }); } }))
                .child(self.text.clone())
                .child("Rotate or background the Activity; count and text must survive. Process death starts a fresh fixture.")
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_gpui_box_example_MainActivity_nativeCreate(
        mut env: JNIEnv,
        activity: JObject,
    ) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            gpui_android::initialize(&mut env, activity, |platform| {
                Application::with_platform(platform).run_embedded(|cx| {
                    gpui_kit::install(cx);
                    cx.open_window(WindowOptions::default(), |window, cx| {
                        let text = cx.new(|cx| {
                            TextInput::new("android-text", window, cx)
                                .name("Native editable fixture")
                                .text("A😀中Z")
                        });
                        cx.new(|_| Demo { count: 0, text })
                    })
                    .expect("create Android GPUI window");
                })
            })
        }));
        let failure = match result {
            Ok(Ok(())) => return,
            Ok(Err(error)) => error.to_string(),
            Err(_) => "Android GPUI initialization panicked".into(),
        };
        if !env.exception_check().unwrap_or(true) {
            let _ = env.throw_new("java/lang/IllegalStateException", failure);
        }
    }
}
