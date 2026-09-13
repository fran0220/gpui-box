//! Keyed flow motion. Ribbon attachments are expressed in displayed node space.
use super::{SankeyData, SankeyLink, SankeyNode};
use crate::motion::{Interpolate, MotionSpec, Transition};
use gpui::{Hsla, Point, SharedString, Size, bounds, point};
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

struct Node {
    raw: SankeyNode,
    origin: Transition<Point<f32>>,
    size: Transition<Size<f32>>,
    alpha: Transition<f32>,
}

struct Link {
    raw: SankeyLink,
    start: Transition<Point<f32>>,
    end: Transition<Point<f32>>,
    widths: Transition<Point<f32>>,
    alpha: Transition<f32>,
}

#[derive(Default)]
pub(super) struct FlowMotion {
    nodes: BTreeMap<SharedString, Node>,
    links: BTreeMap<SharedString, Link>,
    retired: Vec<Link>,
    last: Option<web_time::Instant>,
    spec: Option<MotionSpec>,
}

fn tick<T: Interpolate + PartialEq>(
    value: &mut Transition<T>,
    target: T,
    spec: MotionSpec,
    delta: Duration,
    snap: bool,
) -> T {
    *value = value.spec(spec);
    value.set(target);
    if snap {
        value.snap(target);
    } else {
        value.advance(delta);
    }
    value.value()
}

fn relative(p: Point<f32>, node: &SankeyNode) -> Point<f32> {
    point(
        (p.x - node.bounds.origin.x) / node.bounds.size.width,
        p.y - node.bounds.origin.y,
    )
}

fn absolute(p: Point<f32>, node: &SankeyNode) -> Point<f32> {
    point(
        node.bounds.origin.x + p.x * node.bounds.size.width,
        node.bounds.origin.y + p.y,
    )
}

fn project_link(
    link: &Link,
    nodes: &BTreeMap<SharedString, SankeyNode>,
    accent: Hsla,
) -> Option<SankeyLink> {
    let source = nodes.get(&link.raw.source)?;
    let target = nodes.get(&link.raw.target)?;
    let mut shown = link.raw.clone();
    shown.start = absolute(link.start.value(), source);
    shown.end = absolute(link.end.value(), target);
    shown.start_width = link.widths.value().x.max(0.0);
    shown.end_width = link.widths.value().y.max(0.0);
    shown.color = Some(
        shown
            .color
            .unwrap_or(accent)
            .opacity(link.alpha.value().clamp(0.0, 1.0)),
    );
    Some(shown)
}

