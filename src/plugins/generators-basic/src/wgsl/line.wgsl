// p0 = colour, p1 = (a, b), p2 = (half thickness, feather): a capsule in pixels.
fn pixel(x: u32, y: u32) -> u32 {
    let v = u.p1.zw - u.p1.xy;
    let at = vec2<f32>(f32(x), f32(y)) + 0.5 - u.p1.xy;
    let len2 = dot(v, v);
    var t = 0.0;
    if len2 > 0.0 { t = clamp(dot(at, v) / len2, 0.0, 1.0); }
    return shape(length(at - v * t) - u.p2.x, u.p2.y);
}
