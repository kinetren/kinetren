// Vignette (R.47): a mirror of the plugin's tap, a POSITIONED one (plugin-api-stage1, D11.1): one
// text for the fused pass's tap arm (c = the sub-sample's coordinate in the tap's stage) and its own
// generated pass (c = the pixel). a.yzw = (strength, radius, softness), b.zw = the stage's size
// (the host's half of `b`).
fn tap_vignette(v0: vec4<f32>, a: vec4<f32>, b: vec4<f32>, c: vec2<f32>) -> vec4<f32> {
    let st = a.y;
    let ra = a.z;
    let so = a.w;
    let sw = b.z;
    let sh1 = b.w;
    let ddx = (c.x + 0.5) / max(sw, 1.0) - 0.5;
    let ddy = (c.y + 0.5) / max(sh1, 1.0) - 0.5;
    let r = sqrt((2.0 * ddx) * (2.0 * ddx) + (2.0 * ddy) * (2.0 * ddy)) / sqrt(2.0);
    let tt = clamp((r - ra) / max(so, 1e-6), 0.0, 1.0);
    let ww = tt * tt * (3.0 - 2.0 * tt);
    return vec4<f32>(v0.xyz * (1.0 - st * ww), v0.w);
}
