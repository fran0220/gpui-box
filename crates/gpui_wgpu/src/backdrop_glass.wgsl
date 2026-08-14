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
    lobe_count: u32,
    pad_0: u32,
    pad_1: u32,
    pad_2: u32,
    lobes: array<Lobe, MAX_GLASS_LOBES>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

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

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-0.5 * x * x / (sigma * sigma));
}

@fragment
fn fs_blur(input: Varying) -> @location(0) vec4<f32> {
    let sigma = max(params.sigma, 1.0);
    let radius = min(64, i32(ceil(sigma * 3.0)));
    var color = textureSample(source, source_sampler, input.uv) * gaussian(0.0, sigma);
    var weight = gaussian(0.0, sigma);
    for (var offset = 1; offset <= 64; offset++) {
        if (offset <= radius) {
            let sample_weight = gaussian(f32(offset), sigma);
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

    // Without optics the blurred snapshot is the whole answer, and reading it
    // by texel rather than by sample keeps that path exact.
    if ((params.bevel <= 0.0 || params.refraction == 0.0) && params.specular <= 0.0) {
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

    // The bevel: 0 at the rim rising to 1 once the surface is `bevel` pixels
    // deep. `slope` is how much of it is left to bend light with.
    var height = 1.0;
    if (params.bevel > 0.0) {
        height = clamp(-distance / params.bevel, 0.0, 1.0);
    }
    let slope = 1.0 - height;
    let smooth_slope = slope * slope * (3.0 - 2.0 * slope);
    let normal = normalize(vec3<f32>(gradient * smooth_slope, max(height, 0.001)));

    var color: vec4<f32>;
    if (params.bevel > 0.0 && params.refraction != 0.0) {
        let offset = normal.xy * params.refraction * params.bevel * smooth_slope /
            params.viewport;
        let uv = point / params.viewport;
        if (params.dispersion > 0.0) {
            // Dispersion splits the channels along the same offset, which is
            // what makes a rim read as glass rather than as a smear.
            let red = textureSample(source, source_sampler, uv + offset * (1.0 + params.dispersion));
            let green = textureSample(source, source_sampler, uv + offset);
            let blue = textureSample(source, source_sampler, uv + offset * (1.0 - params.dispersion));
            color = vec4<f32>(red.r, green.g, blue.b, green.a);
        } else {
            color = textureSample(source, source_sampler, uv + offset);
        }
    } else {
        color = textureLoad(source, vec2<i32>(point), 0);
    }

    if (params.specular > 0.0) {
        // The light sits on the unit sphere at `light_angle`, measured
        // clockwise from straight up, tilted towards the viewer so a flat
        // surface is lit rather than black.
        let light = normalize(vec3<f32>(
            sin(params.light_angle), -cos(params.light_angle), 0.6));
        let lobe_value = clamp(dot(normal, light), 0.0, 1.0);
        let highlight = pow(lobe_value, params.specular_sharpness) *
            params.specular * smooth_slope;
        color = vec4<f32>(clamp(color.rgb + highlight, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
    }

    return color;
}

@fragment
fn fs_copy(input: Varying) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(input.position.xy), 0);
}
