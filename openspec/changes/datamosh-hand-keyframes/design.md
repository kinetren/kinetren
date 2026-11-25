# Design

## Context

`Pos.sched_i` is "this position is a scheduled I-frame": start, cut, every `gop` (or never at 0), and reset edges
(`mosh.rs` `sched_i`); `mosh` drops a scheduled I-frame when its gate is on (the artifact). Rows with curves are read per
position (`rows_at`), edges are found by the previous position's value.

## Decisions

### D1. Row `key` (a gate, 0/1, `live`), appended as row 20 (`omit_default`, 0)
A rising edge (value ≥ 0.5 now, < 0.5 before) makes the position a scheduled I-frame; the GOP counter restarts there, as
after any I-frame. The edge is in `enc_hash`'s inputs through the schedule (the row is static per position), so `break_hash`
and the memo keys see it; a project without the row has the same hash as today (the former bytes stand). It is NOT a
reset: a `reset` hands an intact picture, `key` is an ordinary I-frame that `mosh` may drop — which is the point of placing it.

### D2. The sound
`sound.rs` schedules its packets by the picture's frames only through `cut` and `gop`; the audio half has its own AAC frame
grid and no I-frames; a picture keyframe does not restart the sound. The passport is one, so the row is `foreign` to the
sound half (`foreign_rows`); a spectral restart on the same place is not asked (D4 of datamosh-sound names its own).

### D3. The button and the graph
The `key` lane shows Hold keys as marks; "Key here" (an icon with the accelerator of the key, a hint) writes a Hold key
1 at the playhead and one of 0 one frame later, in one undo step (a "pulse"). Dragging a mark drags the two together
(a pair of keys on the same curve, a rule of the lane for gate rows). The same pulse serves `mosh`, `reset`, `bloom`, `freeze`
rows: the class rule (one place, `automation.rs`).

### D4. The class: effects that fire by a period
Walked on the catalogue: Datamosh `gop` (this); VHS Tape's dropouts and head clogs fire by rate (a hand row would be a
"dropout here" gate: not built, named as the second user of the pulse rule); Camera ISP has none; `glitch_blocks`
(`fx-looks-w2`, rate by hash) and `film_damage` — hash, would get a `force` gate when built; audio `gate` is by level; LFOs
(tremolo, chorus) have a phase row instead. Rule: a kind that fires by a period declares its gate row in the passport, named `key` or
`force`, and the lane's pulse works on it.

### D5. Datamosh on a track
A track's stream uses the same rows (`track-stream-stage`): the schedule is the track's timeline, no extra.
