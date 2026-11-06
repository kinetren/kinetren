// p0..p3 = the colours of the corners: top left, top right, bottom left, bottom right.
fn pixel(x: u32, y: u32) -> u32 {
    let f = (vec2<f32>(f32(x), f32(y)) + 0.5) / vec2<f32>(f32(u.w), f32(u.h));
    return pack(vec4<f32>(mix(mix(u.p0.xyz, u.p1.xyz, f.x), mix(u.p2.xyz, u.p3.xyz, f.x), f.y), 255.0));
}
