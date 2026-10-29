// ColorGrade (R.47): a mirror of the plugin's tap on f32 (one text for the fused pass's tap arm and
// its own generated pass — plugin-api-stage1, D5). a.yzw = (brightness, contrast, saturation),
// b.xy = (hue_cos, hue_sin).
fn tap_color_grade(v0: vec4<f32>, a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    var v = v0;
    let br = a.y;
    let con = a.z;
    let sat = a.w;
    let hc = b.x;
    let hs = b.y;
    let l = (77.0 * v.x + 150.0 * v.y + 29.0 * v.z) / 256.0;
    v = vec4<f32>(vec3<f32>(l) + (v.xyz - vec3<f32>(l)) * sat, v.w);
    if (hc != 1.0 || hs != 0.0) {
        let kk = 1.0 / sqrt(3.0);
        let dotv = kk * (v.x + v.y + v.z);
        let cx = kk * (v.y - v.z);
        let cy = kk * (v.z - v.x);
        let cz = kk * (v.x - v.y);
        v = vec4<f32>(
            v.x * hc + cx * hs + kk * dotv * (1.0 - hc),
            v.y * hc + cy * hs + kk * dotv * (1.0 - hc),
            v.z * hc + cz * hs + kk * dotv * (1.0 - hc),
            v.w,
        );
    }
    v = vec4<f32>((v.xyz - vec3<f32>(128.0)) * (1.0 + con) + vec3<f32>(128.0 + br * 255.0), v.w);
    return v;
}
