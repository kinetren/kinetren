## ADDED Requirements

### Requirement: An edit derives what it touched
The player's builder SHALL keep each event's derived audio state (the clip's curves in the source
domain, its tail, its address map, its context flag) and video layer from one build to the next, keyed
by a hash of everything it is derived from: the model event's text (the input of its fingerprint) and
its reel context (the source address, the track's level, the instance; for a layer the source facts,
the cameras, the raster and the frame rate). A rebuild SHALL derive again only the events whose key
changed; the memo SHALL hold the last build's events only.
Why: found-defects row 110 — every edit, and every step of a slider drag, rebuilt the derived state of
every event of the project.

#### Scenario: Slider steps on one event of 2000
- **WHEN** a project of 1000 A/V pairs (reverbs, shaped speed curves, crossfades, a reel placed twice, a morpher layer) is built, built again unchanged, then one event's gain changes five times and one layer's opacity once; and the same five gain steps go through a player
- **THEN** the first build derives every event, the unchanged one none, each step one, and each of the player's builds reports one
- **TEST** t2003 (`src/libs/kt-engine/tests/shared_build.rs`)

### Requirement: An incremental build is a fresh build
A build that reuses the last build's events SHALL equal a fresh build of the same project: every derived
audio event and video layer, the index, the stages, the nested reels, the scene hash of every burst and
the sound, byte for byte (invariant 11). The kept final mix and the FX states keep their keys: the
fingerprint `fp` of an event is unchanged by the memo.
Why: a memo whose key misses an input replays a stale event; the comparison is the only proof that
the key is whole.

#### Scenario: A sequence of edits and an undo
- **WHEN** a gain, a curve key, a trim, an event inside the reel, a removed transition, a muted track, an opacity and an undo to the start are applied in turn
- **THEN** after each the incremental build's state text equals the fresh build's, and their sound at three places and their scene hashes are equal
- **TEST** t2004 (`src/libs/kt-engine/tests/shared_build.rs`)
