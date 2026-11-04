// The decoder of one plane (compute-kernels-render, D2): the block copy by the vector field with
// clamped coordinates, plus the residual scaled by the gain. Integer arithmetic only.
struct U { w: u32, h: u32, bs: u32, shift: u32, gain: i32, mw: u32, intra: u32, pad: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> rf: array<u32>;
@group(0) @binding(2) var<storage, read> mv: array<vec2<i32>>;
@group(0) @binding(3) var<storage, read> res: array<i32>;
@group(0) @binding(4) var<storage, read_write> pic: array<u32>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let pbs = i32(u.bs >> u.shift);
    let w = i32(u.w);
    let h = i32(u.h);
    if i32(id.x) >= w / pbs || i32(id.y) >= h / pbs { return; }
    let ox = i32(id.x) * pbs;
    let oy = i32(id.y) * pbs;
    var v = vec2<i32>(0, 0);
    if u.intra == 0u { v = mv[id.y * u.mw + id.x] >> vec2<u32>(u.shift); }
    for (var y = 0; y < pbs; y++) {
        let sy = clamp(oy + y + v.y, 0, h - 1);
        for (var x = 0; x < pbs; x++) {
            var p = 128;
            if u.intra == 0u { p = i32(rf[sy * w + clamp(ox + x + v.x, 0, w - 1)]); }
            let i = (oy + y) * w + ox + x;
            pic[i] = u32(clamp(p + ((res[i] * u.gain) >> 8u), 0, 255));
        }
    }
}
