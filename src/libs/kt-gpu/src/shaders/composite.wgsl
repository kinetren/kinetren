// Composite of a layer over the accumulator: a mirror of the kernels of kt-engine/src/video.rs
// (alpha_over / scale_alpha_into / scale_alpha_inplace / dissolve_into),
// INTEGER and byte for byte: the over formula cannot be expressed with fixed-function
// blending (the factor (1−sa)·da), so there is a ping-pong of two accumulators.
// T0 = acc_in, T1 = layer A, T2 = layer B (dissolve).
// u0 = (mode, alpha_k, a_has_alpha, b_has_alpha);
// u1 = (first, k | blend mode number, mix_mode, clear).
// mode: 0 direct, 1 scaled, 2 dissolve, 3 solo fade, 4 blend (R.62).

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

// BLEND MODE of a track (R.62, F-CMP-003): a MIRROR of `blend_ch` from
// kt-engine/src/video.rs. It is edited in the SAME COMMIT as the CPU kernel
// (invariant 7): a divergence means the preview and the export show different things.
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

// out = lerp(below, blend(below, above), a_above × level). The level and the
// mode are DIFFERENT axes, both after the alpha; a hole (a = 0) leaves the bottom as it was.
fn blend_over(dst: vec4<u32>, src: vec4<u32>, blend_mode: u32, has_alpha: u32, level_k: u32) -> vec4<u32> {
    var sa = 1.0;
    if (has_alpha == 1u) { sa = f32(src.a) / 255.0; }
    let k = sa * (f32(level_k) / 4096.0);
    if (k <= 0.0) { return dst; }
    // No write through a dynamic vector index: FXC (wgpu's D3D12 compiler on Windows) rejects
    // it when it cannot unroll the loop (windows-build). Same per-channel arithmetic.
    let b = vec3<f32>(dst.rgb) / 255.0;
    let a = vec3<f32>(src.rgb) / 255.0;
    let m = clamp(
        vec3<f32>(blend_ch(blend_mode, b.x, a.x), blend_ch(blend_mode, b.y, a.y), blend_ch(blend_mode, b.z, a.z)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    let rgb = b + (m - b) * k;
    let da = f32(dst.a) / 255.0;
    let oa = k + da * (1.0 - k);
    let c = clamp(floor(vec4<f32>(rgb * 255.0 + 0.5, oa * 255.0 + 0.5)), vec4<f32>(0.0), vec4<f32>(255.0));
    return vec4<u32>(c);
}

// scale_alpha_into: a copy of RGB, alpha ×k/4096 with rounding (+2048>>12);
// a frame without alpha is treated as opaque (A = 255).
fn scale_into(c: vec4<u32>, k: u32, has_alpha: u32) -> vec4<u32> {
    var a = 255u;
    if (has_alpha == 1u) { a = c.a; }
    return vec4<u32>(c.rgb, (a * k + 2048u) >> 12u);
}

// dissolve_into: (a·ik + b·k + 2048) >> 12 over all channels.
fn dis(a: vec4<u32>, b: vec4<u32>, k: u32, a_has: u32, b_has: u32) -> vec4<u32> {
    var av = a;
    var bv = b;
    if (a_has == 0u) { av.a = 255u; }
    if (b_has == 0u) { bv.a = 255u; }
    return (av * (4096u - k) + bv * k + vec4<u32>(2048u)) >> vec4<u32>(12u);
}

// ---- transitions (a mirror of transitions.rs) -------------------------------------

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

// Straight-alpha “top over bottom” in f32: a mirror of over_straight (R.51).
fn over_straight(top: vec4<f32>, bot: vec4<f32>) -> vec4<f32> {
    let ta = clamp(top.a / 255.0, 0.0, 1.0);
    let ba = clamp(bot.a / 255.0, 0.0, 1.0);
    let oa = ta + ba * (1.0 - ta);
    if (oa <= 0.0) { return vec4<f32>(0.0); }
    let rgb = (top.rgb * ta + bot.rgb * ba * (1.0 - ta)) / oa;
    return vec4<f32>(rgb, oa * 255.0);
}

// Cell value-noise on the shared integer hash: a mirror of value_noise.
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

fn trans(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let mode = P.u1.z;
    // every kind but the crossfade is a plugin (plugin-api-stage1, D12): its arm, assembled from
    // the registry by kt-gpu, calls its trans_<id>
    // @PLUGIN_TRANS_ARMS
    return mix(samp(0u, uv), samp(1u, uv), k);
}

// ext scalars of volumetric kinds (R.51): the columns P.m one after another, as bytes() writes them.
fn ext_at(i: u32) -> f32 {
    return P.m[i / 4u][i % 4u];
}

// A panel by the inverse homography from ext (base 0 or 8): (s, t, valid).
fn hpanel(base: u32, x: f32, y: f32) -> vec3<f32> {
    let den = ext_at(base + 6u) * x + ext_at(base + 7u) * y + 1.0;
    if (den <= 1e-6) { return vec3<f32>(0.0, 0.0, 0.0); }
    let s = (ext_at(base + 0u) * x + ext_at(base + 1u) * y + ext_at(base + 2u)) / den;
    let t = (ext_at(base + 3u) * x + ext_at(base + 4u) * y + ext_at(base + 5u)) / den;
    return vec3<f32>(s, t, 1.0);
}

// Coordinate along the curl axis (the front toward dir): a mirror of the CPU kernel’s axis_c.
fn axis_c(uv: vec2<f32>, d: vec2<f32>) -> f32 {
    if (d.x > 0.5) { return 1.0 - uv.x; }
    if (d.x < -0.5) { return uv.x; }
    if (d.y > 0.5) { return 1.0 - uv.y; }
    return uv.y;
}

// A sample with the axial coordinate replaced: a mirror of axis_sample.
fn axis_sample(which: u32, uv: vec2<f32>, d: vec2<f32>, m: f32) -> vec4<f32> {
    if (d.x > 0.5) { return samp(which, vec2<f32>(1.0 - m, uv.y)); }
    if (d.x < -0.5) { return samp(which, vec2<f32>(m, uv.y)); }
    if (d.y > 0.5) { return samp(which, vec2<f32>(uv.x, 1.0 - m)); }
    return samp(which, vec2<f32>(uv.x, m)); 
}

/// Rounding to the nearest integer, the same way the CPU kernel writes bytes.
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
    var dst = vec4<u32>(0u); // first: the accumulator is transparent black
    if (P.u1.x == 0u) { dst = textureLoad(T0, p, 0); }
    let a = textureLoad(T1, p, 0);
    if (mode == 0u) {
        // alpha_over with the fast branch !has_alpha = a copy of the layer as a whole
        if (a_has == 0u) { return a; }
        return over(dst, a);
    }
    if (mode == 1u) {
        return over(dst, scale_into(a, alpha_k, a_has));
    }
    if (mode == 4u) {
        // BLEND MODE of a track (R.62): the mode number arrives in u1.y;
        // there is no fade with blend, and the slot is free by construction
        return blend_over(dst, a, P.u1.y, a_has, alpha_k);
    }
    let od = out_dims();
    let uv = (vec2<f32>(pos.xy)) / od;
    if (mode == 3u) {
        // solo fade with a view: one side is empty, the shape comes from the kernel
        var c = to_u8(trans(uv, f32(P.u1.y) / 4096.0));
        if (alpha_k < 4096u) { c.a = (c.a * alpha_k) / 4096u; }
        return over(dst, c);
    }
    if (P.u1.z != 0u) {
        // transition of a pair with a view (not a crossfade)
        var c = to_u8(trans(uv, f32(P.u1.y) / 4096.0));
        if (alpha_k < 4096u) { c.a = (c.a * alpha_k) / 4096u; }
        return over(dst, c);
    }
    // dissolve of a pair; the level on top is scale_alpha_inplace (WITHOUT rounding)
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
