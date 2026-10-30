// blinds: the fraction inside the strip along the rotated axis
fn trans_blinds(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let ext = max(abs(p0.x) + abs(p0.y), 1e-6);
    let c = ((uv.x - 0.5) * p0.x + (uv.y - 0.5) * p0.y + ext * 0.5) / ext;
    let f = fract(c * max(p0.z, 1.0));
    return mix(samp(0u, uv), samp(1u, uv), front_mask(f, k, p0.w));
}
