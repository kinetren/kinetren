// shake: jerks; the side cuts over at 0.5
fn trans_shake(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let steps = max(p0.y, 1.0);
    let i = min(u32(k * steps), u32(steps) - 1u);
    let amp = p0.x * 0.2 * sin(pi * k);
    let ox = (hash01(i * 2u + 1u) - 0.5) * amp;
    let oy = (hash01(i * 2u + 2u) - 0.5) * amp;
    var which = 1u;
    if (k < 0.5) { which = 0u; }
    return samp(which, uv + vec2<f32>(ox, oy));
}
