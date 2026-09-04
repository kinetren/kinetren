// Тиражирование ячейки мозаики — зеркало event_grid (kt-fx/src/video.rs):
// чёрный непрозрачный холст, ячейка T0 (cw×ch) повторяется по сетке
// grid_w×grid_h со смещением (offx, offy).
// u0 = (cw, ch, offx, offy); u1 = (grid_w, grid_h, 0, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let d = p - vec2<i32>(i32(P.u0.z), i32(P.u0.w));
    if (d.x < 0 || d.y < 0 || d.x >= i32(P.u1.x) || d.y >= i32(P.u1.y)) {
        return vec4<u32>(0u, 0u, 0u, 255u);
    }
    let cell = vec2<i32>(i32(P.u0.x), i32(P.u0.y));
    return textureLoad(T0, d % cell, 0);
}
