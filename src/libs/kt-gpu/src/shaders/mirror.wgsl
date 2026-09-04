// Зеркало — чистая перестановка индексов, байт-в-байт с CPU (mirror в
// kt-fx/src/video.rs). u0 = (w, h, horizontal, vertical).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let w = i32(P.u0.x);
    let h = i32(P.u0.y);
    var sx = p.x;
    var sy = p.y;
    if (P.u0.z == 1u) { sx = w - 1 - p.x; }
    if (P.u0.w == 1u) { sy = h - 1 - p.y; }
    return textureLoad(T0, vec2<i32>(sx, sy), 0);
}
