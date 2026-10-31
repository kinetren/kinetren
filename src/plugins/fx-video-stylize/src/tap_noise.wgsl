// Noise as a TAP of the fused geometry pass (plugin-api-barriers 4.2): the former `tap_kind == 1`
// arm of geom_fused.wgsl verbatim, a mirror of the plugin's f64 `tap_at`. A POSITIONED tap: the
// grain is pinned to the grid of ITS stage (decision R.45), `c` = the sub-sample's coordinate there.
// a.y = the seed's bits, a.z = k's bits, a.w = the cell; b.x = flags (1 colour, 2 the special
// formula of size == 1 && !colour); b.zw = the stage's size (the host's half of `b`).
// The hash is the plugin's own copy (lowbias32), named apart from the fused pass's.

fn noise_tap_nh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

// The grain offset of the common noise formula (integer division, as i64 in Rust).
fn noise_tap_off(gx: u32, gy: u32, ch: u32, seed: u32, k: i32) -> f32 {
    let h = noise_tap_nh(gx ^ noise_tap_nh(gy ^ noise_tap_nh(ch ^ seed)));
    let n = i32((h >> 16u) & 0x7fffu) - 16384;
    return f32(n * k / 32767);
}

fn tap_noise(v0: vec4<f32>, a: vec4<f32>, b: vec4<f32>, c: vec2<f32>) -> vec4<f32> {
    var v = v0;
    let seed = bitcast<u32>(a.y);
    let k = i32(bitcast<u32>(a.z));
    let cell = u32(a.w);
    let flags = u32(b.x);
    let sw = i32(b.z);
    let sh = i32(b.w);
    let xi = u32(clamp(i32(floor(c.x)), 0, sw - 1));
    let yi = u32(clamp(i32(floor(c.y)), 0, sh - 1));
    if ((flags & 2u) == 2u) { // a special formula for size==1 && !color
        let hsh = noise_tap_nh(xi ^ noise_tap_nh(yi ^ seed));
        let nn = i32((hsh >> 16u) & 0x7fffu) - 16384;
        let off = f32(nn * k / 32767);
        v = vec4<f32>(v.xyz + vec3<f32>(off), v.w);
    } else {
        let gx = xi / cell;
        let gy = yi / cell;
        if ((flags & 1u) == 1u) { // colour grain
            v = vec4<f32>(
                v.x + noise_tap_off(gx, gy, 1u, seed, k),
                v.y + noise_tap_off(gx, gy, 2u, seed, k),
                v.z + noise_tap_off(gx, gy, 3u, seed, k),
                v.w,
            );
        } else {
            v = vec4<f32>(v.xyz + vec3<f32>(noise_tap_off(gx, gy, 0u, seed, k)), v.w);
        }
    }
    return v;
}
