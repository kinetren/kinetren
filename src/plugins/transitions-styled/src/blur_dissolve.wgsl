// blur_dissolve
fn trans_blur_dissolve(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let od = out_dims();
    let pi = 3.14159265;
    let r = p0.x * sin(pi * k);
    let rr = vec2<f32>(r / od.x, r / od.y);
    return mix(box9(0u, uv, rr), box9(1u, uv, rr), k);
}
