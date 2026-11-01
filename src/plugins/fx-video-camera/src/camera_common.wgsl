// camera_isp (camera-isp-simulation): the helpers every pass shares — the mirror of the shared
// numerics of `cpu.rs` (hash2, gauss, unit, cfa code, the sensor grid). Prepended to each pass.

var<private> FL: u32;
var<private> TAB: vec4<u32>;

fn nh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

fn hash2(x: u32, y: u32, s: u32) -> u32 {
    return nh(x ^ nh(y ^ s));
}

// Irwin–Hall(4) of four 16-bit fields of two hashes (cpu.rs `gauss`, IH_INV_SD)
fn gauss(h1: u32) -> f32 {
    let h2 = nh(h1 + 0x9E3779B9u);
    let s = (h1 & 0xFFFFu) + (h1 >> 16u) + (h2 & 0xFFFFu) + (h2 >> 16u);
    return (f32(s) - 131070.0) * 2.6428966e-5;
}

fn unit(h1: u32) -> f32 {
    return f32(h1 >> 8u) * 5.9604645e-8;
}

fn KK() -> i32 {
    return max(i32(FL & 255u), 1);
}

fn PER() -> i32 {
    return i32((FL >> 8u) & 15u);
}

fn fdims() -> vec2<i32> {
    return vec2<i32>(textureDimensions(T0));
}

// the sensor's size: the logical frame (D6)
fn sdims() -> vec2<i32> {
    let k = KK();
    return (fdims() + vec2<i32>(k - 1)) / k;
}

// the previous pass at sensor pixel (sx, sy), clamped
fn at(sx: i32, sy: i32) -> vec4<u32> {
    let d = fdims();
    let s = sdims();
    let k = KK();
    let x = min(clamp(sx, 0, s.x - 1) * k, d.x - 1);
    let y = min(clamp(sy, 0, s.y - 1) * k, d.y - 1);
    return textureLoad(T0, vec2<i32>(x, y), 0);
}

// a mosaic neighbour: outside the frame shifted by the period (keeps its colour, D7)
fn wrap1(n: i32, size: i32) -> i32 {
    let p = PER();
    var m = n;
    if (m < 0) {
        m = m + p;
    }
    if (m >= size) {
        m = m - p;
    }
    return clamp(m, 0, size - 1);
}

fn cfa_code(x: i32, y: i32) -> u32 {
    let p = PER();
    let xm = ((x % p) + p) % p;
    let ym = ((y % p) + p) % p;
    let i = u32(ym * p + xm);
    return (TAB[i / 10u] >> ((i % 10u) * 3u)) & 7u;
}

fn v16(t: vec4<u32>) -> u32 {
    return (t.r << 8u) | t.g;
}

fn fix16(v: f32) -> u32 {
    return u32(clamp(floor(v * 16384.0 + 0.5), 0.0, 65535.0));
}

fn byte(v: f32) -> u32 {
    return u32(clamp(floor(v * 255.0 + 0.5), 0.0, 255.0));
}

fn r2_of(sx: i32, sy: i32) -> f32 {
    let s = sdims();
    let hw = f32(s.x) * 0.5;
    let hh = f32(s.y) * 0.5;
    let cx = f32(sx) + 0.5 - hw;
    let cy = f32(sy) + 0.5 - hh;
    return (cx * cx + cy * cy) / (hw * hw + hh * hh);
}

// the dyes' NIR transmission by code (lib.rs DYE_NIR)
fn dye_nir(code: u32) -> f32 {
    switch code {
        case 1u: { return 0.9; }
        case 2u: { return 0.95; }
        default: { return 1.0; }
    }
}

fn src_alpha(p: vec2<i32>) -> u32 {
    return textureLoad(T1, p, 0).a;
}

fn isign(v: i32) -> i32 {
    return select(select(0, 1, v > 0), -1, v < 0);
}
