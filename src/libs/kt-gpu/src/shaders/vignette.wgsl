// Vignette (Р.47): зеркало kt_fx::geom::tap_vig; на барьерной позиции
// координата стадии = координата кадра пасса.
// f0 = (strength, radius, softness, 0), u0.xy = (w, h).
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(pos.xy);
    let t = textureLoad(T0, p, 0);
    let v = vec3<f32>(f32(t.x), f32(t.y), f32(t.z));
    let st = P.f0.x;
    let ra = P.f0.y;
    let so = P.f0.z;
    let w = f32(P.u0.x);
    let h = f32(P.u0.y);
    let dx = (floor(pos.x) + 0.5) / max(w, 1.0) - 0.5;
    let dy = (floor(pos.y) + 0.5) / max(h, 1.0) - 0.5;
    let r = sqrt((2.0 * dx) * (2.0 * dx) + (2.0 * dy) * (2.0 * dy)) / sqrt(2.0);
    let tt = clamp((r - ra) / max(so, 1e-6), 0.0, 1.0);
    let ww = tt * tt * (3.0 - 2.0 * tt);
    let o = vec3<u32>(clamp(round(v * (1.0 - st * ww)), vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<u32>(o, t.w);
}
