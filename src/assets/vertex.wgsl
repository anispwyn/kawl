struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

fn corner(index: u32) -> vec2f {
    var corners = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    return corners[index];
}

@vertex
fn vs_target(@builtin(vertex_index) index: u32) -> VertexOutput {
    let p = corner(index);
    var out: VertexOutput;
    out.position = vec4f(p, 0.0, 1.0);
    out.uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@vertex
fn vs_screen(@builtin(vertex_index) index: u32) -> VertexOutput {
    let p = corner(index);
    var out: VertexOutput;
    out.position = vec4f(p, 0.0, 1.0);
    out.uv = vec2f(p.x * 0.5 + 0.5, 0.5 + p.y * 0.5);
    return out;
}

