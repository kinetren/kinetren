## ADDED Requirements

### Requirement: The Pro Keyer keys against a clean plate with core and edge mattes
The Pro Keyer SHALL work in linear light; it SHALL take a clean plate generated from the
frame (the subject erased, the screen filled by a pyramid from the screen around it) or a held frame of its
clip, compute each pixel's screen-ness against the plate's colour at that pixel, and combine a hard core
matte (clipped, choked, holes filled) with a soft edge matte inside a band around the core's boundary.
Why: the owner's B4 — "учитывающий волосы, учитывающий неравномерность фона".

#### Scenario: An uneven screen with hair
- **WHEN** a synthetic subject with 1-px hair strands is keyed over a green screen whose brightness falls by 40 % from centre to corner, with a generated plate and with a held subject-free frame
- **THEN** the alpha is within 3 % of the known alpha over the body, the screen corners and the centre alike; the hair strands keep at least 80 % of their known coverage; the simple difference key at its best clip leaves either the corners or the hair wrong by more than 10 %
- **TEST** t3585 (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: Shadows are kept as a ratio, spill and bounce light are removed with the subject's colour restored
Over the screen, a darkening by the ratio of the pixel's luma to the plate's SHALL be kept as a shadow layer
with its own amount; on the subject, the plate colour's excess SHALL be removed, the luminance restored, and
the removed light replaced by neutral, a chosen colour or the hue of the pixel's non-spill neighbourhood.
Why: the owner's B4 — "учитывающий тени … засветы … зелень на нужных объектах".

#### Scenario: A cast shadow and a green-lit shoulder
- **WHEN** a subject casts a shadow of 50 % on the screen and a white shirt's shoulder carries green bounce light; keyed over a red background
- **THEN** the shadow appears over the red as a 50 % darkening within 3 %; the shoulder's colour after spill removal with "subject hue" is within 4 code values of the unlit shirt and its luma within one code value of the original
- **TEST** t3586 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: Preview and export agree, the views stay in the preview
- **WHEN** the keyer is composed in the preview, with the Viewer on its Core view, and by an export
- **THEN** the preview's composite and the export are byte-equal on the device; the export carries no view; the stored reference frames hold
- **TEST** t3587 (`src/libs/kt-gpu/tests/gpu_render.rs`)
