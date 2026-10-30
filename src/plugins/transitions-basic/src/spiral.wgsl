// spiral: the Archimedean front around the centre (p0.zw)
fn trans_spiral(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let od = out_dims();
    let aspect = od.x / od.y;
    let pi = 3.14159265;
    let cx = uv.x - p0.z;
    let cy = uv.y - p0.w;
    var ang = atan2(cx, -cy);
    if (ang < 0.0) { ang = ang + 2.0 * pi; }
    let theta = ang / (2.0 * pi);
    let ex = max(p0.z, 1.0 - p0.z) * aspect;
    let ey = max(p0.w, 1.0 - p0.w);
    let rmax = max(sqrt(ex * ex + ey * ey), 1e-6);
    let r = sqrt(cx * aspect * (cx * aspect) + cy * cy) / rmax;
    let turns = max(p0.x, 0.25);
    let sfrac = (theta + turns * r) / (1.0 + turns);
    return mix(samp(0u, uv), samp(1u, uv), front_mask(sfrac, k, p0.y));
}
