// VHS — зеркало vhs (kt-fx/src/video.rs): RGB-сдвиг + скан-линии, шаг
// которых задаётся (Р.33 С5), и дрожание строк. Читается только исходная
// строка (у CPU — её копия) — GPU не in-place по построению.
// u0 = (w, shift, line_off, scan_step); u1 = (seed, jq, 0, 0);
// f0 = (bleed, 0, 0, 0). scan_step = 4 и jq = 0 — прежний кадр
// байт-в-байт. jq — КВАНТ дрожания (jitter×1024): смещение строки
// считается целыми числами, потому что round() у WGSL округляет
// половины к чётному, а у Rust — от нуля (расхождение в t181).

fn nh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let w = i32(P.u0.x);
    let shift = i32(P.u0.y);
    let step = i32(P.u0.w);
    var scan = 0;
    if (step > 0 && (p.y + i32(P.u0.z)) % step == 0) { scan = 1; }
    // дрожание строки: та же формула, что у CPU (хэш строки от seed)
    var jx = 0;
    let jq = i32(P.u1.y);
    if (jq != 0) {
        let hsh = nh(u32(p.y) ^ nh(P.u1.x));
        let n = i32((hsh >> 16u) & 0x7fffu) - 16384;
        jx = ((n * jq) / 32767) * w / 4096;
    }
    let xj = clamp(p.x + jx, 0, w - 1);
    let xr = clamp(xj + shift + scan, 0, w - 1);
    let xb = clamp(xj - shift - scan, 0, w - 1);
    let c = textureLoad(T0, vec2<i32>(xj, p.y), 0);
    let cr = f32(textureLoad(T0, vec2<i32>(xr, p.y), 0).r);
    let cb = f32(textureLoad(T0, vec2<i32>(xb, p.y), 0).b);
    let bleed = P.f0.x;
    let r = u32(round(f32(c.r) * (1.0 - bleed) + cr * bleed));
    let b = u32(round(f32(c.b) * (1.0 - bleed) + cb * bleed));
    var g = c.g;
    if (scan == 1) {
        g = select(0u, g - 18u, g >= 18u); // saturating_sub(18)
    }
    // альфа берётся из НЕсдвинутого пикселя — как у CPU (он пишет in-place)
    let a = textureLoad(T0, p, 0).a;
    return vec4<u32>(r, g, b, a);
}
