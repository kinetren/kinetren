// VHS — зеркало vhs (kt-fx/src/video.rs): RGB-сдвиг + скан-линии каждые
// 4 строки, дрожание line_off считается на CPU. Читается только исходная
// строка (у CPU — её копия) — GPU не in-place по построению.
// u0 = (w, shift, line_off, 0); f0 = (bleed, 0, 0, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let w = i32(P.u0.x);
    let shift = i32(P.u0.y);
    var scan = 0;
    if ((p.y + i32(P.u0.z)) % 4 == 0) { scan = 1; }
    let xr = clamp(p.x + shift + scan, 0, w - 1);
    let xb = clamp(p.x - shift - scan, 0, w - 1);
    let c = textureLoad(T0, p, 0);
    let cr = f32(textureLoad(T0, vec2<i32>(xr, p.y), 0).r);
    let cb = f32(textureLoad(T0, vec2<i32>(xb, p.y), 0).b);
    let bleed = P.f0.x;
    let r = u32(round(f32(c.r) * (1.0 - bleed) + cr * bleed));
    let b = u32(round(f32(c.b) * (1.0 - bleed) + cb * bleed));
    var g = c.g;
    if (scan == 1) {
        g = select(0u, g - 18u, g >= 18u); // saturating_sub(18)
    }
    return vec4<u32>(r, g, b, c.a);
}
