# Spec Delta

## ADDED Requirements

### Requirement: Track in the file — track envelope, `enabled`, track effects, counter migration
`Track.metadata.kt` SHALL carry the id, switches, level, blend mode and track height (defaults are not written); `Track.enabled` SHALL equal `!mute` and be read back as mute (a foreign disabled track becomes muted, not enabled); the track chain SHALL be written as standard `Effect.1` in `Track.effects` by the same code as for a clip; the name — in `Track.name`, default labels are still not written; there is no separate field for the row order — the order of the tracks in the file. Old files with counters, `lane_order` and `ui.lane_names/lane_heights` SHALL be migrated on reading into tracks, one-way. The v11 predicate of round R.62 SHALL treat any non-default property of a track as a novelty; a file without novelties is byte-stable on the former version.
Why: the neighbours have track effects and `enabled`, and the round trip was losing them (`TrackExtra`); the notice "Kinotrain has no per-track mute" is removed in substance.

#### Scenario: Track round trip, foreign `enabled`, migration of an old file
- **WHEN** a project with mute/solo/lock/level/mode/chain of a track is written and read; a foreign file with `enabled: false` on a track is read and re-saved; an old file with `lane_order` and `ui.lane_names` is read
- **THEN** the round trip is idempotent and v11; the foreign track became mute and is written with `enabled: false`; the old file gave tracks with the same names, heights and order; a file without novelties is byte-stable (t433b)
- **TEST** t731 (`src/libs/kt-otio/tests/convert.rs`), t433b (`src/libs/kt-otio/tests/convert.rs`)
