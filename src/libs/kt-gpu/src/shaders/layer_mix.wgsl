// Layer blend strength (R.38 item 12): a mirror of `dissolve_into` in
// kt-engine/src/video.rs, INTEGER and byte-for-byte with it and with
// `dis()` from composite.wgsl (the same formula, a separate pass: the mix
// happens INSIDE the layer’s chain, not at the composite).
//
// T0 = dry side (the state BEFORE the layer’s contribution), T1 = wet (after).
// u0 = (k, dry_has_alpha, wet_has_alpha, by_coverage).
//
// A SLOT MASK (shape-masks) is the same wet/dry mix with a per-pixel weight: `by_coverage`
// = 1 reads the weight from T2 (the CPU rasterizer's coverage in .r, 0..255), a mirror of
// `kt_fx::mask::apply_mix`.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    var a = textureLoad(T0, p, 0);
    var b = textureLoad(T1, p, 0);
    if (P.u0.y == 0u) { a.a = 255u; }
    if (P.u0.z == 0u) { b.a = 255u; }
    if (P.u0.w == 1u) {
        let m = textureLoad(T2, p, 0).r;
        return (a * (255u - m) + b * m + vec4<u32>(127u)) / vec4<u32>(255u);
    }
    let k = P.u0.x;
    return (a * (4096u - k) + b * k + vec4<u32>(2048u)) >> vec4<u32>(12u);
}
