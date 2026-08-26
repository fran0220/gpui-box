//! What the renderer can do to a surface.

use std::{cell::RefCell, collections::HashMap};

use super::support::*;

thread_local! {
    static COMPOSITED_SPRITE_ATLASES: RefCell<HashMap<[u32; 8], Arc<RenderImage>>> =
        RefCell::new(HashMap::new());
}

pub(super) fn composited_sprite_atlas(theme: &Theme) -> Arc<RenderImage> {
    let accent = theme.colors.accent_strong.to_rgb();
    let info = theme.colors.info.to_rgb();
    let key = [
        accent.r.to_bits(),
        accent.g.to_bits(),
        accent.b.to_bits(),
        accent.a.to_bits(),
        info.r.to_bits(),
        info.g.to_bits(),
        info.b.to_bits(),
        info.a.to_bits(),
    ];
    COMPOSITED_SPRITE_ATLASES.with(|atlases| {
        let mut atlases = atlases.borrow_mut();
        atlases
            .entry(key)
            .or_insert_with(|| Arc::new(build_composited_sprite_atlas(theme)))
            .clone()
    })
}

fn build_composited_sprite_atlas(theme: &Theme) -> RenderImage {
    const TILE: i32 = 48;
    const TILES: i32 = 3;

    let accent = theme.colors.accent_strong.to_rgb();
    let info = theme.colors.info.to_rgb();
    let mut pixels = Vec::with_capacity((TILE * TILE * TILES * 4) as usize);
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    for y in 0..TILE {
        for x in 0..TILE * TILES {
            let tile = x / TILE;
            let local_x = x % TILE;
            let dx = local_x as f32 + 0.5 - TILE as f32 / 2.0;
            let dy = y as f32 + 0.5 - TILE as f32 / 2.0;
            let radius = dx.hypot(dy);
            let (red, green, blue, alpha) = match tile {
                0 => {
                    let mix = local_x as f32 / (TILE - 1) as f32;
                    let alpha = ((TILE as f32 * 0.44 - radius) / 1.5).clamp(0.0, 1.0);
                    (
                        accent.r + (info.r - accent.r) * mix,
                        accent.g + (info.g - accent.g) * mix,
                        accent.b + (info.b - accent.b) * mix,
                        alpha,
                    )
                }
                1 => {
                    let alpha = (1.0 - radius / (TILE as f32 * 0.5)).clamp(0.0, 1.0);
                    (1.0, 1.0, 1.0, alpha * alpha)
                }
                _ => {
                    let horizontal = (1.0 - dy.abs() / 2.6).clamp(0.0, 1.0)
                        * (1.0 - dx.abs() / 22.0).clamp(0.0, 1.0);
                    let vertical = (1.0 - dx.abs() / 2.6).clamp(0.0, 1.0)
                        * (1.0 - dy.abs() / 22.0).clamp(0.0, 1.0);
                    let core = (1.0 - radius / 7.0).clamp(0.0, 1.0);
                    (1.0, 1.0, 1.0, horizontal.max(vertical).max(core))
                }
            };
            pixels.extend_from_slice(&[byte(red), byte(green), byte(blue), byte(alpha)]);
        }
    }

    RenderImage::from_rgba(size(DevicePixels(TILE * TILES), DevicePixels(TILE)), pixels)
        .expect("the procedural sprite atlas has exact RGBA8 dimensions")
}

