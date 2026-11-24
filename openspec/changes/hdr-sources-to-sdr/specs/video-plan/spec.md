## ADDED Requirements

### Requirement: A project and a composition choose their picture precision
A project SHALL have a picture precision, 8-bit (the default) or 16-bit float. A composition SHALL either
take its own precision or inherit the project's. A plan SHALL be built for one precision. Its layer
textures, intermediates, memos and readbacks SHALL be `Rgba8Uint` or `Rgba16Float` accordingly, and
`tex_bytes` and every memory ledger tenant SHALL count by the format.

In a 16-bit plan:
- the core passes (geometry, resample, layer mix, the crossfade, masks, premultiply, the composite) SHALL
  run in float;
- the 8-bit shaders and their bytes SHALL be unchanged;
- the display SHALL present HDR on an HDR monitor wherever the system gives an HDR surface (an extended
  linear or HDR10 colour space): the linear picture without a tone map, rolled off above the monitor's
  peak. Elsewhere it SHALL tone-map by BT.2390 to the monitor's SDR white, and the display's facts SHALL
  name which.

**Across precisions:** a 16-bit composition inside an 8-bit parent SHALL be tone-mapped at its boundary,
and an 8-bit one inside a 16-bit parent SHALL be expanded to linear.

**The prerender** of a 16-bit plan SHALL store 10-bit HEVC PQ segments where a 10-bit hardware encoder
opens, and nothing elsewhere, saying why.

The precision SHALL be part of the plan key and of every durable key. A 16-bit frame SHALL be byte-equal
between runs on one device and within the float tolerance classes of `FrameTol` across devices.
Why: kkt-26.11.20b line 10 ("HDR на HDR-монитор сразу"); kkt-26.11.20 line 33 ("нужно от настроек проекта-композиции определять, если включено то тракт f16
чтобы было нормально, если нет - то быстрый i8"); hdr-sources-to-sdr D8, D11, D13, D15.

#### Scenario: The setting, its undo and its key
- **WHEN** a project is switched to 16-bit, a composition set to 8-bit inside it, then undone
- **THEN** the plans' keys and textures follow each precision, undo returns 8-bit with the former keys, and the memory ledger counts 8 bytes a pixel for the 16-bit textures
- **TEST** t3877 (`src/libs/kt-engine/tests/precision.rs`), t3884 (`src/libs/kt-gpu/tests/precision.rs`)

#### Scenario: The core passes in float match within tolerance across devices
- **WHEN** a project of two layers with geometry, a mask, a crossfade and a mix is composed in 16-bit on the software adapter and on a card, twice each
- **THEN** each device's two runs are byte-equal, the two devices agree within the float classes of `FrameTol`, and the 8-bit golden frames do not move
- **TEST** t3885, t3886, t3887 (`src/libs/kt-gpu/tests/precision.rs`)

#### Scenario: The display shows HDR where it can and rolls highlights off where it cannot
- **WHEN** a 16-bit composite holding 1.0, 4.93 and 20.0 is displayed on a test scRGB surface with a 600 cd/m² peak, and on an SDR surface, and the surface choice is resolved for scRGB, EDR, HDR10, GL and a surface with no HDR colour space
- **THEN** on scRGB 1.0 is written as 203/80, 4.93 as its nits over 80, and 20.0 rolled off below 600 cd/m²; on SDR 1.0 is the SDR white and the highlights are below the peak and ordered; the first three resolve to HDR and the last two to SDR, each named in the facts
- **TEST** t3892 (`src/libs/kt-gpu/tests/precision.rs`)

#### Scenario: Compositions of two precisions meet at a boundary
- **WHEN** a 16-bit reel lies in an 8-bit project and an 8-bit reel in a 16-bit project
- **THEN** the first reaches its parent tone-mapped, the second expanded to linear, and each reel's own frames equal its standalone render
- **TEST** t3896 (`src/libs/kt-engine/tests/precision.rs`)

#### Scenario: The prerender of a 16-bit project
- **WHEN** a heavy 16-bit moment is prerendered with a forced 10-bit HEVC encoder, and with only an 8-bit encoder available
- **THEN** the first stores a Main10 PQ segment whose restored picture is within the codec's tolerance of the composite; the second writes nothing and names why
- **TEST** t3897 (`src/libs/kt-engine/tests/render_cache.rs`)
