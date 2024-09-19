struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) coords: vec2<f32>,
};

@vertex fn main(@location(0) position: vec2<f32>, @location(1) coords: vec2<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.coords = vec2<f32>(coords.x, 1.0 - coords.y);
    return output;
}
