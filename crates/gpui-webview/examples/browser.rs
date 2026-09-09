use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, platform_view, prelude::*, px,
    rgb, size,
};
use gpui_webview::{BrowserEvent, BrowserHost, BrowserOptions};

struct Browser {
    host: BrowserHost,
    visible: bool,
    status: String,
    failed: bool,
}

impl Render for Browser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut controls = div().flex().gap_3().p_3();
        for label in [
            "Back",
            "Forward",
            "Reload",
            "Hide/show",
            "Focus page",
            "Offline error",
            "Fixture",
        ] {
            controls = controls.child(
                div()
                    .id(label)
                    .px_2()
                    .py_1()
                    .bg(rgb(0x38536c))
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let result = match label {
                            "Back" => this.host.back(),
                            "Forward" => this.host.forward(),
                            "Reload" => this.host.reload(),
                            "Focus page" => this.host.focus(),
                            "Offline error" => (|| {
                                let closed = std::net::TcpListener::bind("127.0.0.1:0")?;
                                let address = closed.local_addr()?;
                                drop(closed);
                                this.host.navigate(&format!("http://{address}/"))
                            })(),
                            "Fixture" => this.host.load_html(include_str!("browser.html")),
                            _ => {
                                this.visible = !this.visible;
                                Ok(())
                            }
                        };
                        if let Err(error) = result {
                            this.status = error.to_string();
                        }
                        cx.notify();
                    })),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x14202b))
            .text_color(rgb(0xe6eef5))
            .child(controls)
            .child(
                div()
                    .id("browser-status")
                    .px_3()
                    .h(px(50.))
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .id("browser-viewport")
                    .mx_3()
                    .mb_3()
                    .flex_1()
                    .overflow_hidden()
                    .when(self.visible, |element| {
                        element.child(platform_view(self.host.handle()).size_full())
                    }),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1000.), px(720.)),
                    cx,
                ))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("GPUI native browser".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx: &mut Context<Browser>| {
                    let host = BrowserHost::new(
                        window,
                        cx,
                        BrowserOptions {
                            receive_messages: true,
                            ..Default::default()
                        },
                    )
                    .expect("native WebView initialization failed");
                    host.load_html(include_str!("browser.html"))
                        .expect("load authored fixture");
                    cx.spawn(async |weak, cx| {
                        loop {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(30))
                                .await;
                            if weak
                                .update(cx, |this, cx| {
                                    for event in this.host.drain_events() {
                                        // This example prints only its explicitly authored fixture.
                                        eprintln!("{event:?}");
                                        match &event {
                                            BrowserEvent::NavigationStarted(_) => {
                                                this.failed = false
                                            }
                                            BrowserEvent::LoadFailed { .. }
                                            | BrowserEvent::NavigationRefused(_) => {
                                                this.failed = true
                                            }
                                            BrowserEvent::PageFinished(_) if this.failed => {
                                                continue;
                                            }
                                            _ => {}
                                        }
                                        this.status = match event {
                                            BrowserEvent::PageFinished(_) => {
                                                "Load finished (not a success assertion)".into()
                                            }
                                            event => format!("{event:?}"),
                                        };
                                        cx.notify();
                                    }
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    })
                    .detach();
                    Browser {
                        host,
                        visible: true,
                        status: "Loading explicit HTML fixture".into(),
                        failed: false,
                    }
                })
            },
        )
        .expect("open native browser window");
        cx.activate(true);
    });
}
