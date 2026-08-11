struct Params {
    bounds: vec4<f32>,
    mask: vec4<f32>,
    radii: vec4<f32>,
    viewport: vec2<f32>,
    direction: vec2<f32>,
    sigma: f32,
    pad_0: u32,
    pad_1: u32,
    pad_2: u32,
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

fn rounded_distance(point: vec2<f32>) -> f32 {
    let center = params.bounds.xy + params.bounds.zw * 0.5;
    let local = point - center;
    let radius = select(
        select(params.radii.x, params.radii.w, local.y >= 0.0),
        select(params.radii.y, params.radii.z, local.y >= 0.0),
        local.x >= 0.0);
    let delta = abs(local) - params.bounds.zw * 0.5 + vec2<f32>(radius);
    return length(max(delta, vec2<f32>(0.0))) + min(max(delta.x, delta.y), 0.0) - radius;
}

@fragment
fn fs_composite(input: Varying) -> @location(0) vec4<f32> {
    let point = input.position.xy;
    let mask_end = params.mask.xy + params.mask.zw;
    if (point.x < params.mask.x || point.y < params.mask.y || point.x >= mask_end.x ||
        point.y >= mask_end.y || rounded_distance(point) > 0.0) {
        discard;
    }
    return textureLoad(source, vec2<i32>(input.position.xy), 0);
}

@fragment
fn fs_copy(input: Varying) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(input.position.xy), 0);
}
