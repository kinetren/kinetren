# Design

## Decisions

### D1. The seven chains
| Preset | Nodes |
|---|---|
| Telephone | two `filter` nodes, high-pass 300 Hz and low-pass 3400 Hz, `distortion` low, `compressor` |
| Megaphone | two `filter` nodes, 500 and 2500 Hz, `distortion` high, `eq` +6 dB at 1.5 kHz, `delay` one 40 ms repeat at low mix |
| Radio | two `filter` nodes, 100 and 5000 Hz, `bitcrusher` 12 bit, `noise` none (no node adds hiss: the preset says so) |
| Chipmunk | `pitch_shift` +7 semitones |
| Deep voice | `pitch_shift` −5 semitones, `eq` −2 dB at 3 kHz |
| Small hall | `delay` short at low mix, `eq` low shelf |
| Robot | `ring_mod` 60 Hz mix 0.8, `pitch_shift` 0 (bypassed), `compressor` |
The exact values are heard on our fixture voice, not chosen here: the preset file's numbers are tasks 1.2's, the chains
are fixed. Each chain is one undo step, applies to any audio event or track by the existing picker and the rule that a
chain preset sets everything. They stand among the audio presets with the keywords "phone, radio, robot" in five languages.

### D2. `ring_mod`
`out = in · ((1−mix) + mix · sin(2π·φ(n)))`, where φ is the prefix integral of the carrier curve by `LfoDrive` (the LFO
rule: a function of the clip position, not a counter). Rows `hz` 0–4000 (smooth, curvable), `mix` 0–1. State none but
the bound curve; `fork`, `reset` as tremolo; `lookahead` 0; tail 0. Determinism: the phase is f64, the sine is the host's
frozen `FrozenMath` sine (invariant 11). Identity at mix 0 byte for byte (a branch, not a multiply).

### D3. Class walk
The audio picker shows factory chain presets only as video ones do today (`fx-preset-change`, `presets-everywhere` own
the general surface); this adds rows to the file. Track chains, compositions and the render get them by
the same file. The existing audio presets ("Voice up front", "Wide dub echo") keep their names.
