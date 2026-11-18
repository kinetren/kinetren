// Which blocks of a packet carry a residual (mosh-sharp D1): per luma block of the codec's grid a bit per
// plane (1 luma, 2 Cb, 4 Cr) whose residual there is not all zero — `codec::coded_flags`. The residual is
// two 16-bit samples to a word.
struct U { w: u32, bs: u32, mw: u32, n: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> ry: array<u32>;
@group(0) @binding(2) var<storage, read> rcb: array<u32>;
@group(0) @binding(3) var<storage, read> rcr: array<u32>;
@group(0) @binding(4) var<storage, read_write> flags: array<u32>;

fn nz(word: u32, i: u32) -> bool { return ((word >> ((i & 1u) * 16u)) & 65535u) != 0u; }

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
}