pub(super) fn visual_effects(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let radial = radial_gradient_stops(
        point(0.32, 0.36),
        point(0.68, 0.72),
        [
            linear_color_stop(theme.colors.accent_strong, 0.0),
            linear_color_stop(theme.colors.info.opacity(0.86), 0.34),
            linear_color_stop(theme.colors.accent.opacity(0.38), 0.7),
            linear_color_stop(theme.colors.canvas.opacity(0.0), 1.0),
        ],
    )
    .color_space(gpui::ColorSpace::Oklab);
    let conic = conic_gradient_stops(
        -24.0,
        point(0.5, 0.5),
        [
            linear_color_stop(theme.colors.accent_strong, 0.0),
            linear_color_stop(theme.colors.info, 0.22),
            linear_color_stop(theme.colors.success, 0.48),
            linear_color_stop(theme.colors.warning, 0.72),
            linear_color_stop(theme.colors.danger, 1.0),
        ],
    )
    .color_space(gpui::ColorSpace::Oklab);
    let path_fill = conic;
    let stroke_fill = radial;
    let stroke_base = theme.colors.hairline_strong.opacity(0.32);
    let sprite_atlas = composited_sprite_atlas(&theme);
    let additive_tint = theme.colors.accent_strong;
    let additive_tint_alt = theme.colors.info;
    let screen_tint = theme.colors.success;
    let screen_tint_alt = theme.colors.warning;
    let mut effect_planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    let success_particles = effect_planner.plan(
        EffectEvent::new(
            "scene-success",
            "visual-effects",
            "success-card",
            VisualCue::Success,
        ),
        1,
        false,
    );
    let reward_particles = effect_planner.plan(
        EffectEvent::new(
            "scene-reward",
            "visual-effects",
            "reward-card",
            VisualCue::Reward,
        ),
        1,
        false,
    );
    let mut static_planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    let static_particles = static_planner.plan(
        EffectEvent::new(
            "scene-static",
            "visual-effects-static",
            "static-card",
            VisualCue::Reward,
        ),
        1,
        true,
    );
    let label = |text: &'static str| {
        div()
            .absolute()
            .bottom(px(theme.spacing.sm))
            .left(px(theme.spacing.sm))
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Pill)
            // The caption sits on top of artwork it does not control, so it
            // carries its own opaque surface rather than a scrim: a legend
            // that dims with what it labels stops being a legend.
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(crate::foundation::text(&theme, TypeScale::Caption, text))
    };

    stack(&theme)
        .child(crate::foundation::text(
            &theme,
            TypeScale::Subtitle,
            "Renderer-backed gradients",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(
                    div()
                        .relative()
                        .w(px(248.0))
                        .h(px(164.0))
                        .radius(&theme, Radius::Card)
                        .overflow_hidden()
                        .bg(radial)
                        .semantic_in(
                            cx,
                            NodeSpec::new("scene.effects.radial", Role::Image)
                                .text("Elliptical radial gradient")
                                .description("Four ordered Oklab color stops"),
                        )
                        .child(label("Radial · 4 stops")),
                )
                .child(
                    div()
                        .relative()
                        .w(px(248.0))
                        .h(px(164.0))
                        .radius(&theme, Radius::Card)
                        .overflow_hidden()
                        .bg(conic)
                        .semantic_in(
                            cx,
                            NodeSpec::new("scene.effects.conic", Role::Image)
                                .text("Clockwise conic gradient")
                                .description("Five ordered Oklab color stops"),
                        )
                        .child(label("Conic · 5 stops")),
                )
                .child(
                    div()
                        .relative()
                        .w(px(248.0))
                        .h(px(164.0))
                        .radius(&theme, Radius::Card)
                        .overflow_hidden()
                        .bg(theme.colors.sunken)
                        .semantic_in(
                            cx,
                            NodeSpec::new("scene.effects.path-gradient", Role::Image)
                                .text("Conic gradient path fill")
                                .description("The same renderer primitive clipped by a GPUI path"),
                        )
                        .child(
                            div().absolute().inset_0().child(
                                canvas(
                                    |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        let inset = px(22.0);
                                        let center = bounds.center();
                                        let mut builder = gpui::PathBuilder::fill();
                                        builder.move_to(point(center.x, bounds.top() + inset));
                                        builder.line_to(point(bounds.right() - inset, center.y));
                                        builder.line_to(point(center.x, bounds.bottom() - inset));
                                        builder.line_to(point(bounds.left() + inset, center.y));
                                        builder.close();
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, path_fill);
                                        }
                                    },
                                )
                                .size_full(),
                            ),
                        )
                        .child(label("Path fill · conic")),
                ),
        )
        .child(crate::foundation::text(
            &theme,
            TypeScale::Subtitle,
            "Measured path strokes",
        ))
        .child(
            div()
                .relative()
                .w(px(776.0))
                .h(px(180.0))
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .bg(theme.colors.sunken)
                .semantic_in(
                    cx,
                    NodeSpec::new("scene.effects.path-strokes", Role::Image)
                        .text("Measured path stroke effects")
                        .description("Trimmed, dash-offset, and combined stroke geometry"),
                )
                .child(
                    div().absolute().inset_0().child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let build = |mut builder: gpui::PathBuilder, y: f32| {
                                    let start =
                                        point(bounds.left() + px(128.0), bounds.top() + px(y));
                                    let end =
                                        point(bounds.right() - px(24.0), bounds.top() + px(y));
                                    builder.move_to(start);
                                    builder.cubic_bezier_to(
                                        end,
                                        point(start.x + px(132.0), start.y - px(24.0)),
                                        point(end.x - px(132.0), end.y + px(24.0)),
                                    );
                                    builder.build()
                                };

                                if let Ok(path) = build(gpui::PathBuilder::stroke(px(8.0)), 42.0) {
                                    window.paint_path(path, stroke_base);
                                }
                                if let Ok(path) = build(
                                    gpui::PathBuilder::stroke(px(8.0)).stroke_trim(0.0, 0.68),
                                    42.0,
                                ) {
                                    window.paint_path(path, stroke_fill);
                                }
                                if let Ok(path) = build(
                                    gpui::PathBuilder::stroke(px(7.0))
                                        .dash_array(&[px(18.0), px(11.0)])
                                        .dash_offset(px(9.0)),
                                    92.0,
                                ) {
                                    window.paint_path(path, conic);
                                }
                                if let Ok(path) = build(
                                    gpui::PathBuilder::stroke(px(7.0))
                                        .dash_array(&[px(14.0), px(8.0)])
                                        .dash_offset(px(-6.0))
                                        .stroke_trim(0.16, 0.84),
                                    142.0,
                                ) {
                                    window.paint_path(path, stroke_fill);
                                }
                            },
                        )
                        .size_full(),
                    ),
                )
                .child(div().absolute().left(px(16.0)).top(px(31.0)).child(
                    crate::foundation::text(&theme, TypeScale::Caption, "Trim · 68%"),
                ))
                .child(div().absolute().left(px(16.0)).top(px(81.0)).child(
                    crate::foundation::text(&theme, TypeScale::Caption, "Dash phase · 9 px"),
                ))
                .child(div().absolute().left(px(16.0)).top(px(131.0)).child(
                    crate::foundation::text(&theme, TypeScale::Caption, "Trim + phase"),
                )),
        )
        .child(crate::foundation::text(
            &theme,
            TypeScale::Subtitle,
            "Composited sprite batches",
        ))
        .child(
            div()
                .relative()
                .w(px(776.0))
                .h(px(190.0))
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .bg(theme.colors.sunken)
                .semantic_in(
                    cx,
                    NodeSpec::new("scene.effects.sprite-batch", Role::Image)
                        .text("Composited sprite batch")
                        .description(
                            "One atlas with explicit source rectangles, transforms, masks, and normal, additive, and screen blending",
                        ),
                )
                .child(
                    div().absolute().inset_0().child(
                        canvas(
                            |_, _, _| {},
                            move |frame, _, window, _| {
                                let source = |column| {
                                    bounds(
                                        point(DevicePixels(column * 48), DevicePixels(0)),
                                        size(DevicePixels(48), DevicePixels(48)),
                                    )
                                };
                                let destination = |x, y, width, height| {
                                    bounds(
                                        point(frame.left() + px(x), frame.top() + px(y)),
                                        size(px(width), px(height)),
                                    )
                                };
                                let sprites = [
                                    SpriteInstance::new(
                                        destination(56.0, 26.0, 104.0, 104.0),
                                        source(0),
                                    )
                                    .corner_radii(Corners::all(px(28.0)))
                                    .transform(
                                        SpriteTransform::identity().rotate(radians(-0.16)),
                                    ),
                                    SpriteInstance::new(
                                        destination(270.0, 34.0, 96.0, 96.0),
                                        source(1),
                                    )
                                    .color_mode(SpriteColorMode::AlphaMask, additive_tint)
                                    .blend_mode(SpriteBlendMode::Additive),
                                    SpriteInstance::new(
                                        destination(314.0, 34.0, 96.0, 96.0),
                                        source(1),
                                    )
                                    .transform(
                                        SpriteTransform::identity().rotate(radians(0.22)),
                                    )
                                    .color_mode(SpriteColorMode::AlphaMask, additive_tint_alt)
                                    .blend_mode(SpriteBlendMode::Additive),
                                    SpriteInstance::new(
                                        destination(536.0, 24.0, 112.0, 112.0),
                                        source(2),
                                    )
                                    .color_mode(SpriteColorMode::AlphaMask, screen_tint)
                                    .blend_mode(SpriteBlendMode::Screen),
                                    SpriteInstance::new(
                                        destination(582.0, 30.0, 100.0, 100.0),
                                        source(2),
                                    )
                                    .transform(
                                        SpriteTransform::identity().rotate(radians(0.42)),
                                    )
                                    .color_mode(SpriteColorMode::AlphaMask, screen_tint_alt)
                                    .blend_mode(SpriteBlendMode::Screen),
                                ];
                                window
                                    .paint_sprite_batch(sprite_atlas.clone(), 0, &sprites)
                                    .expect("the canonical sprite batch is valid");
                            },
                        )
                        .size_full(),
                    ),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(64.0))
                        .bottom(px(12.0))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Normal · crop + rotate",
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(282.0))
                        .bottom(px(12.0))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Additive · alpha mask",
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(550.0))
                        .bottom(px(12.0))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Screen · atlas reuse",
                        )),
                ),
        )
        .child(crate::foundation::text(
            &theme,
            TypeScale::Subtitle,
            "Policy-owned particle recipes",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(
                    div()
                        .relative()
                        .w(px(248.0))
                        .h(px(164.0))
                        .radius(&theme, Radius::Card)
                        .overflow_hidden()
                        .bg(theme.colors.sunken)
                        .semantic_in(
                            cx,
                            NodeSpec::new("scene.effects.particles-success", Role::Image)
                                .text("Deterministic success burst")
                                .description(
                                    "A policy-owned recipe sampled at a fixed absolute time",
                                ),
                        )
                        .child(
                            EffectParticles::new(success_particles)
                                .sample_at(Duration::from_millis(420)),
                        )
                        .child(label("Success · 36 instances")),
                )
                .child(
                    div()
                        .relative()
                        .w(px(248.0))
                        .h(px(164.0))
                        .radius(&theme, Radius::Card)
                        .overflow_hidden()
                        .bg(theme.colors.sunken)
                        .semantic_in(
                            cx,
                            NodeSpec::new("scene.effects.particles-reward", Role::Image)
                                .text("Deterministic reward celebration")
                                .description(
                                    "Two emitters share one atlas upload and one sprite batch",
                                ),
                        )
                        .child(
                            EffectParticles::new(reward_particles)
                                .sample_at(Duration::from_millis(620)),
                        )
                        .child(label("Reward · 2 emitters")),
                )
                .child(
                    div()
                        .relative()
                        .w(px(248.0))
                        .h(px(164.0))
                        .radius(&theme, Radius::Card)
                        .overflow_hidden()
                        .bg(theme.colors.sunken)
                        .semantic_in(
                            cx,
                            NodeSpec::new("scene.effects.particles-static", Role::Image)
                                .text("Reduced motion particle fallback")
                                .description(
                                    "A fixed bounded constellation with no animation frames",
                                ),
                        )
                        .child(EffectParticles::new(static_particles))
                        .child(label("Static · no timeline")),
                ),
        )
        .into_any_element()
}

