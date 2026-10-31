## ADDED Requirements

### Requirement: A look-ahead window that reads across the event's edges
A kind whose output at a moment is built from the input around that moment (pitch: a grain, an FFT
frame) SHALL declare that its look-ahead window reads across the event's edges
(`Decl.reads_across_edges`); a kind that shapes what the event outputs (the limiter) SHALL NOT. When a
chain has such a node with a non-zero look-ahead, no time warp and no node with a tail before the last
such node ("the context applies", one function `chain_context` for the processor and the window
functions), the chain's input around the event's edges SHALL be the event's own source: the seek
pre-roll reads before the in-point and the input continues past the out-point by the look-ahead. A
content gate right after the last such node SHALL cut the result back to the event (and the dry side
of a layer mix waiting for its marker with it), so nothing outside the event is heard and nodes after
it see the event as cut. The render and the prefetch SHALL extend the source window in the same
function. A chain where the context does not apply SHALL be byte for byte as before.
Why: an event is a window into its material, and pitch is a time-scale operation — the pitched event
is "the material pitched, then cut", as clip pitch is applied to the source media in other editors.
Cut first and pitched after, its grains at the edges were half silence: fifteen short pitched events in
a row tore (kkt-26.11.01 item 15) — WSOLA +7 had 320 frames of total silence before every join, −7
250 frames after it, and the vocoder dropped to 0.42 of its level. A limiter that saw past the
out-point would duck before a peak that is never heard, so it does not read across.

#### Scenario: Pitched events of other material have no dip at their edges
- **WHEN** fifteen 100 ms events, each from a different place of a 440 Hz sine (no event continues another), carry a pitch shift of ±7 semitones, with each engine
- **THEN** no 64-frame window within 1 024 frames of a join falls below half of the steady level of its event (before: total silence with WSOLA); the render is the same in one piece and in pieces of 1 024 and 333 frames
- **TEST** t1106 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Nothing outside the event is heard; a reverb after pitch rings only on the event
- **WHEN** one pitched event (+7) stands in the middle of a timeline over a source that is loud before and after it; then the same event with an echo AFTER the pitch; then with an echo BEFORE it
- **THEN** the output before the event's start and (without a tail) after its end is exactly zero; with the echo after the pitch the tail equals the echo of the gated pitched event; with the echo before it the context does not apply (`chain_context` is false) and the render equals the former edges
- **TEST** t1107 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: The prefetch orders the context
- **WHEN** pitched events (forward and reversed) are rendered with a recording fetcher from a cold seek and through their edges
- **THEN** every source frame read is inside the prefetch request for the same range (`wanted ⊇ read`), including the pre-roll before the in-point and the look-ahead past the out-point
- **TEST** t1108 (`src/libs/kt-engine/tests/pitch_edges.rs`)

#### Scenario: Who reads across edges
- **WHEN** the static `chain_context` and the processor's own decision are compared over chains: pitch alone, a limiter alone, neutral pitch, limiter + pitch, pitch + echo, echo + pitch, Slowed+Reverb + pitch
- **THEN** they agree; only the chains with an active pitch and no tail or warp before it read across; the limiter alone renders an event cut from a longer material byte for byte as the same event with silence around it
- **TEST** t1109 (`src/libs/kt-engine/tests/pitch_edges.rs`)
