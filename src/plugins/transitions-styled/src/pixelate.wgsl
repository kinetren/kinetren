// pixelate
fn trans_pixelate(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let od = out_dims();
    let pi = 3.14159265;
    let s = max(p0.x * sin(pi * k), 1.0);
    let b = vec2<f32>(quant(uv.x, s / od.x), quant(uv.y, s / od.y));
    return mix(samp(0u, b), samp(1u, b), k);
}
