// Композит слоя поверх аккумулятора — зеркало ядер kt-engine/src/video.rs
// (alpha_over / scale_alpha_into / scale_alpha_inplace / dissolve_into),
// ЦЕЛОЧИСЛЕННО и байт-в-байт: формула over не выражается fixed-function
// блендингом (множитель (1−sa)·da), поэтому ping-pong двух аккумуляторов.
// T0 = acc_in, T1 = слой A, T2 = слой B (dissolve).
// u0 = (mode, alpha_k, a_has_alpha, b_has_alpha);
// u1 = (first, k | номер режима наложения, mix_mode, clear).
// mode: 0 direct, 1 scaled, 2 dissolve, 3 сольный фейд, 4 наложение (Р.62).

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

// РЕЖИМ НАЛОЖЕНИЯ дорожки (Р.62, F-CMP-003) — ЗЕРКАЛО `blend_ch` из
// kt-engine/src/video.rs. Правится ТЕМ ЖЕ КОММИТОМ, что CPU-ядро
// (инвариант 7): расхождение = превью и экспорт показывают разное.
fn blend_ch(mode: u32, b: f32, a: f32) -> f32 {
    if (mode == 1u) { return b * a; }                       // multiply
    if (mode == 2u) { return b + a - b * a; }               // screen
    if (mode == 3u) { return min(b + a, 1.0); }             // add
    if (mode == 4u) { return max(b - a, 0.0); }             // subtract
    if (mode == 5u) { return min(b, a); }                   // darken
    if (mode == 6u) { return max(b, a); }                   // lighten
    if (mode == 7u) {                                       // overlay
        if (b <= 0.5) { return 2.0 * b * a; }
        return 1.0 - 2.0 * (1.0 - b) * (1.0 - a);
    }
    if (mode == 8u) { return abs(b - a); }                  // difference
    return a;                                               // normal
}

