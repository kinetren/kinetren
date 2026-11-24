## ADDED Requirements

### Requirement: An effect declares its precision; an 8-bit effect in a 16-bit chain is a marked island
An effect's manifest SHALL list the precisions its kernel has. In a 16-bit plan an effect with a float
kernel SHALL run in float. An effect without one SHALL run as an island:
1. its input passes through the display's SDR tone map to 8 bits;
2. its 8-bit kernel runs;
3. its output is expanded back to linear.

Adjacent islands SHALL share one conversion. Stream stages SHALL be islands. The chain SHALL mark an
island effect "8-bit". In this change Geometry, CamMotion, LayerMix, the crossfade, the title and the
colour family SHALL have float kernels.
Why: about fifty picture kernels are written for 8 bits; the owner's 16-bit path must not lose any effect,
and the person must see which ones limit HDR highlights (kkt-26.11.20 A25; hdr-sources-to-sdr D12).

#### Scenario: An island keeps the picture below white and marks the effect
- **WHEN** an SDR picture and an HDR picture with highlights pass an 8-bit-only effect set to identity in a 16-bit chain, then two such effects in a row
- **THEN** the SDR picture comes back within one 8-bit step of its input, the HDR highlights above white are kept where the effect left the chroma and limited at 1.0 otherwise, the two effects convert once, and the chain marks both "8-bit"
- **TEST** t3888, t3889 (`src/libs/kt-engine/tests/precision.rs`)

#### Scenario: The colour family in float
- **WHEN** colour grade, curves, levels and a LUT run in a 16-bit chain on a picture with values above 1.0
- **THEN** none is an island, values above 1.0 pass through each by its float rule, and an 8-bit run of the same chain equals today's bytes
- **TEST** t3890, t3891 (`src/plugins/fx-video-color/tests/precision.rs`)
