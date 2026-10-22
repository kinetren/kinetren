# Spec Delta

## MODIFIED Requirements

### Requirement: Video at signed speed — reverse motion, freeze frame, source edge, order == read
Video SHALL run through the material backwards at negative speed and turn around at zero; zero SHALL be a freeze frame (one key in the order for the whole event); at the start of the file the frame SHALL hold, not go negative; the blend pair SHALL be taken from the same side as at positive speed; the prefetch order window SHALL match the render's reading, and under a sign change.
The END of the material is the same edge as the start, and it is the same for BOTH paths: an event consuming more material than there is in the file (stretch, trim) SHALL hold the last frame past the end of the file and play silence, and this MUST NOT count as an underrun either on the live or the offline path — an underrun is a miss of what exists. The tail of an event without material SHALL be visible on the event itself (hatching from the point where the material ended, taking speed into account) and named in `kt-cli probe` ("material ends at … s, then silence/freeze frame"). The right-edge trim of an event SHALL stop at the end of material of known length, taking speed into account (as the left edge stops at its start); already existing events longer than the material MUST NOT be shortened by themselves (rule 20) — they are shown and played as stated above; the limit for them is not less than their current end (they may be trimmed down, but not grown further; the edge does not jump back).
Why: a window over two edges would miss the excursion, and a cache miss would become permanent. R.63: the events of Rolling Stones in paintitblack at rate 0.75 consume 237.7 s from a file of 226.3 s — the last 11 s sounded as silence and counted as underruns (180 480 frames over 5 s), while on screen the tail looked no different from the material; by law P the engine does not know the media length, so the edge is reported by the source (the fetcher), not by the plan.

#### Scenario: Backward motion and a freeze frame
- **WHEN** the curve goes 1 → −1; the curve is exactly 0
- **THEN** ∫rate over the event equals zero (it returned); the second half strictly decreases; at zero all frames are the entry-point frame, and the order is one key
- **TEST** t604a, t604b (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The edge of the material and the blend pair
- **WHEN** speed −1 from the entry point at 1 s; a fractional position below minus
- **THEN** the frame does not go past the start of the file and holds at zero; the pair is (a, a+STEP) with weight ½
- **TEST** t604c, t604d (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The order equals the read under a sign change, reverse on top of a minus goes forward
- **WHEN** a curve with an excursion (a maximum inside the window); the "Reverse" flag on a constant −0.5
- **THEN** the prefetch order ⊇ the render's reading, and the scene is not degenerate (the excursion guard); the mirror leads from the lower edge of the span to the entry point, and the motion is monotonic
- **TEST** t604e, t604g (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The end of the material — silence and a freeze frame without underruns, the tail is named
- **WHEN** an event with a slow-down consumes more material than a synthetic source of known length lasts; it is played offline and through the player, with the region loop sitting on its tail; `kt-cli probe` is called on the tail
- **THEN** past the end of the material the audio is silence and the video is the last frame; `underruns == 0` and `warmup_misses == 0` on both paths (on the old code — hundreds of thousands of frames); the prefetch order ⊇ the reading and does not grow past the edge; probe prints the moment where the material ended, and "then silence/freeze frame"
- **TEST** t748 (`src/libs/kt-engine/tests/material_edge.rs`), t748b (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: The trim stops at the end of the material, the existing is not shortened
- **WHEN** for an event with known media length the right edge is dragged past the end of the material at speed 1 and at 0.5; a project opens where the event is already longer than the material
- **THEN** the trim stops at the end of the material, taking speed into account, and reports it with an error (rule 16); with unknown length (generator, reel, file without duration) there is no stop; the existing event passes validation, does not change its length, and is trimmed freely but does not grow further; its tail is drawn with hatching
- **TEST** t748c (`src/libs/kt-model/tests/trim_material.rs`), t748d (`src/apps/kt-app/src/layout_tests.rs`)