// out = lerp(below, blend(below, above), a_above × уровень). Уровень и
// режим — РАЗНЫЕ оси, обе после альфы; дыра (a = 0) оставляет низ как был.
fn blend_over(dst: vec4<u32>, src: vec4<u32>, blend_mode: u32, has_alpha: u32, level_k: u32) -> vec4<u32> {
    var sa = 1.0;
    if (has_alpha == 1u) { sa = f32(src.a) / 255.0; }
    let k = sa * (f32(level_k) / 4096.0);
    if (k <= 0.0) { return dst; }
    var rgb = vec3<f32>(0.0);
    for (var ch = 0u; ch < 3u; ch = ch + 1u) {
        let b = f32(dst[ch]) / 255.0;
        let a = f32(src[ch]) / 255.0;
        let m = clamp(blend_ch(blend_mode, b, a), 0.0, 1.0);
        rgb[ch] = b + (m - b) * k;
    }
    let da = f32(dst.a) / 255.0;
    let oa = k + da * (1.0 - k);
    let c = clamp(floor(vec4<f32>(rgb * 255.0 + 0.5, oa * 255.0 + 0.5)), vec4<f32>(0.0), vec4<f32>(255.0));
    return vec4<u32>(c);
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

fn rot_sample(which: u32, uv: vec2<f32>, ang: f32, zoom: f32, aspect: f32, o: vec2<f32>) -> vec4<f32> {
    let s = sin(ang);
    let c = cos(ang);
    let cx = (uv.x - o.x) * aspect / zoom;
    let cy = (uv.y - o.y) / zoom;
    return samp(which, vec2<f32>((cx * c - cy * s) / aspect + o.x, cx * s + cy * c + o.y));
}

// Straight-alpha «top поверх bottom» в f32 — зеркало over_straight (Р.51).
fn over_straight(top: vec4<f32>, bot: vec4<f32>) -> vec4<f32> {
    let ta = clamp(top.a / 255.0, 0.0, 1.0);
    let ba = clamp(bot.a / 255.0, 0.0, 1.0);
    let oa = ta + ba * (1.0 - ta);
    if (oa <= 0.0) { return vec4<f32>(0.0); }
    let rgb = (top.rgb * ta + bot.rgb * ba * (1.0 - ta)) / oa;
    return vec4<f32>(rgb, oa * 255.0);
}

// Клеточный value-noise на общем целочисленном хэше — зеркало value_noise.
fn cell_h(i: f32, j: f32) -> f32 {
    return hash01(t_hash(u32(i32(i)) * 1973u) ^ t_hash(u32(i32(j)) * 9277u));
}

fn value_noise(x: f32, y: f32) -> f32 {
    let xi = floor(x);
    let yi = floor(y);
    let fx = x - xi;
    let fy = y - yi;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let top = mix(cell_h(xi, yi), cell_h(xi + 1.0, yi), sx);
    let bot = mix(cell_h(xi, yi + 1.0), cell_h(xi + 1.0, yi + 1.0), sx);
    return mix(top, bot, sy);
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
    let p1 = P.f1;
    let od = out_dims();
    let aspect = od.x / od.y;
    let pi = 3.14159265;
    if (mode == 1u) { // wipe
        let ext = max(abs(p0.x) + abs(p0.y), 1e-6);
        let s = ((uv.x - 0.5) * p0.x + (uv.y - 0.5) * p0.y + ext * 0.5) / ext;
        return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.z));
    }
    if (mode == 2u) { // iris: центр (p0.zw), rmax — до дальнего угла (п.400)
        let c = vec2<f32>((uv.x - p0.z) * aspect, uv.y - p0.w);
        let e = vec2<f32>(max(p0.z, 1.0 - p0.z) * aspect, max(p0.w, 1.0 - p0.w));
        let shape = i32(p0.x);
        var r = length(c);
        var rmax = max(length(e), 1e-6);
        if (shape == 1) { r = abs(c.x) + abs(c.y); rmax = max(e.x + e.y, 1e-6); }
        if (shape == 2) { r = max(abs(c.x), abs(c.y)); rmax = max(max(e.x, e.y), 1e-6); }
        return mix(samp(0u, uv), samp(1u, uv), front_mask(r / rmax, k, p0.y));
    }
    if (mode == 3u) { // clock: центр (p0.zw)
        var ang = atan2(uv.x - p0.z, -(uv.y - p0.w));
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
    if (mode == 6u) { // push: B стоит вплотную ЗА A — на экран дальше по +dir (п.396)
        let a = uv - p0.xy * k;
        if (a.x >= 0.0 && a.x < 1.0 && a.y >= 0.0 && a.y < 1.0) { return samp(0u, a); }
        return samp(1u, a + p0.xy);
    }
    if (mode == 7u) { // slide
        let b = uv + p0.xy * (1.0 - k);
        if (b.x >= 0.0 && b.x < 1.0 && b.y >= 0.0 && b.y < 1.0) { return samp(1u, b); }
        return samp(0u, uv);
    }
    if (mode == 8u) { // zoom: центр (p0.yz)
        let o = p0.yz;
        let za = 1.0 + p0.x * k;
        let zb = 1.0 + p0.x * (1.0 - k);
        let sa = samp(0u, (uv - o) / za + o);
        let sb = samp(1u, (uv - o) / zb + o);
        return mix(sa, sb, k);
    }
    if (mode == 9u) { // spin: центр (p0.zw)
        let o = p0.zw;
        let sa = rot_sample(0u, uv, p0.x * k * 2.0 * pi, 1.0 + p0.y * k, aspect, o);
        let sb = rot_sample(1u, uv, -p0.x * (1.0 - k) * 2.0 * pi, 1.0 + p0.y * (1.0 - k), aspect, o);
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
            else { acc = acc + samp(1u, a + p0.xy); } // B за A, +dir как у push (п.396)
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
    if (mode == 16u) { // blinds: фракция внутри полосы по повёрнутой оси
        let ext = max(abs(p0.x) + abs(p0.y), 1e-6);
        let c = ((uv.x - 0.5) * p0.x + (uv.y - 0.5) * p0.y + ext * 0.5) / ext;
        let f = fract(c * max(p0.z, 1.0));
        return mix(samp(0u, uv), samp(1u, uv), front_mask(f, k, p0.w));
    }
    if (mode == 17u) { // checker: встречные волны в клетках
        let cols = max(p0.x, 1.0);
        let rows = max(p0.y, 1.0);
        let i = floor(uv.x * cols);
        let j = floor(uv.y * rows);
        let fx = fract(uv.x * cols);
        var s = fx;
        if (i32(i + j) % 2 != 0) { s = 1.0 - fx; }
        return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.z));
    }
    if (mode == 18u) { // blocks: порядок клеток — хэш клетки
        let cols = max(p0.x, 1.0);
        let rows = max(p0.y, 1.0);
        let i = u32(floor(uv.x * cols));
        let j = u32(floor(uv.y * rows));
        let s = hash01(t_hash(i * 1973u) ^ t_hash(j * 9277u));
        return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.z));
    }
    if (mode == 19u) { // spiral: архимедов фронт вокруг центра (p0.zw)
        let cx = uv.x - p0.z;
        let cy = uv.y - p0.w;
        var ang = atan2(cx, -cy);
        if (ang < 0.0) { ang = ang + 2.0 * pi; }
        let theta = ang / (2.0 * pi);
        let ex = max(p0.z, 1.0 - p0.z) * aspect;
        let ey = max(p0.w, 1.0 - p0.w);
        let rmax = max(sqrt(ex * ex + ey * ey), 1e-6);
        let r = sqrt(cx * aspect * (cx * aspect) + cy * cy) / rmax;
        let turns = max(p0.x, 0.25);
        let sfrac = (theta + turns * r) / (1.0 + turns);
        return mix(samp(0u, uv), samp(1u, uv), front_mask(sfrac, k, p0.y));
    }
    if (mode == 20u) { // split: половинки A разъезжаются, B в щели
        var c = uv.x - 0.5;
        if (p0.x > 0.5) { c = uv.y - 0.5; }
        let h = k * 0.5;
        if (abs(c) >= h) {
            let off = h * sign(c);
            if (p0.x > 0.5) { return samp(0u, vec2<f32>(uv.x, uv.y - off)); }
            return samp(0u, vec2<f32>(uv.x - off, uv.y));
        }
        return samp(1u, uv);
    }
    if (mode == 21u) { // squeeze: A сжимается, B расправляется вдоль dir
        var c = 1.0 - uv.y;
        if (p0.x > 0.5) { c = uv.x; }
        else if (p0.x < -0.5) { c = 1.0 - uv.x; }
        else if (p0.y > 0.5) { c = uv.y; }
        if (c < k) {
            let c2 = c / max(k, 1e-4);
            return samp(1u, sq_back(uv, c2, p0.xy));
        }
        let c2 = (c - k) / max(1.0 - k, 1e-4);
        return samp(0u, sq_back(uv, c2, p0.xy));
    }
    if (mode == 22u) { // stretch: B растягивается из центральной линии
        var c = uv.x - 0.5;
        if (p0.x > 0.5) { c = uv.y - 0.5; }
        let h = k * 0.5;
        if (abs(c) < h) {
            let c2 = c / max(k, 1e-4) + 0.5;
            if (p0.x > 0.5) { return samp(1u, vec2<f32>(uv.x, c2)); }
            return samp(1u, vec2<f32>(c2, uv.y));
        }
        return samp(0u, uv);
    }
    if (mode == 23u) { // swap: 2.5D-обмен, порядок меняется на середине
        let vert = p0.x > 0.5;
        let wave = sin(pi * k);
        let off = 0.3 * wave;
        let scale = max(1.0 - p0.y * wave, 0.05);
        let sa = swap_quad(0u, uv, vert, off, scale, wave);
        let sb = swap_quad(1u, uv, vert, -off, scale, wave);
        if (k < 0.5) { return over_straight(sa, sb); }
        return over_straight(sb, sa);
    }
    if (mode == 24u) { // shake: рывки, сторона катом на 0.5
        let steps = max(p0.y, 1.0);
        let i = min(u32(k * steps), u32(steps) - 1u);
        let amp = p0.x * 0.2 * sin(pi * k);
        let ox = (hash01(i * 2u + 1u) - 0.5) * amp;
        let oy = (hash01(i * 2u + 2u) - 0.5) * amp;
        var which = 1u;
        if (k < 0.5) { which = 0u; }
        return samp(which, uv + vec2<f32>(ox, oy));
    }
    if (mode == 25u) { // film burn: фронт по шуму, горячая кромка
        let n = value_noise(uv.x * p0.x * aspect, uv.y * p0.x);
        let mask = front_mask(n, k, max(p0.y, 0.02));
        let base = mix(samp(0u, uv), samp(1u, uv), mask);
        let rim = clamp(mask * (1.0 - mask) * 4.0, 0.0, 1.0);
        let gate = rim * base.a / 255.0;
        let tint = vec3<f32>(p0.z, p0.w, p1.x);
        return vec4<f32>(mix(base.rgb, tint, gate), base.a);
    }
    if (mode == 26u) { // light leaks: два пятна по диагоналям
        let base = mix(samp(0u, uv), samp(1u, uv), k);
        let wave = sin(pi * k);
        let w = (leak_blob(uv, k * 1.2 - 0.1, 0.3 + 0.4 * k, aspect)
            + leak_blob(uv, 1.1 - 1.2 * k, 0.7 - 0.4 * k, aspect)) * p0.x * wave;
        let gate = clamp(w, 0.0, 1.0) * base.a / 255.0;
        let tint = vec3<f32>(p0.y, p0.z, p0.w);
        return vec4<f32>(mix(base.rgb, tint, gate), base.a);
    }
    if (mode == 27u) { // zoom blur: лучи от центра (p0.yz)
        let o = p0.yz;
        let amt = p0.x * sin(pi * k);
        var which = 1u;
        if (k < 0.5) { which = 0u; }
        var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        for (var i = 0; i < 7; i = i + 1) {
            let z = 1.0 + amt * 0.6 * (f32(i) / 6.0);
            acc = acc + samp(which, (uv - o) / z + o);
        }
        return acc / 7.0;
    }
    if (mode == 28u) { // flip: панель по гомографии из ext (п.401)
        let vert = p0.x > 0.5;
        var sx = uv.x; var sy = uv.y;
        if (vert) { sx = uv.y; sy = uv.x; }
        let st = hpanel(0u, sx, sy);
        if (st.z > 0.5 && st.x >= 0.0 && st.x <= 1.0 && st.y >= 0.0 && st.y <= 1.0) {
            let front = ext_at(8u) >= 0.0;
            var ms = st.x;
            if (!front) { ms = 1.0 - st.x; }
            var mu = ms; var mv = st.y;
            if (vert) { mu = st.y; mv = ms; }
            var which = 1u;
            if (front) { which = 0u; }
            let c = samp(which, vec2<f32>(mu, mv));
            return vec4<f32>(c.rgb * ext_at(9u), c.a);
        }
        return vec4<f32>(0.0);
    }
    if (mode == 29u) { // cube: две грани, порядок фиксирован (B, потом A)
        let vert = abs(p0.y) > 0.5;
        var sx = uv.x; var sy = uv.y;
        if (vert) { sx = uv.y; sy = uv.x; }
        let sh = 1.0 - p0.w * sin(pi * k) * 0.5;
        let stb = hpanel(8u, sx, sy);
        if (stb.z > 0.5 && stb.x >= 0.0 && stb.x <= 1.0 && stb.y >= 0.0 && stb.y <= 1.0) {
            var mu = stb.x; var mv = stb.y;
            if (vert) { mu = stb.y; mv = stb.x; }
            let c = samp(1u, vec2<f32>(mu, mv));
            return vec4<f32>(c.rgb * sh, c.a);
        }
        let sta = hpanel(0u, sx, sy);
        if (sta.z > 0.5 && sta.x >= 0.0 && sta.x <= 1.0 && sta.y >= 0.0 && sta.y <= 1.0) {
            var mu = sta.x; var mv = sta.y;
            if (vert) { mu = sta.y; mv = sta.x; }
            let c = samp(0u, vec2<f32>(mu, mv));
            return vec4<f32>(c.rgb * sh, c.a);
        }
        return vec4<f32>(0.0);
    }
    if (mode == 30u) { // door: створка и её зеркало, за ними входящий
        let vert = p0.x > 0.5;
        var sx = uv.x; var sy = uv.y;
        if (vert) { sx = uv.y; sy = uv.x; }
        let sh = ext_at(8u);
        let stl = hpanel(0u, sx, sy);
        if (stl.z > 0.5 && stl.x >= 0.0 && stl.x <= 0.5 && stl.y >= 0.0 && stl.y <= 1.0) {
            var mu = stl.x; var mv = stl.y;
            if (vert) { mu = stl.y; mv = stl.x; }
            let c = samp(0u, vec2<f32>(mu, mv));
            return vec4<f32>(c.rgb * sh, c.a);
        }
        let str = hpanel(0u, 1.0 - sx, sy);
        if (str.z > 0.5 && str.x >= 0.0 && str.x <= 0.5 && str.y >= 0.0 && str.y <= 1.0) {
            let ms = 1.0 - str.x;
            var mu = ms; var mv = str.y;
            if (vert) { mu = str.y; mv = ms; }
            let c = samp(0u, vec2<f32>(mu, mv));
            return vec4<f32>(c.rgb * sh, c.a);
        }
        return samp(1u, uv);
    }
    if (mode == 31u) { // page peel: цилиндрический заворот (п.401)
        let r = max(p0.z, 0.01);
        let shadow = p0.w;
        let c = axis_c(uv, p0.xy);
        let f = 1.0 - k * (1.0 + pi * r);
        let back = 1.0 - shadow * 0.35;
        let m_tail = 2.0 * f + pi * r - c;
        if (c <= f && m_tail >= f + pi * r && m_tail <= 1.0) {
            let s0 = axis_sample(0u, uv, p0.xy, m_tail);
            return vec4<f32>(s0.rgb * back, s0.a);
        }
        let dc = c - f;
        if (dc >= 0.0 && dc <= r) {
            let th1 = atan2(dc, sqrt(max(r * r - dc * dc, 0.0)));
            let th2 = pi - th1;
            let m2 = f + r * th2;
            if (m2 <= 1.0) {
                let s0 = axis_sample(0u, uv, p0.xy, m2);
                return vec4<f32>(s0.rgb * back, s0.a);
            }
            let m1 = f + r * th1;
            if (m1 <= 1.0) {
                let sh = 1.0 - shadow * 0.3 * sin(th1);
                let s0 = axis_sample(0u, uv, p0.xy, m1);
                return vec4<f32>(s0.rgb * sh, s0.a);
            }
        }
        if (c <= f) { return samp(0u, uv); }
        let sh = 1.0 - shadow * 0.5 * max(1.0 - min(dc / max(r, 1e-4), 1.0), 0.0);
        let s1 = samp(1u, uv);
        return vec4<f32>(s1.rgb * sh, s1.a);
    }
    if (mode == 32u) { // page roll: видимый верхний виток рулона
        let r = max(p0.z, 0.01);
        let shadow = p0.w;
        let c = axis_c(uv, p0.xy);
        let f = 1.0 - k * (1.0 + 2.0 * r);
        let back = 1.0 - shadow * 0.35;
        let dcr = f - c;
        if (abs(dcr) <= r) {
            let ad = abs(dcr);
            let th1 = atan2(ad, sqrt(max(r * r - ad * ad, 0.0)));
            var th = pi + th1;
            if (dcr >= 0.0) { th = pi - th1; }
            let mw = f + r * th;
            if (mw <= 1.0) {
                let sh = back * (1.0 - 0.2 * (1.0 - abs(sin(th))));
                let s0 = axis_sample(0u, uv, p0.xy, mw);
                return vec4<f32>(s0.rgb * sh, s0.a);
            }
            let m1 = f + r * th1;
            if (dcr >= 0.0 && m1 <= 1.0) {
                let s0 = axis_sample(0u, uv, p0.xy, m1);
                return vec4<f32>(s0.rgb * back, s0.a);
            }
        }
        if (c <= f) { return samp(0u, uv); }
        let sh = 1.0 - shadow * 0.5 * max(1.0 - min((c - f) / max(r, 1e-4), 1.0), 0.0);
        let s1 = samp(1u, uv);
        return vec4<f32>(s1.rgb * sh, s1.a);
    }
    return mix(samp(0u, uv), samp(1u, uv), k);
}

