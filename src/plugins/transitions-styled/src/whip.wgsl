// whip
fn trans_whip(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let blur = p0.z * sin(pi * k);
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var i = 0; i < 7; i = i + 1) {
        let o = (f32(i) / 6.0 - 0.5) * blur * 0.25;
        let s = uv + p0.xy * o;
        let a = s - p0.xy * k;
        if (a.x >= 0.0 && a.x < 1.0 && a.y >= 0.0 && a.y < 1.0) { acc = acc + samp(0u, a); }
        else { acc = acc + samp(1u, a + p0.xy); } // B behind A, +dir as in push (item 396)
    }
    return acc / 7.0;
}
