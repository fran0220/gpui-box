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
    edge_mask_edge: f32,
    edge_mask_band: f32,
    saturation: f32,
    _mask_pad: f32,
    wash: vec4<f32>,
    thickness: f32,
    refractive_index: f32,
    backdrop_depth: f32,
    _optics_pad: f32,
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

// Analytic normal and distance. Mirrors `quad_sdf_gradient` / `quad_sdf` in
// Metal and `glass_lobe_field` in scene.rs; ties select an incident face.
fn lobe_field(point: vec2<f32>, bounds: vec4<f32>, radii: vec4<f32>) -> vec3<f32> {
    let center = bounds.xy + bounds.zw * 0.5;
    let local = point - center;
    let radius = select(
        select(radii.x, radii.w, local.y >= 0.0),
        select(radii.y, radii.z, local.y >= 0.0),
        local.x >= 0.0);
    let delta = abs(local) - bounds.zw * 0.5 + vec2<f32>(radius);
    let outside = max(delta, vec2<f32>(0.0));
    let outside_length = length(outside);
    var gradient = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), delta.x > delta.y);
    if (radius != 0.0 && outside_length > 0.0) {
        gradient = outside / outside_length;
    }
    gradient *= select(vec2<f32>(-1.0), vec2<f32>(1.0), local >= vec2<f32>(0.0));
    let distance = select(outside_length + min(max(delta.x, delta.y), 0.0) - radius,
                          max(delta.x, delta.y), radius == 0.0);
    return vec3<f32>(gradient, distance);
}

// The polynomial smooth minimum. Mirrors `glass_smooth_min` in scene.rs.
fn smooth_min(a: f32, b: f32, smoothing: f32) -> f32 {
    if (smoothing <= 0.0) {
        return min(a, b);
    }
    let h = max(smoothing - abs(a - b), 0.0) / smoothing;
    return min(a, b) - h * h * smoothing * 0.25;
}

// Linear fade from a named edge. Mirrors `glass_edge_mask` in scene.rs.
fn glass_edge_mask(point: vec2<f32>) -> f32 {
    if (params.edge_mask_edge <= 0.0 || params.edge_mask_band <= 0.0) {
        return 1.0;
    }
    if (params.edge_mask_edge < 1.5) {
        return 1.0 - clamp((point.y - params.bounds.y) / params.edge_mask_band, 0.0, 1.0);
    }
    if (params.edge_mask_edge < 2.5) {
        return 1.0 - clamp((params.bounds.y + params.bounds.w - point.y) / params.edge_mask_band, 0.0, 1.0);
    }
    if (params.edge_mask_edge < 3.5) {
        return 1.0 - clamp((point.x - params.bounds.x) / params.edge_mask_band, 0.0, 1.0);
    }
    return 1.0 - clamp((params.bounds.x + params.bounds.z - point.x) / params.edge_mask_band, 0.0, 1.0);
}

fn apply_edge_mask(color: vec4<f32>, point: vec2<f32>) -> vec4<f32> {
    let mask = glass_edge_mask(point);
    if (mask >= 1.0) {
        return color;
    }
    let original = textureLoad(sharp_source, vec2<i32>(point), 0);
    return mix(original, color, mask);
}

// Analytic field of the shape. Smooth-min derivatives use h/2, with no
// intermediate normalization. Mirrors `glass_field` in scene.rs.
fn glass_field(point: vec2<f32>) -> vec3<f32> {
    if (params.lobe_count == 0u) {
        return lobe_field(point, params.bounds, params.radii);
    }
    let count = min(params.lobe_count, MAX_GLASS_LOBES);
    var field = lobe_field(point, params.lobes[0].bounds, params.lobes[0].radii);
    for (var index = 1u; index < MAX_GLASS_LOBES; index++) {
        if (index < count) {
            let lobe = params.lobes[index];
            let next = lobe_field(point, lobe.bounds, lobe.radii);
            var h = 0.0;
            if (params.smoothing > 0.0) {
                h = max(params.smoothing - abs(field.z - next.z), 0.0) / params.smoothing;
            }
            let weight = select(1.0 - h * 0.5, h * 0.5, field.z <= next.z);
            field = vec3<f32>(mix(field.xy, next.xy, weight),
                              smooth_min(field.z, next.z, params.smoothing));
        }
    }
    return field;
}

// One refracting interface and an effective optical background plane, not
// volume tracing. Index one is exactly undeformed, including at grazing angles.
fn optical_displacement(normal: vec3<f32>, index: f32, distance: f32) -> vec2<f32> {
    if (index == 1.0) { return vec2<f32>(0.0); }
    let ray = refract(vec3<f32>(0.0, 0.0, -1.0), normal, 1.0 / index);
    return ray.xy / max(-ray.z, 1e-4) * distance;
}

