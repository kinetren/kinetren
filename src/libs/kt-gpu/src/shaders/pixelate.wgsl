// Pixelate (R.47): an integer copy of the cell centre, a mirror of
// kt_fx::video::pixelate byte for byte (EXACT). u0 = (w, h, cell).
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let cell = i32(P.u0.z);
    let sx = min(p.x / cell * cell + cell / 2, i32(P.u0.x) - 1);
    let sy = min(p.y / cell * cell + cell / 2, i32(P.u0.y) - 1);
    return textureLoad(T0, vec2<i32>(sx, sy), 0);
}
