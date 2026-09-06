// Композит слоя поверх аккумулятора — зеркало ядер kt-engine/src/video.rs
// (alpha_over / scale_alpha_into / scale_alpha_inplace / dissolve_into),
// ЦЕЛОЧИСЛЕННО и байт-в-байт: формула over не выражается fixed-function
// блендингом (множитель (1−sa)·da), поэтому ping-pong двух аккумуляторов.
// T0 = acc_in, T1 = слой A, T2 = слой B (dissolve).
// u0 = (mode, alpha_k, a_has_alpha, b_has_alpha);
// u1 = (first, k, 0, 0). mode: 0 direct, 1 scaled, 2 dissolve.

fn over(dst: vec4<u32>, src: vec4<u32>) -> vec4<u32> {
    let sa = src.a;
    if (sa == 255u) { return src; }
    if (sa == 0u) { return dst; }
    let da = dst.a;
    let inv = 255u - sa;
    let rgb = min((src.rgb * sa + dst.rgb * inv * da / 255u) / 255u, vec3<u32>(255u));
    let a = min(sa + da * inv / 255u, 255u);
    return vec4<u32>(rgb, a);
}

// scale_alpha_into: копия RGB, альфа ×k/4096 с округлением (+2048>>12);
// кадр без альфы считается непрозрачным (A = 255).
fn scale_into(c: vec4<u32>, k: u32, has_alpha: u32) -> vec4<u32> {
    var a = 255u;
    if (has_alpha == 1u) { a = c.a; }
    return vec4<u32>(c.rgb, (a * k + 2048u) >> 12u);
}

// dissolve_into: (a·ik + b·k + 2048) >> 12 по всем каналам.
fn dis(a: vec4<u32>, b: vec4<u32>, k: u32, a_has: u32, b_has: u32) -> vec4<u32> {
    var av = a;
    var bv = b;
    if (a_has == 0u) { av.a = 255u; }
    if (b_has == 0u) { bv.a = 255u; }
    return (av * (4096u - k) + bv * k + vec4<u32>(2048u)) >> vec4<u32>(12u);
}

// ---- переходы (зеркало transitions.rs) -------------------------------------

fn t_dims(which: u32) -> vec2<i32> {
    if (which == 0u) { return vec2<i32>(textureDimensions(T1, 0)); }
    return vec2<i32>(textureDimensions(T2, 0));
}

fn out_dims() -> vec2<f32> {
    if ((P.u1.w & 1u) != 0u) { return vec2<f32>(t_dims(1u)); }
    return vec2<f32>(t_dims(0u));
}

fn t_texel(which: u32, q0: vec2<i32>) -> vec4<f32> {
    let d = t_dims(which);
    let q = clamp(q0, vec2<i32>(0, 0), d - vec2<i32>(1, 1));
    var c: vec4<u32>;
    var has = P.u0.w;
    if (which == 0u) { c = textureLoad(T1, q, 0); has = P.u0.z; }
    else { c = textureLoad(T2, q, 0); }
    var a = f32(c.a);
    if (has == 0u) { a = 255.0; }
    return vec4<f32>(f32(c.r), f32(c.g), f32(c.b), a);
}

fn samp(which: u32, uv: vec2<f32>) -> vec4<f32> {
    if ((P.u1.w & (1u << which)) != 0u) { return vec4<f32>(0.0, 0.0, 0.0, 0.0); }
    let d = vec2<f32>(t_dims(which));
    let s = uv * d - vec2<f32>(0.5, 0.5);
    let fl = floor(s);
    let f = s - fl;
    let i = vec2<i32>(fl);
    let top = mix(t_texel(which, i), t_texel(which, i + vec2<i32>(1, 0)), f.x);
    let bot = mix(t_texel(which, i + vec2<i32>(0, 1)), t_texel(which, i + vec2<i32>(1, 1)), f.x);
    return mix(top, bot, f.y);
}

fn front_mask(s: f32, k: f32, soft0: f32) -> f32 {
    let soft = clamp(soft0, 0.0, 1.0);
    if (soft < 1e-4) {
        if (s < k) { return 1.0; }
        return 0.0;
    }
    let front = k * (1.0 + soft) - soft;
    return 1.0 - smoothstep(front, front + soft, s);
}

