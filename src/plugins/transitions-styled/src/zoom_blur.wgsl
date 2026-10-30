// zoom_blur: rays from the centre (p0.yz)
fn trans_zoom_blur(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let o = p0.yz;
    let amt = p0.x * sin(pi * k);
    var which = 1u;
    if (k < 0.5) { which = 0u; }
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var i = 0; i < 7; i = i + 1) {
        let z = 1.0 + amt * 0.6 * (f32(i) / 6.0);
        acc = acc + samp(which, (uv - o) / z + o);
    }
    return acc / 7.0;
}
