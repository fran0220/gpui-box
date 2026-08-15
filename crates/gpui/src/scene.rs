// todo("windows"): remove
#![cfg_attr(windows, allow(dead_code))]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    AtlasTextureId, AtlasTile, Background, Bounds, ContentMask, Corners, Edges, Hsla, Pixels,
    Point, Radians, ScaledPixels, Size, bounds_tree::BoundsTree, point,
};
use std::{
    fmt::Debug,
    iter::Peekable,
    ops::{Add, Range, Sub},
    slice,
};

#[allow(non_camel_case_types, unused)]
#[expect(missing_docs)]
pub type PathVertex_ScaledPixels = PathVertex<ScaledPixels>;

#[expect(missing_docs)]
pub type DrawOrder = u32;

/// A boolean stored as a `u32` so that GPU-facing structs contain no
/// compiler-inserted padding bytes, which would be undefined behavior to
/// reinterpret as `&[u8]` when writing instance buffers. Guaranteed to be
/// `0` or `1` by construction; shaders read it as a `u32`/`uint`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct PaddedBool32(u32);

impl From<bool> for PaddedBool32 {
    fn from(value: bool) -> Self {
        PaddedBool32(value as u32)
    }
}

#[derive(Default)]
#[expect(missing_docs)]
pub struct Scene {
    pub(crate) paint_operations: Vec<PaintOperation>,
    primitive_bounds: BoundsTree<ScaledPixels>,
    layer_stack: Vec<DrawOrder>,
    pub shadows: Vec<Shadow>,
    pub quads: Vec<Quad>,
    pub paths: Vec<Path<ScaledPixels>>,
    pub underlines: Vec<Underline>,
    pub monochrome_sprites: Vec<MonochromeSprite>,
    pub subpixel_sprites: Vec<SubpixelSprite>,
    pub polychrome_sprites: Vec<PolychromeSprite>,
    pub surfaces: Vec<PaintSurface>,
    /// Glass surfaces — deliberately outside the primitive batch stream so
    /// renderers can snapshot the framebuffer at each surface's order.
    pub backdrop_glass: Vec<BackdropGlass>,
}

#[expect(missing_docs)]
impl Scene {
    pub fn clear(&mut self) {
        self.paint_operations.clear();
        self.primitive_bounds.clear();
        self.layer_stack.clear();
        self.paths.clear();
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.subpixel_sprites.clear();
        self.polychrome_sprites.clear();
        self.surfaces.clear();
        self.backdrop_glass.clear();
    }

    pub fn len(&self) -> usize {
        self.paint_operations.len()
    }

    /// Returns whether the scene contains no drawable primitives.
    ///
    /// A scene may have paint operations that only open and close empty layers,
    /// so `len() == 0` is not equivalent to having no visible/input-relevant
    /// overlay content.
    pub fn is_empty(&self) -> bool {
        self.shadows.is_empty()
            && self.quads.is_empty()
            && self.paths.is_empty()
            && self.underlines.is_empty()
            && self.monochrome_sprites.is_empty()
            && self.subpixel_sprites.is_empty()
            && self.polychrome_sprites.is_empty()
            && self.surfaces.is_empty()
    }

    pub fn push_layer(&mut self, bounds: Bounds<ScaledPixels>) {
        let order = self.primitive_bounds.insert(bounds);
        self.layer_stack.push(order);
        self.paint_operations
            .push(PaintOperation::StartLayer(bounds));
    }

    pub fn pop_layer(&mut self) {
        self.layer_stack.pop();
        self.paint_operations.push(PaintOperation::EndLayer);
    }

