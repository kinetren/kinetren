// Level rings. p0 = colour (0..255, alpha 0..255), p1 = (cx, cy, base radius, reaction) px,
// p2 = (rings, spacing, thickness, feather) px. The inner ring follows the level now, the outer ones
// the held level, each fainter.
fn pixel(x: u32, y: u32) -> u32 {
    let q = vec2<f32>(f32(x), f32(y)) + 0.5;
    let r = length(q - u.p1.xy);
    let n = max(u.p2.x, 1.0);
    var best = 0.0;
    for (var k = 0u; k < u32(n); k = k + 1u) {
        let f = f32(k) / n;
        var lvl = au[3];
        if k == 0u { lvl = max(au[0], au[1]); }
        let rk = u.p1.z + f32(k) * u.p2.y + lvl * u.p1.w * (1.0 - 0.5 * f);
        let a = cover(abs(r - rk) - u.p2.z * 0.5, u.p2.w) * (1.0 - f * 0.6);
        best = max(best, a);
    }
    return color(u.p0, best);
}
