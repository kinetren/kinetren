// camera_isp (camera-isp-simulation): the helpers every pass shares — the mirror of the shared
// numerics of `cpu.rs` (hashes, gauss, unit, the cfa code, the sensor grid, the texel forms).
// Prepended to each pass.

var<private> FL: u32;
var<private> TAB: vec4<u32>;
// what every helper asks per call, taken once per pixel by `begin` (gpu-pass-cost): the texture's
// size, the sensor's, the scale and the mosaic period — a textureDimensions and two integer
// divisions per neighbour read were most of a 5×5 window's cost
var<private> FDIM: vec2<i32>;
var<private> SDIM: vec2<i32>;
var<private> KSC: i32;
var<private> PERIOD: i32;

// every pass's first line: its flags, then the sizes the helpers read
fn begin(fl: u32) {
    FL = fl;
    KSC = max(i32(FL & 255u), 1);
    PERIOD = i32((FL >> 8u) & 15u);
    FDIM = vec2<i32>(textureDimensions(T0));
    SDIM = (FDIM + vec2<i32>(KSC - 1)) / KSC;
}

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
    return KSC;
}

fn PER() -> i32 {
    return PERIOD;
}

fn fdims() -> vec2<i32> {
    return FDIM;
}

// the sensor's size: the logical frame (D6)
fn sdims() -> vec2<i32> {
    return SDIM;
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
    return cfa_mod(((x % p) + p) % p, ((y % p) + p) % p);
}

// the code at a position already reduced by the period (a window reduces its five columns and
// five rows once, not 2·25 times)
fn cfa_mod(xm: i32, ym: i32) -> u32 {
    let i = u32(ym * PER() + xm);
    return (TAB[i / 10u] >> ((i % 10u) * 3u)) & 7u;
}

fn pmod(n: i32) -> i32 {
    let p = PER();
    return ((n % p) + p) % p;
}

fn v16(t: vec4<u32>) -> u32 {
    return (t.r << 8u) | t.g;
}

fn t16(v: u32) -> vec4<u32> {
    return vec4<u32>(v >> 8u, v & 255u, 0u, 255u);
}

// the previous pass at frame pixel (x, y), clamped
fn at_frame(x: i32, y: i32) -> vec4<u32> {
    let d = fdims();
    return textureLoad(T0, vec2<i32>(clamp(x, 0, d.x - 1), clamp(y, 0, d.y - 1)), 0);
}

// the scene texel (cpu.rs unpack_scene): R, G 11 bits, B 10, each sqrt(v/2)
fn unpack_scene(t: vec4<u32>) -> vec3<f32> {
    let p = t.r | (t.g << 8u) | (t.b << 16u) | (t.a << 24u);
    let r = f32(p & 0x7FFu) * 4.885198e-4;
    let g = f32((p >> 11u) & 0x7FFu) * 4.885198e-4;
    let b = f32(p >> 22u) * 9.775171e-4;
    return vec3<f32>(r * r * 2.0, g * g * 2.0, b * b * 2.0);
}

fn oetf(x: f32, curve: u32) -> f32 {
    let v = clamp(x, 0.0, 1.0);
    switch curve {
        case 1u: {
            if (v <= 0.0031308) {
                return v * 12.92;
            }
            return 1.055 * pow(v, 0.41666666) - 0.055;
        }
        case 2u: {
            if (v < 0.018) {
                return v * 4.5;
            }
            return 1.099 * pow(v, 0.45) - 0.099;
        }
        case 3u: {
            if (v > 0.0) {
                return pow(v, 0.45454545);
            }
            return 0.0;
        }
        default: {
            return v;
        }
    }
}

// a mosaic neighbour read through the period shift, as a 16-bit fixed fraction
fn vfix(sx: i32, sy: i32) -> f32 {
    let s = sdims();
    return f32(v16(at(wrap1(sx, s.x), wrap1(sy, s.y)))) * 6.1035156e-5;
}

fn texel(r: u32, g: u32, b: u32, a: u32) -> vec4<u32> {
    return vec4<u32>(r, g, b, a);
}

fn fix16(v: f32) -> u32 {
    return u32(clamp(floor(v * 16384.0 + 0.5), 0.0, 65535.0));
}

fn byte(v: f32) -> u32 {
    return u32(clamp(floor(v * 255.0 + 0.5), 0.0, 255.0));
}

// the smallest r² on the frame's border (cpu.rs border_r2_min)
fn border_r2_min(hw: f32, hh: f32, hd2: f32) -> f32 {
    let m = min(hw, hh);
    return m * m / hd2;
}

// the lens's field fitted to the scene (cpu.rs lens_fit)
fn lens_fit(k: f32, ca: f32, m2: f32) -> f32 {
    return (1.0 + max(k * m2, k)) * (1.0 + abs(ca));
}

// the ISP's crop after its lens correction (cpu.rs isp_crop)
fn isp_crop(k: f32, ca: f32, m2: f32) -> f32 {
    return max((1.0 + min(k * m2, k)) * (1.0 - abs(ca)), 0.05);
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