    pub fn insert_backdrop_glass(&mut self, mut glass: BackdropGlass) {
        if !glass.blur_radius.0.is_finite() || glass.blur_radius.0 < 0.0 {
            return;
        }
        let clipped_bounds = glass.bounds.intersect(&glass.content_mask.bounds);
        if clipped_bounds.is_empty() {
            return;
        }
        // A lobe count past the array is a caller error that would otherwise
        // read whatever `Default` left behind, so it is clamped here rather
        // than in three shaders.
        glass.lobe_count = glass.lobe_count.min(MAX_GLASS_LOBES as u32);
        glass.material = glass.material.sanitized();
        glass.order = self
            .layer_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.primitive_bounds.insert(clipped_bounds));
        self.backdrop_glass.push(glass);
        self.paint_operations
            .push(PaintOperation::BackdropGlass(glass));
    }

    pub fn insert_primitive(&mut self, primitive: impl Into<Primitive>) {
        let mut primitive = primitive.into();
        let clipped_bounds = primitive
            .bounds()
            .intersect(&primitive.content_mask().bounds);

        if clipped_bounds.is_empty() {
            return;
        }

        let order = self
            .layer_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.primitive_bounds.insert(clipped_bounds));
        match &mut primitive {
            Primitive::Shadow(shadow) => {
                shadow.order = order;
                self.shadows.push(*shadow);
            }
            Primitive::Quad(quad) => {
                quad.order = order;
                self.quads.push(*quad);
            }
            Primitive::Path(path) => {
                path.order = order;
                path.id = PathId(self.paths.len());
                self.paths.push(path.clone());
            }
            Primitive::Underline(underline) => {
                underline.order = order;
                self.underlines.push(*underline);
            }
            Primitive::MonochromeSprite(sprite) => {
                sprite.order = order;
                self.monochrome_sprites.push(*sprite);
            }
            Primitive::SubpixelSprite(sprite) => {
                sprite.order = order;
                self.subpixel_sprites.push(*sprite);
            }
            Primitive::PolychromeSprite(sprite) => {
                sprite.order = order;
                self.polychrome_sprites.push(*sprite);
            }
            Primitive::Surface(surface) => {
                surface.order = order;
                self.surfaces.push(surface.clone());
            }
        }
        self.paint_operations
            .push(PaintOperation::Primitive(primitive));
    }

    pub fn replay(&mut self, range: Range<usize>, prev_scene: &Scene) {
        for operation in &prev_scene.paint_operations[range] {
            match operation {
                PaintOperation::Primitive(primitive) => self.insert_primitive(primitive.clone()),
                PaintOperation::BackdropGlass(glass) => self.insert_backdrop_glass(*glass),
                PaintOperation::StartLayer(bounds) => self.push_layer(*bounds),
                PaintOperation::EndLayer => self.pop_layer(),
            }
        }
    }

    pub fn finish(&mut self) {
        self.shadows.sort_by_key(|shadow| shadow.order);
        self.quads.sort_by_key(|quad| quad.order);
        self.paths.sort_by_key(|path| path.order);
        self.underlines.sort_by_key(|underline| underline.order);
        self.monochrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.subpixel_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.polychrome_sprites
            .sort_by_key(|sprite| (sprite.order, sprite.tile.tile_id));
        self.surfaces.sort_by_key(|surface| surface.order);
        self.backdrop_glass.sort_by_key(|glass| glass.order);
    }

    #[cfg_attr(
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(any(feature = "x11", feature = "wayland"))
        ),
        allow(dead_code)
    )]
    pub fn batches(&self) -> impl Iterator<Item = PrimitiveBatch> + '_ {
        BatchIterator {
            shadows_start: 0,
            shadows_iter: self.shadows.iter().peekable(),
            quads_start: 0,
            quads_iter: self.quads.iter().peekable(),
            paths_start: 0,
            paths_iter: self.paths.iter().peekable(),
            underlines_start: 0,
            underlines_iter: self.underlines.iter().peekable(),
            monochrome_sprites_start: 0,
            monochrome_sprites_iter: self.monochrome_sprites.iter().peekable(),
            subpixel_sprites_start: 0,
            subpixel_sprites_iter: self.subpixel_sprites.iter().peekable(),
            polychrome_sprites_start: 0,
            polychrome_sprites_iter: self.polychrome_sprites.iter().peekable(),
            surfaces_start: 0,
            surfaces_iter: self.surfaces.iter().peekable(),
            backdrop_glass_iter: self.backdrop_glass.iter().peekable(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackgroundTag, ColorSpace, LinearColorStop, MAX_LINEAR_GRADIENT_STOPS};

    #[test]
    fn empty_layers_do_not_make_a_scene_drawable() {
        let mut scene = Scene::default();
        let bounds = Bounds {
            origin: Point::default(),
            size: Size {
                width: ScaledPixels::from(100.),
                height: ScaledPixels::from(100.),
            },
        };

        scene.push_layer(bounds);
        scene.pop_layer();

        assert_ne!(scene.len(), 0);
        assert!(scene.is_empty());
    }

    #[test]
    fn drawable_primitives_make_a_scene_non_empty() {
        let mut scene = Scene::default();
        let bounds = Bounds {
            origin: Point::default(),
            size: Size {
                width: ScaledPixels::from(100.),
                height: ScaledPixels::from(100.),
            },
        };

        scene.insert_primitive(Quad {
            bounds,
            content_mask: ContentMask { bounds },
            ..Default::default()
        });

        assert!(!scene.is_empty());
    }

    #[test]
    fn replay_preserves_scene_emptiness() {
        let mut source = Scene::default();
        let bounds = Bounds {
            origin: Point::default(),
            size: Size {
                width: ScaledPixels::from(100.),
                height: ScaledPixels::from(100.),
            },
        };
        source.push_layer(bounds);
        source.pop_layer();

        let mut replayed = Scene::default();
        replayed.replay(0..source.len(), &source);

        assert!(replayed.is_empty());
    }

    /// A lobe with uniform rounding, so the field is easy to reason about.
    fn test_lobe(origin: (f32, f32), size: (f32, f32), radius: f32) -> GlassLobe {
        GlassLobe {
            bounds: Bounds {
                origin: Point {
                    x: ScaledPixels(origin.0),
                    y: ScaledPixels(origin.1),
                },
                size: Size {
                    width: ScaledPixels(size.0),
                    height: ScaledPixels(size.1),
                },
            },
            corner_radii: Corners {
                top_left: ScaledPixels(radius),
                top_right: ScaledPixels(radius),
                bottom_right: ScaledPixels(radius),
                bottom_left: ScaledPixels(radius),
            },
        }
    }

    #[test]
    fn invalid_backdrop_glass_radii_are_ignored() {
        let bounds = Bounds {
            origin: Point::default(),
            size: Size {
                width: ScaledPixels::from(100.),
                height: ScaledPixels::from(100.),
            },
        };
        let mut scene = Scene::default();

        for radius in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            scene.insert_backdrop_glass(BackdropGlass {
                order: 0,
                blur_radius: ScaledPixels(radius),
                bounds,
                content_mask: ContentMask { bounds },
                corner_radii: Corners::default(),
                material: GlassMaterial::frosted(),
                lobes: [GlassLobe::default(); MAX_GLASS_LOBES],
                lobe_count: 0,
            });
        }

        assert!(scene.backdrop_glass.is_empty());
        assert_eq!(scene.len(), 0);
    }

    #[test]
    fn a_lobe_count_past_the_array_is_clamped_rather_than_read() {
        let bounds = Bounds {
            origin: Point::default(),
            size: Size {
                width: ScaledPixels::from(100.),
                height: ScaledPixels::from(100.),
            },
        };
        let mut scene = Scene::default();
        scene.insert_backdrop_glass(BackdropGlass {
            order: 0,
            blur_radius: ScaledPixels(8.),
            bounds,
            content_mask: ContentMask { bounds },
            corner_radii: Corners::default(),
            material: GlassMaterial::frosted(),
            lobes: [GlassLobe::default(); MAX_GLASS_LOBES],
            lobe_count: 4096,
        });

        let glass = scene.backdrop_glass[0];
        assert_eq!(glass.lobe_count, MAX_GLASS_LOBES as u32);
        assert_eq!(glass.shape().1, MAX_GLASS_LOBES);
    }

    #[test]
    fn a_surface_with_no_lobes_is_its_own_rounded_rect() {
        let bounds = Bounds {
            origin: Point {
                x: ScaledPixels(10.),
                y: ScaledPixels(20.),
            },
            size: Size {
                width: ScaledPixels(100.),
                height: ScaledPixels(50.),
            },
        };
        let corner_radii = Corners {
            top_left: ScaledPixels(6.),
            top_right: ScaledPixels(6.),
            bottom_right: ScaledPixels(6.),
            bottom_left: ScaledPixels(6.),
        };
        let glass = BackdropGlass {
            order: 0,
            blur_radius: ScaledPixels(8.),
            bounds,
            content_mask: ContentMask { bounds },
            corner_radii,
            material: GlassMaterial::frosted(),
            lobes: [GlassLobe::default(); MAX_GLASS_LOBES],
            lobe_count: 0,
        };

        let (lobes, count) = glass.shape();
        assert_eq!(count, 1);
        assert_eq!(lobes[0].bounds, bounds);
        assert_eq!(lobes[0].corner_radii, corner_radii);
    }

    #[test]
    fn a_material_a_renderer_could_not_act_on_is_replaced_by_one_it_can() {
        let material = GlassMaterial {
            bevel: ScaledPixels(f32::NAN),
            refraction: f32::INFINITY,
            dispersion: 4.,
            specular: -1.,
            light_angle: f32::NAN,
            specular_sharpness: 0.,
            smoothing: ScaledPixels(-8.),
            probe: NO_LUMINANCE_PROBE,
        }
        .sanitized();

        assert_eq!(material.bevel, ScaledPixels(0.));
        assert_eq!(material.refraction, 0.);
        assert_eq!(material.dispersion, 1., "dispersion is a fraction");
        assert_eq!(material.specular, 0.);
        assert_eq!(material.light_angle, 0.);
        assert_eq!(
            material.specular_sharpness, 1.,
            "a lobe flatter than one is not a lobe"
        );
        assert_eq!(material.smoothing, ScaledPixels(0.));
    }

    #[test]
    fn a_frosted_material_asks_for_no_optics() {
        let frosted = GlassMaterial::<ScaledPixels>::frosted();
        assert!(frosted.is_flat());
        assert!(!frosted.bends_light());
        assert_eq!(GlassMaterial::<ScaledPixels>::default(), frosted);
    }

    #[test]
    fn scaling_a_material_moves_its_lengths_and_nothing_else() {
        let logical = GlassMaterial::<Pixels> {
            bevel: Pixels(14.),
            refraction: 0.55,
            dispersion: 0.16,
            specular: 0.4,
            light_angle: 0.78,
            specular_sharpness: 12.,
            smoothing: Pixels(28.),
            probe: NO_LUMINANCE_PROBE,
        };

        let device = logical.scale(2.);

        assert_eq!(device.bevel, ScaledPixels(28.));
        assert_eq!(device.smoothing, ScaledPixels(56.));
        assert_eq!(device.refraction, logical.refraction, "a ratio is a ratio");
        assert_eq!(device.dispersion, logical.dispersion);
        assert_eq!(device.specular, logical.specular);
        assert_eq!(
            device.light_angle, logical.light_angle,
            "an angle does not scale"
        );
        assert_eq!(device.specular_sharpness, logical.specular_sharpness);
        assert_eq!(device.probe, logical.probe);
    }

    fn probe_glass(origin: (f32, f32), extent: (f32, f32)) -> BackdropGlass {
        let bounds = Bounds {
            origin: Point {
                x: ScaledPixels(origin.0),
                y: ScaledPixels(origin.1),
            },
            size: Size {
                width: ScaledPixels(extent.0),
                height: ScaledPixels(extent.1),
            },
        };
        BackdropGlass {
            order: 0,
            blur_radius: ScaledPixels(24.),
            bounds,
            content_mask: ContentMask { bounds },
            corner_radii: Corners::default(),
            material: GlassMaterial::frosted(),
            lobes: [GlassLobe::default(); MAX_GLASS_LOBES],
            lobe_count: 0,
        }
    }

    #[test]
    fn a_probe_samples_the_centre_and_the_quarter_points() {
        let glass = probe_glass((100., 200.), (400., 80.));

        let points = glass.probe_sample_points(1000., 1000.);

        assert_eq!(points[0], [300., 240.], "the centre");
        assert_eq!(points[1], [200., 220.]);
        assert_eq!(points[2], [400., 220.]);
        assert_eq!(points[3], [200., 260.]);
        assert_eq!(points[4], [400., 260.]);
    }

    #[test]
    fn a_probe_never_samples_outside_the_texture() {
        let glass = probe_glass((-50., -50.), (2000., 2000.));

        for [x, y] in glass.probe_sample_points(640., 480.) {
            assert!((0.0..640.0).contains(&x), "column {x} is outside");
            assert!((0.0..480.0).contains(&y), "row {y} is outside");
        }
    }

    #[test]
    fn probe_luminance_weighs_green_heaviest() {
        assert_eq!(probe_sample_luminance(0., 0., 0.), 0.);
        assert!((probe_sample_luminance(1., 1., 1.) - 1.).abs() < 1e-6);
        let green = probe_sample_luminance(0., 1., 0.);
        let red = probe_sample_luminance(1., 0., 0.);
        let blue = probe_sample_luminance(0., 0., 1.);
        assert!(green > red && red > blue);
    }

    #[test]
    fn a_bevel_without_refraction_bends_nothing() {
        let sloped = GlassMaterial {
            bevel: ScaledPixels(12.),
            ..GlassMaterial::frosted()
        };
        assert!(!sloped.bends_light(), "a slope with no index is a pane");

        let dense = GlassMaterial::<ScaledPixels> {
            refraction: 0.4,
            ..GlassMaterial::frosted()
        };
        assert!(!dense.bends_light(), "an index with no slope is a pane");
    }

    #[test]
    fn one_lobe_agrees_with_the_rounded_rect_it_is() {
        let lobe = test_lobe((0., 0.), (100., 60.), 10.);

        for (at, expected) in [
            (point(50., 30.), -30.),
            (point(0., 30.), 0.),
            (point(50., 0.), 0.),
            (point(-10., 30.), 10.),
        ] {
            let field = glass_field(at, &[lobe], 0.);
            assert!(
                (field.distance - expected).abs() < 0.001,
                "at {at:?} expected {expected} but got {}",
                field.distance
            );
        }
    }

    #[test]
    fn the_gradient_points_out_of_the_surface() {
        let lobe = test_lobe((0., 0.), (100., 60.), 0.);

        let left = glass_field(point(10., 30.), &[lobe], 0.);
        assert!(left.gradient.x < -0.9, "the near edge is to the left");

        let right = glass_field(point(90., 30.), &[lobe], 0.);
        assert!(right.gradient.x > 0.9, "the near edge is to the right");

        let top = glass_field(point(50., 5.), &[lobe], 0.);
        assert!(top.gradient.y < -0.9, "the near edge is above");
    }

    #[test]
    fn smoothing_bridges_the_gap_between_two_lobes() {
        let left = test_lobe((0., 0.), (40., 40.), 8.);
        let right = test_lobe((60., 0.), (40., 40.), 8.);
        let between = point(50., 20.);

        let creased = glass_field(between, &[left, right], 0.);
        assert!(
            creased.distance > 0.,
            "with no smoothing the gap is outside both lobes"
        );

        let joined = glass_field(between, &[left, right], 40.);
        assert!(
            joined.distance < creased.distance,
            "smoothing pulls the surface into the gap"
        );
    }

    #[test]
    fn a_smooth_minimum_of_zero_is_an_ordinary_minimum() {
        assert_eq!(glass_smooth_min(3., 7., 0.), 3.);
        assert_eq!(glass_smooth_min(7., 3., 0.), 3.);
        assert!(glass_smooth_min(3., 7., 8.) < 3., "smoothing only deepens");
    }

    #[test]
    fn a_shape_with_no_lobes_is_nowhere_rather_than_everywhere() {
        let field = glass_field(point(0., 0.), &[], 0.);
        assert_eq!(field.distance, f32::MAX);
        assert_eq!(field.gradient, point(0., 0.));
    }

    #[test]
    fn the_gpu_facing_structs_carry_no_compiler_inserted_padding() {
        use std::mem::size_of;

        assert_eq!(
            size_of::<Background>(),
            size_of::<BackgroundTag>()
                + size_of::<ColorSpace>()
                + size_of::<Hsla>()
                + size_of::<f32>()
                + MAX_LINEAR_GRADIENT_STOPS * size_of::<LinearColorStop>()
                + size_of::<u32>()
        );
        assert_eq!(size_of::<GlassLobe>(), 8 * size_of::<f32>());
        assert_eq!(size_of::<GlassMaterial>(), 8 * size_of::<f32>());
        assert_eq!(
            size_of::<BackdropGlass>(),
            size_of::<DrawOrder>()
                + size_of::<ScaledPixels>()
                + size_of::<Bounds<ScaledPixels>>()
                + size_of::<ContentMask<ScaledPixels>>()
                + size_of::<Corners<ScaledPixels>>()
                + size_of::<GlassMaterial>()
                + MAX_GLASS_LOBES * size_of::<GlassLobe>()
                + size_of::<u32>()
        );
    }

    #[test]
    fn backdrop_glass_splits_same_kind_primitive_batches() {
        let shadow = |order| Shadow {
            order,
            blur_radius: ScaledPixels::default(),
            bounds: Bounds::default(),
            corner_radii: Corners::default(),
            content_mask: ContentMask::default(),
            color: Hsla::default(),
            element_bounds: Bounds::default(),
            element_corner_radii: Corners::default(),
            inset: 0,
            pad: 0,
        };
        let mut scene = Scene {
            shadows: vec![shadow(1), shadow(2), shadow(4)],
            backdrop_glass: vec![BackdropGlass {
                order: 3,
                blur_radius: ScaledPixels::default(),
                bounds: Bounds::default(),
                content_mask: ContentMask::default(),
                corner_radii: Corners::default(),
                material: GlassMaterial::frosted(),
                lobes: [GlassLobe::default(); MAX_GLASS_LOBES],
                lobe_count: 0,
            }],
            ..Scene::default()
        };
        scene.finish();

        let batches = scene.batches().collect::<Vec<_>>();
        assert!(matches!(
            batches.as_slice(),
            [PrimitiveBatch::Shadows(first), PrimitiveBatch::Shadows(second)]
                if first == &(0..2) && second == &(2..3)
        ));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Default)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