#[derive(Debug)]
pub(super) struct SceneCinematicClip;

impl DotLottieClip for SceneCinematicClip {
    fn metadata(&self) -> DotLottieMetadata {
        DotLottieMetadata {
            width: 240,
            height: 140,
            frame_count: 120,
            frame_rate_millihertz: 60_000,
            duration: Duration::from_secs(2),
            animation_count: 1,
            state_machine_count: 0,
        }
    }

    fn render(&self, sample: DotLottieSample) -> Result<Arc<RenderImage>, DotLottieError> {
        static HANDOFF_LTR: OnceLock<Arc<RenderImage>> = OnceLock::new();
        static HANDOFF_RTL: OnceLock<Arc<RenderImage>> = OnceLock::new();
        static POSTER: OnceLock<Arc<RenderImage>> = OnceLock::new();

        const WIDTH: u32 = 240;
        const HEIGHT: u32 = 140;
        let cache = match (sample.progress_per_mille(), sample.mirror_x()) {
            (500, false) => Some(&HANDOFF_LTR),
            (500, true) => Some(&HANDOFF_RTL),
            (700, false) => Some(&POSTER),
            _ => None,
        };
        if let Some(image) = cache.and_then(OnceLock::get) {
            return Ok(image.clone());
        }
        let progress = sample.progress_per_mille() as f32 / 1_000.0;
        let direction = if sample.mirror_x() { -1.0 } else { 1.0 };
        let angle = progress * std::f32::consts::TAU;
        let orbit_x = 120.0 + direction * angle.cos() * 64.0;
        let orbit_y = 70.0 + angle.sin() * 31.0;
        let pulse = 0.72 + (angle * 2.0).sin() * 0.12;
        let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let dx = x as f32 - 120.0;
                let dy = y as f32 - 70.0;
                let distance = ((dx / 1.65).powi(2) + dy.powi(2)).sqrt();
                let ring = (1.0 - (distance - 38.0).abs() / 3.2).clamp(0.0, 1.0);
                let haze = (1.0 - distance / 76.0).clamp(0.0, 1.0).powi(3) * 0.28;
                let orb_distance =
                    ((x as f32 - orbit_x).powi(2) + (y as f32 - orbit_y).powi(2)).sqrt();
                let orb = (1.0 - orb_distance / 13.0).clamp(0.0, 1.0).powi(2);
                let directional_streak = if (y as f32 - orbit_y).abs() < 1.4 {
                    let trail = direction * (orbit_x - x as f32);
                    (1.0 - trail / 54.0).clamp(0.0, 1.0) * (trail / 8.0).clamp(0.0, 1.0) * 0.52
                } else {
                    0.0
                };
                let star = if dx.abs() < 1.15 || dy.abs() < 1.15 {
                    (1.0 - distance / 28.0).clamp(0.0, 1.0) * 0.82
                } else {
                    0.0
                };
                let alpha = (ring * pulse + haze + orb + directional_streak + star).clamp(0.0, 1.0);
                let purple = (0.35 + progress * 0.4 + dy.abs() / 180.0).clamp(0.0, 1.0);
                let cyan = 1.0 - purple;
                let index = ((y * WIDTH + x) * 4) as usize;
                pixels[index] = ((72.0 * purple + 54.0 * cyan + orb * 170.0) * alpha) as u8;
                pixels[index + 1] = ((46.0 * purple + 212.0 * cyan + orb * 80.0) * alpha) as u8;
                pixels[index + 2] = ((238.0 * purple + 246.0 * cyan + orb * 45.0) * alpha) as u8;
                pixels[index + 3] = (alpha * 255.0) as u8;
            }
        }

        let image = Arc::new(
            RenderImage::from_rgba(
                size(DevicePixels(WIDTH as i32), DevicePixels(HEIGHT as i32)),
                pixels,
            )
            .map_err(|_| DotLottieError::kind(DotLottieErrorKind::RenderFailed))?,
        );
        if let Some(cache) = cache {
            let _ = cache.set(image.clone());
            return Ok(cache.get().cloned().unwrap_or(image));
        }
        Ok(image)
    }
}