fn t_hash(x0: u32) -> u32 {
    var x = x0;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

fn hash01(x: u32) -> f32 {
    return f32(t_hash(x) >> 8u) / 16777216.0;
}

fn quant(x: f32, step: f32) -> f32 {
    if (step <= 0.0) { return x; }
    return floor(x / step) * step + step * 0.5;
}

fn rot_sample(which: u32, uv: vec2<f32>, ang: f32, zoom: f32, aspect: f32) -> vec4<f32> {
    let s = sin(ang);
    let c = cos(ang);
    let cx = (uv.x - 0.5) * aspect / zoom;
    let cy = (uv.y - 0.5) / zoom;
    return samp(which, vec2<f32>((cx * c - cy * s) / aspect + 0.5, cx * s + cy * c + 0.5));
}

fn box9(which: u32, uv: vec2<f32>, r: vec2<f32>) -> vec4<f32> {
    if (r.x <= 0.0 && r.y <= 0.0) { return samp(which, uv); }
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            acc = acc + samp(which, uv + vec2<f32>(f32(dx) * r.x, f32(dy) * r.y));
        }
    }
    return acc / 9.0;
}

fn glitch_pick(which: u32, uv: vec2<f32>, ch: f32) -> vec4<f32> {
    let r = samp(which, vec2<f32>(uv.x + ch, uv.y));
    let g = samp(which, uv);
    let b = samp(which, vec2<f32>(uv.x - ch, uv.y));
    return vec4<f32>(r.r, g.g, b.b, g.a);
}

fn trans(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let mode = P.u1.z;
    let p0 = P.f0;
    let od = out_dims();
    let aspect = od.x / od.y;
    let pi = 3.14159265;
    if (mode == 1u) { // wipe
        let ext = max(abs(p0.x) + abs(p0.y), 1e-6);
        let s = ((uv.x - 0.5) * p0.x + (uv.y - 0.5) * p0.y + ext * 0.5) / ext;
        return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.z));
    }
    if (mode == 2u) { // iris
        let c = vec2<f32>((uv.x - 0.5) * aspect, uv.y - 0.5);
        let shape = i32(p0.x);
        var r = length(c);
        var rmax = sqrt((aspect * aspect + 1.0) * 0.25);
        if (shape == 1) { r = abs(c.x) + abs(c.y); rmax = (aspect + 1.0) * 0.5; }
        if (shape == 2) { r = max(abs(c.x), abs(c.y)); rmax = max(aspect * 0.5, 0.5); }
        return mix(samp(0u, uv), samp(1u, uv), front_mask(r / rmax, k, p0.y));
    }
    if (mode == 3u) { // clock
        var ang = atan2(uv.x - 0.5, -(uv.y - 0.5));
        if (ang < 0.0) { ang = ang + 2.0 * pi; }
        if (p0.x > 0.5) { ang = 2.0 * pi - ang; }
        return mix(samp(0u, uv), samp(1u, uv), front_mask(ang / (2.0 * pi), k, p0.y));
    }
    if (mode == 4u) { // barn
        var s = abs(uv.y - 0.5) * 2.0;
        if (p0.x > 0.5) { s = abs(uv.x - 0.5) * 2.0; }
        return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.y));
    }
    if (mode == 5u) { // luma
        let c = samp(0u, uv);
        var l = (0.299 * c.r + 0.587 * c.g + 0.114 * c.b) / 255.0;
        if (p0.x > 0.5) { l = 1.0 - l; }
        return mix(c, samp(1u, uv), front_mask(l, k, p0.y));
    }
    if (mode == 6u) { // push
        let a = uv - p0.xy * k;
        if (a.x >= 0.0 && a.x < 1.0 && a.y >= 0.0 && a.y < 1.0) { return samp(0u, a); }
        return samp(1u, a - p0.xy);
    }
    if (mode == 7u) { // slide
        let b = uv + p0.xy * (1.0 - k);
        if (b.x >= 0.0 && b.x < 1.0 && b.y >= 0.0 && b.y < 1.0) { return samp(1u, b); }
        return samp(0u, uv);
    }
    if (mode == 8u) { // zoom
        let za = 1.0 + p0.x * k;
        let zb = 1.0 + p0.x * (1.0 - k);
        let sa = samp(0u, (uv - vec2<f32>(0.5)) / za + vec2<f32>(0.5));
        let sb = samp(1u, (uv - vec2<f32>(0.5)) / zb + vec2<f32>(0.5));
        return mix(sa, sb, k);
    }
    if (mode == 9u) { // spin
        let sa = rot_sample(0u, uv, p0.x * k * 2.0 * pi, 1.0 + p0.y * k, aspect);
        let sb = rot_sample(1u, uv, -p0.x * (1.0 - k) * 2.0 * pi, 1.0 + p0.y * (1.0 - k), aspect);
        return mix(sa, sb, k);
    }
    if (mode == 10u) { // whip
        let blur = p0.z * sin(pi * k);
        var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        for (var i = 0; i < 7; i = i + 1) {
            let o = (f32(i) / 6.0 - 0.5) * blur * 0.25;
            let s = uv + p0.xy * o;
            let a = s - p0.xy * k;
            if (a.x >= 0.0 && a.x < 1.0 && a.y >= 0.0 && a.y < 1.0) { acc = acc + samp(0u, a); }
            else { acc = acc + samp(1u, a - p0.xy); }
        }
        return acc / 7.0;
    }
    if (mode == 11u) { // blur dissolve
        let r = p0.x * sin(pi * k);
        let rr = vec2<f32>(r / od.x, r / od.y);
        return mix(box9(0u, uv, rr), box9(1u, uv, rr), k);
    }
    if (mode == 12u) { // pixelate
        let s = max(p0.x * sin(pi * k), 1.0);
        let b = vec2<f32>(quant(uv.x, s / od.x), quant(uv.y, s / od.y));
        return mix(samp(0u, b), samp(1u, b), k);
    }
    if (mode == 13u) { // glitch
        let amt = p0.x * sin(pi * k);
        let band = u32(floor(uv.y * 24.0));
        let shift = (hash01(band * 1973u) - 0.5) * amt * 0.25;
        let ch = amt * 8.0 / od.x;
        let s = vec2<f32>(uv.x + shift, uv.y);
        return mix(glitch_pick(0u, s, ch), glitch_pick(1u, s, ch), k);
    }
    if (mode == 14u) { // flash
        let base = mix(samp(0u, uv), samp(1u, uv), k);
        let w = clamp(p0.w * sin(pi * k), 0.0, 1.0);
        return vec4<f32>(mix(base.rgb, p0.rgb, w), base.a);
    }
    if (mode == 15u) { // dip
        var src = samp(1u, uv);
        var t = 2.0 - k * 2.0;
        if (k < 0.5) { src = samp(0u, uv); t = k * 2.0; }
        return vec4<f32>(mix(src.rgb, p0.rgb, t), src.a);
    }
    return mix(samp(0u, uv), samp(1u, uv), k);
}

