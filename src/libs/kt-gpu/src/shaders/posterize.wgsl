// Posterize (Р.47): зеркало kt_fx::geom::tap_poster (квант по каналам).
// f0.x = levels.
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(pos.xy);
    let t = textureLoad(T0, p, 0);
    let v = vec3<f32>(f32(t.x), f32(t.y), f32(t.z));
    let n = P.f0.x;
    let idx = clamp(floor(v * n / 256.0), vec3<f32>(0.0), vec3<f32>(n - 1.0));
    let o = vec3<u32>(clamp(round(idx * 255.0 / (n - 1.0)), vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<u32>(o, t.w);
}
