struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

// Uniform block layout. It must stay exactly 64 bytes because the Rust side
// allocates 64-byte uniform buffers (`UNIFORM_BYTES` in renderer.rs) and fills
// them by index. `params.w` is padding so the first two members are vec4s.
struct Uniforms {
    // saturation, dithering, scale, unused
    params: vec4f,
    // xy: per-frame tile shift, zw: unused
    noiseOffset: vec4f,
    // red xy, green zw; blue xy, zw: padding
    channelOffsets: array<vec4f, 2>,
};
  @group(0) @binding(0) var<uniform> u: Uniforms;
  @group(0) @binding(1) var linearSampler: sampler;
  @group(0) @binding(2) var source: texture_2d<f32>;
  @group(0) @binding(3) var blueNoiseTile: texture_2d<f32>;

// 64 is the blue-noise tile edge in texels (BLUE_NOISE_SIZE in renderer.rs).
// Indexing with modulo means the sampler never sees out-of-range coordinates,
// so the texture can stay CLAMP_TO_EDGE.
fn blueNoise(position: vec2f, offset: vec2f) -> f32 {
    let texel = vec2i(floor(position + u.noiseOffset.xy + offset)) % 64;
    return (textureLoad(blueNoiseTile, texel, 0).r * 255.0 + 0.5) / 256.0;
}

  // Uniform (0, 1) to triangular (-1, 1)
fn triangular(value: f32) -> f32 {
    let v = value * 2.0 - 1.0;
    return sign(v) * (1.0 - sqrt(1.0 - abs(v)));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let saturation = u.params.x;
    let dithering = u.params.y;
    let scale = u.params.z;
    let uv = clamp((in.uv - 0.5) / scale + 0.5, vec2f(0.0), vec2f(1.0));

    var color = textureSample(source, linearSampler, uv);

    let center = in.uv - 0.5;
    let vignette = 1.0 - dot(center, center) * 0.3;
    var rgb = color.rgb * vignette;

    let gray = dot(rgb, vec3f(0.299, 0.587, 0.114));
    rgb = mix(vec3f(gray), rgb, saturation);

    let noise = vec3f(
        triangular(blueNoise(in.position.xy, u.channelOffsets[0].xy)),
        triangular(blueNoise(in.position.xy, u.channelOffsets[0].zw)),
        triangular(blueNoise(in.position.xy, u.channelOffsets[1].xy)),
    );
    rgb += noise * (dithering * 0.5);
    return vec4f(rgb, color.a);
}
