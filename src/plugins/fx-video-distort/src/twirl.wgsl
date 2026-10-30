// twirl (R.47): a mirror of the plugin's f64 `warp` (plugin-api-stage1, D11.2); kt-gpu pastes it
// into BOTH primitive loops of the fused pass. a.yzw = k[0..3], b.xyzw = k[3..7].
fn warp_twirl(pt: vec2<f32>, a: vec4<f32>, b: vec4<f32>) -> vec2<f32> {
    // twirl: rotation by a·(1 − r/rad)² inside the radius
    let r = sqrt(pt.x * pt.x + pt.y * pt.y);
    if (r >= a.z) {
        return pt;
    }
    let t = 1.0 - r / a.z;
    let th = a.y * t * t;
    let sn = sin(th);
    let cs = cos(th);
    return vec2<f32>(pt.x * cs - pt.y * sn, pt.x * sn + pt.y * cs);
}
