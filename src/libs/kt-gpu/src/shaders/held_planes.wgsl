// A stream stage's held picture into the layer's texture (stream-picture-stays D3): three 8-bit planes,
// luma `pw` wide and chroma half of it, to opaque RGBA in the integer arithmetic of
// `kt_plugin_api::stream::planes_to_rgba` — the same bytes as the processor's conversion.
struct U {
    pw: u32,
    w: u32,
    h: u32,
    pad: u32,
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read> py: array<u32>;
@group(0) @binding(2) var<storage, read> pcb: array<u32>;
@group(0) @binding(3) var<storage, read> pcr: array<u32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<u32> {
    let x = u32(p.x);
    let y = u32(p.y);
    let li = y * u.pw + x;
    let l = i32((py[li >> 2u] >> ((li & 3u) * 8u)) & 255u);
    let ci = (y / 2u) * (u.pw / 2u) + x / 2u;
    let cb = i32((pcb[ci >> 2u] >> ((ci & 3u) * 8u)) & 255u) - 128;
    let cr = i32((pcr[ci >> 2u] >> ((ci & 3u) * 8u)) & 255u) - 128;
    let r = clamp(l + ((359 * cr + 128) >> 8u), 0, 255);
    let g = clamp(l - ((88 * cb + 183 * cr + 128) >> 8u), 0, 255);
    let b = clamp(l + ((454 * cb + 128) >> 8u), 0, 255);
    return vec4<u32>(u32(r), u32(g), u32(b), 255u);
}
