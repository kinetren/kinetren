// p0 = colour a, p1 = colour b, p2 = (dx, dy, lo, span): the projection onto the angle, stretched.
fn pixel(x: u32, y: u32) -> u32 {
    let n = (vec2<f32>(f32(x), f32(y)) + 0.5) / vec2<f32>(f32(u.w), f32(u.h));
    let t = clamp((n.x * u.p2.x + n.y * u.p2.y - u.p2.z) / u.p2.w, 0.0, 1.0);
    return pack(vec4<f32>(mix(u.p0.xyz, u.p1.xyz, t), 255.0));
}
