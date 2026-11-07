## ADDED Requirements

### Requirement: A placement's sub-mix state exists only where it sounds
The sub-mix stream of a nested reel's instance (its inner chains' FX state, `NestStream`) SHALL be
created when the instance is first read and synced then; `sync_fx` SHALL only carry over the
streams that exist (invariant 3). A stream whose instance lies more than a second from the rendered
range SHALL be released. The bytes do not change: an instance can be read again only after a jump,
and a jump is the canonical move, which resets the stream anyway.
Why: `sync_fx` built every inner processor of every placement at once. A test scene of a reel with
300 reverbs placed 400 times (cached-range-plays, 2026-11-07) grew the player by 1.3 GB a second on a
pause and took a 61 GB machine down; the memory a project needs must follow what plays, not how
many times a reel is placed.

#### Scenario: Many placements, one heard
- **WHEN** a reel with a reverb stands in 21 places and the render plays one of them, then a place far from all
- **THEN** after `sync_fx` no stream exists, the heard placement has the only one and sounds as the reel always did, and far from them none is held
- **TEST** t1937 (`src/libs/kt-engine/tests/nest_render.rs`)
