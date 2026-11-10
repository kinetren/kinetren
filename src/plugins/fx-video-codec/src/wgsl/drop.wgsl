// The residual of a lost macroblock zeroed (stream-history-on-device D3), `codec::drop_block_residual`
// on the device: a copy of one plane's residual (two 16-bit samples to a word) with the words of the
// lost blocks cleared. A block's row is whole words (its width is 16, 8 or 4 at a multiple of itself).
// One invocation per word.
struct U { w: u32, h: u32, pbs: u32, mw: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> res: array<u32>;
@group(0) @binding(2) var<storage, read> lost: array<u32>;
@group(0) @binding(3) var<storage, read_write> out: array<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let j = id.y * 65535u * 64u + id.x;
    if j >= u.w * u.h / 2u { return; }
    let s = j * 2u;
    let x = s % u.w;
    let y = s / u.w;
    if lost[(y / u.pbs) * u.mw + x / u.pbs] != 0u {
        out[j] = 0u;
    } else {
        out[j] = res[j];
    }
}
