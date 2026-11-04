// The residual of one plane (compute-kernels D6): source minus the motion prediction per 8×8 block,
// forward DCT, a uniform step rounded half away from zero, inverse DCT rounded to an integer.
struct U { w: u32, h: u32, bs: u32, shift: u32, q: f32, mw: u32, intra: u32, pad: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> src: array<u32>;
@group(0) @binding(2) var<storage, read> rf: array<u32>;
@group(0) @binding(3) var<storage, read> mv: array<vec2<i32>>;
@group(0) @binding(4) var<storage, read_write> res: array<i32>;

fn round_away(v: f32) -> f32 { return sign(v) * floor(abs(v) + 0.5); }

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= u.w / 8u || id.y >= u.h / 8u { return; }
    let ox = i32(id.x * 8u);
    let oy = i32(id.y * 8u);
    let w = i32(u.w);
    let h = i32(u.h);
    var b: array<f32, 64>;
    for (var y = 0; y < 8; y++) {
        for (var x = 0; x < 8; x++) {
            let px = ox + x;
            let py = oy + y;
            var p = 128.0;
            if u.intra == 0u {
                let pbs = i32(u.bs >> u.shift);
                let at = u32(py / pbs) * u.mw + u32(px / pbs);
                let v = mv[at] >> vec2<u32>(u.shift);
                p = f32(rf[clamp(py + v.y, 0, h - 1) * w + clamp(px + v.x, 0, w - 1)]);
            }
            b[y * 8 + x] = f32(src[py * w + px]) - p;
        }
    }
    var t: array<f32, 64>;
    var f: array<f32, 64>;
    for (var y = 0; y < 8; y++) { for (var c0 = 0; c0 < 8; c0++) {
        var s = 0.0;
        for (var x = 0; x < 8; x++) { s += C[c0 * 8 + x] * b[y * 8 + x]; }
        t[y * 8 + c0] = s;
    } }
    for (var c0 = 0; c0 < 8; c0++) { for (var v = 0; v < 8; v++) {
        var s = 0.0;
        for (var y = 0; y < 8; y++) { s += C[v * 8 + y] * t[y * 8 + c0]; }
        f[v * 8 + c0] = round_away(s / u.q) * u.q;
    } }
    for (var c0 = 0; c0 < 8; c0++) { for (var y = 0; y < 8; y++) {
        var s = 0.0;
        for (var v = 0; v < 8; v++) { s += C[v * 8 + y] * f[v * 8 + c0]; }
        t[y * 8 + c0] = s;
    } }
    for (var y = 0; y < 8; y++) { for (var x = 0; x < 8; x++) {
        var s = 0.0;
        for (var c0 = 0; c0 < 8; c0++) { s += C[c0 * 8 + x] * t[y * 8 + c0]; }
        res[(oy + y) * w + ox + x] = i32(clamp(round_away(s), -512.0, 512.0));
    } }
}
