// vhs_tape stage 2 (`sim::s2`): the wrapping sum of the 32 increments ending at n. T0 = stage 1.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    var s = 0u;
    for (var i = 0; i < 32; i = i + 1) {
        let j = n - i;
        if (j >= 0) { s = s + ld0(j, y); }
    }
    return put(s);
}
