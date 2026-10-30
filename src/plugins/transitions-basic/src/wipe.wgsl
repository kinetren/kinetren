// wipe (R.33): a mirror of the plugin's `pixel` (plugin-api-stage1, D12); kt-gpu pastes it into
// composite.wgsl. The edge moves across the frame along p0.xy (the direction turned by the angle).
fn trans_wipe(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let ext = max(abs(p0.x) + abs(p0.y), 1e-6);
    let s = ((uv.x - 0.5) * p0.x + (uv.y - 0.5) * p0.y + ext * 0.5) / ext;
    return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.z));
}
