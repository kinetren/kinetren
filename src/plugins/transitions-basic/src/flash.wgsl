// flash
fn trans_flash(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let base = mix(samp(0u, uv), samp(1u, uv), k);
    let w = clamp(p0.w * sin(pi * k), 0.0, 1.0);
    return vec4<f32>(mix(base.rgb, p0.rgb, w), base.a);
}