// ext-скаляры объёмных видов (Р.51): столбцы P.m подряд, как пишет bytes().
fn ext_at(i: u32) -> f32 {
    return P.m[i / 4u][i % 4u];
}

// Панель по обратной гомографии из ext (base — 0 или 8): (s, t, valid).
fn hpanel(base: u32, x: f32, y: f32) -> vec3<f32> {
    let den = ext_at(base + 6u) * x + ext_at(base + 7u) * y + 1.0;
    if (den <= 1e-6) { return vec3<f32>(0.0, 0.0, 0.0); }
    let s = (ext_at(base + 0u) * x + ext_at(base + 1u) * y + ext_at(base + 2u)) / den;
    let t = (ext_at(base + 3u) * x + ext_at(base + 4u) * y + ext_at(base + 5u)) / den;
    return vec3<f32>(s, t, 1.0);
}

// Координата вдоль оси заворота (фронт к dir) — зеркало axis_c CPU-ядра.
fn axis_c(uv: vec2<f32>, d: vec2<f32>) -> f32 {
    if (d.x > 0.5) { return 1.0 - uv.x; }
    if (d.x < -0.5) { return uv.x; }
    if (d.y > 0.5) { return 1.0 - uv.y; }
    return uv.y;
}

// Сэмпл с подменённой осевой координатой — зеркало axis_sample.
fn axis_sample(which: u32, uv: vec2<f32>, d: vec2<f32>, m: f32) -> vec4<f32> {
    if (d.x > 0.5) { return samp(which, vec2<f32>(1.0 - m, uv.y)); }
    if (d.x < -0.5) { return samp(which, vec2<f32>(m, uv.y)); }
    if (d.y > 0.5) { return samp(which, vec2<f32>(uv.x, 1.0 - m)); }
    return samp(which, vec2<f32>(uv.x, m)); 
}

