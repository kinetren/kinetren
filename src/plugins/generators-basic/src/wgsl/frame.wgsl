// A border frame. p0 = colour, p1 = (centre, half sizes), p2 = (corner radius, thickness, feather).
fn pixel(x: u32, y: u32) -> u32 {
    let q = vec2<f32>(f32(x), f32(y)) + 0.5;
    let d = abs(sd_box(q, u.p1.xy, u.p1.zw, u.p2.x)) - u.p2.y * 0.5;
    return shape(d, u.p2.z);
}
