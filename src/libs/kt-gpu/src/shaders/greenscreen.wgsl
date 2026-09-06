// Хромакей — зеркало greenscreen (kt-fx/src/video.rs): евклидова дистанция
// до ключа, жёсткий порог hard, мягкий спад до tol. f32 вместо f64 —
// у жёсткой границы законные точечные выбросы (конформанс t181).
// f0 = (key_r, key_g, key_b, tol); f1 = (hard, spill, 0, 0);
// u0 = (kc, 0, 0, 0) — доминирующий канал ключа (Р.33 С5, despill).
// spill = 0 — прежний путь байт-в-байт.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let d = distance(vec3<f32>(c.rgb), P.f0.xyz);
    var a = c.a;
    if (d <= P.f1.x) {
        a = 0u;
    } else if (d < P.f0.w) {
        let t = (d - P.f1.x) / max(P.f0.w - P.f1.x, 1e-9);
        a = u32(round(f32(c.a) * t));
    }
    var rgb = c.rgb;
    let spill = P.f1.y;
    if (spill > 0.0 && a > 0u) {
        let kc = P.u0.x % 3u;
        let other = max(rgb[(kc + 1u) % 3u], rgb[(kc + 2u) % 3u]);
        if (rgb[kc] > other) {
            let v = f32(rgb[kc]);
            rgb[kc] = u32(round(v - (v - f32(other)) * spill));
        }
    }
    return vec4<u32>(rgb, a);
}
