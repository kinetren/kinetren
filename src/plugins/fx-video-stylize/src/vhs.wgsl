// VHS: a mirror of vhs (kt-fx/src/video.rs): the RGB shift plus scan lines, whose step is set
// (R.33 S5), and row jitter. Only the source row is read (on the CPU it is a copy of it),
// so the GPU is not in-place by construction.
// u0 = (w, shift, line_off, scan_step); u1 = (seed, jq, res_scale, 0);
// R.45 (item 313): shift arrives already ×K, and the scan lines and the jitter hash
// are computed from the LOGICAL row y/K (a block of K rows). K=1 is the former
// frame, byte for byte (y/1).
// f0 = (bleed, 0, 0, 0). scan_step = 4 and jq = 0 give the former frame
// byte for byte. jq is the jitter QUANTUM (jitter×1024): the row offset
// is computed in integers, because WGSL round() rounds
// halves to even, while Rust rounds away from zero (a divergence in t181).

fn nh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let w = i32(P.u0.x);
    let shift = i32(P.u0.y);
    let step = i32(P.u0.w);
    let rs = max(i32(P.u1.z), 1);
    let yl = p.y / rs; // logical row (R.45)
    var scan = 0;
    if (step > 0 && (yl + i32(P.u0.z)) % step == 0) { scan = 1; }
    // row jitter: the same formula as on the CPU (the row hash from the seed)
    var jx = 0;
    let jq = i32(P.u1.y);
    if (jq != 0) {
        let hsh = nh(u32(yl) ^ nh(P.u1.x));
        let n = i32((hsh >> 16u) & 0x7fffu) - 16384;
        jx = ((n * jq) / 32767) * w / 4096;
    }
    let xj = clamp(p.x + jx, 0, w - 1);
    let xr = clamp(xj + shift + scan, 0, w - 1);
    let xb = clamp(xj - shift - scan, 0, w - 1);
    let c = textureLoad(T0, vec2<i32>(xj, p.y), 0);
    let cr = f32(textureLoad(T0, vec2<i32>(xr, p.y), 0).r);
    let cb = f32(textureLoad(T0, vec2<i32>(xb, p.y), 0).b);
    let bleed = P.f0.x;
    let r = u32(round(f32(c.r) * (1.0 - bleed) + cr * bleed));
    let b = u32(round(f32(c.b) * (1.0 - bleed) + cb * bleed));
    var g = c.g;
    if (scan == 1) {
        g = select(0u, g - 18u, g >= 18u); // saturating_sub(18)
    }
    // alpha is taken from the NON-shifted pixel, as on the CPU (it writes in-place)
    let a = textureLoad(T0, p, 0).a;
    return vec4<u32>(r, g, b, a);
}
