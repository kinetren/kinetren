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
