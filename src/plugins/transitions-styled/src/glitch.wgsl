fn glitch_pick(which: u32, uv: vec2<f32>, ch: f32) -> vec4<f32> {
    let r = samp(which, vec2<f32>(uv.x + ch, uv.y));
    let g = samp(which, uv);
    let b = samp(which, vec2<f32>(uv.x - ch, uv.y));
    return vec4<f32>(r.r, g.g, b.b, g.a);
}

// glitch
fn trans_glitch(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let od = out_dims();
    let pi = 3.14159265;
    let amt = p0.x * sin(pi * k);
    let band = u32(floor(uv.y * 24.0));
    let shift = (hash01(band * 1973u) - 0.5) * amt * 0.25;
    let ch = amt * 8.0 / od.x;
    let s = vec2<f32>(uv.x + shift, uv.y);
    return mix(glitch_pick(0u, s, ch), glitch_pick(1u, s, ch), k);
}
