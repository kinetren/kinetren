# Proposal

## Why

ktprompts v26.10.18 item 7 (`docs/user-feedback.md`, round of 2026-10-27/28): "The pitch audio FX has
some strange buffering: on a loop a piece of silence is always heard, and on a small loop — total
silence." Reading the code gives a precise cause: the pitch node is a look-ahead node (R.48, PDC) that
re-initialises its rings on every position jump, and at a loop wrap the chain's input is re-seated only
when the OUTPUT wraps — so the first `PITCH_LOOKAHEAD_FRAMES` = 4 608 frames (96 ms) after each wrap are
synthesised from an empty ring, and a loop not much longer than that never leaves the silence. The
look-ahead limiter has the mirror defect (it plays its window of material from past the region end).
This is not R.63's "silence in the loop" (an event reading past the end of its file).

## What Changes

- At an armed loop wrap the chain's INPUT wraps first: an event covering the region end reads the
  region head `La` frames before the output wraps, so the look-ahead holds what will actually sound.
  The source window and the prefetch request fold the same way.
- Look-ahead nodes (pitch, limiter) keep their rings and grids across every seam: they are keyed by the
  continuous stream position since the last real seek; the pitch curves are still read at the clip
  position of the grain.
- A seam that is not a loop wrap (a clip shifted during playback, a re-anchored material) no longer
  empties the pitch node; it continues on the stream like the reverb does.
- The same fold applies to a track's chain with look-ahead (the track stage, R.62 B).
- Neutral pitch, a real seek, one piece == pieces, and `GOLDEN_AUDIO` stay byte-for-byte.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `audio-fx`: "Lookahead — input runs ahead of output, no delay for the host" — the look-ahead reads the
  next lap at a loop wrap; look-ahead rings survive every seam; scenarios for a long loop, a short loop,
  determinism/prefetch, a seek and a moved clip.
- `audio-render`: "Chain lookahead (PDC) — input runs ahead of output" — the region wrap folds the
  look-ahead window, the source window and the prefetch; the track stage obeys the same rule; scenarios
  for the limiter at a wrap and a track chain.

## Impact

- Code: `src/libs/kt-fx/src/audio/mod.rs` (`ClipProcessor::read`/`produce`/`seam`: an optional
  clip-local loop span, the input-side wrap), `src/libs/kt-fx/src/audio/nodes.rs` (`PitchNode`:
  stream-keyed ring/accumulator/grid, no restart on a seam; `LimiterNode` unchanged — it is already a
  stream FIFO and only needs the right input), `src/libs/kt-engine/src/audio.rs` (`render_looped_with`
  passes the region down; the source-window/prefetch fold), the track stage in the same file.
- Docs: `src/libs/kt-fx/CLAUDE.md` (the look-ahead node rules gain "rings are stream-keyed; the input
  wraps first").
- Tests: t906–t911 (kt-engine `tests/loop_fx.rs`, `tests/audio_lookahead.rs`, `tests/audio_pitch.rs`,
  `tests/track_fx.rs`).
- No format change, no new dependency; invariants 1–3 and 11 hold (the render thread alone owns the
  state; an edit does not destroy it; audio stays byte-deterministic).
