
struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

struct Uniforms {
    response: f32,
    minStep: f32};
  @group(0) @binding(0) var<uniform> u: Uniforms;
  @group(0) @binding(1) var linearSampler: sampler;
  @group(0) @binding(2) var current: texture_2d<f32>;
  @group(0) @binding(3) var history: texture_2d<f32>;

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    let previous = textureSample(history, linearSampler, in.uv);
    let next = textureSample(current, linearSampler, in.uv);
    let blended = mix(previous, next, u.response);
    let remaining = next - previous;
    let minimal = previous + sign(remaining) * min(abs(remaining), vec4f(u.minStep));
    return mix(minimal, blended, step(abs(minimal - previous), abs(blended - previous)));
}
