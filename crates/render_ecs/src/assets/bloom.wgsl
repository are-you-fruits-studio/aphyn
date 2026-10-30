@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> bloom_uniform: BloomUniform;
@group(0) @binding(3) var<storage, read> storage_layers: array<BloomStorageLayer>;

struct BloomUniform {
    threshold: vec2<f32>,
    intensity: f32,
}

struct BloomStorageLayer {
    uv_scale_offset: vec4<f32>,
    direction: vec2<f32>,
    intensity: f32,
}
