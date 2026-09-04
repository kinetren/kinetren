// Pan 3D (Р.29): слой — текстурированный квад в 3D, рендерится камерой в
// размер Quality. MVP собрана на CPU (glam, см. pan3d_mvp в engine.rs);
// мир — px проекта × world_scale, X вправо, Y вниз, Z вглубь. Цель
// чистится в прозрачное — вне квада слой прозрачен (has_alpha = true).
// f0 = (quad_w, quad_h, anchor_x, anchor_y); u0 = (sw, sh, 0, 0).

struct V3 {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs3d(@builtin(vertex_index) vi: u32) -> V3 {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let q = corners[vi];
    // локаль слоя: якорь в начале координат, размеры в px
    let local = vec4<f32>((q.x - P.f0.z) * P.f0.x, (q.y - P.f0.w) * P.f0.y, 0.0, 1.0);
    var o: V3;
    o.pos = P.m * local;
    o.uv = q;
    return o;
}

@fragment
fn fs(v: V3) -> @location(0) vec4<u32> {
    // ручная билинейка источника по uv (перспективно-корректная
    // интерполяция uv — растеризатором)
    let sw = i32(P.u0.x);
    let sh = i32(P.u0.y);
    let sx = v.uv.x * f32(sw) - 0.5;
    let sy = v.uv.y * f32(sh) - 0.5;
    let xf = floor(sx);
    let yf = floor(sy);
    let fx = sx - xf;
    let fy = sy - yf;
    let x = i32(xf);
    let y = i32(yf);
    var acc = vec4<f32>(0.0);
    for (var dy = 0; dy < 2; dy = dy + 1) {
        let wy = select(fy, 1.0 - fy, dy == 0);
        let yi = clamp(y + dy, 0, sh - 1);
        for (var dx = 0; dx < 2; dx = dx + 1) {
            let wx = select(fx, 1.0 - fx, dx == 0);
            let xi = clamp(x + dx, 0, sw - 1);
            acc = acc + vec4<f32>(textureLoad(T0, vec2<i32>(xi, yi), 0)) * (wy * wx);
        }
    }
    return vec4<u32>(clamp(round(acc), vec4<f32>(0.0), vec4<f32>(255.0)));
}
