// Straight alpha from premultiplied (R.50): the embedding boundary.
//
// A MIRROR of `kt_engine::video::unpremultiply`, byte for byte, because
// the composite accumulator stores premultiplied RGB under STRAIGHT alpha
// (`alpha_over`: at da == 0 it yields dst.rgb = s·sa/255, dst.a = sa), while
// the blend expects straight alpha. Without the conversion the sub-composite lands twice as dark
// exactly in the semi-transparent places.
//
// The arithmetic is INTEGER and matches the CPU to the last digit: all the engine’s
// textures are `Rgba8Uint`, division with rounding to the nearest (`+ a/2`).
// Edited in the SAME commit as the function in kt-engine.
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(pos.xy);
    let t = textureLoad(T0, p, 0);
    let a = t.w;
    if (a == 255u) {
        return t;
    }
    if (a == 0u) {
        return vec4<u32>(0u, 0u, 0u, 0u);
    }
    let rgb = vec3<u32>(t.x, t.y, t.z);
    let o = min((rgb * 255u + a / 2u) / a, vec3<u32>(255u));
    return vec4<u32>(o, a);
}
