@group(3) @binding(0) var clip_texture: texture_2d<u32>;

fn clip_word(index: u32) -> u32 {
    let texel = index / 4u;
    let width = textureDimensions(clip_texture).x;
    return textureLoad(clip_texture, vec2<i32>(i32(texel % width), i32(texel / width)), 0)[index % 4u];
}
