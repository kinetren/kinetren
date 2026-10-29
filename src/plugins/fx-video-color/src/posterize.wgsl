// Posterize (R.47; plugin-api-stage1): quantization per channel. ONE text: kt-gpu pastes it into
// the fused pass's tap dispatch and into a pass of its own. a.y = levels (2..32).
fn tap_posterize(v: vec4<f32>, a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    let n = a.y;
    let idx3 = clamp(floor(v.xyz * n / 256.0), vec3<f32>(0.0), vec3<f32>(n - 1.0));
    return vec4<f32>(idx3 * 255.0 / (n - 1.0), v.w);
}
