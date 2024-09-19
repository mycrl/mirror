@group(0) @binding(0) var any_texture: texture_2d<f32>;
@group(0) @binding(1) var any_sampler: sampler;

@fragment fn main(@location(0) coords: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(any_texture, any_sampler, coords);
}
