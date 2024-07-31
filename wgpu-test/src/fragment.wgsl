@group(0) @binding(0) var mySampler: sampler;
@group(0) @binding(1) var myTexture: texture_external;

// The main function of the fragment shader
@fragment
fn frag_main(@location(0) uv : vec2<f32>) -> @location(0) vec4<f32> {
    return textureSampleBaseClampToEdge(myTexture, mySampler, uv);
}
