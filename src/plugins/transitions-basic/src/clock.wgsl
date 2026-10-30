// clock: centre (p0.zw)
fn trans_clock(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    var ang = atan2(uv.x - p0.z, -(uv.y - p0.w));
    if (ang < 0.0) { ang = ang + 2.0 * pi; }
    if (p0.x > 0.5) { ang = 2.0 * pi - ang; }
    return mix(samp(0u, uv), samp(1u, uv), front_mask(ang / (2.0 * pi), k, p0.y));
}
