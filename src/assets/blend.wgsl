struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

struct Uniforms {
    amount: f32};
  @group(0) @binding(0) var<uniform> u: Uniforms;
  @group(0) @binding(1) var linearSampler: sampler;
  @group(0) @binding(2) var current: texture_2d<f32>;
  @group(0) @binding(3) var next: texture_2d<f32>;

  @fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    return mix(
        textureSample(current, linearSampler, in.uv),
        textureSample(next, linearSampler, in.uv),
        u.amount,
    );
}
