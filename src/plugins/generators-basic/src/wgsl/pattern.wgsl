// Moving patterns (gen-backgrounds). a = the pattern (0 stripes, 1 dots, 2 grid, 3 waves);
// p0 = colour A, p1 = colour B (0..255); p2 = (phase = t·speed, cells across the width, angle in
// radians, fill ratio 0..1); p3.x = aspect w/h, p3.y = the edge's softness in cells.
fn pixel(x: u32, y: u32) -> u32 {
    let a = u.p3.x;
    let n = vec2<f32>((f32(x) + 0.5) / f32(u.w) * a, (f32(y) + 0.5) / f32(u.h)) * u.p2.y / a;
    let ca = cos(u.p2.z);
    let sa = sin(u.p2.z);
    let r = vec2<f32>(n.x * ca - n.y * sa, n.x * sa + n.y * ca) + vec2<f32>(u.p2.x, 0.0);
    let f = fract(r);
    let ratio = clamp(u.p2.w, 0.02, 0.98);
    let soft = max(u.p3.y, 0.001);
    var d: f32;
    if u.a == 0u {
        d = abs(f.x - 0.5) - ratio * 0.5;
    } else if u.a == 1u {
        d = length(f - vec2<f32>(0.5)) - ratio * 0.5;
    } else if u.a == 2u {
        d = min(abs(f.x - 0.5), abs(f.y - 0.5)) - ratio * 0.15;
    } else {
        let wv = 0.5 + 0.25 * sin(r.x * 6.2831853);
        d = abs(fract(r.y) - wv) - ratio * 0.25;
    }
    let m = clamp(0.5 - d / soft, 0.0, 1.0);
    return pack(vec4<f32>(mix(u.p1.xyz, u.p0.xyz, m), 255.0));
}
