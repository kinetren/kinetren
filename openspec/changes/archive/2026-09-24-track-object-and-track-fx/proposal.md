# Proposal

## Why

There is no track as an entity: there are counters `video_lanes/audio_lanes/morph_lanes/camera_lanes`,
a vector of the row order and a field `lane: u32` on the event; the name and height live outside undo. Audio
is summed by clips straight into the output, video is stacked per clip by alpha-over, and the morpher's input is per clip.
Therefore a track has no mute, no solo, no lock, no level, no FX chain, no
blend mode — the owner's third claim in R.62 ("I would like to mute tracks, apply
effects, etc., so that tracks have their own blending capabilities"). The key "(kind, number)" for
FX state is unsuitable: inserting a track renumbers its neighbours and would kill the state
of the chains (invariant 3) — a track needs its own stable id.

## What Changes

- **A track is a model object with a stable id** (`Track { id, kind, name, height, mute,
  solo, lock, level, blend, fx… }`) at the root and in each reel; the row order = the order
  of the vector of tracks; the track number of an event is still the rank inside the kind (the invariant "number =
  row rank" does not change). **BREAKING (model):** the track counters, `lane_order`,
  `UiState.lane_names/lane_heights` are removed; the name and height go into undo.
- **Mute and solo are project content and affect the export** (the owner's decision). Mute —
  a multiplier 0 / exclusion from the composite, not removal from the graph (invariant 3); solo —
  a set by kind; the render window and kt-cli list the muted and soloed tracks before the start.
- **Lock**: edits of the events of a locked track are rejected by one model gate with a reason;
  the track's own properties remain editable.
- **Track level**: volume in dB (audio) / opacity (video) — one more multiplier
  after the weights and the event gain; static, in the property passport (envelopes — the next change).
- **Track FX chain.** Video: a track with a chain, a level ≠ 1, or a mode ≠ Normal
  becomes in the plan an implicit reel — the composite of its events, then the chain, level, blending
  downward; a track without this goes the former way, byte for byte. Audio: the sum of the track's events →
  the streaming chain processor (the state key is the track id) → level → mute → into the mix;
  a track without a stage is summed in the former order (GOLDEN_AUDIO does not move).
- **Video track blend mode** (Normal, Multiply, Screen, Add, Subtract, Darken,
  Lighten, Overlay, Difference) on the track composite; the "mode" and "level" axes are separate, both
  after alpha; a pair of CPU + WGSL by tolerance classes (note F-CMP-003).
- **Interface**: the track header gets the badges M / S / lock / FX, a "Track properties" window (name, level, mode, switches, chain button), the FX editor addresses a track
  with the same chain widget; the menu and hotkeys; the full set of verbs is checked by a machine.
- **Format**: `Track.metadata.kt` (id, switches, level, mode, height), `Track.enabled
  = !mute`, the track chain — standard `Effect.1` on the track; the v11 predicate of round R.62
  is extended.
- **Deferred, with a note**: track envelopes over the timeline's time; buses/sends; a clip
  blend mode.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-timeline`: the track as an object of rows (name/height in undo), a header with badges, a track
  properties window, the FX editor by track, the lock as a refusal to gestures.
- `ui-reach`: the track has the full set of verbs (rename, delete, insert, mute, solo,
  lock, level, chain, mode), a live scenario.
- `audio-render`: the track stage (sum → chain → level → mute/solo), the track FX state key,
  the PDC of the track chain, the tail after the last event; the path without a stage is byte for byte
  as before.
- `video-plan`: the track as an implicit reel in the plan; blend modes as a pair of CPU + WGSL; video mute/solo;
  the memo key of the track composite.
- `morpher`: "below" by rows is counted by the order of the track objects; morph and camera
  tracks are the same objects without level and chain.
- `compositions`: the invariant "number = row rank" through the vector of tracks; the tracks of a reel are
  objects of the reel; the sub-mix of a reel applies its track stages.
- `automation`: the properties of a track in the passport; curves on a track are rejected loudly until the
  next change.
- `project-format`: the track envelope, `enabled`, track effects, migration of counters and names on
  reading, the v11 predicate.
- `export`: listing of mute/solo before the start in the render window and in kt-cli; mute/solo — in the file.

## Impact

- `src/libs/kt-model` (`project.rs`: `Track`, removal of the counters/`lane_order`/`lane_names`/
  `lane_heights`, `rows()`, rank validation; `model.rs`: track commands and the lock gate, six
  renumbering commands), `src/libs/kt-engine` (`audio.rs` track stage, `fxstore.rs`
  composite key, `video_plan.rs`/`video.rs` track-as-reel and blend, `export.rs`
  preflight), `src/libs/kt-gpu` (`composite.wgsl` blend modes as a pair with the CPU kernels),
  `src/libs/kt-fx` (the streaming chain processor without the clip shape), `src/libs/kt-otio`
  (track envelope, track effects, migration), `src/apps/kt-app` (`timeline.rs` header and
  gutter, properties window, `fx_editor.rs` track target, `render_dialog.rs`, i18n),
  `src/apps/kt-cli` (probe/render print mute/solo).
- Tests: t2/t3/GOLDEN_AUDIO and GOLDEN_VIDEO do not move; t49/t241/t588–t594 move to the vector of
  tracks; new t729–t746. Notes: F-TL-006, F-AUD-004 (first part), F-MPH-005
  (track level), F-CMP-003 → "done" after the archive.
