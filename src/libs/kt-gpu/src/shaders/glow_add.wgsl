// Финал bloom — зеркало хвоста glow (kt-fx/src/video.rs): o + up·s с
// потолком 255 и УСЕЧЕНИЕМ (CPU `as u8` после .min(255.0)).
// T0 = оригинал, T1 = апскейл малого мипа. f0 = (s, 0, 0, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let u = textureLoad(T1, p, 0);
    let s = P.f0.x;
    let rgb = vec3<u32>(vec3<f32>(
        min(f32(c.r) + f32(u.r) * s, 255.0),
        min(f32(c.g) + f32(u.g) * s, 255.0),
        min(f32(c.b) + f32(u.b) * s, 255.0),
    ));
    return vec4<u32>(rgb, c.a);
}
