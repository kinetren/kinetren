# Design

## Context

See proposal.md — Why. In the code (`kt-fx` `ClipProcessor::read`): a position jump is a hard seek
when the processor is cold, nothing when it is the expected input-first wrap (pitch-loop-silence D2),
a hard seek on a FRESH ENTRY (pitch-event-edges D4: the context applies, the chain has NO tail, the
event entered at its first frame or after its end), otherwise a SEAM: the source re-seats, every node
keeps its state. The look-ahead nodes keep their rings through a seam (pitch-loop-silence D1), which is
right for continuous output INSIDE an event and wrong for a re-entry: the ring of a pitch at the wrap
holds `La` frames of where the previous lap stopped.

D4 kept tail chains on the seam for R.28 ("a reverb rings through the circle"). For a chain without the
context that is exact: a linear reverb's state at the wrap plus the new input is the old tail plus the
response to the new input. With the context (an edge-reading pitch after the tail) the ring is not
linear storage of the past — it is the window the pitch reads ahead, and it holds the wrong moment.

## Goals / Non-Goals

**Goals:** a lap starts as playing from that point does; the previous lap's ring still rings through
the circle; no loop — byte for byte the former sound; one piece == pieces.

**Non-Goals:** chains without the context (a plain reverb, a limiter) keep the seam — exact already;
an event covering the region END keeps the input-first wrap of pitch-loop-silence D2.

## Decisions

### D1. A lap entry is a seek, for tail chains too

`ClipProcessor::lap_entry(local)`: warm, the chain has a tail, and the same re-entry rule as
`fresh_entry` (`reentry`: the context applies, the output jumps to the event's first frame or the
processor has played past its out-point, not the expected fold). `read` hard-seeks on it, exactly as
on a fresh entry. `fresh_entry` keeps its meaning (the hand-off of a split asks it, D3 of
pitch-event-edges) — split into `reentry` without a change of what it answers.

### D2. The previous lap rings on in its own processor

A seek alone would cut the previous lap's echo at the wrap (with "rings on" it can still be loud: a
feedback echo, a region ending in the tail). So the engine, before the read, asks
`FxStateStore::start_lap`: on a lap entry, when the processor stands in its tail (`past_out_point`:
its next frame is at or past `clip_len + tail_ext`) and short of the event's tail-zone end, it moves to
the event's ringing laps and the event gets a cold processor synced to its clip; the read then seeks.
`render_fx_clip` reads every ringing lap for the same output frames, continuing its own clip-local
stream, and adds it at the tail zone's weight (1.0 × the clip gain); a lap that reaches the tail-zone
end retires. That is exactly what straight play gives for two copies of the event one after another:
the next one starts cold with its context while the previous one rings on (pitch-tail-choice D2's
join). A lap cut by the wrap before its out-point is not heard past the wrap, so it is just reset.

Alternatives: (a) fold the input `La` frames before the wrap (pitch-loop-silence D2) for an event whose
TAIL covers the region end — the new lap's grains at its first frames would read the previous lap's
tail instead of the material before the in-point (the original edge dip), and it does nothing when the
tail has rung out before the wrap; (b) a seek only, no ringing laps — cuts a ringing echo at every
wrap, a click with a feedback echo; (c) fork only the nodes before the pitch (the pitch-tail-choice
twin) — the pitch's own ring is the problem, and the nodes after the pitch ring too.

### D3. Bounded and allocation-free in steady state

At most `MAX_LAPS` = 4 earlier laps ring at once: a region shorter than the tail drops the oldest,
four laps into its decay (a 0.6-feedback echo of 100 ms is at −44 dB there). A lap that retires is
reset and kept as the spare; the next lap starts with it (`sync_clip` re-syncs its parameters), so after
the first laps a loop allocates nothing on the render thread. A seek (`reset_all`) drops every ringing
lap; a removed event drops its laps (`retain_clips`). Model edits re-sync the event's processor; an
earlier lap keeps the parameters it played with for the rest of its tail.

### D4. What is byte for byte

Without a loop the output never jumps back into an event, so no lap entry happens: GOLDEN_AUDIO and
t1105–t1114, t1330–t1339, t973c stay. With "cut" an earlier lap past its out-point gives zeros (the
content gate stands after the pitch), so a lap equals the play from its start byte for byte (t1440,
t1441); with "rings on" it equals the play from its start plus the earlier laps' play continued past
the region end (t1441 rebuilds that sum from one straight render).

## Risks / Trade-offs

- [The earlier laps cost a chain each for their tail] → bounded by `MAX_LAPS` and the tail; their input
  is past the out-point, so no source is read for them (the tail chunk skip, R.43 item 304).
- [On a pure tone two pitched streams of one frequency can partly cancel] → measured on 440 Hz, a
  feedback echo ringing over the new lap: the deepest start window 0.62–0.79 of the play from there with
  the vocoder, 0.95–1.07 with WSOLA — the same nature as the copies' joins of pitch-tail-choice; on music
  it is a tail under the next sound.
- [A lap entry with a mouse move during playback] → an event moved so that the output lands on its first
  frame starts as a seek there; such a move was a seam before. Rare and not worse.

## Migration Plan

None: no format, no setting.
