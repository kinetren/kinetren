// Layer blend strength (R.38 item 12): a mirror of `dissolve_into` in
// kt-engine/src/video.rs, INTEGER and byte-for-byte with it and with
// `dis()` from composite.wgsl (the same formula, a separate pass: the mix
// happens INSIDE the layer’s chain, not at the composite).
//
// T0 = dry side (the state BEFORE the layer’s contribution), T1 = wet (after).
// u0 = (k, dry_has_alpha, wet_has_alpha, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    var a = textureLoad(T0, p, 0);
    var b = textureLoad(T1, p, 0);
    if (P.u0.y == 0u) { a.a = 255u; }
    if (P.u0.z == 0u) { b.a = 255u; }
    let k = P.u0.x;
    return (a * (4096u - k) + b * k + vec4<u32>(2048u)) >> vec4<u32>(12u);
}
