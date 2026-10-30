// ripple (R.47): a mirror of the plugin's f64 `warp` (plugin-api-stage1, D11.2); kt-gpu pastes it
// into BOTH primitive loops of the fused pass. a.yzw = k[0..3], b.xyzw = k[3..7].
fn warp_ripple(pt: vec2<f32>, a: vec4<f32>, b: vec4<f32>) -> vec2<f32> {
    // ripple: r' = r + amp·sin(k·r − φ)
    let r = sqrt(pt.x * pt.x + pt.y * pt.y);
    if (r <= 1e-9) {
        return pt;
    }
    let k = (r + a.y * sin(a.z * r - a.w)) / r;
    return vec2<f32>(pt.x * k, pt.y * k);
}
