// Duotone (Р.47): зеркало kt_fx::geom::tap_duo.
// f0.xyz = shadow, f1.xyz = high, f1.w = amount.
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(pos.xy);
    let t = textureLoad(T0, p, 0);
    var v = vec3<f32>(f32(t.x), f32(t.y), f32(t.z));
    let l = clamp((77.0 * v.x + 150.0 * v.y + 29.0 * v.z) / 256.0 / 255.0, 0.0, 1.0);
    let outc = P.f0.xyz + (P.f1.xyz - P.f0.xyz) * l;
    v = v + (outc - v) * P.f1.w;
    let o = vec3<u32>(clamp(round(v), vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<u32>(o, t.w);
}
