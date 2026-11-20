// Floating particles — soft bokeh discs drifting up (gen-backgrounds). a = count, b = seed;
// p0 = background (0..255), p1 = particle colour (0..255) and its strength (w, 0..1),
// p2 = (phase = t·speed, size as a fraction of the height, softness 0..1, aspect w/h).
fn h32(v: u32) -> u32 {
    var x = v;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    return x ^ (x >> 16u);
}

fn unit(v: u32) -> f32 {
    return f32(h32(v) & 0xffffu) / 65535.0;
}

fn pixel(x: u32, y: u32) -> u32 {
    let a = u.p2.w;
    let q = vec2<f32>((f32(x) + 0.5) / f32(u.w) * a, (f32(y) + 0.5) / f32(u.h));
    var glow = 0.0;
    for (var i = 0u; i < u.a; i += 1u) {
        let k = u.b * 7919u + i * 4u;
        let r = u.p2.y * (0.4 + 1.2 * unit(k + 2u));
        // drift: up at its own pace, a small sway; wrapped so the field never empties
        let rise = u.p2.x * (0.3 + 0.7 * unit(k + 3u));
        let cx = fract(unit(k) + 0.03 * sin(rise * 6.2831853 + f32(i))) * a;
        let cy = 1.0 - fract(unit(k + 1u) + rise);
        let d = length(q - vec2<f32>(cx, cy)) / r;
        let edge = max(u.p2.z, 0.02);
        glow += (0.35 + 0.65 * unit(k + 5u)) * clamp((1.0 - d) / edge, 0.0, 1.0);
    }
    let col = u.p0.xyz + u.p1.xyz * u.p1.w * min(glow, 1.5);
    return pack(vec4<f32>(col, 255.0));
}
