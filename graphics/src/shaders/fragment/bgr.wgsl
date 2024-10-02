@group(0) @binding(0) var bgr_texture: texture_2d<f32>;
@group(0) @binding(1) var bgr_sampler: sampler;

@fragment fn main(@location(0) coords: vec2<f32>) -> @location(0) vec4<f32> {
    let r = textureSample(bgr_texture, bgr_sampler, coords).b;
    let g = textureSample(bgr_texture, bgr_sampler, coords).g;
    let b = textureSample(bgr_texture, bgr_sampler, coords).r;
    return vec4<f32>(r, g, b, 1.0);
}
