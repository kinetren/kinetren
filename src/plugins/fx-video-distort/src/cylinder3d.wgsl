// cylinder3d (R.47): a mirror of the plugin's f64 `warp` (plugin-api-stage1, D11.2); kt-gpu pastes it
// into BOTH primitive loops of the fused pass. a.yzw = k[0..3], b.xyzw = k[3..7].
fn warp_cylinder3d(pt: vec2<f32>, a: vec4<f32>, b: vec4<f32>) -> vec2<f32> {
    // cylinder: u' = asin(u·sin a)/a along X, edges → edges
    return vec2<f32>(asin(clamp(pt.x * a.y, -1.0, 1.0)) * a.z, pt.y);
}
