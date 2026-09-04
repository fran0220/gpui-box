const MAX_GLASS_LOBES: u32 = 8u;

struct Lobe {
    bounds: vec4<f32>,
    radii: vec4<f32>,
}

struct Params {
    bounds: vec4<f32>,
    mask: vec4<f32>,
    radii: vec4<f32>,
    viewport: vec2<f32>,
    direction: vec2<f32>,
    sigma: f32,
    bevel: f32,
    refraction: f32,
    dispersion: f32,
    specular: f32,
    light_angle: f32,
    specular_sharpness: f32,
    smoothing: f32,
    transmission_gain: f32,
    hairline: f32,
    lobe_count: u32,
    blur_radius: u32,
    optical_lift: vec4<f32>,
    lobes: array<Lobe, MAX_GLASS_LOBES>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var sharp_source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;
@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var blur_weights: texture_2d<f32>;

struct Varying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vertex: u32) -> Varying {
    let uv = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    return Varying(
        vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0),
        uv,
    );
}

@fragment
fn fs_blur_weight(input: Varying) -> @location(0) f32 {
    let offset = f32(u32(input.position.x));
    let sigma = max(params.sigma, 1.0);
    return exp(-0.5 * offset * offset / (sigma * sigma));
}

@fragment
fn fs_blur(input: Varying) -> @location(0) vec4<f32> {
    let radius = min(64u, params.blur_radius);
    var color = textureSample(source, source_sampler, input.uv) * textureLoad(blur_weights, vec2(0, 0), 0).x;
    var weight = textureLoad(blur_weights, vec2(0, 0), 0).x;
    for (var offset = 1; offset <= 64; offset++) {
        if (u32(offset) <= radius) {
            let sample_weight = textureLoad(blur_weights, vec2(offset, 0), 0).x;
            let delta = params.direction * f32(offset) / params.viewport;
            color += (textureSample(source, source_sampler, input.uv + delta) +
                textureSample(source, source_sampler, input.uv - delta)) * sample_weight;
            weight += 2.0 * sample_weight;
        }
    }
    return color / weight;
}

// Signed distance to one rounded rect, negative inside. Mirrors `quad_sdf` in
// the Metal shaders and `glass_lobe_sdf` in scene.rs.
fn lobe_distance(point: vec2<f32>, bounds: vec4<f32>, radii: vec4<f32>) -> f32 {
    let center = bounds.xy + bounds.zw * 0.5;
    let local = point - center;
    let radius = select(
        select(radii.x, radii.w, local.y >= 0.0),
        select(radii.y, radii.z, local.y >= 0.0),
        local.x >= 0.0);
    let delta = abs(local) - bounds.zw * 0.5 + vec2<f32>(radius);
    return length(max(delta, vec2<f32>(0.0))) + min(max(delta.x, delta.y), 0.0) - radius;
}

fn rounded_distance(point: vec2<f32>) -> f32 {
    return lobe_distance(point, params.bounds, params.radii);
}

// The polynomial smooth minimum. Mirrors `glass_smooth_min` in scene.rs.
fn smooth_min(a: f32, b: f32, smoothing: f32) -> f32 {
    if (smoothing <= 0.0) {
        return min(a, b);
    }
    let h = max(smoothing - abs(a - b), 0.0) / smoothing;
    return min(a, b) - h * h * smoothing * 0.25;
}

// Distance to the surface's shape. Mirrors `glass_sdf` in the Metal shaders
// and the `union` helper inside `glass_field` in scene.rs: no lobes means the
// surface is the single rounded rect it already named.
fn glass_distance(point: vec2<f32>) -> f32 {
    if (params.lobe_count == 0u) {
        return rounded_distance(point);
    }
    let count = min(params.lobe_count, MAX_GLASS_LOBES);
    var distance = lobe_distance(point, params.lobes[0].bounds, params.lobes[0].radii);
    for (var index = 1u; index < MAX_GLASS_LOBES; index++) {
        if (index < count) {
            let lobe = params.lobes[index];
            distance = smooth_min(
                distance,
                lobe_distance(point, lobe.bounds, lobe.radii),
                params.smoothing);
        }
    }
    return distance;
}

