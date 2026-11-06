// vhs_tape: the mirrors of `sig.rs` (our f32 sine/atan2 in cycles, the counter hash, the grain,
// the u16 halves) and the texel packing of the stages. Edited in the same commit as sig.rs/sim.rs.

fn hsh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

fn hash3(a: u32, b: u32, c: u32) -> u32 {
    return hsh(a ^ hsh(b ^ hsh(c)));
}

fn unit(h: u32) -> f32 {
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

fn gauss(h: u32) -> f32 {
    let h1 = hsh(h);
    let h2 = hsh(h1);
    let h3 = hsh(h2);
    return ((unit(h) + unit(h1)) + (unit(h2) + unit(h3)) - 2.0) * 1.7320508;
}

fn sin_cyc(x: f32) -> f32 {
    var r = x - floor(x + 0.5);
    if (r > 0.25) { r = 0.5 - r; }
    if (r < -0.25) { r = -0.5 - r; }
    let t2 = r * r;
    return r * (6.2831855 + t2 * (-41.3417 + t2 * (81.60525 + t2 * (-76.70586 + t2 * (42.05869 + t2 * -15.094643)))));
}

fn cos_cyc(x: f32) -> f32 {
    return sin_cyc(x + 0.25);
}

fn atan2_rad(y: f32, x: f32) -> f32 {
    let ax = abs(x);
    let ay = abs(y);
    let mx = max(ax, ay);
    if (mx == 0.0) { return 0.0; }
    let a = min(ax, ay) / mx;
    let s = a * a;
    var r = a * (0.99997726 + s * (-0.33262347 + s * (0.19354346 + s * (-0.11643287 + s * (0.05265332 + s * -0.0117212)))));
    if (ay > ax) { r = 1.5707964 - r; }
    if (x < 0.0) { r = 3.1415927 - r; }
    if (y < 0.0) { r = -r; }
    return r;
}

fn atan2_cyc(y: f32, x: f32) -> f32 {
    return atan2_rad(y, x) * 0.15915494;
}

fn q16(v: f32, off: f32, scale: f32) -> f32 {
    let q = clamp(round((v + off) * scale), 0.0, 65535.0);
    return q / scale - off;
}

// a u32 in a texel of the word target, and back (`BarrierKernel::word_passes`)
fn put(v: u32) -> vec4<u32> {
    return vec4<u32>(v, 0u, 0u, 0u);
}
fn putf(v: f32) -> vec4<u32> {
    return put(bitcast<u32>(v));
}
fn word(t: vec4<u32>) -> u32 {
    return t.x;
}
// two u16 halves: off/scale of the lower one, then the upper one
fn put2(a: f32, ao: f32, as_: f32, b: f32, bo: f32, bs: f32) -> vec4<u32> {
    let qa = u32(clamp(round((a + ao) * as_), 0.0, 65535.0));
    let qb = u32(clamp(round((b + bo) * bs), 0.0, 65535.0));
    return put(qa | (qb << 16u));
}
fn lo16(v: u32, off: f32, scale: f32) -> f32 {
    return f32(v & 65535u) / scale - off;
}
fn hi16(v: u32, off: f32, scale: f32) -> f32 {
    return f32(v >> 16u) / scale - off;
}

// the texture readers, the column clamped to the grid line
fn ld0(x: i32, y: i32) -> u32 {
    let d = vec2<i32>(textureDimensions(T0));
    return word(textureLoad(T0, vec2<i32>(clamp(x, 0, d.x - 1), y), 0));
}
fn ld1(x: i32, y: i32) -> u32 {
    let d = vec2<i32>(textureDimensions(T1));
    return word(textureLoad(T1, vec2<i32>(clamp(x, 0, d.x - 1), y), 0));
}
fn ld2(x: i32, y: i32) -> u32 {
    let d = vec2<i32>(textureDimensions(T2));
    return word(textureLoad(T2, vec2<i32>(clamp(x, 0, d.x - 1), y), 0));
}
fn ld0f(x: i32, y: i32) -> f32 { return bitcast<f32>(ld0(x, y)); }
fn ld1f(x: i32, y: i32) -> f32 { return bitcast<f32>(ld1(x, y)); }
fn ld2f(x: i32, y: i32) -> f32 { return bitcast<f32>(ld2(x, y)); }
