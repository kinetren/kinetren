// A progress bar. p0 = the track's colour, p3 = the fill's colour (0..255, alpha 0..255),
// p1 = (left, top, width, height) px, p2 = (corner radius, progress 0..1, feather).
fn pixel(x: u32, y: u32) -> u32 {
    let q = vec2<f32>(f32(x), f32(y)) + 0.5;
    let half = u.p1.zw * 0.5;
    let track = sd_box(q, u.p1.xy + half, half, u.p2.x);
    let edge = q.x - (u.p1.x + u.p1.z * clamp(u.p2.y, 0.0, 1.0));
    let fe = u.p2.z;
    let b = vec4<f32>(u.p0.xyz, u.p0.w / 255.0 * cover(track, fe));
    let t = vec4<f32>(u.p3.xyz, u.p3.w / 255.0 * cover(max(track, edge), fe));
    return over(t, b);
}
