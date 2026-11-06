// p0 = colour, p1 = (centre, half sizes), p2 = (radius, feather): a rounded rectangle in pixels.
fn pixel(x: u32, y: u32) -> u32 {
    let at = abs(vec2<f32>(f32(x), f32(y)) + 0.5 - u.p1.xy) - (u.p1.zw - u.p2.x);
    var d = max(at.x, at.y) - u.p2.x;
    if at.x > 0.0 && at.y > 0.0 { d = length(at) - u.p2.x; }
    return shape(d, u.p2.y);
}
