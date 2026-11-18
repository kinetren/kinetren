// The stream's input to the codec's size (mosh-sharp D3): per axis a box over the source footprint when
// shrinking, bilinear with 8-bit weights when growing (centre-aligned), the identity at equal sizes; the
// 2-D weight is the product, rounded once — `codec::resample_rgba`, the CPU reference, to the byte. RGBA,
// one word a pixel; the output is opaque.
struct U { sw: u32, sh: u32, dw: u32, dh: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> src: array<u32>;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;

// (first tap, end or second tap, bilinear?, fraction)
fn axis(o: u32, s: u32, d: u32) -> vec4<u32> {
    if d == s { return vec4<u32>(o, o + 1u, 0u, 0u); }
    if d < s {
        let lo = o * s / d;
        return vec4<u32>(lo, max((o + 1u) * s / d, lo + 1u), 0u, 0u);
    }
    let p = max(i32((2u * o + 1u) * s * 128u / d) - 128, 0);
    let i0 = u32(p >> 8u);
    return vec4<u32>(i0, min(i0 + 1u, s - 1u), 1u, u32(p & 255));
}

// tap `t` of an axis: (index, weight); a box runs over [lo, end), bilinear has two
fn tap(a: vec4<u32>, t: u32) -> vec2<u32> {
    if a.z == 0u { return vec2<u32>(a.x + t, 1u); }
    if t == 0u { return vec2<u32>(a.x, 256u - a.w); }
    return vec2<u32>(a.y, a.w);
}

fn taps(a: vec4<u32>) -> u32 {
    if a.z == 0u { return a.y - a.x; }
    return 2u;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= u.dw || id.y >= u.dh { return; }
    let ax = axis(id.x, u.sw, u.dw);
    let ay = axis(id.y, u.sh, u.dh);
    var acc = vec3<u32>(0u, 0u, 0u);
    var wsum = 0u;
    for (var ty = 0u; ty < taps(ay); ty++) {
        let y = tap(ay, ty);
        for (var tx = 0u; tx < taps(ax); tx++) {
            let x = tap(ax, tx);
            let p = src[y.x * u.sw + x.x];
            let w = x.y * y.y;
            acc += vec3<u32>(p & 255u, (p >> 8u) & 255u, (p >> 16u) & 255u) * w;
            wsum += w;
        }
    }
    let r = (acc + vec3<u32>(wsum / 2u)) / wsum;
    dst[id.y * u.dw + id.x] = r.x | (r.y << 8u) | (r.z << 16u) | (255u << 24u);
}
