# Design

## Context

See proposal.md — Why. Observed in the code (read-only):

- **The seam.** `ClipProcessor::read` (`src/libs/kt-fx/src/audio/mod.rs:707-735`) treats any jump of
  the clip-local output position during continuous output as a seam (`mod.rs:725`); `seam`
  (`mod.rs:748-770`) re-seats the source at `local + La` and, by its own comment, expects "the
  lookahead nodes' rings live on". The renderer splits a looped burst at the region end and calls the
  clip with the jumped position (`render_looped_with`, `src/libs/kt-engine/src/audio.rs:1450-1473`;
  the player uses it, `src/libs/kt-engine/src/player.rs:1219`; so does `kt-cli probe`).
- **Input before the wrap.** `produce` (`mod.rs:821…`) reads the chain input at
  `chunk_in = chunk_local + La` (`mod.rs:837`). In the last `La` frames before the output reaches the
  region end the input therefore reads the material AFTER the region end — material that will never
  sound in a loop.
- **The pitch node empties itself on a seam.** `PitchNode::process` (`src/libs/kt-fx/src/audio/nodes.rs:1881-1920`)
  calls `restart(in_pos)` whenever `written != in_pos` (`nodes.rs:1886-1888`); `restart`
  (`nodes.rs:1871-1879`) zeroes the input ring and the OLA accumulator and restarts the grain grid at
  `in_pos − La`. The output covers `[in_pos − La, …)` (`nodes.rs:1898`) and its grains read the ring
  around those positions, which after the restart hold zeros: `La` = `PITCH_LOOKAHEAD_FRAMES` = 4 608
  frames (`nodes.rs:1670`, 96 ms) of silence after every wrap, plus up to half a grain. A loop of
  about `La + PITCH_GRAIN/2` ≈ 5 100 frames (107 ms) or less is silent throughout. This breaks the
  crate's own rule "the state survives `seam`" (`src/libs/kt-fx/CLAUDE.md:182`).
- **The limiter is the mirror case.** `LimiterNode` (`nodes.rs:1543-1559`, `process` at
  `nodes.rs:1603…`) is a FIFO: it does not restart, so after a wrap it outputs the `La` frames it read
  past the region end (5 ms = 240 frames by default) — a click of foreign material, not silence.
- **Why the guards missed it.** t166/t167 (`src/libs/kt-engine/tests/loop_fx.rs:135-260`) loop a
  slowed+reverb clip (`La = 0`); t337–t339 (`tests/audio_pitch.rs`) render pitch without a loop;
  t334–t336 (`tests/audio_lookahead.rs`) — the limiter without a loop. No test combines a look-ahead
  node with a wrap.
- **Constraints.** `La < MIN_PREROLL_FRAMES` = 4 800 (`src/libs/kt-fx/src/lib.rs:20`); grids are
  absolute so that one piece == pieces and seek == cold render (t338, audio-fx design); the dry ring of
  LayerMix (`mod.rs`, `dry_ring`) lives in the input domain and must stay aligned.

Root-cause status: the arithmetic above is read from the code, not yet measured. Stage 1 measures it
(t906/t907 written first and RED with the predicted run length) before any fix.

## Goals / Non-Goals

**Goals:**
- No silent run and no foreign material at a loop wrap, for any region length, both pitch engines and
  the limiter, on events and on track chains.
- Everything that was byte-for-byte stays so: neutral pitch, `GOLDEN_AUDIO`, seek == cold render,
  slicing independence, non-looped renders.

**Non-Goals:**
- Changing the pitch algorithms, the hop/grain sizes or `PITCH_LOOKAHEAD_FRAMES`.
- The reverse transport (it plays the raw path without FX, transport design) and nested-reel submixes
  inside a loop (they render their own stream; a separate check if a complaint comes).

## Decisions

