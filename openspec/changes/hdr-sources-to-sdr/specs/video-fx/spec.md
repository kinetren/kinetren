## ADDED Requirements

### Requirement: An 8-bit effect in a 16-bit chain is a marked island
In a 16-bit plan the geometry, the layer mix, the crossfade and the host's resampling SHALL run in float; a point-wise effect that declares it goes beyond white SHALL run its own text on encoded values with no round and no
clamp; a transition SHALL run its own text on encoded values. Any other effect SHALL run as an island:
1. its input passes through the display's SDR tone map to 8 bits;
2. its 8-bit kernel runs;
3. its output is expanded back to linear, the input's own value returning where the effect changed nothing.

Adjacent islands SHALL share one conversion. Stream stages SHALL be islands. The chain SHALL mark an
island effect "8-bit".
Why: about fifty picture kernels are written for 8 bits; the owner's 16-bit path must not lose any effect,
and the person must see which ones limit HDR highlights (kkt-26.11.20 A25; hdr-sources-to-sdr D12).

#### Scenario: An island keeps the picture below white and marks the effect
- **WHEN** an SDR picture and an HDR picture with highlights pass an 8-bit-only effect set to identity in a 16-bit chain, then two such effects in a row
- **THEN** the SDR picture comes back within one 8-bit step of its input, the HDR highlights above white are kept where the effect left the chroma and limited at 1.0 otherwise, the two effects convert once, and the chain marks both "8-bit"
- **TEST** t3888, t3889 (`src/libs/kt-engine/tests/precision.rs`)

#### Scenario: The colour family in float
- **WHEN** colour grade and vignette run in a 16-bit chain on a picture with values above 1.0, then Posterize after them, and a wipe joins two such events
- **THEN** the first two are no islands and values above 1.0 pass through them, Posterize is one, the wipe keeps both sides' highlights, and an 8-bit run of the same chains equals today's bytes
- **TEST** t3890, t3891 (`src/plugins/fx-video-color/tests/precision.rs`)
