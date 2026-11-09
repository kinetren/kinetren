// The source frame into the codec's planes (stream-layout-per-build): RGBA → Y'CbCr 4:2:0, BT.601 full
// range in integers, the chroma averaged over 2×2, the padding replicating the frame's last column and
// row — `Planes::from_rgba`, the CPU reference, to the byte (t2274). One invocation per four chroma
// samples of a row: 8×2 luma samples, so every word it writes is its own. RGBA is one word a pixel;
// the planes are bytes, four to a word.
struct U { sw: u32, sh: u32, w: u32, h: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> src: array<u32>;
@group(0) @binding(2) var<storage, read_write> py: array<u32>;
@group(0) @binding(3) var<storage, read_write> pcb: array<u32>;
@group(0) @binding(4) var<storage, read_write> pcr: array<u32>;

fn px(x: u32, y: u32) -> vec3<i32> {
    let p = src[min(y, u.sh - 1u) * u.sw + min(x, u.sw - 1u)];
    return vec3<i32>(i32(p & 255u), i32((p >> 8u) & 255u), i32((p >> 16u) & 255u));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= u.w / 8u || id.y >= u.h / 2u { return; }
    let x0 = id.x * 8u;
    let y0 = id.y * 2u;
    var yw = array<u32, 4>(0u, 0u, 0u, 0u);
    var cbw = 0u;
    var crw = 0u;
    for (var j = 0u; j < 4u; j++) {
        var cbs = 0;
        var crs = 0;
        for (var dy = 0u; dy < 2u; dy++) {
            for (var dx = 0u; dx < 2u; dx++) {
                let lx = 2u * j + dx;
                let c = px(x0 + lx, y0 + dy);
                let l = clamp((77 * c.x + 150 * c.y + 29 * c.z + 128) >> 8u, 0, 255);
                yw[dy * 2u + lx / 4u] |= u32(l) << ((lx % 4u) * 8u);
                cbs += ((-43 * c.x - 85 * c.y + 128 * c.z + 128) >> 8u) + 128;
                crs += ((128 * c.x - 107 * c.y - 21 * c.z + 128) >> 8u) + 128;
            }
        }
        cbw |= u32(clamp((cbs + 2) >> 2u, 0, 255)) << (j * 8u);
        crw |= u32(clamp((crs + 2) >> 2u, 0, 255)) << (j * 8u);
    }
    let row = u.w / 4u;
    let at = y0 * row + x0 / 4u;
    py[at] = yw[0];
    py[at + 1u] = yw[1];
    py[at + row] = yw[2];
    py[at + row + 1u] = yw[3];
    let ci = id.y * (u.w / 8u) + id.x;
    pcb[ci] = cbw;
    pcr[ci] = crw;
}
