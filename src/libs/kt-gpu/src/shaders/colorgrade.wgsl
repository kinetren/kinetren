// ColorGrade (Р.47): зеркало kt_fx::geom::tap_grade на f32.
// f0 = (brightness, contrast, saturation, hue_cos), f1.x = hue_sin.
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(pos.xy);
    let t = textureLoad(T0, p, 0);
    var v = vec3<f32>(f32(t.x), f32(t.y), f32(t.z));
    let br = P.f0.x;
    let con = P.f0.y;
    let sat = P.f0.z;
    let hc = P.f0.w;
    let hs = P.f1.x;
    let l = (77.0 * v.x + 150.0 * v.y + 29.0 * v.z) / 256.0;
    v = vec3<f32>(l) + (v - vec3<f32>(l)) * sat;
    if (hc != 1.0 || hs != 0.0) {
        let kk = 1.0 / sqrt(3.0);
        let dotv = kk * (v.x + v.y + v.z);
        let c = vec3<f32>(kk * (v.y - v.z), kk * (v.z - v.x), kk * (v.x - v.y));
        v = v * hc + c * hs + vec3<f32>(kk * dotv * (1.0 - hc));
    }
    v = (v - vec3<f32>(128.0)) * (1.0 + con) + vec3<f32>(128.0 + br * 255.0);
    let o = vec3<u32>(clamp(round(v), vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<u32>(o, t.w);
}
