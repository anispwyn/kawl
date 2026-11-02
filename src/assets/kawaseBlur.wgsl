struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

struct Uniforms {
    resolution: vec2f,
    offset: f32};
  @group(0) @binding(0) var<uniform> u: Uniforms;
  @group(0) @binding(1) var linearSampler: sampler;
  @group(0) @binding(2) var source: texture_2d<f32>;

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    let texelSize = 1.0 / u.resolution;
    var color = vec4f(0.0);
    color += textureSample(source, linearSampler, in.uv + vec2f(-u.offset, -u.offset) * texelSize);
    color += textureSample(source, linearSampler, in.uv + vec2f(u.offset, -u.offset) * texelSize);
    color += textureSample(source, linearSampler, in.uv + vec2f(-u.offset, u.offset) * texelSize);
    color += textureSample(source, linearSampler, in.uv + vec2f(u.offset, u.offset) * texelSize);
    return color * 0.25;
}
