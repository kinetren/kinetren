// Shadow under a layer (R.47): a mirror of the tail of kt_fx::video::drop_shadow:
// the source OVER the shadow from the blurred alpha with an offset (straight alpha).
// T0 = original, T1 = blurred copy. u0 = (w, h, ox, oy — bitcast i32);
// f0 = (opacity, tint_r, tint_g, tint_b).
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let off = vec2<i32>(bitcast<i32>(P.u0.z), bitcast<i32>(P.u0.w));
    let b = p - off;
    var ba = 0.0;
    if (b.x >= 0 && b.y >= 0 && b.x < i32(P.u0.x) && b.y < i32(P.u0.y)) {
        ba = f32(textureLoad(T1, b, 0).a) / 255.0;
    }
    let ash = ba * P.f0.x;
    let asrc = f32(c.a) / 255.0;
    let oa = asrc + ash * (1.0 - asrc);
    if (oa <= 0.0) {
        return vec4<u32>(0u);
    }
    let tint = vec3<f32>(P.f0.y, P.f0.z, P.f0.w);
    let rgb = (vec3<f32>(f32(c.r), f32(c.g), f32(c.b)) * asrc + tint * ash * (1.0 - asrc)) / oa;
    return vec4<u32>(
        vec3<u32>(clamp(round(rgb), vec3<f32>(0.0), vec3<f32>(255.0))),
        u32(clamp(round(oa * 255.0), 0.0, 255.0)),
    );
}
