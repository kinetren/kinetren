// a = the frame's seed rotated, b = mono, p0.x = amount. The cell of the base grid 1920×1080 and the
// tick decide the grain, so the picture is the same at any size.
fn lowbias32(v: u32) -> u32 {
    var x = v;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    return x ^ (x >> 16u);
}

fn pixel(x: u32, y: u32) -> u32 {
    let row = lowbias32((y * 1080u / max(u.h, 1u)) ^ u.a);
    let s = lowbias32((x * 1920u / max(u.w, 1u)) ^ row);
    var c = vec3<f32>(f32(s & 255u), f32((s >> 8u) & 255u), f32((s >> 16u) & 255u));
    if u.b != 0u { c = c.xxx; }
    return pack(vec4<f32>(128.0 + (c - 128.0) * u.p0.x, 255.0));
}
