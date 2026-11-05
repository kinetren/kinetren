// vhs_tape stage 3: the DDS phase = the row's start phase + the prefix of the increments, as the
// blocks of stage 2 every 32 samples (wrapping u32 is associative: a running sum gives the
// same bits). T0 = stage 2; u0 = (W, frame).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    var p = hash3(P.u0.y, u32(y), 0xF00Du);
    for (var j = n; j >= 0; j = j - 32) {
        p = p + ld0(j, y);
    }
    return put(p);
}
