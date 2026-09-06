// Финал bloom — зеркало хвоста glow (kt-fx/src/video.rs): o + up·s·tint с
// потолком 255 и УСЕЧЕНИЕМ (CPU `as u8` после .min(255.0)).
// T0 = оригинал, T1 = апскейл малого мипа.
// u0 = (threshold, 0, 0, 0); f0 = (s, tint_r, tint_g, tint_b).
// threshold = 0 и белый оттенок — прежняя формула байт-в-байт.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let u = textureLoad(T1, p, 0);
    let s = P.f0.x;
    let th = i32(P.u0.x);
    // мягкое колено по яркости размытого кадра (0 — порога нет)
    var wgt = 1.0;
    if (th > 0) {
        let luma = (77 * i32(u.r) + 150 * i32(u.g) + 29 * i32(u.b)) >> 8u;
        wgt = clamp(f32(luma - th) / f32(th), 0.0, 1.0);
    }
    let k = vec3<f32>(s * wgt) * P.f0.yzw / 255.0;
    let rgb = vec3<u32>(vec3<f32>(
        min(f32(c.r) + f32(u.r) * k.x, 255.0),
        min(f32(c.g) + f32(u.g) * k.y, 255.0),
        min(f32(c.b) + f32(u.b) * k.z, 255.0),
    ));
    return vec4<u32>(rgb, c.a);
}
