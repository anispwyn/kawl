struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

struct Uniforms {
    time: f32,
    intensity: f32};
  @group(0) @binding(0) var<uniform> u: Uniforms;
  @group(0) @binding(1) var linearSampler: sampler;
  @group(0) @binding(2) var source: texture_2d<f32>;

fn mod289v3(x: vec3f) -> vec3f { return x - floor(x * (1.0 / 289.0)) * 289.0; }
fn mod289v2(x: vec2f) -> vec2f { return x - floor(x * (1.0 / 289.0)) * 289.0; }
fn permute(x: vec3f) -> vec3f { return mod289v3(((x * 34.0) + 1.0) * x); }

fn snoise(v: vec2f) -> f32 {
    let C = vec4f(0.211324865405187, 0.366025403784439,
        -0.577350269189626, 0.024390243902439);
    var i = floor(v + dot(v, C.yy));
    let x0 = v - i + dot(i, C.xx);
    let i1 = select(vec2f(0.0, 1.0), vec2f(1.0, 0.0), x0.x > x0.y);
    var x12 = x0.xyxy + C.xxzz;
    x12 = vec4f(x12.xy - i1, x12.zw);
    i = mod289v2(i);
    let p = permute(permute(i.y + vec3f(0.0, i1.y, 1.0)) + i.x + vec3f(0.0, i1.x, 1.0));
    var m = max(0.5 - vec3f(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), vec3f(0.0));
    m = m * m;
    m = m * m;
    let x = 2.0 * fract(p * C.www) - 1.0;
    let h = abs(x) - 0.5;
    let ox = floor(x + 0.5);
    let a0 = x - ox;
    m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
    let g = vec3f(a0.x * x0.x + h.x * x0.y, a0.yz * x12.xz + h.yz * x12.yw);
    return 130.0 * dot(m, g);
}

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4f {
    let uv = in.uv;
    let t = u.time * 0.05;

    let center = uv - 0.5;
    let centerWeight = 1.0 - smoothstep(0.0, 0.7, length(center));

    // Large-scale movement (slow, big blobs)
    let n1 = snoise(uv * 0.35 + vec2f(t, t * 0.7));
    let n2 = snoise(uv * 0.35 + vec2f(-t * 0.8, t * 0.5) + vec2f(50.0, 50.0));

    // Medium-scale detail (adds organic movement)
    let n3 = snoise(uv * 0.9 + vec2f(t * 1.2, -t) + vec2f(100.0, 0.0));
    let n4 = snoise(uv * 0.9 + vec2f(-t, t * 1.1) + vec2f(0.0, 100.0));

    // Combine two octaves
    let warp = vec2f(n1 * 0.65 + n3 * 0.35, n2 * 0.65 + n4 * 0.35) * centerWeight;
    let warpedUV = clamp(uv + warp * u.intensity, vec2f(0.0), vec2f(1.0));
    return textureSample(source, linearSampler, warpedUV);
}
