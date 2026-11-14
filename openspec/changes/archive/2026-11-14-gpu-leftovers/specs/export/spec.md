## ADDED Requirements

### Requirement: An export composes an incomplete frame again before it writes it
Every export (serial and pipelined) SHALL ask a frame its decode span did not give alone, up to two times,
before anything composes it; a frame that still comes out incomplete SHALL have its missing frames decoded
alone again (serial) and SHALL be composed again (both, and the readback ring's collected frames), up to two
times, before it is written. Only what the input still does not give after that is written without its layer
and reported (the requirement "A frame written without a layer is reported where it happens"). An export's
frame SHALL NOT be memoized, and its point-wise runs take the settled form (video-plan).
Why: found-defects 522 — catch-up-shows-loading D5 made a blend pair lacking a side an absent layer and said
"the export retries"; the serial export asked a missing frame once (its pre-roll) and the pipelined one once
(alone after the span), then wrote the frame without the layer. The serial export also composed through the
memo path.

#### Scenario: A read that failed is not a lost layer
- **WHEN** a source whose every frame fails its first two reads (in a span or alone) is exported serially and pipelined
- **THEN** both report no missing layer and no incomplete run, and the two files' video is the same
- **TEST** t2518 (`src/libs/kt-engine/tests/export_options.rs`)
