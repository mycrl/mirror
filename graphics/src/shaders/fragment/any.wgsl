@group(0) @binding(0) var texture_: texture_2d<f32>;
@group(0) @binding(1) var sampler_: sampler;

@fragment fn main(@location(0) coords: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(texture_, sampler_, coords);
}
