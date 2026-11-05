// Mask alpha (shape-masks): INTEGER arithmetic.
// The coverage is the CPU rasterizer's bytes, uploaded as a texture — the GPU never rasterizes.
//
// T0 = the frame, T1 = the coverage (in .r). u0 = (frame_has_alpha, 0, 0, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    var a = textureLoad(T0, p, 0);
    if (P.u0.x == 0u) { a.a = 255u; }
    let m = textureLoad(T1, p, 0).r;
    a.a = (a.a * m + 127u) / 255u;
    return a;
}
