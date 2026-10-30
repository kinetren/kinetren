// kaleidoscope (R.47): a mirror of the plugin's f64 `warp` (plugin-api-stage1, D11.2); kt-gpu pastes it
// into BOTH primitive loops of the fused pass. a.yzw = k[0..3], b.xyzw = k[3..7].
fn warp_kaleidoscope(pt: vec2<f32>, a: vec4<f32>, b: vec4<f32>) -> vec2<f32> {
    // kaleidoscope: a triangle wave over the sector (mirror repetition without int modulo —
    // floor is the same in both texts)
    let r = sqrt(pt.x * pt.x + pt.y * pt.y);
    let q = (atan2(pt.y, pt.x) - a.z) / a.y;
    let hh = q / 2.0 - floor(q / 2.0);
    let m = 1.0 - abs(2.0 * hh - 1.0);
    let ang = a.z + m * a.y;
    return vec2<f32>(r * cos(ang), r * sin(ang));
}
