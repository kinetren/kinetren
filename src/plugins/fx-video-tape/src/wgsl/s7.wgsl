// vhs_tape stage 7 (`sim::s7`), the second half: the playback LPF ∗ de-emphasis. T0 = the level
// with the dropouts compensated (s7a).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let n = i32(floor(pos.x));
    let y = i32(floor(pos.y));
    var acc = 0.0;
    // @TAPS PLAY acc = acc + {c} * ld0f(n + K_PLAY_C - {i}, y);
    return putf(acc);
}
