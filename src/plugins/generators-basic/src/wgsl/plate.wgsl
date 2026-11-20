// A lower-third plate. p0 = the bar's colour (0..255, alpha 0..255), p1 = (left, top, width, height) px,
// p2 = (corner radius, accent width, reveal 0..1, feather), p3 = the accent's colour (0..255, alpha).
fn pixel(x: u32, y: u32) -> u32 {
    let q = vec2<f32>(f32(x), f32(y)) + 0.5;
    let half = u.p1.zw * 0.5;
    let c = u.p1.xy + half;
    let bar = sd_box(q, c, half, u.p2.x);
    // the reveal wipes in from the left edge
    let shown = q.x - (u.p1.x + u.p1.z * clamp(u.p2.z, 0.0, 1.0));
    let fe = u.p2.w;
    let cov = cover(max(bar, shown), fe);
    let acc = cover(max(max(bar, q.x - (u.p1.x + u.p2.y)), shown), fe);
    let b = vec4<f32>(u.p0.xyz, u.p0.w / 255.0 * cov);
    let t = vec4<f32>(u.p3.xyz, u.p3.w / 255.0 * acc);
    return over(t, b);
}
