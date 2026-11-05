// The preview's display pass (preview-throughput D2): the integer composite into the Rgba8Unorm
// texture egui draws, premultiplied in gamma space as `Color32::from_rgba_unmultiplied` does, so
// the picture is the one the readback path shows. Nearest sampling at the target's size: 1:1 for
// the frame, the 32×18 picks for the ambient glow.
// u0 = (source w, source h, target w, target h)
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let x = u32(pos.x) * P.u0.x / P.u0.z;
    let y = u32(pos.y) * P.u0.y / P.u0.w;
    let c = vec4<f32>(textureLoad(T0, vec2<i32>(i32(x), i32(y)), 0));
    let a = c.a / 255.0;
    return vec4<f32>(c.rgb * a / 255.0, a);
}
