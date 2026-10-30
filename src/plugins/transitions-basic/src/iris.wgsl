// iris: centre (p0.zw), rmax up to the far corner (item 400)
fn trans_iris(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let od = out_dims();
    let aspect = od.x / od.y;
    let c = vec2<f32>((uv.x - p0.z) * aspect, uv.y - p0.w);
    let e = vec2<f32>(max(p0.z, 1.0 - p0.z) * aspect, max(p0.w, 1.0 - p0.w));
    let shape = i32(p0.x);
    var r = length(c);
    var rmax = max(length(e), 1e-6);
    if (shape == 1) { r = abs(c.x) + abs(c.y); rmax = max(e.x + e.y, 1e-6); }
    if (shape == 2) { r = max(abs(c.x), abs(c.y)); rmax = max(max(e.x, e.y), 1e-6); }
    return mix(samp(0u, uv), samp(1u, uv), front_mask(r / rmax, k, p0.y));
}
