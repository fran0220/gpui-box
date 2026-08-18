//! Every scene must publish a tree that assistive technology and automation
//! can use. The audit runs against the rendering, not against source text.

use gpui::TestAppContext;
use gpui_kit::scenes;
use gpui_kit_testkit::audit_or_error;
use gpui_kit_testkit::harness::Harness;

#[gpui::test]
fn every_scene_publishes_an_auditable_tree(cx: &mut TestAppContext) {
    let mut catalog = scenes::catalog().into_iter();
    let first = catalog.next().expect("the catalog is not empty");
    let mut harness = Harness::new(cx, gpui_kit::install, first.build);
    audit_scene(&mut harness, first.name);
    for scene in catalog {
        harness.remount(scene.build);
        audit_scene(&mut harness, scene.name);
    }
}

#[gpui::test]
fn every_scene_renders_in_both_themes(cx: &mut TestAppContext) {
    let catalog = scenes::catalog();
    let first = catalog.first().expect("the catalog is not empty");
    let mut harness = Harness::new(cx, gpui_kit::install, first.build);
    for theme in ["studio-dark", "studio-light"] {
        harness.update(|_, cx| {
            assert!(gpui_kit::theme::activate_theme(theme, cx));
        });
        for scene in &catalog {
            harness.remount(scene.build);
            assert!(
                !harness.snapshot().nodes.is_empty(),
                "scene `{}` published nothing under {theme}",
                scene.name
            );
        }
    }
}

fn audit_scene(harness: &mut Harness, name: &str) {
    let snapshot = harness.snapshot();
    assert!(
        !snapshot.nodes.is_empty(),
        "scene `{name}` published nothing to assert against"
    );
    if let Err(error) = audit_or_error(&snapshot) {
        panic!("scene `{name}` failed the audit:\n{error}");
    }
}
