// Sharpen (Р.47): зеркало хвоста kt_fx::video::sharpen — src +
// (src − мип-база)·amount, альфа исходника. T0 = оригинал, T1 = база
// (down/up ресемплами). f0.x = amount.
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let b = textureLoad(T1, p, 0);
    let a = P.f0.x;
    var o = vec3<f32>(0.0);
    for (var i = 0; i < 3; i = i + 1) {
        let s = f32(c[i]);
        o[i] = s + (s - f32(b[i])) * a;
    }
    let rgb = vec3<u32>(clamp(round(o), vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<u32>(rgb, c.a);
}
