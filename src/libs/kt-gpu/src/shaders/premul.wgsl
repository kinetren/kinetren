// Premultiplication (R.53, item 418): rgb' = rgb·a/255, rounding to the
// nearest.
//
// A MIRROR of `kt_fx::video::premultiply`, byte for byte: the arithmetic is
// INTEGER (all engine textures are `Rgba8Uint`), division with `+ 127`.
// Edited in the SAME commit as the function in kt-fx.
//
// Why: the blur mip mixes neighbouring pixels, and a transparent pixel’s colour is
// garbage. On straight alpha it seeps into the visible pixels and gives a dark
// halo along the edge of the title. At a == 255 it is an identity, so the pass is set up
// only where the frame HAS alpha.
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(pos.xy);
    let t = textureLoad(T0, p, 0);
    let a = t.w;
    if (a == 255u) {
        return t;
    }
    let rgb = vec3<u32>(t.x, t.y, t.z);
    return vec4<u32>((rgb * a + 127u) / 255u, a);
}