// Осевое «обратно в uv» у squeeze — зеркало back() CPU-ядра (Р.51).
fn sq_back(uv: vec2<f32>, c2: f32, d: vec2<f32>) -> vec2<f32> {
    if (d.x > 0.5) { return vec2<f32>(c2, uv.y); }
    if (d.x < -0.5) { return vec2<f32>(1.0 - c2, uv.y); }
    if (d.y > 0.5) { return vec2<f32>(uv.x, c2); }
    return vec2<f32>(uv.x, 1.0 - c2);
}

// Квад стороны 2.5D-обмена: сэмпл в прямоугольнике, вне — прозрачно,
// дальняя сторона чуть темнее (зеркало quad() CPU-ядра).
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

// Пятно блика — зеркало blob() CPU-ядра.
fn leak_blob(uv: vec2<f32>, bx: f32, by: f32, aspect: f32) -> f32 {
    let dx = (uv.x - bx) * aspect;
    let dy = uv.y - by;
    let d = sqrt(dx * dx + dy * dy) / 0.45;
    let w = max(1.0 - d, 0.0);
    return w * w;
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
    if (mode == 4u) {
        // РЕЖИМ НАЛОЖЕНИЯ дорожки (Р.62): номер режима приезжает в u1.y —
        // фейда при наложении не бывает, и слот свободен по построению
        return blend_over(dst, a, P.u1.y, a_has, alpha_k);
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
