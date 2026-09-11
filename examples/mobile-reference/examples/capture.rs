//! Actual 390px offscreen windows, not a narrow child inside a desktop window.
use anyhow::{Context as _, Result};
use gpui::{
    AnyWindowHandle, HeadlessAppContext, InputEvent, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, point, px, size,
};
use gpui_box_mobile_reference::{mount, mount_checkpoint, state::Tab};
use gpui_kit_semantics::{SemanticCoordinator, Snapshot};
use std::{path::Path, sync::Arc};

fn settle(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> Result<()> {
    for _ in 0..5 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
        })?;
    }
    Ok(())
}

fn snapshot(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> Result<Snapshot> {
    settle(cx, window)?;
    Ok(cx.update(|cx| {
        SemanticCoordinator::global(cx)
            .snapshot(window.window_id())
            .expect("rendered semantics")
    }))
}

fn click(cx: &mut HeadlessAppContext, window: AnyWindowHandle, id: &str) -> Result<()> {
    let tree = snapshot(cx, window)?;
    let node = tree.find(id).with_context(|| format!("missing {id}"))?;
    anyhow::ensure!(!node.disabled, "cannot activate disabled {id}");
    let (x, y) = node.bounds.center();
    let viewport = cx.update_window(window, |_, window, _| window.viewport_size())?;
    anyhow::ensure!(
        node.bounds.width > 0.0
            && node.bounds.height > 0.0
            && x >= 0.0
            && y >= 0.0
            && x < f32::from(viewport.width)
            && y < f32::from(viewport.height),
        "{id} is outside the visible viewport: {:?}",
        node.bounds
    );
    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            MouseDownEvent {
                position: point(px(x), px(y)),
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            MouseUpEvent {
                position: point(px(x), px(y)),
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
    })?;
    settle(cx, window)
}

fn save(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    directory: &Path,
    name: &str,
) -> Result<()> {
    settle(cx, window)?;
    let frame = cx.capture_screenshot(window)?;
    let viewport = cx.update_window(window, |_, window, _| window.viewport_size())?;
    anyhow::ensure!(
        viewport.width == px(390.0),
        "expected true 390px logical viewport"
    );
    frame.save(directory.join(format!("{name}.png")))?;
    println!(
        "captured {name}: logical {viewport:?}, image {}x{}",
        frame.width(),
        frame.height()
    );
    Ok(())
}

fn main() -> Result<()> {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/mobile-reference".into());
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    let mut cx = HeadlessAppContext::with_platform(
        Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            "Geist",
        )),
        Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(true);
        gpui_kit_theme::activate_theme("studio-light", cx);
    });
    let _diagnostics = cx.update(|cx| SemanticCoordinator::global(cx).arm());
    let handle = cx.open_window(size(px(390.0), px(844.0)), mount)?;
    let window = handle.into();
    save(&mut cx, window, directory, "library")?;
    click(&mut cx, window, "reference.refresh.refresh")?;
    let tree = snapshot(&mut cx, window)?;
    anyhow::ensure!(
        tree.find("reference.open.travel").is_some(),
        "refresh discarded rows"
    );
    save(&mut cx, window, directory, "refresh-failure")?;
    click(&mut cx, window, "reference.row.travel.reveal.right")?;
    save(&mut cx, window, directory, "explicit-actions")?;
    click(&mut cx, window, "reference.row.travel.action.remove")?;
    anyhow::ensure!(
        snapshot(&mut cx, window)?
            .find("reference.open.travel")
            .is_some(),
        "refusal removed row"
    );
    click(&mut cx, window, "reference.open.notes")?;
    anyhow::ensure!(
        snapshot(&mut cx, window)?
            .find("reference.detail")
            .is_some(),
        "detail did not mount"
    );
    save(&mut cx, window, directory, "detail")?;
    click(&mut cx, window, "reference.back")?;
    click(&mut cx, window, "reference.nav.form")?;
    save(&mut cx, window, directory, "chinese-form")?;
    click(&mut cx, window, "reference.form.category")?;
    save(&mut cx, window, directory, "bottom-picker")?;
    // Exercise the same host policy as OS back, without claiming OS delivery.
    anyhow::ensure!(handle.update(&mut cx, |app, window, cx| app.request_back(window, cx))?);
    let tree = snapshot(&mut cx, window)?;
    anyhow::ensure!(
        tree.find("reference.form.category.work").is_none()
            && tree.find("reference.form.name").is_some(),
        "back must close the picker before leaving the form"
    );
    click(&mut cx, window, "reference.form.category")?;
    let saved = handle.update(&mut cx, |app, window, cx| {
        app.prepare_background(window, cx)
    })?;
    anyhow::ensure!(
        saved.category == "personal",
        "background cleanup changed selection"
    );
    anyhow::ensure!(
        snapshot(&mut cx, window)?
            .find("reference.form.category.work")
            .is_none(),
        "background retained open picker"
    );
    click(&mut cx, window, "reference.form.category")?;
    click(&mut cx, window, "reference.form.category.work")?;
    click(&mut cx, window, "reference.back")?;
    let tree = snapshot(&mut cx, window)?;
    anyhow::ensure!(
        tree.find("reference.form.name").is_some(),
        "refused back lost form"
    );
    save(&mut cx, window, directory, "refused-back")?;
    click(&mut cx, window, "reference.checkpoint")?;
    click(&mut cx, window, "reference.restore")?;
    save(&mut cx, window, directory, "restored-draft")?;
    click(&mut cx, window, "reference.form.accept")?;
    click(&mut cx, window, "reference.nav.image")?;
    click(&mut cx, window, "reference.image.fit.actual")?;
    let tree = snapshot(&mut cx, window)?;
    let actual = tree
        .find("reference.image.fit.actual")
        .context("actual-size control")?;
    anyhow::ensure!(
        actual.checked == Some(true),
        "host did not accept actual image size"
    );
    anyhow::ensure!(
        actual.bounds.width >= 48.0 && actual.bounds.height >= 48.0,
        "image fit control is not touch-sized"
    );
    click(&mut cx, window, "reference.image.fit.contain")?;
    anyhow::ensure!(
        snapshot(&mut cx, window)?
            .find("reference.image.fit.contain")
            .is_some_and(|node| node.checked == Some(true)),
        "host did not restore contained image fit"
    );
    save(&mut cx, window, directory, "image")?;
    click(&mut cx, window, "reference.sheet.open")?;
    save(&mut cx, window, directory, "sheet")?;
    anyhow::ensure!(handle.update(&mut cx, |app, window, cx| app.request_back(window, cx))?);
    let tree = snapshot(&mut cx, window)?;
    anyhow::ensure!(
        tree.find("reference.sheet.query").is_none() && tree.find("reference.image").is_some(),
        "back must close the sheet before leaving the image"
    );
    click(&mut cx, window, "reference.sheet.open")?;
    handle.update(&mut cx, |app, window, cx| {
        app.prepare_background(window, cx)
    })?;
    anyhow::ensure!(
        snapshot(&mut cx, window)?
            .find("reference.sheet.query")
            .is_none(),
        "background retained open sheet"
    );
    // A second, genuinely shorter viewport proves resized layout only. It does
    // not claim a native keyboard appeared or that its lifecycle was delivered.
    let mut saved = gpui_box_mobile_reference::state::FixtureState::default().checkpoint();
    saved.tab = Tab::Form;
    let shorter = cx.open_window(size(px(390.0), px(520.0)), move |window, cx| {
        mount_checkpoint(saved, window, cx).expect("valid fixture checkpoint")
    })?;
    save(&mut cx, shorter.into(), directory, "resized-form")?;
    cx.update_window(shorter.into(), |_, window, cx| {
        window.dispatch_event(
            gpui::ScrollWheelEvent {
                position: point(px(380.0), px(360.0)),
                delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-400.0))),
                touch_phase: gpui::TouchPhase::Moved,
                modifiers: Modifiers::none(),
            }
            .to_platform_input(),
            cx,
        );
    })?;
    click(&mut cx, shorter.into(), "reference.form.accept")?;
    click(&mut cx, shorter.into(), "reference.sheet.open")?;
    save(&mut cx, shorter.into(), directory, "resized-sheet")?;
    println!(
        "PASS: retained rows, explicit refusal, detail/back, dirty-form refusal, checkpoint restore; 390px viewport captures"
    );
    Ok(())
}
