// The packet's decisions per block of the codec's grid, for the carrier (mosh-sharp D1, sharp-cost):
// a bit per plane (1 luma, 2 Cb, 4 Cr) whose residual there is not all zero — `codec::coded_flags` — and
// the block's applied and own vectors scaled to the carrier's luma pels (rounded half away from zero), so
// the carrier's pixels divide nothing (a division a pixel was most of its cost on a software device).
// The residual is two 16-bit samples to a word.
struct U { w: u32, bs: u32, mw: u32, n: u32, cw: u32, ch: u32, sw: u32, sh: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> ry: array<u32>;
@group(0) @binding(2) var<storage, read> rcb: array<u32>;
@group(0) @binding(3) var<storage, read> rcr: array<u32>;
@group(0) @binding(4) var<storage, read> mv: array<vec2<i32>>;
@group(0) @binding(5) var<storage, read> own: array<vec2<i32>>;
@group(0) @binding(6) var<storage, read_write> flags: array<u32>;
@group(0) @binding(7) var<storage, read_write> vecs: array<vec4<i32>>;

fn nz(word: u32, i: u32) -> bool { return ((word >> ((i & 1u) * 16u)) & 65535u) != 0u; }

// a codec pel in carrier pels, rounded half away from zero
fn sc(v: i32, num: u32, den: u32) -> i32 {
    let a = v * i32(num);
    let d = i32(den);
    if a >= 0 { return (2 * a + d) / (2 * d); }
    return -((-2 * a + d) / (2 * d));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= u.n { return; }
    let bx = id.x % u.mw;
    let by = id.x / u.mw;
    var f = 0u;
    for (var y = 0u; y < u.bs; y++) {
        for (var x = 0u; x < u.bs; x++) {
            let i = (by * u.bs + y) * u.w + bx * u.bs + x;
            if nz(ry[i >> 1u], i) { f |= 1u; }
        }
    }
    let cs = u.bs / 2u;
    let cw = u.w / 2u;
    for (var y = 0u; y < cs; y++) {
        for (var x = 0u; x < cs; x++) {
            let i = (by * cs + y) * cw + bx * cs + x;
            if nz(rcb[i >> 1u], i) { f |= 2u; }
            if nz(rcr[i >> 1u], i) { f |= 4u; }
        }
    }
    flags[id.x] = f;
    let v = mv[id.x];
    let o = own[id.x];
    vecs[id.x] = vec4<i32>(sc(v.x, u.sw, u.cw), sc(v.y, u.sh, u.ch), sc(o.x, u.sw, u.cw), sc(o.y, u.sh, u.ch));
}
