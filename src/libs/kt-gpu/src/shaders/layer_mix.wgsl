// Сила наложения слоя (Р.38 п.12) — зеркало `dissolve_into` в
// kt-engine/src/video.rs, ЦЕЛОЧИСЛЕННО и байт-в-байт с ним и с
// `dis()` из composite.wgsl (та же формула, отдельный пасс: смешение
// идёт ВНУТРИ цепочки слоя, а не при композите).
//
// T0 = сухая сторона (состояние ДО вклада слоя), T1 = мокрая (после).
// u0 = (k, dry_has_alpha, wet_has_alpha, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    var a = textureLoad(T0, p, 0);
    var b = textureLoad(T1, p, 0);
    if (P.u0.y == 0u) { a.a = 255u; }
    if (P.u0.z == 0u) { b.a = 255u; }
    let k = P.u0.x;
    return (a * (4096u - k) + b * k + vec4<u32>(2048u)) >> vec4<u32>(12u);
}