### D1. Look-ahead nodes are keyed by the stream position, not the clip position
The pitch node's input ring, OLA accumulator and grain grid use a STREAM position: set from the clip
position at a real seek (the cold start/`hard_seek` path, so seek == cold render and every non-looped
render are unchanged bit for bit), then advanced by the frames processed; a seam never resets it. The
node keeps a short list of `(stream position, clip offset)` pairs (one per seam within the look-ahead
window) so a grain's pitch curve is read at its CLIP position. The limiter already behaves this way.
Until the first seam the stream position equals the clip position, so t338's "grids absolute from the
event position" stays literally true for every render it covers.
Alternatives: (a) keep clip positions and "relabel" the ring by `−span` at a wrap — for a region shorter
than the ring (8 192 frames) the head of the next lap and the tail of the current one collide modulo
the ring; (b) a mini warm-up of the look-ahead nodes at each seam — it would push extra material through
the nodes before them (an EQ, a compressor) and change their state, against invariant 3; (c) crossfade
from the dry signal over the empty window — hides the gap, plays the wrong pitch for 96 ms.

### D2. At an armed loop wrap the input wraps first
`render_looped_with` passes the region to the clip as a clip-local span `[rs − start, re − start)`
when the event covers the region end. `produce` folds the INPUT position into that span:
`chunk_in` crossing the span end continues from its start (splitting the chunk at the fold, as the
1 024-frame automation step is split today), and the source is re-seated there through the same
`seam` code (`source_offset` of the folded position). When the OUTPUT then jumps back to the span start,
the input is already continuous: `read` recognises the expected jump (`next_local` = span end, `local` =
span start) and does not re-seat the source a second time. A region shorter than `La` folds several
times inside one window (`rem_euclid`), which is what makes t907's 700- and 3 000-frame loops sound.
An event that starts inside the region (its next-lap entry is at clip position 0) gets the fold
too: the input before its start is the pre-roll silence of the gate, exactly as on the first lap.
Alternative: re-seat on the output jump and refill the ring from the source in place (a second read of
`[local, local + La)`) — the nodes before the look-ahead node would have to process that window twice.

### D3. The source window and the prefetch fold with the input
`chain_source_window_ext` extends the window forward by `La`; under an armed loop the extension is the
head of the region (`[rs, rs + La)` in clip terms), computed by the same function the render uses. The
region head is already pinned in the cache (transport t14), so in practice this only removes wasted
reads past the end; t908 asserts `wanted ⊇ read` across the wrap.

### D4. Non-loop seams continue the stream
A clip shifted with the mouse during playback or a re-anchored material (`set_src_in`) is still a seam:
the source is re-seated, and with D1 the pitch node continues its stream (the content under the jump
changes, as it does for the reverb). Today it empties itself for 96 ms there too; t910 covers it.

## Risks / Trade-offs

- [Lap 2 sounds slightly different from lap 1 — the grain grid's phase against the material shifts by
  `span mod HOP`] → that is continuity, not a defect; the first lap and every non-looped render are
  unchanged, and each run is deterministic (t908).
- [The input-side fold touches the hot path of every looped event] → only when a loop span is passed
  AND `La > 0`; with `La = 0` the code path is the old one (the `GOLDEN_AUDIO` argument of R.48);
  t166/t167/t14 must stay green unchanged.
- [The hypothesis is wrong for the owner's "small loop" (e.g. his loop is 0.5 s and still silent)] →
  t907 sweeps lengths; if 0.5 s is silent after the fix, stage 1's measurement names the other cause
  before stage 2 is closed; the owner is asked to repeat item 7 on the new build.

## Migration Plan

None: no format, no setting. The audio of looped playback with pitch/limiter changes (silence and
foreign material disappear); exports are not looped, so no export changes.

## Open Questions

1. Should a seam that is not a loop wrap (a clip dragged during playback) also refill the look-ahead
   from the new position instead of continuing the stream (D4)? Recommended: no — it is a gesture
   during an edit, the reverb behaves the same way, and refilling needs the double processing D2
   rejected; revisit only on a complaint.