@fragment
fn fs_composite(input: Varying) -> @location(0) vec4<f32> {
    let point = input.position.xy;
    let mask_end = params.mask.xy + params.mask.zw;
    let distance = glass_distance(point);
    if (point.x < params.mask.x || point.y < params.mask.y || point.x >= mask_end.x ||
        point.y >= mask_end.y || distance > 0.0) {
        discard;
    }

    // A plain frost is an exact copy of its blurred source. Liquid reaches the
    // path below even with blur zero: scattering is not the optics switch.
    if ((params.bevel <= 0.0 || params.refraction == 0.0) && params.specular <= 0.0 &&
        params.transmission_gain == 1.0 && params.optical_lift.a <= 0.0 &&
        params.hairline <= 0.0) {
        return textureLoad(source, vec2<i32>(point), 0);
    }

    // The gradient by central differences, on the same half-pixel stencil as
    // `glass_field` in scene.rs. See that function for why the normal is
    // differenced in all four implementations rather than derived in each.
    let epsilon = 0.5;
    var gradient = vec2<f32>(
        glass_distance(point + vec2<f32>(epsilon, 0.0)) -
            glass_distance(point - vec2<f32>(epsilon, 0.0)),
        glass_distance(point + vec2<f32>(0.0, epsilon)) -
            glass_distance(point - vec2<f32>(0.0, epsilon)));
    let gradient_length = length(gradient);
    if (gradient_length > 0.0) {
        gradient = gradient / gradient_length;
    }

    // `depth` is zero at the rim and one once the dome has flattened. The
    // corresponding spherical slope diverges at the rim, then the measured
    // reach cap below bounds its displacement to 45% of the profile depth.
    var depth = 1.0;
    if (params.bevel > 0.0) {
        depth = clamp(-distance / params.bevel, 0.0, 1.0);
    }
    let rise = 1.0 - depth;
    let slope = rise / sqrt(max(1.0 - rise * rise, 1e-4));
    var displacement = -gradient * slope * params.bevel * params.refraction;
    let reach_limit = params.bevel * 0.45;
    let reach = length(displacement);
    if (reach > reach_limit && reach > 0.0) {
        displacement *= reach_limit / reach;
    }

    // Frost in the interior, the sharp snapshot at the bent rim. Both sources
    // are sampled at the same displaced coordinate so blur changes scattering,
    // never the geometry of the refraction.
    let sharpness = rise * rise;
    let red_uv = (point + displacement * (1.0 - params.dispersion)) / params.viewport;
    let green_uv = (point + displacement) / params.viewport;
    let blue_uv = (point + displacement * (1.0 + params.dispersion)) / params.viewport;
    let frosted_red = textureSample(source, source_sampler, red_uv);
    let frosted_green = textureSample(source, source_sampler, green_uv);
    let frosted_blue = textureSample(source, source_sampler, blue_uv);
    let sharp_red = textureSample(sharp_source, source_sampler, red_uv);
    let sharp_green = textureSample(sharp_source, source_sampler, green_uv);
    let sharp_blue = textureSample(sharp_source, source_sampler, blue_uv);
    var color = vec4<f32>(
        mix(frosted_red.r, sharp_red.r, sharpness),
        mix(frosted_green.g, sharp_green.g, sharpness),
        mix(frosted_blue.b, sharp_blue.b, sharpness),
        mix(frosted_green.a, sharp_green.a, sharpness),
    );

    color = vec4<f32>(
        color.rgb * params.transmission_gain + params.optical_lift.rgb * params.optical_lift.a,
        color.a,
    );

    if (params.specular > 0.0) {
        // The light sits on the unit sphere at `light_angle`, measured
        // clockwise from straight up, tilted towards the viewer so a flat
        // surface is lit rather than black.
        let normal = normalize(vec3<f32>(gradient * rise, max(depth, 0.001)));
        let light = normalize(vec3<f32>(
            sin(params.light_angle), -cos(params.light_angle), 0.6));
        let lobe_value = clamp(dot(normal, light), 0.0, 1.0);
        let highlight = pow(lobe_value, params.specular_sharpness) *
            params.specular * rise;
        color = vec4<f32>(color.rgb + highlight, color.a);
    }

    if (params.hairline > 0.0) {
        let hair = 1.0 - smoothstep(0.0, params.hairline * 1.5, -distance);
        let facing_up = clamp(-gradient.y, 0.0, 1.0);
        color = vec4<f32>(color.rgb + hair * (1.0 - 0.18 * facing_up) * 0.18, color.a);
    }

    return vec4<f32>(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
}

@fragment
fn fs_copy(input: Varying) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(input.position.xy), 0);
}
