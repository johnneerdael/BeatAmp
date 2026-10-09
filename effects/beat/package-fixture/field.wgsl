// Independently authored portable graph fixture. CC0-1.0.
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let corners = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var output: VertexOut;
    output.position = vec4(corners[index], 0.0, 1.0);
    output.uv = corners[index] * vec2(0.5, -0.5) + vec2(0.5);
    return output;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4<f32> {
    let ribbon = 0.5 + 0.5 * sin(input.uv.x * 13.0 + sin(input.uv.y * 9.0) * 2.0);
    let color = mix(vec3(0.01, 0.025, 0.06), vec3(0.15, 0.53, 0.43), pow(ribbon, 3.0));
    return vec4(color, 1.0);
}
