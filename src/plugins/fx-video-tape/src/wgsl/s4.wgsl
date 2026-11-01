// vhs_tape stage 4 (`sim::s4`): write — FM plus the colour-under through the tape's saturation.
// T0 = the phase, T1 = the picture A×H. u0 = (std, W, M, A), u1.x = PAL V switch;
// f0 = (a_c, sat, fcu_n, cu_line), f1 = (rotation of tracks A and B, carrier gain, chroma gain).

fn kch(i: i32, s: u32) -> f32 {
    if (s == 0u) { return K_CHROMA_P[i]; }
    return K_CHROMA_N[i];
}

fn uv_at(j: i32, y: i32) -> vec2<f32> {
    let x = j - i32(P.u0.z);
    if (x < 0 || x >= i32(P.u0.w)) { return vec2<f32>(0.0, 0.0); }
    let t = textureLoad(T1, vec2<i32>(x, y), 0);
    let r = f32(t.r) * (1.0 / 255.0);
    let g = f32(t.g) * (1.0 / 255.0);
    let b = f32(t.b) * (1.0 / 255.0);
    let l = 0.299 * r + 0.587 * g + 0.114 * b;
    return vec2<f32>(0.492 * (b - l), 0.877 * (r - l));
}

fn cu_phase(j: i32, yf: f32, rot: f32) -> f32 {
    let l = P.f0.w * yf + rot * yf * 0.25;
    return P.f0.z * f32(j) + (l - floor(l));
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let s = P.u0.x;
    let fm = cos_cyc(f32(ld0(n, y) >> 8u) * (1.0 / 16777216.0));
    var len = K_CHROMA_P_LEN;
    if (s != 0u) { len = K_CHROMA_N_LEN; }
    var uc = 0.0;
    var vc = 0.0;
    for (var i = 0; i < len; i = i + 1) {
        let c = kch(i, s);
        let ab = uv_at(n + K_CHROMA_C - i, y);
        uc = uc + c * ab.x;
        vc = vc + c * ab.y;
    }
    let yf = u32(y) >> 1u;
    if (P.u1.x != 0u && (yf & 1u) == 1u) { vc = -vc; }
    var rot = P.f1.x;
    if ((y & 1) == 1) { rot = P.f1.y; }
    let th = cu_phase(n, f32(yf), rot);
    let c = P.f0.x * (uc * cos_cyc(th) + vc * sin_cyc(th));
    return putf(P.f1.z * fm + P.f1.w * (c / (1.0 + P.f0.y * abs(c))));
}
