//! Exercises real NSMenu/TrackPopupMenuEx loops on a logged-in native desktop.
//! Not a headless/TestPlatform test: unsupported native menus fail this smoke.
use gpui::{
    App, AsyncApp, Bounds, Context, Menu, MenuItem, NativeMenuOutcome, NativeMenuSession,
    NativeMenuSessionId, Window, WindowBounds, WindowHandle, WindowOptions, div, point, prelude::*,
    px, size,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

gpui::actions!(native_menu_smoke, [MustNotRun]);

struct Smoke;
impl Render for Smoke {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn mark(phase: &Mutex<&'static str>, name: &'static str) {
    *phase.lock().expect("phase lock") = name;
    eprintln!("native-menu-smoke: {name}");
}

fn open_window(cx: &mut App) -> WindowHandle<Smoke> {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(500.), px(350.)),
                cx,
            ))),
            ..Default::default()
        },
        |_, cx| cx.new(|_| Smoke),
    )
    .expect("open native smoke window")
}

fn present(window: WindowHandle<Smoke>, cx: &mut AsyncApp) -> NativeMenuSession {
    window
        .update(cx, |_, window, cx| {
            window
                .show_context_menu(
                    Menu::new("Native lifecycle smoke")
                        .items([MenuItem::action("Must not dispatch", MustNotRun)]),
                    point(px(40.), px(50.)),
                    cx,
                )
                .expect("native menu must be supported and accepted")
        })
        .expect("live smoke window")
}

fn tracking(window: WindowHandle<Smoke>, cx: &mut AsyncApp) -> Option<NativeMenuSessionId> {
    window
        .update(cx, |_, window, _| window.native_context_menu_tracking())
        .expect("live smoke window")
}

fn cancel(window: WindowHandle<Smoke>, id: NativeMenuSessionId, cx: &mut AsyncApp) -> bool {
    window
        .update(cx, |_, window, _| window.cancel_context_menu(id))
        .expect("live smoke window")
        .expect("native cancellation accepted")
}

async fn wait_tracking(window: WindowHandle<Smoke>, id: NativeMenuSessionId, cx: &mut AsyncApp) {
    let start = Instant::now();
    loop {
        // Always yield: the native popup starts only after the current GPUI
        // borrow has unwound. The observation is set around the OS call, not
        // at show_context_menu, so a queued cancellation cannot pass this.
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        if tracking(window, cx) == Some(id) {
            eprintln!("native-menu-smoke: observed tracking {id:?}");
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "native menu never entered tracking: {id:?}"
        );
    }
}

async fn remains_tracking(window: WindowHandle<Smoke>, id: NativeMenuSessionId, cx: &mut AsyncApp) {
    // Give an erroneous asynchronous stale EndMenu/cancelTracking time to
    // execute; checking only immediately after cancel could miss it.
    for _ in 0..5 {
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        assert_eq!(
            tracking(window, cx),
            Some(id),
            "stale close ended the newer native menu"
        );
    }
}

fn main() {
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        eprintln!("native-menu smoke requires macOS or Windows; no simulated fallback");
        std::process::exit(1);
    }
    let phase = Arc::new(Mutex::new("starting application"));
    let passed = Arc::new(AtomicBool::new(false));
    let watchdog_phase = phase.clone();
    // Independent OS thread: a foreground timer cannot detect a blocked
    // modal loop. Timeout is always failure, never a substitute for closure.
    // Keep watching through application shutdown, even after assertions pass.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(45));
        eprintln!(
            "native-menu-smoke: FAIL watchdog at {}",
            watchdog_phase.lock().expect("phase lock")
        );
        std::process::exit(1);
    });
    let result = passed.clone();
    gpui_platform::application().run(move |cx| {
        cx.on_action::<MustNotRun>(|_, _| {
            eprintln!("native-menu-smoke: FAIL cancelled menu dispatched a command");
            std::process::exit(1);
        });
        let first = open_window(cx);
        let other = open_window(cx);
        cx.activate(true);
        cx.spawn(async move |cx| {
            mark(&phase, "cancel queued replacement before native invocation");
            let queued = present(first, cx);
            let queued_id = queued.id();
            let replacement = present(first, cx);
            assert_eq!(tracking(first, cx), None);
            assert!(!cancel(first, queued_id, cx));
            assert!(cancel(first, replacement.id(), cx));
            assert_eq!(queued.await, NativeMenuOutcome::Cancelled);
            assert_eq!(replacement.await, NativeMenuOutcome::Cancelled);
            assert_eq!(tracking(first, cx), None);

            mark(&phase, "cancel active native menu");
            let session = present(first, cx);
            let id = session.id();
            wait_tracking(first, id, cx).await;
            assert!(cancel(first, id, cx));
            assert_eq!(session.await, NativeMenuOutcome::Cancelled);
            assert_eq!(tracking(first, cx), None);
            assert!(
                !cancel(first, id, cx),
                "completed cancellation must be stale"
            );

            mark(&phase, "replace active menu in the same window");
            let old = present(first, cx);
            let old_id = old.id();
            wait_tracking(first, old_id, cx).await;
            let new = present(first, cx);
            let new_id = new.id();
            assert_ne!(old_id, new_id);
            assert_eq!(old.await, NativeMenuOutcome::Cancelled);
            wait_tracking(first, new_id, cx).await;
            assert!(!cancel(first, old_id, cx));
            remains_tracking(first, new_id, cx).await;
            assert!(cancel(first, new_id, cx));
            assert_eq!(new.await, NativeMenuOutcome::Cancelled);

            mark(&phase, "cross-window replacement and stale owner close");
            let old = present(first, cx);
            let old_id = old.id();
            wait_tracking(first, old_id, cx).await;
            let new = present(other, cx);
            let new_id = new.id();
            assert_eq!(old.await, NativeMenuOutcome::Cancelled);
            wait_tracking(other, new_id, cx).await;
            assert!(!cancel(first, old_id, cx));
            assert!(
                !cancel(first, new_id, cx),
                "wrong window must not cancel a current identity"
            );
            remains_tracking(other, new_id, cx).await;
            first
                .update(cx, |_, window, _| window.remove_window())
                .expect("remove stale menu owner");
            remains_tracking(other, new_id, cx).await;
            assert!(cancel(other, new_id, cx));
            assert_eq!(new.await, NativeMenuOutcome::Cancelled);

            mark(&phase, "destroy window while its native loop is active");
            let first = cx.update(open_window);
            let session = present(first, cx);
            wait_tracking(first, session.id(), cx).await;
            first
                .update(cx, |_, window, _| window.remove_window())
                .expect("remove menu owner");
            assert_eq!(session.await, NativeMenuOutcome::Unavailable);
            assert!(
                first.update(cx, |_, _, _| ()).is_err(),
                "owner window must be gone"
            );

            mark(&phase, "new owner remains usable after teardown");
            let session = present(other, cx);
            let id = session.id();
            wait_tracking(other, id, cx).await;
            assert!(cancel(other, id, cx));
            assert_eq!(session.await, NativeMenuOutcome::Cancelled);
            assert_eq!(tracking(other, cx), None);
            mark(
                &phase,
                "PASS cancel, replacement, stale close, cross-window isolation, teardown",
            );
            result.store(true, Ordering::SeqCst);
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    assert!(
        passed.load(Ordering::SeqCst),
        "native-menu smoke exited before all assertions passed"
    );
}
