// The broken decoder's vector edit (stream-history-on-device D3): swap, scale and rotation by a table
// the CPU computed for every vector the search can give (`mosh::transform` to the integer, so the
// device never rounds a float), then macroblock loss by the integer hash of `mosh::decode_inter`:
// a lost block takes a random vector within the search range and is marked for the residual drop.
// One invocation per luma block.
struct U { n: u32, flags: u32, thr: u32, seed: u32, k: u32, rr: i32, tr: i32, pad: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> src: array<vec2<i32>>;
@group(0) @binding(2) var<storage, read> table: array<vec2<i32>>;
@group(0) @binding(3) var<storage, read_write> mv: array<vec2<i32>>;
@group(0) @binding(4) var<storage, read_write> lost: array<u32>;

fn lowbias32(v: u32) -> u32 {
    var x = v;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.y * 65535u * 64u + id.x;
    if i >= u.n { return; }
    var v = src[i];
    // bit 0: the table (a transform that is not the identity)
    if (u.flags & 1u) != 0u {
        let c = clamp(v, vec2<i32>(-u.tr), vec2<i32>(u.tr)) + vec2<i32>(u.tr);
        v = table[u32(c.y) * u32(2 * u.tr + 1) + u32(c.x)];
    }
    var gone = 0u;
    // bit 1: loss; bit 2: every block (loss 1)
    if (u.flags & 2u) != 0u {
        let h = lowbias32(u.seed ^ lowbias32(u.k ^ lowbias32(i ^ 0x6d6f7368u)));
        if (u.flags & 4u) != 0u || h < u.thr {
            let a = lowbias32(h ^ 0x5bd1e995u);
            let span = u32(2 * u.rr + 1);
            v = vec2<i32>(i32(a % span) - u.rr, i32((a >> 16u) % span) - u.rr);
            gone = 1u;
        }
    }
    mv[i] = v;
    lost[i] = gone;
}
