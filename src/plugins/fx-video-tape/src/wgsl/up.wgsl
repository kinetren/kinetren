// vhs_tape, the parameter upload (`deck::words`): pass b renders 16·(b+1) texels, copies the
// previous blocks from T0 (a 1×1 stub before the first) and writes its 16 words — u0..u1 are
// words 0..7 of the block (the integers), f0..f1 the f32 words 8..15 by their bits.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let x = i32(floor(pos.x));
    let dw = i32(textureDimensions(T0).x);
    var b = dw / 16;
    if (dw < 16) { b = 0; }
    if (x < b * 16) {
        return textureLoad(T0, vec2<i32>(x, 0), 0);
    }
    let i = x - b * 16;
    var v = 0u;
    if (i < 4) {
        v = P.u0[i];
    } else if (i < 8) {
        v = P.u1[i - 4];
    } else if (i < 12) {
        v = bitcast<u32>(P.f0[i - 8]);
    } else {
        v = bitcast<u32>(P.f1[i - 12]);
    }
    return put(v);
}
