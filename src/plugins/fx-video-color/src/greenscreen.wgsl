// Chroma key: a mirror of greenscreen (kt-fx/src/video.rs): Euclidean distance
// to the key, a hard threshold hard, a soft falloff up to tol. f32 instead of f64:
// at the hard boundary there are legitimate pointwise outliers (conformance t181).
// f0 = (key_r, key_g, key_b, tol); f1 = (hard, spill, 0, 0);
// u0 = (kc, 0, 0, 0): the dominant channel of the key (R.33 S5, despill).
// spill = 0 is the former path, byte for byte.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let d = distance(vec3<f32>(c.rgb), P.f0.xyz);
    var a = c.a;
    if (d <= P.f1.x) {
        a = 0u;
    } else if (d < P.f0.w) {
        let t = (d - P.f1.x) / max(P.f0.w - P.f1.x, 1e-9);
        a = u32(round(f32(c.a) * t));
    }
    var rgb = c.rgb;
    let spill = P.f1.y;
    if (spill > 0.0 && a > 0u) {
        let kc = P.u0.x % 3u;
        let other = max(rgb[(kc + 1u) % 3u], rgb[(kc + 2u) % 3u]);
        if (rgb[kc] > other) {
            let v = f32(rgb[kc]);
            // no write through a dynamic index (`rgb[kc] = …`): FXC, the D3D12 shader
            // compiler wgpu uses on Windows, rejects it ("array reference cannot be used as
            // an l-value; not natively addressable", windows-build). Same arithmetic.
            let nv = u32(round(v - (v - f32(other)) * spill));
            rgb = select(rgb, vec3<u32>(nv), vec3<bool>(kc == 0u, kc == 1u, kc == 2u));
        }
    }
    return vec4<u32>(rgb, a);
}