pub(crate) enum PrimitiveKind {
    Shadow,
    #[default]
    Quad,
    Path,
    Underline,
    MonochromeSprite,
    SubpixelSprite,
    PolychromeSprite,
    Surface,
}

pub(crate) enum PaintOperation {
    Primitive(Primitive),
    BackdropGlass(BackdropGlass),
    StartLayer(Bounds<ScaledPixels>),
    EndLayer,
}

#[derive(Clone)]
#[expect(missing_docs)]
pub enum Primitive {
    Shadow(Shadow),
    Quad(Quad),
    Path(Path<ScaledPixels>),
    Underline(Underline),
    MonochromeSprite(MonochromeSprite),
    SubpixelSprite(SubpixelSprite),
    PolychromeSprite(PolychromeSprite),
    Surface(PaintSurface),
}

#[expect(missing_docs)]
impl Primitive {
    pub fn bounds(&self) -> &Bounds<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.bounds,
            Primitive::Quad(quad) => &quad.bounds,
            Primitive::Path(path) => &path.bounds,
            Primitive::Underline(underline) => &underline.bounds,
            Primitive::MonochromeSprite(sprite) => &sprite.bounds,
            Primitive::SubpixelSprite(sprite) => &sprite.bounds,
            Primitive::PolychromeSprite(sprite) => &sprite.bounds,
            Primitive::Surface(surface) => &surface.bounds,
        }
    }

    pub fn content_mask(&self) -> &ContentMask<ScaledPixels> {
        match self {
            Primitive::Shadow(shadow) => &shadow.content_mask,
            Primitive::Quad(quad) => &quad.content_mask,
            Primitive::Path(path) => &path.content_mask,
            Primitive::Underline(underline) => &underline.content_mask,
            Primitive::MonochromeSprite(sprite) => &sprite.content_mask,
            Primitive::SubpixelSprite(sprite) => &sprite.content_mask,
            Primitive::PolychromeSprite(sprite) => &sprite.content_mask,
            Primitive::Surface(surface) => &surface.content_mask,
        }
    }
}

