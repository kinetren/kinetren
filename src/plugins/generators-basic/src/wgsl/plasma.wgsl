// Plasma (gen-backgrounds): four travelling sine fields summed, mapped through a palette of three
// colours (p0, p1, p2, 0..255). p3 = (phase = t·speed, frequency, aspect w/h, 0).
fn pixel(x: u32, y: u32) -> u32 {
    let a = u.p3.z;
    let q = vec2<f32>((f32(x) + 0.5) / f32(u.w) * a, (f32(y) + 0.5) / f32(u.h)) * u.p3.y;
    let t = u.p3.x * 6.2831853;
    var v = sin(q.x + t);
    v += sin(q.y * 1.3 - t * 1.1);
    v += sin((q.x + q.y) * 0.7 + t * 0.7);
    let c = q - vec2<f32>(0.5 * a, 0.5) * u.p3.y + vec2<f32>(sin(t * 0.3), cos(t * 0.4));
    v += sin(length(c) * 1.5 - t);
    // v in [-4, 4] → a smooth walk round the palette: p0 → p1 → p2 → p0
    let k = fract(v * 0.125 + 0.5) * 3.0;
    var col: vec3<f32>;
    if k < 1.0 {
        col = mix(u.p0.xyz, u.p1.xyz, smoothstep(0.0, 1.0, k));
    } else if k < 2.0 {
        col = mix(u.p1.xyz, u.p2.xyz, smoothstep(0.0, 1.0, k - 1.0));
    } else {
        col = mix(u.p2.xyz, u.p0.xyz, smoothstep(0.0, 1.0, k - 2.0));
    }
    return pack(vec4<f32>(col, 255.0));
}
