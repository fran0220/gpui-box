@group(3) @binding(0) var<storage, read> clip_words: array<u32>;

fn clip_word(index: u32) -> u32 {
    return clip_words[index];
}
