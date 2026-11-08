## ADDED Requirements

### Requirement: A reel's placements cost its revision once per build
The revision of a reel (`content_hash`) SHALL be computed once per reel within a build
(`kt_engine::revisions_scope`, which the audio and video builds and `gen_sources` open), and a
nested reel SHALL enter its parent's revision by its own revision, computed once per reel, not by
its text fed again per placement. A reel without nested reels keeps its former revision bit for
bit; a reel holding reels gets a new one once (its sidecar previews are rendered again once).
Why: every placement hashed the Debug text of its whole reel — twice in the audio build, four times
in the video one and once in the timeline's preview keys — and every level of nesting multiplied
it: placements × reel events per edit.

#### Scenario: Placements of a growing reel
- **WHEN** a reel of n A/V pairs is placed n times, n = 8, 32, 128
- **THEN** the builds grow at most 8× and 32× (the former code: ~16× and ~256× in reel events hashed); `effective_clips`, which flattens every placement, is held to its output's size
- **TEST** t2064 (`src/libs/kt-engine/tests/build_cost.rs`)

#### Scenario: A reel of placements of a reel
- **WHEN** a reel holds n placements of a reel of n pairs and is placed once
- **THEN** the builds grow at most 8× and 32×
- **TEST** t2065 (`src/libs/kt-engine/tests/build_cost.rs`)
