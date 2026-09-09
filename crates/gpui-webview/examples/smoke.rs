//! Real native engine smoke; run on X11/XWayland, macOS or Windows.
//! The HTTP server serves only authored fixtures on a random loopback port.
use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, platform_view, prelude::*, px,
    size,
};
use gpui_webview::{BrowserEvent, BrowserHost, BrowserOptions};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct Smoke {
    host: BrowserHost,
}
impl Render for Smoke {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(platform_view(self.host.handle()).size_full())
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
    let base = format!("http://{}", listener.local_addr().expect("fixture address"));
    let server_running = Arc::new(AtomicBool::new(true));
    let running = server_running.clone();
    listener
        .set_nonblocking(true)
        .expect("nonblocking fixture server");
    let server = std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .expect("bounded fixture read");
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let html = "<!doctype html><h1>Local navigation fixture</h1><script>window.onpageshow=()=>window.ipc.postMessage(location.pathname)</script>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
                html.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    let passed = Arc::new(AtomicBool::new(false));
    let result = passed.clone();
    gpui_platform::application().run(move |cx: &mut App| {
        cx.open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(900.), px(650.)), cx))),
            ..Default::default()
        }, |window, cx| cx.new(|cx: &mut Context<Smoke>| {
            let host = BrowserHost::new(window, cx, BrowserOptions { receive_messages: true, ..Default::default() }).expect("create native host");
            assert!(host.navigate("file:///etc/passwd").is_err());
            assert!(host.navigate("data:text/html,<script>alert(1)</script>").is_err());
            host.load_html(include_str!("browser.html")).expect("load HTML fixture");
            // A pending internal HTML load must not relax the public URL policy.
            assert!(host.navigate("data:text/html;charset=utf-8;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==").is_err());
            cx.spawn(async move |weak, cx| {
                let start = Instant::now();
                let mut phase = 0;
                let mut page_finished = false;
                let mut script_ready = false;
                loop {
                    cx.background_executor().timer(Duration::from_millis(15)).await;
                    let finished = weak.update(cx, |this, cx| {
                        for event in this.host.drain_events() {
                            eprintln!("phase {phase}: {event:?}");
                            match (phase, event) {
                                (0, BrowserEvent::Message { body, .. }) if body.contains("fixture") => {
                                    let metrics: serde_json::Value = serde_json::from_str(&body).expect("fixture metrics JSON");
                                    assert_eq!(metrics["grid"], "grid");
                                    assert!(metrics["width"].as_u64().expect("viewport width") >= 600);
                                    assert!(metrics["height"].as_u64().expect("viewport height") >= 300);
                                    this.host.evaluate_script("window.ipc.postMessage(JSON.stringify({smoke:'script-evaluated',userAgent:navigator.userAgent}))").expect("evaluate authored script");
                                    phase = 1;
                                }
                                (1, BrowserEvent::Message { body, .. }) if body.contains("script-evaluated") => script_ready = true,
                                (2 | 4, BrowserEvent::Message { body, .. }) if body == "/one" => script_ready = true,
                                (3 | 5 | 6, BrowserEvent::Message { body, .. }) if body == "/two" => script_ready = true,
                                (_, BrowserEvent::PageFinished { .. }) => page_finished = true,
                                (7, BrowserEvent::LoadFailed { .. }) => {
                                    eprintln!("native browser smoke passed: CSS/viewport, IPC, script, navigation, back, forward, reload, native failure, policy refusal");
                                    result.store(true, Ordering::Relaxed);
                                    cx.quit();
                                    return true;
                                }
                                (_, BrowserEvent::LoadFailed { .. } | BrowserEvent::ProcessFailed { .. } | BrowserEvent::EventsDropped { .. }) => {
                                    panic!("unexpected native failure or lost events at phase {phase}");
                                }
                                _ => {}
                            }
                            // Either callback can arrive first. Do not cancel an in-flight
                            // load and then mistake its cancellation for the offline test.
                            if page_finished && script_ready {
                                page_finished = false;
                                script_ready = false;
                                match phase {
                                    1 => this.host.navigate(&format!("{base}/one")).expect("navigate one"),
                                    2 => this.host.navigate(&format!("{base}/two")).expect("navigate two"),
                                    3 => {
                                        assert!(this.host.can_go_back().expect("back availability"));
                                        this.host.back().expect("go back");
                                    }
                                    4 => {
                                        assert!(this.host.can_go_forward().expect("forward availability"));
                                        this.host.forward().expect("go forward");
                                    }
                                    5 => this.host.reload().expect("reload"),
                                    6 => {
                                        let closed = TcpListener::bind("127.0.0.1:0").expect("reserve unavailable endpoint");
                                        let address = closed.local_addr().expect("reserved address");
                                        drop(closed);
                                        this.host.navigate(&format!("http://{address}/")).expect("accept offline navigation command");
                                    }
                                    _ => unreachable!("script readiness outside navigation phases"),
                                }
                                phase += 1;
                            }
                        }
                        if start.elapsed() > Duration::from_secs(30) {
                            eprintln!("native browser smoke timed out at phase {phase}");
                            cx.quit(); return true;
                        }
                        false
                    }).expect("smoke window alive");
                    if finished { break; }
                }
            }).detach();
            Smoke { host }
        })).expect("open smoke window");
        cx.activate(true);
    });
    server_running.store(false, Ordering::Relaxed);
    server.join().expect("fixture server exits cleanly");
    assert!(
        passed.load(Ordering::Relaxed),
        "native browser smoke failed"
    );
}
