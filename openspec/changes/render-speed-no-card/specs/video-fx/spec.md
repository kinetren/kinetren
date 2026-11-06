## ADDED Requirements

### Requirement: A fused pass of one program shape runs as written out
A fused geometry pass with no Fold and no absorbed taps SHALL run a shader written for its program
shape, and give the interpreter's bytes.
Why: the interpreter's run-time loop and varying table cost a software rasterizer up to half the pass,
and a camera puts the pass on nearly every frame.

#### Scenario: A shape against the interpreter
- **WHEN** affine, supersampled, clamped, perspective and warp programs run both ways, with and
  without alpha
- **THEN** the frames are byte for byte equal
- **TEST** t1766 (`src/libs/kt-gpu/tests/parity.rs`)

### Requirement: The tape's convolving passes hold no loop
The VHS passes that convolve SHALL be written out tap by tap, once per standard, and keep their words in
one channel.
Why: a loop whose bound the compiler cannot see, or past its unroll limit, cost a software rasterizer
twice the pass.

#### Scenario: The passes
- **WHEN** the plugin's shaders are assembled
- **THEN** no convolving pass has a loop or a tap line left, and each standard has its own
- **TEST** t1764 (`src/plugins/fx-video-tape/src/lib.rs`)
