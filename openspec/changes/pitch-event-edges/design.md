# Design

## Context

See proposal.md — Why (the measured numbers). Observed in the code:

- **The pre-roll stops at the in-point.** `ClipProcessor::hard_seek`
  (`src/libs/kt-fx/src/audio/mod.rs`) takes `preroll = max(tail, MIN_PREROLL).min(local + head_ext)`:
  at the start of an event without transition handles the pre-roll is ZERO, and the warm-up only
  fills `[0, La)`. A pitch grain covering output frame 0 starts at −512 and, for a downshift, reads
  behind itself — into the zeros of the cleared ring (`PitchNode::restart`,
  `src/plugins/fx-audio-modulation/src/lib.rs`).
- **The input gate cuts at the out-point.** `produce` zeroes the input at `gate_end = clip_len +
  tail_ext` — that is what keeps the dry signal out of a reverb tail. An upshift grain covering the
  last frames of the event reads up to `r · 1024` frames ahead, past the gate: zeros.
- **Each event is its own stream.** `FxStateStore` keys processors by `ClipId`; the next event's pitch
  node starts at its own clip zero with an empty accumulator (WSOLA picks offset 0 against nothing),
  its own grid and zero vocoder phases. A sine pitched by two independent streams has an arbitrary
  phase difference at the cut — the step measured at every join.
- **The window functions clamp to the material.** `chain_source_window_ext` and
  `chain_consumption_window_ext` clamp to `material` (`−head_ext .. clip_len + tail_ext`) passed by
  the engine (`render_fx_clip`, `wanted_audio_bursts_for`); `fill_source` returns silence past the
  window end. Any extension must live inside these functions so that render and prefetch agree
  (item 13, t17, t336).
- **What `pitch-loop-silence` established and this change keeps:** stream-keyed rings (a seam
  continues the stream, the curves are read at the grain's clip position), the input-first wrap for
  an event covering the loop region, and "a folded input that lands before the event's material is
  silence" (the next lap belongs to other events).

## Goals / Non-Goals

**Goals:**
- No dip and no silence at the edges of a pitched event, for any material and both engines.
- A run of events cut from one material with the same pitch chain sounds as the uncut event — byte for
  byte (the strongest form of "no tear").
- Everything else byte for byte: chains without an edge-reading node, neutral pitch, `GOLDEN_AUDIO`,
  t906–t911.

**Non-Goals:**
- The vocoder's level dependence on its starting phase (two independent cold starts may land 6.5 dB
  apart, recorded by `pitch-loop-silence`). A run of a split is one stream and does not show it; events
  of DIFFERENT material are cut against each other as they are without pitch.
- Chains with a time warp (Slowed+Reverb) before the pitch: the source beyond the edges of a warped
  event is a function of a rate curve that does not exist there. They keep today's edges.
- Pitch per event engine, formants, phase locking.

## Decisions

### D1. The pitch node reads the event's own source across the edges (context)

The physical model: an event is a WINDOW into its material; pitch is a time-scale operation whose
output at `t` is built from the input around `t` (a grain, an FFT frame). The sound of a pitched event
is therefore "the material pitched, then cut to the event" — the way Premiere, Resolve and Reaper
apply clip pitch (to the source media, with handles), not "the event cut, then pitched", whose edges are
grains half full of silence. Concretely, for a chain whose context applies (D2):

- `hard_seek` does not clamp the pre-roll to the material: the warm-up reads the source before the
  in-point (silence before the file start, as `fill_source` already does);
- the input gate moves from `gate_end` to `gate_end + La` (the source continues past the out-point by
  exactly what the look-ahead reads);
- a CONTENT GATE right after the last edge-reading node zeroes every content position outside
  `[−head_ext, clip_len + tail_ext)`; the dry side of a layer mix still waiting for its marker is gated
  the same way. Everything after that point sees the event as cut: a reverb after pitch rings on the
  event only, nothing outside the event is heard;
- `chain_source_window_ext` and `chain_consumption_window_ext` extend `material` by the pre-roll and
  `La` themselves, so the render and the prefetch read and order the same frames.

Alternatives: (a) anchor the grain grid to source time instead of the clip start — it only aligns the
grids; the rings at the edges are still empty and the WSOLA offsets/vocoder phases of two streams still
differ, so the dips and the step remain; (b) pad the ring past the out-point by reflecting the
event's own tail — invents material; a reflected sine has a phase reversal, i.e. a new step; (c) a
short crossfade at every join — cures neither the silence (both sides are silent at the cut) nor the
phase difference (a crossfade of two equal-frequency sines out of phase dips by up to 100 %); physics
does not need it once D3 makes a split one stream.

