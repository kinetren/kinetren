// A waveform. p0 = colour (0..255, alpha 0..255), p1 = (centre y, half height, thickness, gain) px,
// p2 = (style: 0 line, 1 filled, 2 mirror; feather; 0; 0).
fn pixel(x: u32, y: u32) -> u32 {
    let q = vec2<f32>(f32(x), f32(y)) + 0.5;
    let v = clamp(wave_at(q.x / f32(u.w)) * u.p1.w, -1.0, 1.0) * u.p1.y;
    let dy = q.y - u.p1.x;
    let fe = u.p2.y;
    let half = u.p1.z * 0.5;
    var d = abs(dy + v) - half;
    if u.p2.x > 1.5 {
        d = abs(dy) - max(abs(v), half);
    } else if u.p2.x > 0.5 {
        let lo = min(0.0, -v);
        let hi = max(0.0, -v);
        d = max(lo - half - dy, dy - hi - half);
    }
    return color(u.p0, cover(d, fe));
}
