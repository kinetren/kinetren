// Grain: a counter-based hash (R.29): the same integer formula as
// noise/noise_hash in kt-fx/src/video.rs, so byte for byte with the CPU on any
// hardware. u0 = (seed, k, size, color); u1 = (res_scale, 0, 0, 0).
// R.45 (item 313): at K× the cell is size·K and the coordinates of the special formula
// are divided by K, so the grain pattern matches the 1× one bit for bit; the branch of the formula is
// chosen by the USER size.
//
// size is the grain size in pixels, color is its own offset per channel
// (R.33 S5). IMPORTANT: with size == 1 and color == 0 the FORMER formula is computed
// (without the extra grain hash), exactly how the CPU branches too, and the goldens of both
// paths do not move.

fn nh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

fn off_of(h: u32, k: u32) -> i32 {
    let n = i32((h >> 16u) & 0x7fffu) - 16384;
    return (n * i32(k)) / 32767;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<u32>(floor(pos.xy));
    let c = textureLoad(T0, vec2<i32>(p), 0);
    let k = P.u0.y;
    let size = max(P.u0.z, 1u);
    let color = P.u0.w == 1u;
    let rs = max(P.u1.x, 1u);
    if (size == 1u && !color) {
        // former formula: seed goes into the row hash without a wrapper; at K×
        // the coordinates are divided by K (the 1× pattern block by block)
        let q = p / rs;
        let off = off_of(nh(q.x ^ nh(q.y ^ P.u0.x)), k);
        let rgb = clamp(vec3<i32>(c.rgb) + vec3<i32>(off), vec3<i32>(0), vec3<i32>(255));
        return vec4<u32>(vec3<u32>(rgb), c.a);
    }
    let g = p / (size * rs);
    if (!color) {
        let off = off_of(nh(g.x ^ nh(g.y ^ nh(P.u0.x))), k);
        let rgb = clamp(vec3<i32>(c.rgb) + vec3<i32>(off), vec3<i32>(0), vec3<i32>(255));
        return vec4<u32>(vec3<u32>(rgb), c.a);
    }
    // No write through a dynamic vector index: FXC (wgpu's D3D12 compiler on Windows) rejects
    // it when it cannot unroll the loop (windows-build). Same per-channel arithmetic.
    let off = vec3<i32>(
        off_of(nh(g.x ^ nh(g.y ^ nh(1u ^ P.u0.x))), k),
        off_of(nh(g.x ^ nh(g.y ^ nh(2u ^ P.u0.x))), k),
        off_of(nh(g.x ^ nh(g.y ^ nh(3u ^ P.u0.x))), k),
    );
    let rgb = clamp(vec3<i32>(c.rgb) + off, vec3<i32>(0), vec3<i32>(255));
    return vec4<u32>(vec3<u32>(rgb), c.a);
}
