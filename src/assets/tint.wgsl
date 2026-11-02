struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

struct Uniforms {
    color: vec3f,
    intensity: f32};
  @group(0) @binding(0) var<uniform> u: Uniforms;
  @group(0) @binding(1) var linearSampler: sampler;
  @group(0) @binding(2) var source: texture_2d<f32>;

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    var color = textureSample(source, linearSampler, in.uv);
    let luma = dot(color.rgb, vec3f(0.299, 0.587, 0.114));
    // darkMask: 1.0 for black, 0.0 for luma >= 0.5
    let darkMask = 1.0 - smoothstep(0.0, 0.5, luma);
    color = vec4f(mix(color.rgb, u.color, darkMask * u.intensity), color.a);
    return color;
}
