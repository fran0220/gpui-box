//! Native GPUI rendering smoke app, distinct from the direct-Metal host fixture.
#[cfg(any(target_os = "ios", feature = "platform-check"))]
#[cfg_attr(
    not(target_os = "ios"),
    expect(
        dead_code,
        reason = "type-check native entry without linking UIKit on Linux"
    )
)]
fn run_native() {
    use gpui::{prelude::*, *};
    use std::{borrow::Cow, rc::Rc};

    struct Smoke;
    impl Render for Smoke {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("ios-gpui-smoke")
                .size_full()
                .bg(rgb(0x142033))
                .text_color(rgb(0xffffff))
                .font_family("Geist")
                .flex()
                .flex_col()
                .justify_center()
                .items_center()
                .child("GPUI Box — UIKit / Metal")
                .child("Native rendering smoke; acceptance pending")
        }
    }
    let platform = Rc::new(
        gpui_ios::IosPlatform::new(
            "Geist",
            vec![Cow::Borrowed(include_bytes!(
                "../../gpui-kit-assets/assets/fonts/Geist.ttf"
            ))],
        )
        .expect("initialize UIKit platform"),
    );
    platform
        .verify_next_frame(|completion| match completion {
            Ok(()) => println!("IOS_GPUI_FRAME_COMPLETED"),
            Err(error) => {
                eprintln!("IOS_GPUI_FRAME_FAILED {error:#}");
                std::process::exit(1);
            }
        })
        .expect("arm native frame verification");
    Application::with_platform(platform).run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Smoke))
            .expect("open GPUI Metal window");
        // This only records successful window construction, not GPU completion.
        println!("IOS_GPUI_WINDOW_OPENED");
    });
}

#[cfg(target_os = "ios")]
fn main() {
    run_native();
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("This application requires native iOS. No desktop substitute is provided.");
    std::process::exit(2);
}
