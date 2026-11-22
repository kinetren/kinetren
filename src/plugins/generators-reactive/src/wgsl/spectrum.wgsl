// A spectrum of 16 bars. p0 = bottom colour, p3 = top colour (0..255, alpha 0..255),
// p1 = (left, base line y, width, height) px, p2 = (gap fraction, held 0/1, style: 0 bars 1 mirror, feather).
fn pixel(x: u32, y: u32) -> u32 {
    let q = vec2<f32>(f32(x), f32(y)) + 0.5;
    let bw = u.p1.z / 16.0;
    let fx = (q.x - u.p1.x) / bw;
    if fx < 0.0 || fx >= 16.0 { return 0u; }
    let i = u32(floor(fx));
    var lvl = au[4u + i];
    if u.p2.y > 0.5 { lvl = au[20u + i]; }
    let inner = abs(fract(fx) - 0.5) * bw - (1.0 - u.p2.x) * bw * 0.5;
    let h = max(lvl * u.p1.w, 1.0);
    var up = u.p1.y - q.y;
    if u.p2.z > 0.5 { up = abs(up) * 2.0; }
    let d = max(inner, max(-up, up - h));
    let a = cover(d, u.p2.w);
    let k = clamp(up / max(u.p1.w, 1.0), 0.0, 1.0);
    let c = mix(u.p0, u.p3, k);
    return color(c, a);
}
