// A star. p0 = colour, p1 = (centre, outer radius, points), p2 = (rotation rad, feather, inner ratio).
fn pixel(x: u32, y: u32) -> u32 {
    let n = max(u.p1.w, 3.0);
    let q = rot(vec2<f32>(f32(x), f32(y)) + 0.5 - u.p1.xy, -u.p2.x);
    let an = 3.14159265 / n;
    // fold into one half-sector: the tip at angle 0, the valley at angle an
    let a = atan2(q.x, -q.y);
    let b = abs(a - 2.0 * an * floor((a + an) / (2.0 * an)));
    let p = length(q) * vec2<f32>(cos(b), sin(b));
    let tip = vec2<f32>(u.p1.z, 0.0);
    let valley = u.p1.z * u.p2.z * vec2<f32>(cos(an), sin(an));
    let d = sd_segment(p, tip, valley);
    // inside when on the origin's side of the edge tip→valley
    let e = valley - tip;
    let side = e.x * (p.y - tip.y) - e.y * (p.x - tip.x);
    let s = select(1.0, -1.0, side > 0.0);
    return shape(d * s, u.p2.y);
}
