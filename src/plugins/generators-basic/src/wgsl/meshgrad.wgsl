// A moving mesh gradient (gen-backgrounds): three colour blobs drift on slow orbits and blend by soft
// weights. p0/p1/p2 = the colours (0..255), p3 = (phase = t·speed, softness, aspect w/h, 0).
fn blob(q: vec2<f32>, c: vec2<f32>, s: f32) -> f32 {
    let d = q - c;
    return exp(-dot(d, d) / (s * s));
}

fn pixel(x: u32, y: u32) -> u32 {
    let a = u.p3.z;
    let q = vec2<f32>((f32(x) + 0.5) / f32(u.w) * a, (f32(y) + 0.5) / f32(u.h));
    let t = u.p3.x * 6.2831853;
    let s = u.p3.y;
    let c1 = vec2<f32>(a * (0.30 + 0.22 * sin(t)), 0.35 + 0.25 * cos(t * 0.8));
    let c2 = vec2<f32>(a * (0.70 + 0.20 * cos(t * 0.7 + 1.7)), 0.40 + 0.28 * sin(t * 0.9 + 0.6));
    let c3 = vec2<f32>(a * (0.50 + 0.30 * sin(t * 0.5 + 3.1)), 0.75 + 0.18 * cos(t * 0.6 + 2.2));
    let w1 = blob(q, c1, s) + 1e-4;
    let w2 = blob(q, c2, s) + 1e-4;
    let w3 = blob(q, c3, s) + 1e-4;
    let c = (u.p0.xyz * w1 + u.p1.xyz * w2 + u.p2.xyz * w3) / (w1 + w2 + w3);
    return pack(vec4<f32>(c, 255.0));
}
