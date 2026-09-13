//! Measured decorative heatmap exits; live positioning uses shared FLIP.
use crate::foundation::StyledExt;
use crate::motion::{MotionSpec, Transition};
use gpui::{
    AnyElement, App, Bounds, Hsla, IntoElement, ParentElement, Pixels, SharedString, Styled,
    Window, div, px,
};
use gpui_kit_theme::{Theme, TypeScale};
use std::{
    cell::Cell,
    collections::{BTreeMap, HashSet},
    rc::Rc,
};

struct Entry {
    measured: Rc<Cell<Bounds<Pixels>>>,
    alpha: Transition<f32>,
    fill: Hsla,
    text: SharedString,
}

#[derive(Default)]
pub(super) struct HeatLayout {
    entries: BTreeMap<SharedString, Entry>,
    live: HashSet<SharedString>,
    motion: Option<(MotionSpec, bool)>,
}

impl HeatLayout {
    pub(super) fn begin(&mut self, spec: MotionSpec, enabled: bool) {
        self.live.clear();
        if self.motion.is_some_and(|(previous, _)| previous != spec) {
            for entry in self.entries.values_mut() {
                crate::display::plot::retime(&mut entry.alpha, spec);
            }
        }
        self.motion = Some((spec, enabled));
    }

    pub(super) fn track(
        &mut self,
        id: SharedString,
        fill: Hsla,
        text: SharedString,
        window: &mut Window,
        cx: &mut App,
    ) -> (Rc<Cell<Bounds<Pixels>>>, f32) {
        let (spec, enabled) = self.motion.expect("heatmap frame begun");
        self.live.insert(id.clone());
        let entry = self.entries.entry(id.clone()).or_insert_with(|| Entry {
            measured: crate::layout::measure::cell(&id, window, cx),
            alpha: Transition::new(0.0, spec),
            fill,
            text: text.clone(),
        });
        entry.fill = fill;
        entry.text = text;
        entry.alpha = entry.alpha.spec(spec);
        if enabled {
            entry.alpha.set(1.0);
        } else {
            entry.alpha.snap(1.0);
        }
        let alpha = entry.alpha.animate(window, cx).clamp(0.0, 1.0);
        (entry.measured.clone(), alpha)
    }

    pub(super) fn exits(
        &mut self,
        frame: Bounds<Pixels>,
        spec: MotionSpec,
        enabled: bool,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> (Vec<AnyElement>, f32) {
        let mut exits = Vec::new();
        let mut height: f32 = 0.0;
        self.entries.retain(|id, entry| {
            if self.live.contains(id) {
                return true;
            }
            entry.alpha = entry.alpha.spec(spec);
            if enabled {
                entry.alpha.set(0.0);
            } else {
                entry.alpha.snap(0.0);
            }
            let alpha = entry.alpha.animate(window, cx).clamp(0.0, 1.0);
            let bounds = entry.measured.get();
            if alpha > 0.0 && bounds.size.width > px(0.0) && bounds.size.height > px(0.0) {
                let local = bounds.origin - frame.origin;
                height = height.max(f32::from(local.y + bounds.size.height));
                // No semantic marker, hitbox, tooltip, focus or action handler.
                exits.push(
                    div()
                        .absolute()
                        .left(local.x)
                        .top(local.y)
                        .w(bounds.size.width)
                        .h(bounds.size.height)
                        .overflow_hidden()
                        .bg(entry.fill)
                        .opacity(alpha)
                        .text_color(theme.colors.text)
                        .type_scale(theme, TypeScale::Caption)
                        .child(entry.text.clone())
                        .into_any_element(),
                );
            }
            entry.alpha.is_animating()
        });
        (exits, height)
    }
}
