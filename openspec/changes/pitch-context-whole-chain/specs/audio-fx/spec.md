## MODIFIED Requirements

### Requirement: A look-ahead window that reads across the event's edges
A kind whose output at a moment is built from the input around that moment (pitch: a grain, an FFT
frame) SHALL declare that its look-ahead window reads across the event's edges
(`Decl.reads_across_edges`); a kind that shapes what the event outputs (the limiter) SHALL NOT. When a
chain has such a node with a non-zero look-ahead and, if it warps time, its speed at the event's start
moves forward ("the context applies", one function `chain_context` for the processor and the window
functions), the chain's input around the event's edges SHALL be the event's own source: the seek
pre-roll reads before the in-point and the input continues past the out-point by the look-ahead. A
content gate right after the last such node SHALL cut the result back to the event (and the dry side
of a layer mix waiting for its marker with it), so nothing outside the event is heard and nodes after
it see the event as cut; nodes BEFORE it process the material too, and a tail of theirs ends with the
event. The render and the prefetch SHALL extend the source window in the same
function. A chain where the context does not apply SHALL be byte for byte as before.
Why: an event is a window into its material, and pitch is a time-scale operation — the pitched event
is "the material pitched, then cut", as clip pitch is applied to the source media in other editors.
Cut first and pitched after, its grains at the edges were half silence: fifteen short pitched events in
a row tore (kkt-26.11.01 item 15) — WSOLA +7 had 320 frames of total silence before every join, −7
250 frames after it, and the vocoder dropped to 0.42 of its level. A limiter that saw past the
out-point would duck before a peak that is never heard, so it does not read across. The owner's own
case (pitch-context-whole-chain) was a morpher's pitch AFTER the event's Slowed+Reverb on copies of
one piece of a reel: limited to chains with nothing before the pitch, the fix left it at 3 % of its
level at every join.

#### Scenario: Pitched events of other material have no dip at their edges
- **WHEN** fifteen 100 ms events, each from a different place of a 440 Hz sine (no event continues another), carry a pitch shift of ±7 semitones, with each engine
- **THEN** no 64-frame window within 1 024 frames of a join falls below half of the steady level of its event (before: total silence with WSOLA); the render is the same in one piece and in pieces of 1 024 and 333 frames
- **TEST** t1106 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Nothing outside the event is heard; a reverb after pitch rings only on the event
- **WHEN** one pitched event (+7) stands in the middle of a timeline over a source that is loud before and after it; then the same event with an echo AFTER the pitch
- **THEN** the output before the event's start and (without a tail) after its end is exactly zero; with the echo after the pitch the tail equals the echo of the gated pitched event
- **TEST** t1107 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: The prefetch orders the context
- **WHEN** pitched events (forward and reversed) are rendered with a recording fetcher from a cold seek and through their edges
- **THEN** every source frame read is inside the prefetch request for the same range (`wanted ⊇ read`), including the pre-roll before the in-point and the look-ahead past the out-point
- **TEST** t1108 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Who reads across edges
- **WHEN** the static `chain_context` and the processor's own decision are compared over chains: pitch alone, a limiter alone, neutral pitch, limiter + pitch, pitch + echo, echo + pitch, Slowed+Reverb + pitch
- **THEN** they agree; every chain with an active pitch reads across (an echo or Slowed+Reverb before it included), the limiter alone and neutral pitch do not; the limiter alone renders an event cut from a longer material byte for byte as the same event with silence around it
- **TEST** t1109 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Copies of a reel piece with Slowed+Reverb and a morpher's pitch do not tear
- **WHEN** five abutting copies of the same 0.72 s piece (the same in-point) carry Slowed+Reverb (rate 1, mix 0.1, a rate curve that turns negative far past the events) and a pitch +5 — in the event's chain and appended by a morpher — over a 440 Hz sine and over a reel holding it, with each engine
- **THEN** no 64-frame window within 1 024 frames of a join falls below half of the steady level (before: 0.034); the render is the same in one piece and in pieces of 1 024 and 333 frames
- **TEST** t1113 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: A node before the pitch processes the material, and its tail ends with the event
- **WHEN** one event (+7) over a loud source carries an echo BEFORE the pitch, then Slowed+Reverb (rate 0.8) before it, forward and reversed, rendered with a recording fetcher
- **THEN** the output outside the event is exactly zero (the echo's tail ends with the event), the edges sound, and every source frame read is inside the prefetch request (`wanted ⊇ read`), the pre-roll before the in-point included
- **TEST** t1114 (`src/libs/kt-engine/tests/pitch_edges.rs`)
