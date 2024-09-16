@group(0) @binding(0) var y_texture: texture_2d<f32>;
@group(0) @binding(1) var u_texture: texture_2d<f32>;
@group(0) @binding(2) var v_texture: texture_2d<f32>;
@group(0) @binding(3) var sampler_: sampler;

@fragment fn main(@location(0) coords: vec2<f32>) -> @location(0) vec4<f32> {
    let y = textureSample(y_texture, sampler_, coords).r;
    let u = textureSample(u_texture, sampler_, coords).r - 0.5;
    let v = textureSample(v_texture, sampler_, coords).r - 0.5;

    let r = y + 1.402 * v;
    let g = y - 0.344 * u - 0.714 * v;
    let b = y + 1.772 * u;

    return vec4<f32>(r, g, b, 1.0);
}