@fragment
fn fs_composite(input: Varying) -> @location(0) vec4<f32> {
    let point = input.position.xy;
    let mask_end = params.mask.xy + params.mask.zw;
    let field = glass_field(point);
    let distance = field.z;
    if (point.x < params.mask.x || point.y < params.mask.y || point.x >= mask_end.x ||
        point.y >= mask_end.y || distance > 0.0) {
        discard;
    }

    // A plain frost is an exact copy of its blurred source. Liquid reaches the
    // path below even with blur zero: scattering is not the optics switch.
    if ((params.bevel <= 0.0 || params.thickness == 0.0 || params.refractive_index == 1.0) &&
        (params.specular <= 0.0 || params.refractive_index == 1.0) &&
        params.transmission_gain == 1.0 && params.optical_lift.a <= 0.0 &&
        params.saturation == 1.0 && params.wash.a <= 0.0 &&
        params.hairline <= 0.0) {
        return apply_edge_mask(textureLoad(source, vec2<i32>(point), 0), point);
    }

    // Keep the smooth-union derivative magnitude for the height derivative.
    // Only the decorative hairline needs a unit contour direction.
    var gradient = field.xy;
    let gradient_length = length(gradient);
    if (gradient_length > 0.0) {
        gradient = gradient / gradient_length;
    }

    var u = 0.0;
    if (params.bevel > 0.0) {
        u = clamp(1.0 + distance / params.bevel, 0.0, 1.0);
    }
    let profile = sqrt(max(1.0 - u * u, 0.0));
    let height = params.thickness * select(profile, 1.0 - profile, params.refraction < 0.0);
    var normal = vec3<f32>(0.0, 0.0, 1.0);
    if (params.bevel > 0.0 && params.thickness > 0.0) {
        normal = normalize(vec3<f32>(field.xy * (params.thickness / params.bevel) *
            u / sqrt(max(1.0 - u * u, 1e-4)) * sign(params.refraction), 1.0));
    }
    let travel = height + params.backdrop_depth;
    let index = params.refractive_index;

    // Refract the scattered source even at the rim: mixing the sharp snapshot
    // back in would resurrect readable backdrop text. With blur zero this
    // source is already sharp, preserving Clear optics.
    let red_uv = (point + optical_displacement(normal, 1.0 + (index - 1.0) * (1.0 - params.dispersion), travel)) / params.viewport;
    let green_uv = (point + optical_displacement(normal, index, travel)) / params.viewport;
    let blue_uv = (point + optical_displacement(normal, 1.0 + (index - 1.0) * (1.0 + params.dispersion), travel)) / params.viewport;
    let frosted_red = textureSample(source, source_sampler, red_uv);
    let frosted_green = textureSample(source, source_sampler, green_uv);
    let frosted_blue = textureSample(source, source_sampler, blue_uv);
    var color = vec4<f32>(frosted_red.r, frosted_green.g, frosted_blue.b, frosted_green.a);

    // Sample (including the rim), saturation, gain, source-over wash, then lift.
    let luminance = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturated = max(mix(vec3<f32>(luminance), color.rgb, params.saturation), vec3<f32>(0.0));
    let transmitted = saturated * params.transmission_gain;
    color = vec4<f32>(
        mix(transmitted, params.wash.rgb, params.wash.a) + params.optical_lift.rgb * params.optical_lift.a,
        params.wash.a + color.a * (1.0 - params.wash.a),
    );

    if (params.specular > 0.0 && index > 1.0) {
        // Analytic directional environment, not a real environment reflection.
        let light = normalize(vec3<f32>(
            sin(params.light_angle), -cos(params.light_angle), 0.6));
        let reflected = reflect(vec3<f32>(0.0, 0.0, -1.0), normal);
        let lobe_value = pow(max(dot(reflected, light), 0.0), params.specular_sharpness);
        let f0 = pow((index - 1.0) / (index + 1.0), 2.0);
        let fresnel = f0 + (1.0 - f0) * pow(1.0 - normal.z, 5.0);
        color = vec4<f32>(mix(color.rgb, vec3<f32>(lobe_value),
            clamp(params.specular * fresnel, 0.0, 1.0)), color.a);
    }

    if (params.hairline > 0.0) {
        let hair = 1.0 - smoothstep(0.0, params.hairline * 1.5, -distance);
        let facing_up = clamp(-gradient.y, 0.0, 1.0);
        color = vec4<f32>(color.rgb + hair * (1.0 - 0.18 * facing_up) * 0.18, color.a);
    }

    return apply_edge_mask(
        vec4<f32>(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), color.a),
        point,
    );
}

@fragment
fn fs_copy(input: Varying) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(input.position.xy), 0);
}