#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
struct BatchIterator<'a> {
    shadows_start: usize,
    shadows_iter: Peekable<slice::Iter<'a, Shadow>>,
    quads_start: usize,
    quads_iter: Peekable<slice::Iter<'a, Quad>>,
    paths_start: usize,
    paths_iter: Peekable<slice::Iter<'a, Path<ScaledPixels>>>,
    underlines_start: usize,
    underlines_iter: Peekable<slice::Iter<'a, Underline>>,
    monochrome_sprites_start: usize,
    monochrome_sprites_iter: Peekable<slice::Iter<'a, MonochromeSprite>>,
    subpixel_sprites_start: usize,
    subpixel_sprites_iter: Peekable<slice::Iter<'a, SubpixelSprite>>,
    polychrome_sprites_start: usize,
    polychrome_sprites_iter: Peekable<slice::Iter<'a, PolychromeSprite>>,
    surfaces_start: usize,
    surfaces_iter: Peekable<slice::Iter<'a, PaintSurface>>,
    backdrop_glass_iter: Peekable<slice::Iter<'a, BackdropGlass>>,
}

impl<'a> Iterator for BatchIterator<'a> {
    type Item = PrimitiveBatch;

    fn next(&mut self) -> Option<Self::Item> {
        let mut orders_and_kinds = [
            (
                self.shadows_iter.peek().map(|s| s.order),
                PrimitiveKind::Shadow,
            ),
            (self.quads_iter.peek().map(|q| q.order), PrimitiveKind::Quad),
            (self.paths_iter.peek().map(|q| q.order), PrimitiveKind::Path),
            (
                self.underlines_iter.peek().map(|u| u.order),
                PrimitiveKind::Underline,
            ),
            (
                self.monochrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::MonochromeSprite,
            ),
            (
                self.subpixel_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::SubpixelSprite,
            ),
            (
                self.polychrome_sprites_iter.peek().map(|s| s.order),
                PrimitiveKind::PolychromeSprite,
            ),
            (
                self.surfaces_iter.peek().map(|s| s.order),
                PrimitiveKind::Surface,
            ),
        ];
        orders_and_kinds.sort_by_key(|(order, kind)| (order.unwrap_or(u32::MAX), *kind));

        let first = orders_and_kinds[0];
        let second = orders_and_kinds[1];
        while self
            .backdrop_glass_iter
            .next_if(|glass| first.0.is_some_and(|order| glass.order <= order))
            .is_some()
        {}
        let next_glass_order = self
            .backdrop_glass_iter
            .peek()
            .map_or(u32::MAX, |glass| glass.order);
        let (batch_kind, max_order_and_kind) = if first.0.is_some() {
            (first.1, (second.0.unwrap_or(u32::MAX), second.1))
        } else {
            return None;
        };

        match batch_kind {
            PrimitiveKind::Shadow => {
                let shadows_start = self.shadows_start;
                let mut shadows_end = shadows_start + 1;
                self.shadows_iter.next();
                while self
                    .shadows_iter
                    .next_if(|shadow| {
                        shadow.order < next_glass_order
                            && (shadow.order, batch_kind) < max_order_and_kind
                    })
                    .is_some()
                {
                    shadows_end += 1;
                }
                self.shadows_start = shadows_end;
                Some(PrimitiveBatch::Shadows(shadows_start..shadows_end))
            }
            PrimitiveKind::Quad => {
                let quads_start = self.quads_start;
                let mut quads_end = quads_start + 1;
                self.quads_iter.next();
                while self
                    .quads_iter
                    .next_if(|quad| {
                        quad.order < next_glass_order
                            && (quad.order, batch_kind) < max_order_and_kind
                    })
                    .is_some()
                {
                    quads_end += 1;
                }
                self.quads_start = quads_end;
                Some(PrimitiveBatch::Quads(quads_start..quads_end))
            }
            PrimitiveKind::Path => {
                let paths_start = self.paths_start;
                let mut paths_end = paths_start + 1;
                self.paths_iter.next();
                while self
                    .paths_iter
                    .next_if(|path| {
                        path.order < next_glass_order
                            && (path.order, batch_kind) < max_order_and_kind
                    })
                    .is_some()
                {
                    paths_end += 1;
                }
                self.paths_start = paths_end;
                Some(PrimitiveBatch::Paths(paths_start..paths_end))
            }
            PrimitiveKind::Underline => {
                let underlines_start = self.underlines_start;
                let mut underlines_end = underlines_start + 1;
                self.underlines_iter.next();
                while self
                    .underlines_iter
                    .next_if(|underline| {
                        underline.order < next_glass_order
                            && (underline.order, batch_kind) < max_order_and_kind
                    })
                    .is_some()
                {
                    underlines_end += 1;
                }
                self.underlines_start = underlines_end;
                Some(PrimitiveBatch::Underlines(underlines_start..underlines_end))
            }
            PrimitiveKind::MonochromeSprite => {
                let texture_id = self
                    .monochrome_sprites_iter
                    .peek()
                    .expect("required framework invariant must hold")
                    .tile
                    .texture_id;
                let sprites_start = self.monochrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.monochrome_sprites_iter.next();
                while self
                    .monochrome_sprites_iter
                    .next_if(|sprite| {
                        sprite.order < next_glass_order
                            && (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.monochrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::MonochromeSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::SubpixelSprite => {
                let texture_id = self
                    .subpixel_sprites_iter
                    .peek()
                    .expect("required framework invariant must hold")
                    .tile
                    .texture_id;
                let sprites_start = self.subpixel_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.subpixel_sprites_iter.next();
                while self
                    .subpixel_sprites_iter
                    .next_if(|sprite| {
                        sprite.order < next_glass_order
                            && (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.subpixel_sprites_start = sprites_end;
                Some(PrimitiveBatch::SubpixelSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::PolychromeSprite => {
                let texture_id = self
                    .polychrome_sprites_iter
                    .peek()
                    .expect("required framework invariant must hold")
                    .tile
                    .texture_id;
                let sprites_start = self.polychrome_sprites_start;
                let mut sprites_end = sprites_start + 1;
                self.polychrome_sprites_iter.next();
                while self
                    .polychrome_sprites_iter
                    .next_if(|sprite| {
                        sprite.order < next_glass_order
                            && (sprite.order, batch_kind) < max_order_and_kind
                            && sprite.tile.texture_id == texture_id
                    })
                    .is_some()
                {
                    sprites_end += 1;
                }
                self.polychrome_sprites_start = sprites_end;
                Some(PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    range: sprites_start..sprites_end,
                })
            }
            PrimitiveKind::Surface => {
                let surfaces_start = self.surfaces_start;
                let mut surfaces_end = surfaces_start + 1;
                self.surfaces_iter.next();
                while self
                    .surfaces_iter
                    .next_if(|surface| {
                        surface.order < next_glass_order
                            && (surface.order, batch_kind) < max_order_and_kind
                    })
                    .is_some()
                {
                    surfaces_end += 1;
                }
                self.surfaces_start = surfaces_end;
                Some(PrimitiveBatch::Surfaces(surfaces_start..surfaces_end))
            }
        }
    }
}

#[derive(Debug)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
#[allow(missing_docs)]
pub enum PrimitiveBatch {
    Shadows(Range<usize>),
    Quads(Range<usize>),
    Paths(Range<usize>),
    Underlines(Range<usize>),
    MonochromeSprites {
        texture_id: AtlasTextureId,
        range: Range<usize>,
    },
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    SubpixelSprites {
        texture_id: AtlasTextureId,
        range: Range<usize>,
    },
    PolychromeSprites {
        texture_id: AtlasTextureId,
        range: Range<usize>,
    },
    Surfaces(Range<usize>),
}

impl PrimitiveBatch {
    #[expect(missing_docs)]
    pub fn label(&self) -> String {
        match self {
            Self::Shadows(range) => format!("shadows ({})", range.len()),
            Self::Quads(range) => format!("quads ({})", range.len()),
            Self::Paths(range) => format!("paths ({})", range.len()),
            Self::Underlines(range) => format!("underlines ({})", range.len()),
            Self::MonochromeSprites { texture_id, range } => {
                format!(
                    "monochrome sprites ({}) on atlas {}",
                    range.len(),
                    texture_id.index
                )
            }
            Self::SubpixelSprites { texture_id, range } => {
                format!(
                    "subpixel sprites ({}) on atlas {}",
                    range.len(),
                    texture_id.index
                )
            }
            Self::PolychromeSprites { texture_id, range } => {
                format!(
                    "polychrome sprites ({}) on atlas {}",
                    range.len(),
                    texture_id.index
                )
            }
            Self::Surfaces(range) => format!("surfaces ({})", range.len()),
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Quad {
    pub order: DrawOrder,
    pub border_style: BorderStyle,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners<ScaledPixels>,
    pub border_widths: Edges<ScaledPixels>,
}

impl From<Quad> for Primitive {
    fn from(quad: Quad) -> Self {
        Primitive::Quad(quad)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Underline {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub thickness: ScaledPixels,
    pub wavy: PaddedBool32,
}

impl From<Underline> for Primitive {
    fn from(underline: Underline) -> Self {
        Primitive::Underline(underline)
    }
}

/// How many rounded rectangles one glass surface may be made of.
///
/// The shape is evaluated per fragment and per hit test, so the count is
/// bounded rather than allocated: a loop a shader can unroll is the reason
/// this is a fixed array inside the instance rather than a second buffer.
pub const MAX_GLASS_LOBES: usize = 8;

/// The value of [`GlassMaterial::probe`] that asks for no luminance probe.
pub const NO_LUMINANCE_PROBE: u32 = u32::MAX;

/// How many luminance probe slots a window carries.
///
/// A probe is a slot a glass surface fills each frame and a caller reads a
/// frame later, so the count bounds the readback buffer every renderer keeps,
/// the same way [`MAX_GLASS_LOBES`] bounds the instance.
pub const MAX_LUMINANCE_PROBES: usize = 16;

/// How many points of the blurred backdrop one probe averages.
pub const LUMINANCE_PROBE_SAMPLES: usize = 5;

/// The relative luminance of one probe sample, from encoded channel values in
/// `0..=1`.
///
/// The weights are Rec. 709. The values are the encoded bytes the framebuffer
/// holds rather than linearized light, which every renderer shares, so a probe
/// means the same thing on each of them: a perceptual reading, not a photometric
/// one.
pub fn probe_sample_luminance(red: f32, green: f32, blue: f32) -> f32 {
    red * 0.2126 + green * 0.7152 + blue * 0.0722
}

/// A length carried by a glass surface, in whichever pixel unit the surface
/// is expressed in.
///
/// [`GlassMaterial`] and [`GlassLobe`] are parameterised over their unit for
/// the same reason [`Bounds`] is: a caller states a surface in logical pixels
/// and [`crate::Window::paint_backdrop_glass`] scales it, so the two are
/// different types and the conversion is the one operation that turns one
/// into the other. This trait is what lets the shared arithmetic be written
/// once instead of twice.
pub trait GlassLength: Copy + Default + std::fmt::Debug + PartialEq {
    /// The length as a bare pixel count.
    fn raw(self) -> f32;
    /// A length of this unit from a bare pixel count.
    fn from_raw(raw: f32) -> Self;
}

impl GlassLength for Pixels {
    fn raw(self) -> f32 {
        self.0
    }

    fn from_raw(raw: f32) -> Self {
        Pixels(raw)
    }
}

impl GlassLength for ScaledPixels {
    fn raw(self) -> f32 {
        self.0
    }

    fn from_raw(raw: f32) -> Self {
        ScaledPixels(raw)
    }
}

/// One rounded rectangle of a glass surface's shape.
///
/// A surface with a single lobe is an ordinary rounded rect. Several lobes
/// are combined by a smooth minimum, so two that come within
/// [`GlassMaterial::smoothing`] of each other join into one body instead of
/// overlapping as two outlines.
#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[repr(C)]
#[expect(missing_docs)]
pub struct GlassLobe<P: GlassLength = ScaledPixels> {
    pub bounds: Bounds<P>,
    pub corner_radii: Corners<P>,
}

impl GlassLobe<Pixels> {
    /// The same lobe in device pixels.
    pub fn scale(self, factor: f32) -> GlassLobe<ScaledPixels> {
        GlassLobe {
            bounds: self.bounds.scale(factor),
            corner_radii: self.corner_radii.scale(factor),
        }
    }
}

/// The optical response of a glass surface, over and above blurring what is
/// behind it.
///
/// [`GlassMaterial::frosted()`] is all zeroes and asks for none of it, which is
/// what [`crate::Window::paint_backdrop_blur`] paints: the backdrop is blurred
/// and nothing is bent, split or lit. Every field is independent, so a caller
/// may take refraction without dispersion, or a specular rim without either.
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct GlassMaterial<P = ScaledPixels> {
    /// How far in from the edge the bevel that bends the backdrop reaches.
    /// Zero leaves the backdrop flat however large the other fields are,
    /// because there is no slope for them to act on.
    pub bevel: P,
    /// How far the bevel displaces the sample, as a fraction of `bevel`. This
    /// is a thickness in disguise: 0 is a flat pane and larger values read as
    /// a deeper body of glass.
    pub refraction: f32,
    /// How far apart the red and blue samples land, as a fraction of the
    /// refraction offset. Zero samples all three channels together.
    pub dispersion: f32,
    /// Peak brightness of the rim highlight, 0 for none.
    pub specular: f32,
    /// Where the light is, in radians clockwise from straight up.
    pub light_angle: f32,
    /// How tight the specular lobe is. Larger is a smaller, harder highlight.
    pub specular_sharpness: f32,
    /// How far apart two lobes may be and still join. Zero makes the union a
    /// plain minimum, so lobes meet at a crease.
    pub smoothing: P,
    /// Which luminance probe slot this surface fills, or [`NO_LUMINANCE_PROBE`].
    ///
    /// A probed surface has the mean luminance of the backdrop it blurred
    /// reported back through [`crate::Window::backdrop_luminance`], one frame
    /// later. See that method for what the delay means for a caller.
    pub probe: u32,
}

impl<P: GlassLength> GlassMaterial<P> {
    /// Blur the backdrop and bend nothing: the material a plain frosted
    /// surface is made of.
    ///
    /// This is a function rather than a constant because the zero length
    /// depends on the unit; every field in it is nevertheless fixed.
    pub fn frosted() -> Self {
        Self {
            bevel: P::from_raw(0.),
            refraction: 0.,
            dispersion: 0.,
            specular: 0.,
            light_angle: 0.,
            specular_sharpness: 1.,
            smoothing: P::from_raw(0.),
            probe: NO_LUMINANCE_PROBE,
        }
    }

    /// Whether this material asks the renderer for anything a blur alone does
    /// not already do. A renderer without the optics may skip them and still
    /// be painting what the caller asked for when this is false.
    pub fn is_flat(&self) -> bool {
        !self.bends_light() && self.specular <= 0.
    }

    /// Whether the backdrop sample is displaced at all.
    pub fn bends_light(&self) -> bool {
        self.bevel.raw() > 0. && self.refraction != 0.
    }

    /// Replaces every field that a renderer could not act on with one it can:
    /// non-finite values become zero, and negative values that have no
    /// meaning below zero are clamped there. A caller that computed a
    /// material from an animation gets a legible surface rather than a hole.
    pub fn sanitized(mut self) -> Self {
        fn finite(value: f32, fallback: f32) -> f32 {
            if value.is_finite() { value } else { fallback }
        }
        self.bevel = P::from_raw(finite(self.bevel.raw(), 0.).max(0.));
        self.refraction = finite(self.refraction, 0.);
        self.dispersion = finite(self.dispersion, 0.).clamp(0., 1.);
        self.specular = finite(self.specular, 0.).max(0.);
        self.light_angle = finite(self.light_angle, 0.);
        self.specular_sharpness = finite(self.specular_sharpness, 1.).max(1.);
        self.smoothing = P::from_raw(finite(self.smoothing.raw(), 0.).max(0.));
        self
    }
}

impl GlassMaterial<Pixels> {
    /// The same material in device pixels. Only the two lengths change: every
    /// other field is a ratio, an angle or a slot index, and means the same
    /// thing at any scale.
    pub fn scale(self, factor: f32) -> GlassMaterial<ScaledPixels> {
        GlassMaterial {
            bevel: self.bevel.scale(factor),
            refraction: self.refraction,
            dispersion: self.dispersion,
            specular: self.specular,
            light_angle: self.light_angle,
            specular_sharpness: self.specular_sharpness,
            smoothing: self.smoothing.scale(factor),
            probe: self.probe,
        }
    }
}

impl<P: GlassLength> Default for GlassMaterial<P> {
    fn default() -> Self {
        Self::frosted()
    }
}

/// A within-window glass surface: the renderer snapshots everything painted
/// below this order, blurs it, and paints it back through the surface's
/// shape and material. See [`crate::Window::paint_backdrop_glass`].
#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct BackdropGlass {
    pub order: DrawOrder,
    pub blur_radius: ScaledPixels,
    /// The region the surface occupies. With more than one lobe this is the
    /// union's bounding box, which is what the renderer rasterizes and what
    /// clips the blur; the shape inside it comes from `lobes`.
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    /// The rounding used when `lobe_count` is 0.
    pub corner_radii: Corners<ScaledPixels>,
    pub material: GlassMaterial<ScaledPixels>,
    /// The length is written out rather than named so that the C header
    /// generated for the Metal shaders carries a literal bound, which needs no
    /// integer typedef and no include. [`MAX_GLASS_LOBES`] is asserted equal
    /// to it below, so the two cannot drift.
    pub lobes: [GlassLobe<ScaledPixels>; 8],
    /// How many entries of `lobes` are real. Zero means the surface is the
    /// single rounded rect named by `bounds` and `corner_radii`.
    pub lobe_count: u32,
}

const _: () = assert!(
    MAX_GLASS_LOBES == 8,
    "the lobe array's literal length and MAX_GLASS_LOBES must agree"
);

impl BackdropGlass {
    /// The lobes that make up the shape, which is the explicit list when
    /// there is one and the surface's own rounded rect when there is not.
    ///
    /// Callers that evaluate the shape must go through this rather than
    /// reading `lobes` directly, so that the single-lobe case cannot drift
    /// away from the many-lobe case.
    pub fn shape(&self) -> ([GlassLobe; MAX_GLASS_LOBES], usize) {
        if self.lobe_count == 0 {
            let mut lobes = [GlassLobe::default(); MAX_GLASS_LOBES];
            lobes[0] = GlassLobe {
                bounds: self.bounds,
                corner_radii: self.corner_radii,
            };
            (lobes, 1)
        } else {
            (self.lobes, (self.lobe_count as usize).min(MAX_GLASS_LOBES))
        }
    }

    /// Where this surface's luminance probe samples the blurred backdrop:
    /// the centre of the surface and the four quarter points, in device
    /// pixels, each clamped inside a `width` by `height` texture.
    ///
    /// Five points rather than one because a probe summarises the whole
    /// surface: the blur has already averaged each point's neighbourhood, so
    /// a sparse cross is what stops a single bright stripe under the centre
    /// from speaking for the corners. Every renderer copies exactly these
    /// texels, which is what makes one probe value mean the same thing on
    /// each of them.
    pub fn probe_sample_points(
        &self,
        width: f32,
        height: f32,
    ) -> [[f32; 2]; LUMINANCE_PROBE_SAMPLES] {
        let centre_x = self.bounds.origin.x.0 + self.bounds.size.width.0 / 2.0;
        let centre_y = self.bounds.origin.y.0 + self.bounds.size.height.0 / 2.0;
        let step_x = self.bounds.size.width.0 / 4.0;
        let step_y = self.bounds.size.height.0 / 4.0;
        let clamp = |x: f32, y: f32| {
            [
                x.clamp(0.0, (width - 1.0).max(0.0)).floor(),
                y.clamp(0.0, (height - 1.0).max(0.0)).floor(),
            ]
        };
        [
            clamp(centre_x, centre_y),
            clamp(centre_x - step_x, centre_y - step_y),
            clamp(centre_x + step_x, centre_y - step_y),
            clamp(centre_x - step_x, centre_y + step_y),
            clamp(centre_x + step_x, centre_y + step_y),
        ]
    }

    /// How many separable gaussian passes this surface's blur needs, or
    /// `None` when it needs more than [`MAX_GLASS_GAUSSIAN_PASSES`] and the
    /// renderer should leave the backdrop unblurred rather than spend an
    /// unbounded amount of the frame on it.
    ///
    /// A renderer that hands the blur to the platform (Metal, through
    /// `MPSImageGaussianBlur`) has no use for this. The two that convolve it
    /// themselves both do, and they share this rather than each deriving it,
    /// because the number is a property of the shader's tap budget and the
    /// two shaders have the same one.
    pub fn gaussian_pass_count(&self) -> Option<u32> {
        let radius = self.blur_radius.0;
        if !radius.is_finite() || radius < 0. {
            return None;
        }

        let sigma = radius.max(1.);
        let passes = (sigma * sigma / MAX_GLASS_SIGMA_PER_PASS.powi(2))
            .ceil()
            .max(1.) as u32;
        (passes <= MAX_GLASS_GAUSSIAN_PASSES).then_some(passes)
    }
}

/// The largest standard deviation one separable gaussian pass can carry.
///
/// The shaders take 64 taps either side of centre and a gaussian is spent by
/// three standard deviations, so a pass wider than this is sampling a kernel
/// it has already run out of room for. Wider blurs are split into several
/// passes, whose variances add.
pub const MAX_GLASS_SIGMA_PER_PASS: f32 = 64. / 3.;

/// The most gaussian passes one glass surface may be given.
pub const MAX_GLASS_GAUSSIAN_PASSES: u32 = 16;

/// The shape of a glass surface at a point: how far outside it the point is,
/// and which way the surface falls away from there.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct GlassField {
    /// Signed distance to the surface's edge, negative inside.
    pub distance: f32,
    /// The direction the distance increases in, normalized. Zero-length where
    /// the field is flat, which happens at the exact centre of a lobe.
    pub gradient: Point<f32>,
}

/// Signed distance from `point` to one lobe, negative inside.
///
/// This mirrors `quad_sdf` in the shaders exactly, including the unrounded
/// fast path, because the two must not disagree about where an edge is.
pub fn glass_lobe_sdf(point: Point<f32>, lobe: &GlassLobe) -> f32 {
    let half_width = lobe.bounds.size.width.0 / 2.;
    let half_height = lobe.bounds.size.height.0 / 2.;
    let center_x = lobe.bounds.origin.x.0 + half_width;
    let center_y = lobe.bounds.origin.y.0 + half_height;
    let to_center_x = point.x - center_x;
    let to_center_y = point.y - center_y;

    let radius = if to_center_x < 0. {
        if to_center_y < 0. {
            lobe.corner_radii.top_left.0
        } else {
            lobe.corner_radii.bottom_left.0
        }
    } else if to_center_y < 0. {
        lobe.corner_radii.top_right.0
    } else {
        lobe.corner_radii.bottom_right.0
    };

    let corner_x = to_center_x.abs() - half_width + radius;
    let corner_y = to_center_y.abs() - half_height + radius;
    if radius == 0. {
        return corner_x.max(corner_y);
    }
    let outside = (corner_x.max(0.).powi(2) + corner_y.max(0.).powi(2)).sqrt();
    outside + corner_x.max(corner_y).min(0.) - radius
}

/// The polynomial smooth minimum, which is what makes two lobes join into one
/// body rather than cross as two outlines.
///
/// `smoothing` of zero is an ordinary minimum, so a caller that wants a crease
/// gets one without a second code path.
pub fn glass_smooth_min(a: f32, b: f32, smoothing: f32) -> f32 {
    if smoothing <= 0. {
        return a.min(b);
    }
    let h = (smoothing - (a - b).abs()).max(0.) / smoothing;
    a.min(b) - h * h * smoothing * 0.25
}

/// The distance to the union of `lobes` and the direction it increases in.
///
/// The gradient is taken by central differences rather than analytically. A
/// smooth minimum's analytic gradient is a weighted sum whose weights each of
/// the three shading languages would have to reproduce, and the four
/// implementations disagreeing about a normal is exactly the kind of drift
/// this function exists to prevent. Differencing is the same four extra
/// evaluations everywhere, and its result is defined by this function's own
/// output rather than by a derivation done four times.
pub fn glass_field(at: Point<f32>, lobes: &[GlassLobe], smoothing: f32) -> GlassField {
    /// Half the width of the differencing stencil, in device pixels. Below
    /// half a pixel the difference is dominated by float error near a corner.
    const EPSILON: f32 = 0.5;

    fn union(at: Point<f32>, lobes: &[GlassLobe], smoothing: f32) -> f32 {
        let mut distance = f32::MAX;
        for (index, lobe) in lobes.iter().enumerate() {
            let lobe_distance = glass_lobe_sdf(at, lobe);
            distance = if index == 0 {
                lobe_distance
            } else {
                glass_smooth_min(distance, lobe_distance, smoothing)
            };
        }
        distance
    }

    if lobes.is_empty() {
        return GlassField {
            distance: f32::MAX,
            gradient: point(0., 0.),
        };
    }

    let distance = union(at, lobes, smoothing);
    let dx = union(point(at.x + EPSILON, at.y), lobes, smoothing)
        - union(point(at.x - EPSILON, at.y), lobes, smoothing);
    let dy = union(point(at.x, at.y + EPSILON), lobes, smoothing)
        - union(point(at.x, at.y - EPSILON), lobes, smoothing);
    let length = (dx * dx + dy * dy).sqrt();
    let gradient = if length > 0. {
        point(dx / length, dy / length)
    } else {
        point(0., 0.)
    };
    GlassField { distance, gradient }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[expect(missing_docs)]
pub struct Shadow {
    pub order: DrawOrder,
    pub blur_radius: ScaledPixels,
    pub bounds: Bounds<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub element_bounds: Bounds<ScaledPixels>,
    pub element_corner_radii: Corners<ScaledPixels>,
    /// 0 = drop shadow (rendered outside the element), 1 = inset shadow (rendered inside).
    pub inset: u32,
    pub pad: u32, // align to 8 bytes
}

impl From<Shadow> for Primitive {
    fn from(shadow: Shadow) -> Self {
        Primitive::Shadow(shadow)
    }
}

/// The style of a border.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[repr(C)]
pub enum BorderStyle {
    /// A solid border.
    #[default]
    Solid = 0,
    /// A dashed border.
    Dashed = 1,
}

/// A data type representing a 2 dimensional transformation that can be applied to an element.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct TransformationMatrix {
    /// 2x2 matrix containing rotation and scale,
    /// stored row-major
    pub rotation_scale: [[f32; 2]; 2],
    /// translation vector
    pub translation: [f32; 2],
}

impl Eq for TransformationMatrix {}

impl TransformationMatrix {
    /// The unit matrix, has no effect.
    pub fn unit() -> Self {
        Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [0.0, 0.0],
        }
    }

    /// Move the origin by a given point
    pub fn translate(mut self, point: Point<ScaledPixels>) -> Self {
        self.compose(Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [point.x.0, point.y.0],
        })
    }

    /// Clockwise rotation in radians around the origin
    pub fn rotate(self, angle: Radians) -> Self {
        self.compose(Self {
            rotation_scale: [
                [angle.0.cos(), -angle.0.sin()],
                [angle.0.sin(), angle.0.cos()],
            ],
            translation: [0.0, 0.0],
        })
    }

    /// Scale around the origin
    pub fn scale(self, size: Size<f32>) -> Self {
        self.compose(Self {
            rotation_scale: [[size.width, 0.0], [0.0, size.height]],
            translation: [0.0, 0.0],
        })
    }

    /// Perform matrix multiplication with another transformation
    /// to produce a new transformation that is the result of
    /// applying both transformations: first, `other`, then `self`.
    #[inline]
    pub fn compose(self, other: TransformationMatrix) -> TransformationMatrix {
        if other == Self::unit() {
            return self;
        }
        // Perform matrix multiplication
        TransformationMatrix {
            rotation_scale: [
                [
                    self.rotation_scale[0][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][0],
                    self.rotation_scale[0][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[0][1] * other.rotation_scale[1][1],
                ],
                [
                    self.rotation_scale[1][0] * other.rotation_scale[0][0]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][0],
                    self.rotation_scale[1][0] * other.rotation_scale[0][1]
                        + self.rotation_scale[1][1] * other.rotation_scale[1][1],
                ],
            ],
            translation: [
                self.translation[0]
                    + self.rotation_scale[0][0] * other.translation[0]
                    + self.rotation_scale[0][1] * other.translation[1],
                self.translation[1]
                    + self.rotation_scale[1][0] * other.translation[0]
                    + self.rotation_scale[1][1] * other.translation[1],
            ],
        }
    }

    /// Apply transformation to a point, mainly useful for debugging
    pub fn apply(&self, point: Point<Pixels>) -> Point<Pixels> {
        let input = [point.x.0, point.y.0];
        let mut output = self.translation;
        for (i, output_cell) in output.iter_mut().enumerate() {
            for (k, input_cell) in input.iter().enumerate() {
                *output_cell += self.rotation_scale[i][k] * *input_cell;
            }
        }
        Point::new(output[0].into(), output[1].into())
    }
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct MonochromeSprite {
    pub order: DrawOrder,
    pub pad: u32,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub tile: AtlasTile,
    pub transformation: TransformationMatrix,
}

impl From<MonochromeSprite> for Primitive {
    fn from(sprite: MonochromeSprite) -> Self {
        Primitive::MonochromeSprite(sprite)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct SubpixelSprite {
    pub order: DrawOrder,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub color: Hsla,
    pub tile: AtlasTile,
    pub transformation: TransformationMatrix,
}

impl From<SubpixelSprite> for Primitive {
    fn from(sprite: SubpixelSprite) -> Self {
        Primitive::SubpixelSprite(sprite)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PolychromeSprite {
    pub order: DrawOrder,
    pub pad: u32,
    pub grayscale: PaddedBool32,
    pub opacity: f32,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    pub corner_radii: Corners<ScaledPixels>,
    pub tile: AtlasTile,
}

impl From<PolychromeSprite> for Primitive {
    fn from(sprite: PolychromeSprite) -> Self {
        Primitive::PolychromeSprite(sprite)
    }
}

#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub struct PaintSurface {
    pub order: DrawOrder,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    #[cfg(target_os = "macos")]
    pub image_buffer: core_video::pixel_buffer::CVPixelBuffer,
}

impl From<PaintSurface> for Primitive {
    fn from(surface: PaintSurface) -> Self {
        Primitive::Surface(surface)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[expect(missing_docs)]
pub struct PathId(pub usize);

/// A line made up of a series of vertices and control points.
#[derive(Clone, Debug)]
#[expect(missing_docs)]
pub struct Path<P: Clone + Debug + Default + PartialEq> {
    pub id: PathId,
    pub order: DrawOrder,
    pub bounds: Bounds<P>,
    pub content_mask: ContentMask<P>,
    pub vertices: Vec<PathVertex<P>>,
    pub color: Background,
    start: Point<P>,
    current: Point<P>,
    contour_count: usize,
}

impl Path<Pixels> {
    /// Create a new path with the given starting point.
    pub fn new(start: Point<Pixels>) -> Self {
        Self {
            id: PathId(0),
            order: DrawOrder::default(),
            vertices: Vec::new(),
            start,
            current: start,
            bounds: Bounds {
                origin: start,
                size: Default::default(),
            },
            content_mask: Default::default(),
            color: Default::default(),
            contour_count: 0,
        }
    }

    /// Scale this path by the given factor.
    pub fn scale(&self, factor: f32) -> Path<ScaledPixels> {
        Path {
            id: self.id,
            order: self.order,
            bounds: self.bounds.scale(factor),
            content_mask: self.content_mask.scale(factor),
            vertices: self
                .vertices
                .iter()
                .map(|vertex| vertex.scale(factor))
                .collect(),
            start: self.start.map(|start| start.scale(factor)),
            current: self.current.scale(factor),
            contour_count: self.contour_count,
            color: self.color,
        }
    }

    /// Move the start, current point to the given point.
    pub fn move_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        self.start = to;
        self.current = to;
    }

    /// Draw a straight line from the current point to the given point.
    pub fn line_to(&mut self, to: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }
        self.current = to;
    }

    /// Draw a curve from the current point to the given point, using the given control point.
    pub fn curve_to(&mut self, to: Point<Pixels>, ctrl: Point<Pixels>) {
        self.contour_count += 1;
        if self.contour_count > 1 {
            self.push_triangle(
                (self.start, self.current, to),
                (point(0., 1.), point(0., 1.), point(0., 1.)),
            );
        }

        self.push_triangle(
            (self.current, ctrl, to),
            (point(0., 0.), point(0.5, 0.), point(1., 1.)),
        );
        self.current = to;
    }

    /// Push a triangle to the Path.
    pub fn push_triangle(
        &mut self,
        xy: (Point<Pixels>, Point<Pixels>, Point<Pixels>),
        st: (Point<f32>, Point<f32>, Point<f32>),
    ) {
        self.bounds = self
            .bounds
            .union(&Bounds {
                origin: xy.0,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.1,
                size: Default::default(),
            })
            .union(&Bounds {
                origin: xy.2,
                size: Default::default(),
            });

        self.vertices.push(PathVertex {
            xy_position: xy.0,
            st_position: st.0,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.1,
            st_position: st.1,
            content_mask: Default::default(),
        });
        self.vertices.push(PathVertex {
            xy_position: xy.2,
            st_position: st.2,
            content_mask: Default::default(),
        });
    }
}

impl<T> Path<T>
where
    T: Clone + Debug + Default + PartialEq + PartialOrd + Add<T, Output = T> + Sub<Output = T>,
{
    #[allow(unused)]
    #[expect(missing_docs)]
    pub fn clipped_bounds(&self) -> Bounds<T> {
        self.bounds.intersect(&self.content_mask.bounds)
    }
}

impl From<Path<ScaledPixels>> for Primitive {
    fn from(path: Path<ScaledPixels>) -> Self {
        Primitive::Path(path)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
#[expect(missing_docs)]
pub struct PathVertex<P: Clone + Debug + Default + PartialEq> {
    pub xy_position: Point<P>,
    pub st_position: Point<f32>,
    pub content_mask: ContentMask<P>,
}

#[expect(missing_docs)]
impl PathVertex<Pixels> {
    pub fn scale(&self, factor: f32) -> PathVertex<ScaledPixels> {
        PathVertex {
            xy_position: self.xy_position.scale(factor),
            st_position: self.st_position,
            content_mask: self.content_mask.scale(factor),
        }
    }
}
