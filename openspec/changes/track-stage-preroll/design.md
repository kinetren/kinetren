# Design: track-stage-preroll

## Context

- audio-render "There are two breaks — seek and splice": a real seek (generation change, rate sign flip,
  loop flip) goes through `reset` + `hard_seek` with a pre-roll of `max(tail, 100 ms)` (+ look-ahead) into
  the void; a break of position during continuous output is a SEAM. `kt_fx::audio::…::hard_seek`
  (`src/libs/kt-fx/src/audio/mod.rs`) resets the nodes, starts the input `preroll` frames earlier and
  renders into garbage.
- Track stages (R.62, `AudioRenderer::render_stages`): per stage, the lane's events summed over the window
  `[from + la, to + la)` → the stage's processor (`store.track_mut(st.id)`) with its own rings; sidechain
  keys summed on their own branch (`store.key_mut`). No pre-roll: after a seek the processor's rings are
  whatever `reset` left — empty.
- kept-mix-settle: `cold_after_seek(head)` = `tail + la` when a stage's lane rang within that reach before
  `head`, so the kept mix does not keep a cold start (t2373); `settle_frames` covers leaving a kept range.
- `stage_reach = (max tail, max la)` over the stages.

## D1. The stage pre-roll

On a real seek to `head`, after the event chains' `hard_seek` decisions, each stage `st` whose lane has an
event heard in `[head − tail_st − la_st, head)` (the same test as `cold_after_seek`, per stage):

1. reset the stage's processor and its key branch;
2. render the lane sum over `[head − P, head)` with `P = tail_st + la_st` (option Q1 b: `min(P, 0.5 s)` in the
   preview) through `render_stages`' own code into a pool scratch whose output is discarded — the events of
   the lane are positioned at `head − P` by their own `hard_seek` (they pre-roll themselves from there), so
   the stage's input is what continuous play would have given;
3. the key branch of a keyed compressor pre-rolls over the same window on its key lane;
4. a stage with no lane sound in that window skips (cost zero, the common case).

Stages in dependency order (the sidechain order), as `render_stages` runs them. A stage whose key lane is
another stage's output reads that stage's pre-rolled output (the order guarantees it was pre-rolled first).

## D2. Where it runs and what it costs

On the audio render thread, at the seek, like event pre-rolls (invariant 6: never in the callback; the
device thread waits for the first chunk as it does after any seek). Its misses count in `warmup_misses`,
not underruns. A trace record of its own (a new `Ev` kind, or an aux of the existing seek record if one exists) carries the frames and the time, so the
owner's trace shows what a seek cost. On release the measured audio render runs at ~10 % of real time on a
dense edit (item 301): a 4 s tail is ~0.4 s at worst, usually less (one lane, not the whole mix).

## D3. The export range

An export of `[a, b)` seeks to `a` once: with D1 its audio equals the whole export's samples of `[a, b)`
byte for byte (the state at `a` equals continuous play's when the pre-roll covers the stage's full tail; the
events already pre-roll their full tail). That is the property t3363 pins; `GOLDEN_AUDIO` (from 0, nothing
rings before) is unchanged. With Q1 b the export still takes the full tail.

## D4. The kept mix

`cold_after_seek` returns 0 for a stage that pre-rolled its full tail (the start is warm), and keeps today's
answer for a stage whose pre-roll was cut (Q1 b). So after a seek the kept mix keeps the start again — the
seconds `cold_after_seek` excluded are kept from the first play.

## D5. Class walk: every chain that can start cold after a jump

| Chain | Pre-roll on a real seek | After this change |
|---|---|---|
| Event chain (`hard_seek`) | `max(tail, 100 ms)` + look-ahead | unchanged |
| Track stage | none | D1 |
| Sidechain key branch | none | D1 step 3 |
| Nested reel's stream (`move` from its block boundary minus its pre-roll) | yes | unchanged |
| Pitch's twin, warp | inside the event's `hard_seek` | unchanged |
| Kept mix leaving a range (kept-mix-settle) | settle through replayed frames | unchanged |
| Loop wrap | a seam, no pre-roll | unchanged (a stage rings across the wrap) |
| Video stream stages (history walks) | a history walk | not audio; unchanged |
| Datamosh / VHS / ISP audio halves (paired effects) | their event chains | unchanged |

## D6. Tests

t3361 (a seek into a track reverb's tail rings as continuous play: the samples after the seek equal the
continuous render's), t3362 (a seek where the lane is silent before costs no pre-roll: no lane read before
head), t3363 (an export of `[a, b)` with `a` inside a track reverb's tail equals the whole export's `[a, b)`
byte for byte; GOLDEN_AUDIO unchanged), t3364 (a keyed compressor's key branch pre-rolls: ducking at the
seek point equals continuous play's), t3365 (the kept mix keeps the start after a seek: `cold_after_seek` 0
after a full pre-roll), t3366 (a rate sign flip into a stage's tail pre-rolls; a loop wrap stays a seam: no
pre-roll counted over three laps — t166's rule for stages).
