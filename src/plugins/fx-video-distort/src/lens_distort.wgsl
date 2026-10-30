// lens_distort (R.47): a mirror of the plugin's f64 `warp` (plugin-api-stage1, D11.2); kt-gpu pastes it
// into BOTH primitive loops of the fused pass. a.yzw = k[0..3], b.xyzw = k[3..7].
fn warp_lens_distort(pt: vec2<f32>, a: vec4<f32>, b: vec4<f32>) -> vec2<f32> {
    // lens: a polynomial s = 1 + k1·r² + k2·r⁴ (no trigonometry)
    let r2 = pt.x * pt.x + pt.y * pt.y;
    let sc = 1.0 + a.y * r2 + a.z * r2 * r2;
    return vec2<f32>(pt.x * sc, pt.y * sc);
}
