// One tile, copied (morpher-event-grid-cost, D2): the replicate half of the tile path, a
// mirror of `kt_fx::geom::exec_geom_cpu` when `GeomProgram::tile` is set. T0 is the cell at the
// grid's origin, computed by the fused pass (`GeomProgram::tile_program`, cw×ch). Integer
// `textureLoad`, no filtering — every pixel inside the grid IS a texel of the tile. Outside the
// grid: opaque black, what the Fold gives there (`Mapped::Black`).
// u0 = (offx, offy, cw, ch), u1 = (cols, rows, 0, 0).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let off = vec2<i32>(i32(P.u0.x), i32(P.u0.y));
    let cell = vec2<i32>(i32(P.u0.z), i32(P.u0.w));
    let grid = cell * vec2<i32>(i32(P.u1.x), i32(P.u1.y));
    let d = p - off;
    if (d.x < 0 || d.y < 0 || d.x >= grid.x || d.y >= grid.y) {
        return vec4<u32>(0u, 0u, 0u, 255u);
    }
    return textureLoad(T0, d % cell, 0);
}
