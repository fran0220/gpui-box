//! Every scene must publish a tree that assistive technology and automation
//! can use. The audit runs against the rendering, not against source text.

use gpui::TestAppContext;
use gpui_kit::scenes;
use gpui_kit_testkit::audit_or_error;
use gpui_kit_testkit::harness::Harness;

#[gpui::test]
fn every_scene_publishes_an_auditable_tree(cx: &mut TestAppContext) {
    for scene in scenes::catalog() {
        let build = scene.build;
        let mut harness = Harness::new(cx, gpui_kit::install, build);
        let snapshot = harness.snapshot();

        assert!(
            !snapshot.nodes.is_empty(),
            "scene `{}` published nothing to assert against",
            scene.name
        );

        if let Err(error) = audit_or_error(&snapshot) {
            panic!("scene `{}` failed the audit:\n{error}", scene.name);
        }
    }
}

#[gpui::test]
fn every_scene_renders_in_both_themes(cx: &mut TestAppContext) {
    for theme in ["studio-dark", "studio-light"] {
        for scene in scenes::catalog() {
            let build = scene.build;
            let mut harness = Harness::new(cx, gpui_kit::install, build);
            harness.update(|_, cx| {
                assert!(gpui_kit::theme::activate_theme(theme, cx));
            });
            assert!(
                !harness.snapshot().nodes.is_empty(),
                "scene `{}` published nothing under {theme}",
                scene.name
            );
        }
    }
}
