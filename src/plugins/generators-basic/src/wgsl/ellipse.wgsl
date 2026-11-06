// p0 = colour, p1 = (centre, half sizes), p2.y = feather. The distance is scaled to the smaller
// half size: exact on a circle.
fn pixel(x: u32, y: u32) -> u32 {
    let m = min(u.p1.z, u.p1.w);
    let e = (vec2<f32>(f32(x), f32(y)) + 0.5 - u.p1.xy) * m / u.p1.zw;
    return shape(length(e) - m, u.p2.y);
}
