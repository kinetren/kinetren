## ADDED Requirements

### Requirement: A warm audio render allocates nothing
Once its rings, pools and the sub-mix rings have reached their size, the audio render of a burst SHALL
allocate nothing: an event's chain output, the earlier laps' sum, a track stage's sum and key signals
and the active-event lists come from a pool the render thread's `FxStateStore` keeps; the reel maps are
made once per build; a sub-mix burst the ring let go is filled again; the warp map's reading at a seam
is memoized. The pool holds no state between calls (each buffer is cleared when taken), so the bytes
do not move.
Why: found-defects row 352 and what t2684 found beside it (per-sample-costs).

#### Scenario: Four warm seconds without an allocation
- **WHEN** a project with a warped event under a moving speed and a strength, a plain event, a track stage keyed by another track and a nested reel renders 4 s of bursts after warming, linearly and around a loop region, the source bursts already cached
- **THEN** the render thread allocates nothing in either
- **TEST** t2684 (`src/libs/kt-engine/tests/mixdown_alloc.rs`)
