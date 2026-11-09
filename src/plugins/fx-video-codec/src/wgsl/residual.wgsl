// The residual of one plane (compute-kernels D6): source minus the motion prediction per 8×8 block,
// forward DCT, a uniform step rounded half away from zero, inverse DCT rounded to an integer.
// One workgroup per block, one invocation per pixel and per coefficient (gpu-pass-cost): each
// stage of the separable DCT is a row of eight products summed in the same order as before, the
// stages joined by barriers. A thread per block with three 64-float arrays spilled to scratch on a
// card and left most of its lanes idle. The planes `src` and `rf` are bytes, four to a word; the
// residual is two samples to a word, 16 bits each (stream-layout-per-build: a word a sample was read
// back whole). `c` is the DCT basis, row u = frequency.
struct U { w: u32, h: u32, bs: u32, shift: u32, q: f32, mw: u32, intra: u32, pad: u32, c: array<vec4<f32>, 16> };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> src: array<u32>;
@group(0) @binding(2) var<storage, read> rf: array<u32>;
@group(0) @binding(3) var<storage, read> mv: array<vec2<i32>>;
@group(0) @binding(4) var<storage, read_write> res: array<u32>;

fn src_at(i: i32) -> u32 { return (src[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u; }
fn rf_at(i: i32) -> u32 { return (rf[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u; }

var<workgroup> B: array<f32, 64>;
var<workgroup> T: array<f32, 64>;
var<workgroup> F: array<f32, 64>;
var<workgroup> R: array<i32, 64>;

fn round_away(v: f32) -> f32 { return sign(v) * floor(abs(v) + 0.5); }

fn C(i: i32) -> f32 { return u.c[i / 4][i % 4]; }

@compute @workgroup_size(8, 8)
fn main(@builtin(workgroup_id) g: vec3<u32>, @builtin(local_invocation_id) l: vec3<u32>) {
    // the dispatch is exactly the plane's blocks: every invocation reaches every barrier
    let ox = i32(g.x * 8u);
    let oy = i32(g.y * 8u);
    let w = i32(u.w);
    let h = i32(u.h);
    let x = i32(l.x);
    let y = i32(l.y);
    let px = ox + x;
    let py = oy + y;
    var p = 128.0;
    if u.intra == 0u {
        let pbs = i32(u.bs >> u.shift);
        let at = u32(py / pbs) * u.mw + u32(px / pbs);
        let v = mv[at] >> vec2<u32>(u.shift);
        p = f32(rf_at(clamp(py + v.y, 0, h - 1) * w + clamp(px + v.x, 0, w - 1)));
    }
    B[y * 8 + x] = f32(src_at(py * w + px)) - p;
    workgroupBarrier();
    // rows: t[y][c0], c0 = this invocation's x
    var s = 0.0;
    for (var k = 0; k < 8; k++) { s += C(x * 8 + k) * B[y * 8 + k]; }
    T[y * 8 + x] = s;
    workgroupBarrier();
    // columns, quantised: f[v][c0], v = this invocation's y
    s = 0.0;
    for (var k = 0; k < 8; k++) { s += C(y * 8 + k) * T[k * 8 + x]; }
    F[y * 8 + x] = round_away(s / u.q) * u.q;
    workgroupBarrier();
    // inverse columns: t[y][c0]
    s = 0.0;
    for (var k = 0; k < 8; k++) { s += C(k * 8 + y) * F[k * 8 + x]; }
    T[y * 8 + x] = s;
    workgroupBarrier();
    // inverse rows: the pixel (x, y)
    s = 0.0;
    for (var k = 0; k < 8; k++) { s += C(k * 8 + x) * T[y * 8 + k]; }
    R[y * 8 + x] = i32(clamp(round_away(s), -512.0, 512.0));
    workgroupBarrier();
    // a pair of samples to a word: the even column writes it (the plane's width is a multiple of 8)
    if (x & 1) == 0 {
        res[u32(py * w + px) >> 1u] = (u32(R[y * 8 + x]) & 0xffffu) | (u32(R[y * 8 + x + 1]) << 16u);
    }
}
