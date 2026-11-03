# Design

## Context

See proposal.md — Why. Observed in the code (`src/libs/kt-fx/src/audio/mod.rs`, `ClipProcessor`):

- With the context (`chain_context`) the chain's input runs `La` past the out-point (`gate_in =
  gate_end + la`) and the seek pre-roll reads before the in-point; the CONTENT GATE right after the
  last edge-reading node (index `ctx`) zeroes everything outside `[−head_ext, gate_end)`. Every node
  before `ctx` (the "prefix") therefore processes the material on both sides: its echo of the material
  before the in-point sounds at the event's start, its echo of the event's own sound past the end is
  cut (pitch-context-whole-chain D1 — the cost the owner was asked about).
- The prefix's state past the out-point is a function of the material AFTER the event too (the
  context of `La` frames); one stream cannot give both "the pitch's look-ahead reads the next
  material" and "the echo past the end does not contain it" — that is why alternative (b) of
  pitch-context-whole-chain D1 ("≈ 40 ms of foreign pitched sound after every event, or a 96 ms hole")
  was rejected.
- `AudioState` (kt-plugin-api) has no clone; every audio kind is a plugin state (18 built-ins in
  `src/plugins/fx-audio-*`); their fields are plain buffers, `kt_plugin_api::dsp` primitives, FFT plans
  behind `Arc` and `Box<dyn CurveRead>` curve readers (two implementations: `NoCurve`, kt-fx's
  `OwnedCurve`).
- A plugin row with `omit_default` is not written at its default; the slot reader IGNORES a key its
  passport does not name — an older build would open a "rings on" file and silently cut the echo.

## Goals / Non-Goals

**Goals:**
- "Cut" (default): byte for byte today's sound — every new path is behind the choice.
- "Rings on": the prefix hears only the event's own sound — it starts with the event and its echo
  rings past the out-point, pitched; nothing of the material outside the event reaches the echo, and
  the echo starts at the out-point without a hole.
- No dips at the joins in both modes; one piece == pieces in both modes.

**Non-Goals:**
- A choice for the nodes AFTER the pitch: they already ring (the gate stands before them).
- Ringing a prefix whose kind cannot fork (a foreign plugin without `fork`): it stays cut (named in
  the spec).
- The hand-off of a split under "rings on" (below, D5).

## Decisions

### D1. The switch is a row of the pitch effect, declared by the kind

A choice row `echo_before` = `cut` | `ring` on the `pitch_shift` passport (`Store::Choice`,
`omit_default`), declared to the host as `Decl.echo_row = Some("echo_before")`: the row of an
edge-reading kind whose non-zero value lets the echo of the nodes before it ring on. kt-fx reads the
value of the slot at `ctx` through the declaration and never names the kind (the kt-fx checklist rule).

Why on the effect, not in the project settings next to the pitch engine: the engine is the device
(one algorithm for the edit, R.48); the echo is about WHICH effects stand before THIS pitch — a voice
with a reverb wants it to ring, a beat cut tight wants it cut, both in one edit. The row travels with
the slot into morpher batches, presets, copy-paste and the clipboard without a line of new code, and
it stands in the panel where the owner sets the semitones.
Alternatives: (a) a project setting — one answer for different chains, and the owner's morpher pitch
and an event's own pitch could not differ; (b) a parameter of the reverb — the reverb does not know
what stands after it, and several nodes may stand before the pitch; (c) a per-event flag in the
model — a new model field for what is a property of one effect.

### D2. "Rings on" at the start: the ringing prefix starts with the event

A prefix node that rings (`tail() > 0`) and does not read ahead (`lookahead() == 0`) is RESET when its
content position crosses the in-point (`−head_ext`) continuously — inside a chunk the node's call is
split at the crossing. The pitch keeps the material before the in-point in its ring (its look-behind
reads it — no dip), but the echo of that material does not reach the event: as without the pitch, the
reverb starts with the event. Without this the owner's copies would get two echoes at every join: the
previous copy's (ringing on) and the echo of the material before the piece's in-point (never heard).
A seam that lands past the in-point (a loop lap) is not a crossing: the echo rings through, R.28.
Look-ahead nodes before the pitch are not reset: their ring holds the context the pitch reads.

### D3. "Rings on" at the end: a twin forked at the out-point

When the chain INPUT reaches the out-point (`gate_end`), the nodes `0..=ctx` (and the dry ring of a
layer mix) are FORKED: the twin starts from exactly the running state and from there is fed silence
(the chunk that contains the out-point is copied with its frames from `gate_end` on zeroed). Both run
the same per-node code with the same content positions and the same curve targets until the main
output reaches the out-point; at the content gate the main stream gives the frames before `gate_end`
and the twin the frames from `gate_end` on. When the gate's content position is at or past `gate_end`
for a whole chunk, the twin's nodes REPLACE the main nodes `0..=ctx` (the main prefix holds the echo of
the material past the out-point, which is never heard) and the gate stops cutting on the right
(`ringing`). A reset or a seek clears `ringing`; a seam keeps the promoted state (the echo of the
event's own sound rings through, R.28) and returns the gate to `[−head_ext, gate_end)` for a seam back
into the body, which forks again at the next out-point.

What is heard: the body is the cut mode's body (the pitch read the next material for its last grains);
from the out-point on, the pitch of "the prefix's output on the event's own sound, then silence" —
the echo and the event's own last grains dying out, no frame of the next material, no hole. The twin
lives only between the input and the output reaching the out-point (`La` + the look-ahead after the
pitch, ≈ 96 ms): the cost is one fork per event end and twice the prefix + pitch for that span.

Alternatives: (a) one stream with the context gated at the out-point for the prefix — the pitch's
last grains read silence: the end of every event dips again (the original complaint); (b) the earlier
rejected (b) — foreign sound or a hole; (c) a second processor warmed by a seek pre-roll at the
out-point — inexact (its WSOLA offsets and phases are its own: a click at the switch) and costs a
whole tail of pre-roll per event end; (d) a twin running from the event's start — exact but doubles
the prefix + pitch for the whole event.

### D4. `fork` in the plugin contract

`AudioState::fork(&self) -> Option<Box<dyn AudioState>>`, default `None`: "a state that continues
exactly as this one would on the same input". Every built-in audio kind implements it by `Clone`
(`kt_plugin_api::dsp` primitives derive `Clone`; `CurveRead` gains `clone_box`). A node that cannot
fork leaves its event cut at the end (the fork is all-or-nothing over `0..=ctx`); the harness checks
that every built-in kind forks and continues byte for byte (t1330).

### D5. The split rule under "rings on"

`run_predecessors` is unchanged: it already requires a predecessor without a tail, and "rings on"
matters only when the prefix has one. So a split under "rings on" is two streams for the prefix, as
without the pitch: the left piece's echo rings into the right piece, the right piece's prefix starts
with it (D2); the pitch reads across the join on both sides, so the join does not dip. A prefix
without a tail makes "rings on" identical to "cut" byte for byte, and the hand-off applies as before.

### D6. Format 3 by content

`project_file::version_for` writes format 3 when any slot of the document (events, tracks, morpher
patches, inside every reel) has the echo row at a non-default value; the reader reads 1–3. A build
that reads up to 2 refuses such a file with "written by a newer Kinetren" instead of cutting the echo
silently. A file without it is format 1 or 2 byte for byte. The OTIO door writes slots in their file
form inside `metadata["kt"]`, so the row travels (no "OTIO export loses" row).

## Risks / Trade-offs

- [Splitting a node's call at the in-point crossing changes the sound if a kind depends on its call
  sizes] → kinds are slicing-independent by the contract (t338); one piece == pieces is checked in
  ring mode (t1333).
- [A fork allocates on the render thread once per event end] → the render thread is not the cpal
  callback; the delay line (1.5 s) is the largest clone, ≈ 0.6 MB.
- [The echo past the end of a downward pitch is shorter than the reverb's tail by the pitch's
  window] → it is the event's own grains dying out; the window is inside `chain_tail`.
- [A seek into the tail of a ringing event equals continuous play only within the seek pre-roll] →
  the same rule as every stateful effect (seek == cold render of that position).

## Migration Plan

None: the default is the current sound; a file is format 3 only when the owner chooses "rings on".
