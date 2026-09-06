// Зерно — counter-based hash (Р.29): та же целочисленная формула, что
// noise/noise_hash в kt-fx/src/video.rs → байт-в-байт с CPU на любом
// железе. u0 = (seed, k, size, color).
//
// size — размер зерна в пикселях, color — своё смещение на канал
// (Р.33 С5). ВАЖНО: при size == 1 и color == 0 считается ПРЕЖНЯЯ формула
// (без лишнего хэша зерна) — ровно так же ветвится CPU, и golden'ы обоих
// путей не двигаются.

fn nh(v: u32) -> u32 {
    var x = v;
    x = x ^ (x >> 16u);
    x = x * 0x7feb352du;
    x = x ^ (x >> 15u);
    x = x * 0x846ca68bu;
    x = x ^ (x >> 16u);
    return x;
}

fn off_of(h: u32, k: u32) -> i32 {
    let n = i32((h >> 16u) & 0x7fffu) - 16384;
    return (n * i32(k)) / 32767;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<u32>(floor(pos.xy));
    let c = textureLoad(T0, vec2<i32>(p), 0);
    let k = P.u0.y;
    let size = max(P.u0.z, 1u);
    let color = P.u0.w == 1u;
    if (size == 1u && !color) {
        // прежняя формула: seed идёт в хэш строки без обёртки
        let off = off_of(nh(p.x ^ nh(p.y ^ P.u0.x)), k);
        let rgb = clamp(vec3<i32>(c.rgb) + vec3<i32>(off), vec3<i32>(0), vec3<i32>(255));
        return vec4<u32>(vec3<u32>(rgb), c.a);
    }
    let g = p / size;
    if (!color) {
        let off = off_of(nh(g.x ^ nh(g.y ^ nh(P.u0.x))), k);
        let rgb = clamp(vec3<i32>(c.rgb) + vec3<i32>(off), vec3<i32>(0), vec3<i32>(255));
        return vec4<u32>(vec3<u32>(rgb), c.a);
    }
    var rgb = vec3<i32>(c.rgb);
    for (var ch = 0u; ch < 3u; ch = ch + 1u) {
        let off = off_of(nh(g.x ^ nh(g.y ^ nh((ch + 1u) ^ P.u0.x))), k);
        rgb[ch] = clamp(rgb[ch] + off, 0, 255);
    }
    return vec4<u32>(vec3<u32>(rgb), c.a);
}