/// Округление к ближайшему целому — так же, как CPU-ядро пишет байты.
fn to_u8(c: vec4<f32>) -> vec4<u32> {
    return vec4<u32>(clamp(floor(c + vec4<f32>(0.5)), vec4<f32>(0.0), vec4<f32>(255.0)));
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let mode = P.u0.x;
    let alpha_k = P.u0.y;
    let a_has = P.u0.z;
    let b_has = P.u0.w;
    var dst = vec4<u32>(0u); // first: аккумулятор прозрачно-чёрный
    if (P.u1.x == 0u) { dst = textureLoad(T0, p, 0); }
    let a = textureLoad(T1, p, 0);
    if (mode == 0u) {
        // alpha_over с быстрой веткой !has_alpha = копия слоя целиком
        if (a_has == 0u) { return a; }
        return over(dst, a);
    }
    if (mode == 1u) {
        return over(dst, scale_into(a, alpha_k, a_has));
    }
    let od = out_dims();
    let uv = (vec2<f32>(pos.xy)) / od;
    if (mode == 3u) {
        // сольный фейд с видом: одна сторона — пустота, форма из ядра
        var c = to_u8(trans(uv, f32(P.u1.y) / 4096.0));
        if (alpha_k < 4096u) { c.a = (c.a * alpha_k) / 4096u; }
        return over(dst, c);
    }
    if (P.u1.z != 0u) {
        // переход пары с видом (не кроссфейд)
        var c = to_u8(trans(uv, f32(P.u1.y) / 4096.0));
        if (alpha_k < 4096u) { c.a = (c.a * alpha_k) / 4096u; }
        return over(dst, c);
    }
    // dissolve пары; уровень поверх — scale_alpha_inplace (БЕЗ округления)
    let b = textureLoad(T2, p, 0);
    var m = dis(a, b, P.u1.y, a_has, b_has);
    var has = max(a_has, b_has);
    if (alpha_k < 4096u) {
        m.a = (m.a * alpha_k) / 4096u;
        has = 1u;
    }
    if (has == 0u) { return m; }
    return over(dst, m);
}
