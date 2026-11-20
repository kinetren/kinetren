// A regular polygon. p0 = colour, p1 = (centre, radius, sides), p2 = (rotation rad, feather).
fn pixel(x: u32, y: u32) -> u32 {
    let n = max(u.p1.w, 3.0);
    let q = rot(vec2<f32>(f32(x), f32(y)) + 0.5 - u.p1.xy, -u.p2.x);
    let an = 3.14159265 / n;
    let a = atan2(q.x, -q.y);
    let bn = a - 2.0 * an * floor((a + an) / (2.0 * an));
    var v = length(q) * vec2<f32>(cos(bn), abs(sin(bn)));
    v -= u.p1.z * vec2<f32>(cos(an), sin(an));
    v.y += clamp(-v.y, 0.0, u.p1.z * sin(an));
    return shape(length(v) * sign(v.x), u.p2.y);
}
