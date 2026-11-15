// C: the source's transfer as a table (per-sample-costs, found-defects 362): texel x of a 256×1 pass
// is `decode(x, curve)` — the same function s00 calls — as the f32's four bytes, so s00's defocus
// disc reads 51 table texels a pixel instead of taking 51 `pow`.
// u0 = (flags, 0, 0, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let curve = (P.u0.x >> 17u) & 7u;
    let b = bitcast<u32>(decode(u32(floor(pos.x)), curve));
    return vec4<u32>(b & 255u, (b >> 8u) & 255u, (b >> 16u) & 255u, b >> 24u);
}
