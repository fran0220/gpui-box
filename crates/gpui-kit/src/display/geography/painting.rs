//! The same validated paths paint both authoritative and decorative lifetimes.
use super::*;
use gpui::{
    Bounds, FillOptions, FillRule, Hsla, PathBuilder, PathStyle, Pixels, Window, point, px,
};
use gpui_kit_theme::Theme;

pub(super) struct Paint<'a> {
    pub viewport: GeoViewport,
    pub bounds: Bounds<Pixels>,
    pub theme: &'a Theme,
    pub selected: Option<&'a SharedString>,
}

impl Paint<'_> {
    pub fn draw(
        &self,
        data: &GeoData,
        index: usize,
        fill: Option<Hsla>,
        alpha: f32,
        window: &mut Window,
    ) {
        if alpha <= 0.0 {
            return;
        }
        let size = exploration::extent(self.bounds);
        let at = |p| {
            let p = self.viewport.screen(p, size);
            point(
                self.bounds.left() + px(p[0] as f32),
                self.bounds.top() + px(p[1] as f32),
            )
        };
        let selected = self.selected == Some(data.identity(index));
        if let Some(polygons) = data.polygons.get(index) {
            let color = fill
                .unwrap_or_else(|| super::view::color(data, data.features[index].value, self.theme))
                .opacity(alpha);
            for polygon in polygons {
                let mut fill = PathBuilder::fill().with_style(PathStyle::Fill(
                    FillOptions::default().with_fill_rule(FillRule::EvenOdd),
                ));
                let mut stroke = PathBuilder::stroke(px(if selected {
                    self.theme.borders.thick
                } else {
                    self.theme.borders.hairline
                }));
                for ring in std::iter::once(&polygon.exterior).chain(&polygon.holes) {
                    fill.move_to(at(ring[0]));
                    stroke.move_to(at(ring[0]));
                    for p in &ring[1..] {
                        fill.line_to(at(*p));
                        stroke.line_to(at(*p));
                    }
                    fill.close();
                    stroke.close();
                }
                if let Ok(path) = fill.build() {
                    window.paint_path(path, color);
                }
                if let Ok(path) = stroke.build() {
                    window.paint_path(
                        path,
                        if selected {
                            self.theme.colors.text.opacity(alpha)
                        } else {
                            self.theme.colors.hairline_strong.opacity(alpha)
                        },
                    );
                }
            }
        } else {
            let center = at(data.projected_points[index - data.features.len()]);
            // The renderer's rounded quad is an exact circular mask at radius
            // half the diameter; no per-marker path allocation or tessellation.
            window.paint_quad(
                gpui::fill(
                    Bounds::new(
                        center - point(px(5.0), px(5.0)),
                        gpui::size(px(10.0), px(10.0)),
                    ),
                    if selected {
                        self.theme.colors.text.opacity(alpha)
                    } else {
                        self.theme.colors.warning.opacity(alpha)
                    },
                )
                .corner_radii(px(5.0)),
            );
        }
    }
}
