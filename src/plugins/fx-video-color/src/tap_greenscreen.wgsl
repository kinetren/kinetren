// Greenscreen as a TAP of the fused geometry pass (plugin-api-barriers 4.2): the former
// `tap_kind == 0` arm of geom_fused.wgsl verbatim — the f32 continuum of the kernel's formula, a
// mirror of the plugin's f64 `tap`. a.yzw = the key colour, b = (tolerance, hard, spill, kc).

fn tap_greenscreen(v0: vec4<f32>, a: vec4<f32>, b: vec4<f32>, c: vec2<f32>) -> vec4<f32> {
    var v = v0;
    let key = a.yzw;
    let tol = b.x;
    let hard = b.y;
    let spill = b.z;
    let kc = u32(b.w);
    let dd = distance(v.xyz, key);
    if (dd <= hard) {
        v.w = 0.0;
    } else if (dd < tol) {
        v.w = v.w * (dd - hard) / max(tol - hard, 1e-9);
    }
    if (spill > 0.0 && v.w > 0.0) {
        var vc = 0.0;
        var o1 = 0.0;
        var o2 = 0.0;
        if (kc == 0u) { vc = v.x; o1 = v.y; o2 = v.z; }
        else if (kc == 1u) { vc = v.y; o1 = v.z; o2 = v.x; }
        else { vc = v.z; o1 = v.x; o2 = v.y; }
        let other = max(o1, o2);
        if (vc > other) {
            vc = vc - (vc - other) * spill;
            if (kc == 0u) { v.x = vc; }
            else if (kc == 1u) { v.y = vc; }
            else { v.z = vc; }
        }
    }
    return v;
}
