// The carrier decoder of one plane (mosh-sharp D1): the codec's decisions at its size executed on a
// picture of the stream's input size. A carrier pixel takes the codec block covering it (the batch's
// table of blocks by luma column and row), predicts from the reference by that block's applied vector in
// carrier pels (the `coded` step scaled it once per block), and where the block is coded in this plane
// adds `gain ×` the source's own difference: the frame minus the previous frame moved by the packet's own
// vector. Integer only — `codec::carry_planes`, the CPU reference, to the byte. Planes are bytes, four to
// a word; each invocation writes one word.
struct U { pw: u32, ph: u32, mw: u32, shift: u32, gain: i32, bit: u32, lw: u32, pad: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> rf: array<u32>;
@group(0) @binding(2) var<storage, read> cur: array<u32>;
@group(0) @binding(3) var<storage, read> prev: array<u32>;
@group(0) @binding(4) var<storage, read> vecs: array<vec4<i32>>;
@group(0) @binding(5) var<storage, read> flags: array<u32>;
// the block column of each luma x, then (from `lw`) the block row of each luma y
@group(0) @binding(6) var<storage, read> grid: array<u32>;
@group(0) @binding(7) var<storage, read_write> pic: array<u32>;

fn rf_at(i: i32) -> i32 { return i32((rf[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u); }
fn cur_at(i: i32) -> i32 { return i32((cur[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u); }
fn prev_at(i: i32) -> i32 { return i32((prev[u32(i) >> 2u] >> ((u32(i) & 3u) * 8u)) & 255u); }

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= u.pw / 4u || id.y >= u.ph { return; }
    let w = i32(u.pw);
    let h = i32(u.ph);
    let y = i32(id.y);
    let row = grid[u.lw + (id.y << u.shift)] * u.mw;
    var word = 0u;
    for (var j = 0u; j < 4u; j++) {
        let x = i32(id.x * 4u + j);
        let b = row + grid[u32(x) << u.shift];
        let v = vecs[b] >> vec4<u32>(u.shift);
        var out = rf_at(clamp(y + v.y, 0, h - 1) * w + clamp(x + v.x, 0, w - 1));
        if (flags[b] & u.bit) != 0u {
            let r = cur_at(y * w + x) - prev_at(clamp(y + v.w, 0, h - 1) * w + clamp(x + v.z, 0, w - 1));
            out += (r * u.gain) >> 8u;
        }
        word |= u32(clamp(out, 0, 255)) << (j * 8u);
    }
    pic[(id.y * u.pw + id.x * 4u) >> 2u] = word;
}