pub(super) fn cinematic_effects(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let clip: Rc<dyn DotLottieClip> = Rc::new(SceneCinematicClip);
    let mut planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    let success = planner.plan(
        EffectEvent::new(
            "scene-cinematic-success",
            "cinematic-effects",
            "success",
            VisualCue::Success,
        ),
        1,
        false,
    );
    let handoff = planner.plan(
        EffectEvent::new(
            "scene-cinematic-handoff",
            "cinematic-effects",
            "handoff",
            VisualCue::Handoff,
        ),
        1,
        false,
    );
    let reward = planner.plan(
        EffectEvent::new(
            "scene-cinematic-reward",
            "cinematic-effects",
            "reward",
            VisualCue::Reward,
        ),
        1,
        false,
    );
    let mut static_planner = EffectPlanner::new(EffectPolicy::new(EffectQuality::Cinematic));
    let static_reward = static_planner.plan(
        EffectEvent::new(
            "scene-cinematic-static",
            "cinematic-effects-static",
            "reward",
            VisualCue::Reward,
        ),
        1,
        true,
    );

    let card = |title: &'static str, detail: &'static str, effect: AnyElement| {
        div()
            .column()
            .w(px(372.0))
            .h(px(228.0))
            .radius(&theme, Radius::Card)
            .overflow_hidden()
            .bg(theme.colors.sunken)
            .child(div().relative().w_full().h(px(154.0)).child(effect))
            .child(
                div()
                    .column()
                    .gap_token(&theme, Space::Xs)
                    .px_token(&theme, Space::Md)
                    .py_token(&theme, Space::Sm)
                    .child(crate::foundation::text(&theme, TypeScale::Label, title))
                    .child(
                        crate::foundation::text(&theme, TypeScale::Caption, detail)
                            .text_color(theme.colors.text_muted),
                    ),
            )
    };

    stack(&theme)
        .w(px(808.0))
        .child(crate::foundation::text(
            &theme,
            TypeScale::Subtitle,
            "Semantic cinematic effects",
        ))
        .child(caption(
            &theme,
            "events choose recipes; hosts resolve bytes and own playback requests",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(card(
                    "Handoff · deterministic sample",
                    "A resolved clip sampled at 575 ms; RTL mirrors its directional trail.",
                    CinematicEffect::new("scene.cinematic.handoff", handoff)
                        .clip(clip.clone())
                        .sample_at(Duration::from_millis(575))
                        .into_any_element(),
                ))
                .child(card(
                    "Success · runtime unavailable",
                    "The semantic recipe falls back to the policy-owned particle burst.",
                    CinematicEffect::new("scene.cinematic.success", success)
                        .unavailable(DotLottieError::new(
                            DotLottieErrorKind::RuntimeUnavailable,
                            "dotLottie backend is not installed",
                        ))
                        .sample_at(Duration::from_millis(520))
                        .into_any_element(),
                )),
        )
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(card(
                    "Reward · invalid asset fallback",
                    "A refused archive stays explicit while a bounded particle recipe renders.",
                    CinematicEffect::new("scene.cinematic.invalid", reward)
                        .unavailable(DotLottieError::new(
                            DotLottieErrorKind::ArchiveInvalid,
                            "animation archive did not pass validation",
                        ))
                        .sample_at(Duration::from_millis(620))
                        .into_any_element(),
                ))
                .child(card(
                    "Reward · reduced-motion poster",
                    "The same recipe becomes a deterministic poster with no frame timeline.",
                    CinematicEffect::new("scene.cinematic.poster", static_reward)
                        .clip(clip)
                        .into_any_element(),
                )),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composited_sprite_atlas_is_stable_for_the_same_colors() {
        let theme = Theme::studio_dark();

        let first = composited_sprite_atlas(&theme);
        let second = composited_sprite_atlas(&theme);

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn composited_sprite_atlas_keeps_different_colors_separate() {
        let dark = composited_sprite_atlas(&Theme::studio_dark());
        let light = composited_sprite_atlas(&Theme::studio_light());

        assert!(!Arc::ptr_eq(&dark, &light));
    }
}
