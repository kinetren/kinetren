// The decoded planes back to opaque RGBA (stream-layout-per-build), cropped to the stream's size;
// chroma by nearest neighbour — `Planes::to_rgba`, the CPU reference, to the byte (t2274). One
// invocation per pixel; the planes are bytes, four to a word; RGBA is one word a pixel.
struct U { w: u32, ow: u32, oh: u32, pad: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> py: array<u32>;
@group(0) @binding(2) var<storage, read> pcb: array<u32>;
@group(0) @binding(3) var<storage, read> pcr: array<u32>;
@group(0) @binding(4) var<storage, read_write> out: array<u32>;

fn byte_of(v: u32, i: u32) -> i32 { return i32((v >> ((i & 3u) * 8u)) & 255u); }

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= u.ow || id.y >= u.oh { return; }
    let i = id.y * u.w + id.x;
    let l = byte_of(py[i >> 2u], i);
    let ci = (id.y / 2u) * (u.w / 2u) + id.x / 2u;
    let cb = byte_of(pcb[ci >> 2u], ci) - 128;
    let cr = byte_of(pcr[ci >> 2u], ci) - 128;
    let r = clamp(l + ((359 * cr + 128) >> 8u), 0, 255);
    let g = clamp(l - ((88 * cb + 183 * cr + 128) >> 8u), 0, 255);
    let b = clamp(l + ((454 * cb + 128) >> 8u), 0, 255);
    out[id.y * u.ow + id.x] = u32(r) | (u32(g) << 8u) | (u32(b) << 16u) | 0xff000000u;
}
