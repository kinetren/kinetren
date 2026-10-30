// The quad of the sides of the 2.5D exchange: the sample inside the rectangle, outside transparent,
// the far side slightly darker (a mirror of the CPU kernel’s quad()).
fn swap_quad(which: u32, uv: vec2<f32>, vert: bool, off: f32, scale: f32, wave: f32) -> vec4<f32> {
    var q = vec2<f32>(0.5 + off, 0.5);
    if (vert) { q = vec2<f32>(0.5, 0.5 + off); }
    let su = (uv.x - q.x) / scale + 0.5;
    let sv = (uv.y - q.y) / scale + 0.5;
    if (su >= 0.0 && su < 1.0 && sv >= 0.0 && sv < 1.0) {
        let c = samp(which, vec2<f32>(su, sv));
        let shade = 1.0 - 0.25 * wave;
        return vec4<f32>(c.rgb * shade, c.a);
    }
    return vec4<f32>(0.0);
}

// swap: 2.5D exchange, the order changes at the midpoint
fn trans_swap(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let vert = p0.x > 0.5;
    let wave = sin(pi * k);
    let off = 0.3 * wave;
    let scale = max(1.0 - p0.y * wave, 0.05);
    let sa = swap_quad(0u, uv, vert, off, scale, wave);
    let sb = swap_quad(1u, uv, vert, -off, scale, wave);
    if (k < 0.5) { return over_straight(sa, sb); }
    return over_straight(sb, sa);
}