impl FlowMotion {
    pub(super) fn animate(
        &mut self,
        data: &mut SankeyData,
        spec: MotionSpec,
        enabled: bool,
        accent: Hsla,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> SankeyData {
        let now = cx.background_executor().now();
        let delta = self
            .last
            .map(|t| now.saturating_duration_since(t))
            .unwrap_or_default();
        let painted = self.update(data, spec, delta, !enabled || cx.reduce_motion(), accent);
        let running =
            self.nodes.values().any(|n| {
                n.origin.is_animating() || n.size.is_animating() || n.alpha.is_animating()
            }) || self.links.values().chain(self.retired.iter()).any(|l| {
                l.start.is_animating()
                    || l.end.is_animating()
                    || l.widths.is_animating()
                    || l.alpha.is_animating()
            });
        self.last = running.then_some(now);
        if running {
            window.request_animation_frame();
        }
        painted
    }

    pub(super) fn update(
        &mut self,
        data: &mut SankeyData,
        spec: MotionSpec,
        delta: Duration,
        snap: bool,
        accent: Hsla,
    ) -> SankeyData {
        if self.spec.is_some_and(|previous| previous != spec) {
            for node in self.nodes.values_mut() {
                super::retime(&mut node.origin, spec);
                super::retime(&mut node.size, spec);
                super::retime(&mut node.alpha, spec);
            }
            for link in self.links.values_mut().chain(self.retired.iter_mut()) {
                super::retime(&mut link.start, spec);
                super::retime(&mut link.end, spec);
                super::retime(&mut link.widths, spec);
                super::retime(&mut link.alpha, spec);
            }
        }
        self.spec = Some(spec);
        let node_ids: HashSet<_> = data.nodes.iter().map(|n| n.id.clone()).collect();
        let link_ids: HashSet<_> = data.links.iter().map(|l| l.id.clone()).collect();
        for node in &data.nodes {
            let entry = self.nodes.entry(node.id.clone()).or_insert_with(|| Node {
                raw: node.clone(),
                origin: Transition::new(node.bounds.origin, spec),
                size: Transition::new(node.bounds.size, spec),
                alpha: Transition::new(0.0, spec),
            });
            entry.raw = node.clone();
        }
        for link in &data.links {
            let source = data
                .nodes
                .iter()
                .find(|n| n.id == link.source)
                .expect("validated source");
            let target = data
                .nodes
                .iter()
                .find(|n| n.id == link.target)
                .expect("validated target");
            let start = relative(link.start, source);
            let end = relative(link.end, target);
            // Keep vertical offsets and thickness in plot units. Multiplying
            // independently interpolated fractions by node height would make
            // the two ends of a conserved ribbon disagree mid-transition.
            let widths = point(link.start_width, link.end_width);
            let entry = self.links.entry(link.id.clone()).or_insert_with(|| Link {
                raw: link.clone(),
                start: Transition::new(start, spec),
                end: Transition::new(end, spec),
                widths: Transition::new(widths, spec),
                alpha: Transition::new(0.0, spec),
            });
            // A changed endpoint is a changed connection, not a ribbon moving
            // through unrelated nodes. Retire the old connection decoratively
            // and enter its replacement under the caller's unchanged live id.
            if entry.raw.source != link.source || entry.raw.target != link.target {
                self.retired.push(std::mem::replace(
                    entry,
                    Link {
                        raw: link.clone(),
                        start: Transition::new(start, spec),
                        end: Transition::new(end, spec),
                        widths: Transition::new(widths, spec),
                        alpha: Transition::new(0.0, spec),
                    },
                ));
            }
            entry.raw = link.clone();
            tick(&mut entry.start, start, spec, delta, snap);
            tick(&mut entry.end, end, spec, delta, snap);
            tick(&mut entry.widths, widths, spec, delta, snap);
        }
        let mut shown_nodes = BTreeMap::new();
        for (id, node) in &mut self.nodes {
            let mut shown = node.raw.clone();
            shown.bounds = bounds(
                tick(&mut node.origin, node.raw.bounds.origin, spec, delta, snap),
                tick(&mut node.size, node.raw.bounds.size, spec, delta, snap),
            );
            // Motion cannot publish an invalid negative/out-of-frame node.
            // Rendering and exact picking consume this same clipped geometry.
            if !snap {
                shown.bounds.origin.x = shown.bounds.origin.x.clamp(0.0, 1.0);
                shown.bounds.origin.y = shown.bounds.origin.y.clamp(0.0, 1.0);
                shown.bounds.size.width = shown.bounds.size.width.max(0.0);
                shown.bounds.size.height = shown.bounds.size.height.max(0.0);
                if shown.bounds.origin.x + shown.bounds.size.width > 1.0 {
                    shown.bounds.size.width = 1.0 - shown.bounds.origin.x;
                }
                if shown.bounds.origin.y + shown.bounds.size.height > 1.0 {
                    shown.bounds.size.height = 1.0 - shown.bounds.origin.y;
                }
            }
            let alpha = tick(
                &mut node.alpha,
                if node_ids.contains(id) { 1.0 } else { 0.0 },
                spec,
                delta,
                snap,
            )
            .clamp(0.0, 1.0);
            shown.color = Some(shown.color.unwrap_or(accent).opacity(alpha));
            shown_nodes.insert(id.clone(), shown);
        }
        let mut shown_links = BTreeMap::new();
        for (id, link) in &mut self.links {
            tick(
                &mut link.alpha,
                if link_ids.contains(id) { 1.0 } else { 0.0 },
                spec,
                delta,
                snap,
            );
            if let Some(shown) = project_link(link, &shown_nodes, accent) {
                shown_links.insert(id.clone(), shown);
            }
        }
        let mut painted = SankeyData::default();
        self.retired.retain_mut(|link| {
            tick(&mut link.alpha, 0.0, spec, delta, snap);
            if link.alpha.is_animating() {
                painted
                    .links
                    .extend(project_link(link, &shown_nodes, accent));
                true
            } else {
                false
            }
        });
        // Exits behind live records, matching the live reverse-order picker.
        painted.nodes.extend(
            shown_nodes
                .values()
                .filter(|n| !node_ids.contains(&n.id))
                .cloned(),
        );
        painted.links.extend(
            shown_links
                .values()
                .filter(|l| !link_ids.contains(&l.id))
                .cloned(),
        );
        for node in &mut data.nodes {
            let shown = &shown_nodes[&node.id];
            node.bounds = shown.bounds;
            painted.nodes.push(shown.clone());
        }
        for link in &mut data.links {
            let shown = &shown_links[&link.id];
            link.start = shown.start;
            link.end = shown.end;
            link.start_width = shown.start_width;
            link.end_width = shown.end_width;
            painted.links.push(shown.clone());
        }
        self.links
            .retain(|id, l| link_ids.contains(id) || l.alpha.is_animating());
        self.nodes.retain(|id, n| {
            node_ids.contains(id)
                || n.alpha.is_animating()
                || self
                    .links
                    .values()
                    .chain(self.retired.iter())
                    .any(|l| &l.raw.source == id || &l.raw.target == id)
        });
        painted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{hsla, size};
    fn fixture(y: f32, value: &str) -> SankeyData {
        SankeyData::new(
            [
                SankeyNode::new(
                    "a",
                    "A",
                    value.to_owned(),
                    bounds(point(0.0, y), size(0.1, 0.2)),
                ),
                SankeyNode::new(
                    "b",
                    "B",
                    value.to_owned(),
                    bounds(point(0.9, 0.6), size(0.1, 0.3)),
                ),
            ],
            [SankeyLink::new(
                "ab",
                "a",
                "b",
                "A to B",
                value.to_owned(),
                point(0.1, y + 0.1),
                point(0.9, 0.75),
                0.2,
            )],
        )
    }
    #[test]
    fn keyed_motion_keeps_ribbons_attached_exact_values_and_noninteractive_exits() {
        let spec = MotionSpec::new(1000, crate::motion::CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        let mut motion = FlowMotion::default();
        let tint = hsla(0.0, 1.0, 0.5, 1.0);
        motion.update(&mut fixture(0.0, "7"), spec, Duration::ZERO, true, tint);
        let mut next = fixture(0.6, "23");
        motion.update(&mut next, spec, Duration::from_millis(500), false, tint);
        assert!((next.nodes[0].bounds.origin.y - 0.3).abs() < 0.001);
        assert!((next.links[0].start.y - 0.4).abs() < 0.001);
        assert_eq!(next.links[0].value.as_ref(), "23");
        assert_eq!(
            next.links[0].start.x,
            next.nodes[0].bounds.origin.x + next.nodes[0].bounds.size.width
        );
        let mut retarget = fixture(0.1, "11");
        motion.update(&mut retarget, spec, Duration::ZERO, false, tint);
        assert_eq!(next.links[0].start, retarget.links[0].start);
        let mut empty = SankeyData::default();
        let exits = motion.update(&mut empty, spec, Duration::from_millis(100), false, tint);
        assert!(empty.nodes.is_empty() && empty.links.is_empty());
        assert_eq!(exits.links.len(), 1);
        let mut reinsert = fixture(0.2, "19");
        motion.update(&mut reinsert, spec, Duration::ZERO, true, tint);
        assert!((reinsert.links[0].start.y - 0.3).abs() < 0.001);
        motion.update(&mut empty, spec, Duration::ZERO, true, tint);
        assert!(motion.nodes.is_empty() && motion.links.is_empty());
    }

    #[test]
    fn asymmetric_throughput_keeps_equal_ribbon_ends_and_retiming_is_continuous() {
        let layout = |weights: [f64; 3]| {
            SankeyData::new(
                ["a", "b", "c", "d", "e"].map(|id| SankeyNode::new(id, id, "", Default::default())),
                [("ac", "a", "c"), ("ad", "a", "d"), ("be", "b", "e")].map(|(id, s, t)| {
                    SankeyLink::new(
                        id,
                        s,
                        t,
                        id,
                        "",
                        Default::default(),
                        Default::default(),
                        0.0,
                    )
                }),
            )
            .layout(&weights, 0.08, 0.05, super::super::SankeyAlignment::Left)
            .expect("conserved layout")
            .0
        };
        let spec = MotionSpec::new(1000, crate::motion::CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        let tint = hsla(0.0, 1.0, 0.5, 1.0);
        let mut motion = FlowMotion::default();
        motion.update(
            &mut layout([10.0, 20.0, 70.0]),
            spec,
            Duration::ZERO,
            true,
            tint,
        );
        let mut mid = layout([60.0, 10.0, 30.0]);
        motion.update(&mut mid, spec, Duration::from_millis(500), false, tint);
        // Three sinks leave 0.9 plot height for total100: 35×0.009.
        assert!((mid.links[0].start_width - 0.315).abs() < 1e-6);
        for link in &mid.links {
            assert_eq!(link.start_width, link.end_width);
            let source = mid
                .nodes
                .iter()
                .find(|n| n.id == link.source)
                .expect("source");
            let target = mid
                .nodes
                .iter()
                .find(|n| n.id == link.target)
                .expect("target");
            assert_eq!(
                link.start.x,
                source.bounds.origin.x + source.bounds.size.width
            );
            assert_eq!(link.end.x, target.bounds.origin.x);
            assert!(link.start.y - link.start_width / 2.0 >= source.bounds.origin.y - 1e-6);
            assert!(
                link.start.y + link.start_width / 2.0
                    <= source.bounds.origin.y + source.bounds.size.height + 1e-6
            );
        }
        let mut retimed = layout([60.0, 10.0, 30.0]);
        let spring = MotionSpec::sprung(crate::motion::Spring::new(400.0, 28.0, 1.0));
        motion.update(&mut retimed, spring, Duration::ZERO, false, tint);
        assert_eq!(mid, retimed);
        retimed = layout([60.0, 10.0, 30.0]);
        motion.update(&mut retimed, spring, Duration::ZERO, true, tint);
        assert_eq!(retimed, layout([60.0, 10.0, 30.0]));
    }

    #[test]
    fn rewiring_retires_the_old_connection_without_duplicate_live_identity() {
        let spec = MotionSpec::new(1000, crate::motion::CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        let tint = hsla(0.0, 1.0, 0.5, 1.0);
        let mut motion = FlowMotion::default();
        motion.update(&mut fixture(0.0, "7"), spec, Duration::ZERO, true, tint);
        let mut next = fixture(0.0, "23");
        next.nodes[1].id = "c".into();
        next.nodes[1].bounds.origin.y = 0.1;
        next.links[0].target = "c".into();
        next.links[0].end.y = 0.25;
        let painted = motion.update(&mut next, spec, Duration::ZERO, false, tint);
        assert_eq!(next.links.len(), 1);
        assert_eq!(next.links[0].value.as_ref(), "23");
        assert_eq!(next.links[0].target.as_ref(), "c");
        assert_eq!(next.links[0].end.y, 0.25);
        assert_eq!(painted.links.len(), 2);
        assert_eq!(painted.links[0].target.as_ref(), "b");
        assert_eq!(painted.links[0].end.y, 0.75);
        assert!(!next.nodes.iter().any(|n| n.id.as_ref() == "b"));
        motion.update(&mut SankeyData::default(), spec, Duration::ZERO, true, tint);
        assert!(motion.retired.is_empty() && motion.nodes.is_empty() && motion.links.is_empty());
    }
}
