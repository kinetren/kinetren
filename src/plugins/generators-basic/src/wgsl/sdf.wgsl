// Signed distances in pixels shared by the shapes of gen-shapes (`v26.10` G5); negative inside.
fn sd_box(q: vec2<f32>, c: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let at = abs(q - c) - (half - r);
    var d = max(at.x, at.y) - r;
    if at.x > 0.0 && at.y > 0.0 { d = length(at) - r; }
    return d;
}

fn sd_segment(q: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let v = b - a;
    let len2 = dot(v, v);
    var t = 0.0;
    if len2 > 0.0 { t = clamp(dot(q - a, v) / len2, 0.0, 1.0); }
    return length(q - a - v * t);
}

fn rot(q: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(q.x * c - q.y * s, q.x * s + q.y * c);
}

// the coverage of a distance under a feather, 0..1
fn cover(d: f32, fe: f32) -> f32 {
    return clamp(0.5 - d / fe, 0.0, 1.0);
}

// `top` over `bottom`, straight colours and alphas 0..1 → a straight RGBA8 word
fn over(top: vec4<f32>, bottom: vec4<f32>) -> u32 {
    let a = top.w + bottom.w * (1.0 - top.w);
    if a <= 0.0 { return 0u; }
    let c = (top.xyz * top.w + bottom.xyz * bottom.w * (1.0 - top.w)) / a;
    return pack(vec4<f32>(c, floor(a * 255.0)));
}
