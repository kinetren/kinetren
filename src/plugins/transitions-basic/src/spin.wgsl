// spin: centre (p0.zw)
fn trans_spin(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let od = out_dims();
    let aspect = od.x / od.y;
    let pi = 3.14159265;
    let o = p0.zw;
    let sa = rot_sample(0u, uv, p0.x * k * 2.0 * pi, 1.0 + p0.y * k, aspect, o);
    let sb = rot_sample(1u, uv, -p0.x * (1.0 - k) * 2.0 * pi, 1.0 + p0.y * (1.0 - k), aspect, o);
    return mix(sa, sb, k);
}
