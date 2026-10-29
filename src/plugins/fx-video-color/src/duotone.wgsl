// Duotone (R.47): a mirror of the plugin's tap (one text for the fused pass's tap arm and its own
// generated pass — plugin-api-stage1, D5). a.yzw = shadow, b.xyz = high, b.w = amount.
fn tap_duotone(v0: vec4<f32>, a: vec4<f32>, b: vec4<f32>, c: vec2<f32>) -> vec4<f32> {
    var v = v0;
    let sh3 = a.yzw;
    let hi3 = b.xyz;
    let am = b.w;
    let l = clamp((77.0 * v.x + 150.0 * v.y + 29.0 * v.z) / 256.0 / 255.0, 0.0, 1.0);
    let outc = sh3 + (hi3 - sh3) * l;
    v = vec4<f32>(v.xyz + (outc - v.xyz) * am, v.w);
    return v;
}
