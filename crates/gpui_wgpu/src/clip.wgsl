// Scene-local, parent-first rounded clips. Ten words per node. The index is
// one-based; zero preserves the unmodified framebuffer write. No depth cap:
// construction guarantees that each parent precedes its child.
fn rounded_clip_coverage(position: vec2<f32>, clip_id: u32) -> f32 {
    var cursor = clip_id;
    var coverage = 1.0;
    loop {
        if (cursor == 0u) { break; }
        let base = (cursor - 1u) * 10u;
        let origin = vec2<f32>(bitcast<f32>(clip_word(base)), bitcast<f32>(clip_word(base + 1u)));
        let size = vec2<f32>(bitcast<f32>(clip_word(base + 2u)), bitcast<f32>(clip_word(base + 3u)));
        if (any(size <= vec2<f32>(0.0))) { return 0.0; }
        let local = position - (origin + size * 0.5);
        var corner = 4u;
        if (local.x < 0.0) {
            if (local.y >= 0.0) { corner = 7u; }
        } else {
            corner = select(5u, 6u, local.y >= 0.0);
        }
        let radius = bitcast<f32>(clip_word(base + corner));
        let q = abs(local) - size * 0.5 + radius;
        var distance = max(q.x, q.y);
        if (radius != 0.0) {
            distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
        }
        coverage = min(coverage, clamp(0.5 - distance, 0.0, 1.0));
        cursor = clip_word(base + 8u);
    }
    return coverage;
}