### D2. Who reads across edges: a kind's declaration, not "every look-ahead node"

`Decl.reads_across_edges` (kt-plugin-api, default `false`); pitch declares it. The limiter does NOT:
it limits what the event outputs, and the event's output is cut — a limiter that saw the source past
the out-point would duck before a peak that is never heard. `chain_context(fx, auto)` (kt-fx, the one
home, used by the processor and by the window functions) is true when the chain has an edge-reading
node with a non-zero look-ahead, no time warp, and no node with a tail BEFORE the last edge-reading
node (a reverb before pitch would carry the context into its tail; there the reverb's own tail is the
real continuation, and the chain keeps today's edges).
Alternative: every look-ahead node reads context — changes the limiter at every edge (above).

### D3. A run of a split is one stream: the processor is handed on

`AudioRenderer` marks for each event its RUN PREDECESSOR: the event on the same track that ends exactly
where it starts, plays the same media from the frame right after (`prev.source_in + prev.len ==
source_in`), has the same chain structure and look-ahead, the context applies to both chains (D2),
neither is reversed or overlapped, there is no transition window at the join, and the predecessor
has no tail. When such an event is about to read its first frame on a fresh entry (D4) and the
predecessor's processor has just played to its own end, the two processors are SWAPPED in the store:
the warm one is rebased (`ClipProcessor::hand_on`: its clip-local position moves by the predecessor's
length, its source anchor becomes the event's) and synced to the event's parameters; the other one goes
to the predecessor, reset. The pitch node sees a seam and continues its stream (pitch-loop-silence D1),
the source cursor is already where the event's material continues, and the ring holds that material
because of D1's context. Clips are rendered in ascending order inside every burst, so "the predecessor
has played to its end" holds exactly when the event's first frame is reached by continuous play,
whatever the slicing.

Result: a split of a pitched event is the uncut event byte for byte (t1105) — the strongest form of
"no tear". Different parameters on the two sides arrive as a step, like an automation step
(the grain straddling the join was synthesised by the predecessor).

Alternatives: (a) one processor per run keyed by the run's first clip — a model edit that moves one
cut would re-key states and destroy them (invariant 3); (b) clone the predecessor's state — `AudioState`
has no clone, and a copy keeps two owners of one stream; (c) only D1 without D3 — the dips go, the phase
step stays (two streams).

### D4. A fresh entry of a tail-less edge-reading chain is a seek, not a seam

A seam means "the OUTPUT is continuous" (a loop wrap inside the event, a clip moved during playback).
An event whose context applies and whose chain has no tail, read again at its FIRST frame after it
had stopped (a loop lap over several events: the event ended, others played, the region wrapped), was
silent in between: there is nothing to keep, and a seam would reuse a ring holding the material from
where it stopped. Such an entry goes through the D3 hand-off when the predecessor qualifies, otherwise
through `hard_seek` with the context pre-roll. Chains with a tail keep the seam (R.28: a reverb rings
through the circle). The expected input-first wrap of pitch-loop-silence (an event covering the whole
region) is not a fresh entry.

### D5. The loop fold reads own context only where the event is the next lap

An event that covers the loop region's END but starts inside the region keeps the pitch-loop-silence
fold only when it also covers the region START (it is what sounds after the wrap). Otherwise, with the
context applying, it gets no span: its input continues into its own material, its output stops at the
region end, and the event at the region start enters fresh (D4) — the wrap is a cut between two events,
as it is without pitch. Before, the folded input landed before the event's material and was silence:
the last grains of every lap read zeros (the same dip as at an edge).

## Risks / Trade-offs

- [Pitch sounds different at every event edge than before] → it is the fix; t973c's pitch row and
  t337–t340 must stay green (the row renders a whole event from source zero: no context before it,
  context after it is cut by the gate).
- [The hand-off makes continuous play through a join differ from a seek to the join] → the same holds
  for every stateful effect (a reverb); a seek equals a cold render of that position (t1110).
- [Extra source reads before/after each pitched event] → at most 4 800 + 4 608 frames per edge, ordered
  by the same window function; the cost of a pitched event grows by one pre-roll per entry (measured
  in tasks, debug only — no release builds for this).
- [A run of 15 events with a reverb after pitch] → context applies (reverb after the gate), the
  hand-off does not (tail): the joins have no dips, the phase step remains and is masked by the tail.

## Migration Plan

None: no format, no setting. The audio of pitched events changes at their edges; exports follow.

## Open Questions

None blocking. If the owner's 15 events are not cut from one material (different takes), the joins are
cuts as they are without pitch: no dips, but no continuity either — the report asks him which case his is.
