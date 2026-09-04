// Зерно — counter-based hash (Р.29): та же целочисленная формула, что
// noise/noise_hash в kt-fx/src/video.rs → байт-в-байт с CPU на любом
// железе. u0 = (seed, k, 0, 0).

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
    let p = vec2<u32>(floor(pos.xy));
    let c = textureLoad(T0, vec2<i32>(p), 0);
    let h = nh(p.x ^ nh(p.y ^ P.u0.x));
    let n = i32((h >> 16u) & 0x7fffu) - 16384;
    let off = (n * i32(P.u0.y)) / 32767;
    let rgb = clamp(vec3<i32>(c.rgb) + vec3<i32>(off), vec3<i32>(0), vec3<i32>(255));
    return vec4<u32>(vec3<u32>(rgb), c.a);
}
