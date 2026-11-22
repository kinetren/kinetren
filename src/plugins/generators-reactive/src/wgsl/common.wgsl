// Shared by the audio-reactive kinds (gen-audio-reactive): `au` is `audio_series::at`'s layout —
// 0..3 levels (RMS L, RMS R, peak, held), 4..19 bands, 20..35 bands held, 36..99 waveform −1..1.
fn cover(d: f32, fe: f32) -> f32 {
    return clamp(0.5 - d / fe, 0.0, 1.0);
}
fn color(c: vec4<f32>, a: f32) -> u32 {
    return pack(vec4<f32>(c.xyz, floor(c.w * clamp(a, 0.0, 1.0))));
}
// the waveform at a fraction 0..1 of the frame's width, between its 64 points
fn wave_at(f: f32) -> f32 {
    let x = clamp(f, 0.0, 1.0) * 63.0;
    let i = u32(floor(x));
    let j = min(i + 1u, 63u);
    return mix(au[36u + i], au[36u + j], x - floor(x));
}
