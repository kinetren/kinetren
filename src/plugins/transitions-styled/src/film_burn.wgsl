// film_burn: the front by noise, a hot edge
fn trans_film_burn(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let p1 = P.f1;
    let od = out_dims();
    let aspect = od.x / od.y;
    let n = value_noise(uv.x * p0.x * aspect, uv.y * p0.x);
    let mask = front_mask(n, k, max(p0.y, 0.02));
    let base = mix(samp(0u, uv), samp(1u, uv), mask);
    let rim = clamp(mask * (1.0 - mask) * 4.0, 0.0, 1.0);
    let gate = rim * base.a / 255.0;
    let tint = vec3<f32>(p0.z, p0.w, p1.x);
    return vec4<f32>(mix(base.rgb, tint, gate), base.a);
}
