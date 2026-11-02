struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

struct Uniforms {
    texelSize: vec2f,
    flipY: f32};
  @group(0) @binding(0) var<uniform> u: Uniforms;
  @group(0) @binding(1) var linearSampler: sampler;
  @group(0) @binding(2) var source: texture_external;

fn tap(uv: vec2f) -> vec4f {
    return textureSampleBaseClampToEdge(source, linearSampler, uv);
}

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    let uv = vec2f(in.uv.x, mix(in.uv.y, 1.0 - in.uv.y, u.flipY));
    let offset = u.texelSize * 0.25;
    return 0.25 * (tap(uv + vec2f(-offset.x, -offset.y)) +
      tap(uv + vec2f(offset.x, -offset.y)) +
      tap(uv + vec2f(-offset.x, offset.y)) +
      tap(uv + vec2f(offset.x, offset.y)));
}
