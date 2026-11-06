// vhs_tape stage 7 (`sim::s7`), the first half: the dropout compensator. The level at a sample, or
// the line before in the field's where the head lost the track. A pass of its own, so the FIR after
// it reads one word per tap, not this branch per tap. T0 = stage 6 (level | envelope). u0.z = doc,
// f0.x = threshold.

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    let t = ld0(n, y);
    if (P.u0.z != 0u && y >= 2 && hi16(t, 0.0, 16384.0) < P.f0.x) {
        return putf(lo16(ld0(n, y - 2), 2.0, 8192.0));
    }
    return putf(lo16(t, 2.0, 8192.0));
}
