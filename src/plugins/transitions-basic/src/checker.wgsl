// checker: counter-moving waves in the cells
fn trans_checker(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let cols = max(p0.x, 1.0);
    let rows = max(p0.y, 1.0);
    let i = floor(uv.x * cols);
    let j = floor(uv.y * rows);
    let fx = fract(uv.x * cols);
    var s = fx;
    if (i32(i + j) % 2 != 0) { s = 1.0 - fx; }
    return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.z));
}
