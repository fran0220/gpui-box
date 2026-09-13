//! Keyed decorative geometry lifetimes. Only current, nonzero-opacity geometry
//! can be picked or measured; retired layers never publish source authority.
use super::*;
use crate::motion::{MotionSpec, Transition};
use gpui::{App, Window};
use std::{collections::HashMap, rc::Rc};

#[derive(Clone)]
pub(super) struct Shape {
    pub data: Rc<GeoData>,
    pub index: usize,
}

impl Shape {
    fn same(&self, other: &Self) -> bool {
        match (
            self.data.polygons.get(self.index),
            other.data.polygons.get(other.index),
        ) {
            (Some(a), Some(b)) => a == b,
            (None, None) => {
                self.data.projected_points[self.index - self.data.features.len()]
                    == other.data.projected_points[other.index - other.data.features.len()]
            }
            _ => false,
        }
    }
}

struct Layer {
    shape: Shape,
    opacity: Transition<f32>,
    current: bool,
    order: usize,
}

#[derive(Default)]
pub(super) struct Presentation {
    previous: Option<Rc<GeoData>>,
    changing: HashMap<SharedString, Vec<Layer>>,
    next_order: usize,
}

pub(super) struct Frame {
    pub data: Rc<GeoData>,
    opacity: HashMap<SharedString, f32>,
    pub retired: Vec<(Shape, f32)>,
}

impl Frame {
    pub fn opacity(&self, index: usize) -> f32 {
        self.opacity
            .get(self.data.identity(index))
            .copied()
            .unwrap_or(1.0)
    }
    pub fn hit_test(
        &self,
        viewport: GeoViewport,
        size: [f64; 2],
        at: [f64; 2],
    ) -> Option<SharedString> {
        self.data
            .hit_test_visible(viewport, size, at, |index| self.opacity(index) > 0.0)
    }
}

impl Presentation {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn sample(
        &mut self,
        data: Rc<GeoData>,
        spec: MotionSpec,
        snap: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Rc<Frame> {
        if snap || cx.reduce_motion() {
            self.changing.clear();
            self.previous = Some(data.clone());
        }
        if let Some(previous) = self.previous.as_ref().filter(|old| !Rc::ptr_eq(old, &data)) {
            let before: HashMap<_, _> = (0..previous.count())
                .map(|index| (previous.identity(index).clone(), index))
                .collect();
            let after: HashMap<_, _> = (0..data.count())
                .map(|index| (data.identity(index).clone(), index))
                .collect();
            let ids = (0..data.count()).map(|i| data.identity(i).clone()).chain(
                (0..previous.count())
                    .filter(|i| !after.contains_key(previous.identity(*i)))
                    .map(|i| previous.identity(i).clone()),
            );
            for id in ids {
                let target = after.get(&id).map(|index| Shape {
                    data: data.clone(),
                    index: *index,
                });
                let old = before.get(&id).map(|index| Shape {
                    data: previous.clone(),
                    index: *index,
                });
                if !self.changing.contains_key(&id)
                    && old
                        .as_ref()
                        .zip(target.as_ref())
                        .is_some_and(|(old, new)| old.same(new))
                {
                    continue;
                }
                let layers = self.changing.entry(id).or_insert_with(|| {
                    old.into_iter()
                        .map(|shape| {
                            self.next_order += 1;
                            Layer {
                                shape,
                                opacity: Transition::new(1.0, spec),
                                current: false,
                                order: self.next_order,
                            }
                        })
                        .collect()
                });
                let mut matched = false;
                for layer in layers.iter_mut() {
                    layer.current = target
                        .as_ref()
                        .is_some_and(|target| layer.shape.same(target));
                    if layer.current {
                        layer.shape = target.as_ref().expect("matching shape").clone();
                        matched = true;
                    }
                    layer.opacity.set(if layer.current { 1.0 } else { 0.0 });
                }
                if let Some(shape) = target.filter(|_| !matched) {
                    let mut opacity = Transition::new(0.0, spec);
                    opacity.set(1.0);
                    self.next_order += 1;
                    layers.push(Layer {
                        shape,
                        opacity,
                        current: true,
                        order: self.next_order,
                    });
                }
            }
        }
        self.previous = Some(data.clone());
        let mut opacity = HashMap::new();
        let mut retired = Vec::new();
        self.changing.retain(|id, layers| {
            layers.retain_mut(|layer| {
                let alpha = layer.opacity.animate(window, cx).clamp(0.0, 1.0);
                if layer.current {
                    opacity.insert(id.clone(), alpha);
                } else if alpha > 0.0 {
                    retired.push((layer.order, layer.shape.clone(), alpha));
                }
                layer.current || alpha > 0.0 || layer.opacity.is_animating()
            });
            !(layers.is_empty()
                || (layers.len() == 1 && layers[0].current && !layers[0].opacity.is_animating()))
        });
        retired.sort_by_key(|(order, _, _)| *order);
        Rc::new(Frame {
            data,
            opacity,
            retired: retired
                .into_iter()
                .map(|(_, shape, alpha)| (shape, alpha))
                .collect(),
        })
    }
}

impl GeoData {
    pub(super) fn count(&self) -> usize {
        self.features.len() + self.points.len()
    }
    pub(super) fn identity(&self, index: usize) -> &SharedString {
        self.features
            .get(index)
            .map_or_else(|| &self.points[index - self.features.len()].id, |f| &f.id)
    }
}
