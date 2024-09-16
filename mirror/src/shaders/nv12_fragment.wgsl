@group(0) @binding(0) var texture_: texture_2d<f32>;
@group(0) @binding(1) var sampler_: sampler;

@fragment fn main(@location(0) coords: vec2<f32>) -> @location(0) vec4<f32> {
    let y = textureSample(texture_, sampler_, coords);
    let uv_coord = vec2(coords.x * 2.0, floor(coords.y * 2.0) / 2.0);
    let uv = textureSample(texture_, sampler_, uv_coord);
    
    let r = y + 1.402 * (uv.r - 0.5);
    let g = y - 0.344 * (uv.r - 0.5) - 0.714 * (uv.g - 0.5);
    let b = y + 1.772 * (uv.g - 0.5);

    return vec4(r, g, b, 1.0);
}
