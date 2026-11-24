// A Merge's matte (fx-graph-dag D3): a coverage made from a picture, INTEGER arithmetic.
//
// T0 = the matte frame. u0 = (by, has_alpha, k, 0): by 0 alpha, 1 luma (Rec.709 integer weights),
// 2 inverted alpha, 3 inverted luma; k the Merge's mix (0..4096). The coverage goes to .r, as the
// CPU rasterizer's coverage does, and `layer_mix.wgsl`'s coverage branch reads it.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    var a = c.a;
    if (P.u0.y == 0u) { a = 255u; }
    var m = a;
    if (P.u0.x == 1u || P.u0.x == 3u) {
        m = (54u * c.r + 183u * c.g + 19u * c.b + 128u) >> 8u;
    }
    if (P.u0.x >= 2u) { m = 255u - m; }
    let cov = (m * P.u0.z + 2048u) >> 12u;
    return vec4<u32>(cov, 0u, 0u, 255u);
}
