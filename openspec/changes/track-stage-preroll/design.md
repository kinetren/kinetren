# Design: track-stage-preroll

> Revised 2026-11-22 against the tree (`873934f7`) and the owner's Q1 a. D1's "dependency order" premise was
> wrong and is replaced (D1, step 3); the pre-roll is driven by a flag on the FX store (D1a); the start's
> prefetch takes the pre-roll window (D2).

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

On a real seek to `head`, each stage `st` whose lane has an event heard in `[head − P_st, head)` (the same test
as `cold_after_seek`, per stage):

1. its processor and its key branches are already reset by `FxStateStore::reset_all` (`fxstore.rs:517–544`);
2. the lane is summed over `[head − P_st, head)` by `sum_lane` (`audio.rs:1487`) in 1024-frame pool chunks and
   run through the stage's processor into a discarded scratch. Each lane event is positioned at the
   pre-roll's start by its own lazy `hard_seek` (`kt-fx/src/audio/mod.rs:1047`: `max(tail, 100 ms)` + its
   look-ahead, rendered as `PullKind::Warmup`). It then continues into `head` with no break, so the stage's
   input is what continuous play would have given;
3. **a keyed stage's key branches pre-roll too.** `render_stages` runs the stages in plain vector order, and a
   key branch never reads another stage's output: it runs its OWN copy of the key track's chain
   (`store.key_mut(st.id)` → `sub.track_mut(key_id)`, `audio.rs:1460–1462`). So for a keyed stage:
   - `P_st = max(tail_st + la_st, tail_key + la_key + slot_delay)`;
   - the pre-roll sums the key lane over the same window through that branch copy, then calls
     `process_keyed` as the play does;
4. a stage with no lane sound in its window skips (cost zero, the common case).

## D1a. What drives it: a flag on the FX store

There is no event-side seek hook to piggyback on, so the trigger is `FxStateStore.stages_warm: bool`. It is
false in a derived `Default`, so a fresh export store is cold.
- `reset_all` clears it.
- `render_stages` checks it on entry. When it is false, the stages pre-roll at `range.start` (D1), and then
  the flag is set. The pre-roll runs inside `render_chunk`, so its misses reach the mix's clean test.
- `NestStream::rewind` (`fxstore.rs:229`) sets it right after its reset. A reel's move already pre-rolls its
  stages through `compute_preroll_bursts` (`audio.rs:1143–1162`, which includes `stage_reach`); t2376's bytes
  stay.
- `MixPlay::warm` (`mix.rs:709, 717`) sets it after its `reset_all`, since its settle replay already covers
  the stages.
- The pause meter (`player.rs:841`) sets it too. The meter shows the level of what plays, and a pre-roll
  per meter tick would cost work at rest.

## D2. Where it runs and what it costs

On the audio render thread, at the seek, like event pre-rolls (invariant 6: never in the callback; the
clock stands before the first frame, so sync is unaffected). Its misses count in `warmup_misses`, not
underruns. A trace record of its own, `kt_time::trace::play::STAGE_PREROLL` beside `play::SEEK`
(`kt-time/src/trace.rs:481`: b = frames, c = µs), carries the frames and the time, so the owner's trace shows
what a seek cost.

**The start's prefetch takes the pre-roll window.** The start's hold ("A start waits for its sound",
`START_AUDIO_GRACE` 500 ms, `player.rs:45`; the hold at `:2793–2822`) waits for the cached audio of
`lookahead_windows(render_head, …)`, which runs forward from the head. The window `[head − P, head)` is never
prefetched, so on a cold cache the pre-roll would read silence and count misses. When moving forward, the
hold adds `(head − P_max)..head` to its windows, `P_max` being the largest `P_st` of the stages that ring
there. Reverse play renders dry with no stages, so only a flip back to forward pre-rolls. On release the measured audio render runs at ~10 % of real time on a
dense edit (item 301): a 4 s tail is ~0.4 s at worst, usually less (one lane, not the whole mix).

## D3. The export range

An export of `[a, b)` seeks to `a` once: with D1 its audio equals the whole export's samples of `[a, b)`
byte for byte (the state at `a` equals continuous play's when the pre-roll covers the stage's full tail; the
events already pre-roll their full tail). That is the property t3363 pins; `GOLDEN_AUDIO` (from 0, nothing
rings before) is unchanged. With Q1 b the export still takes the full tail.

## D4. The kept mix

With the owner's Q1 a there is no cut pre-roll, so `cold_after_seek` (`audio.rs:1047–1055`) is removed and
`MixPlay::fx_reset` (`mix.rs:751–754`) sets `keep_from = head`. After a seek the kept mix keeps the start from
the first play. t2373 (`tests/mix_settle.rs`), which asserted the cold start, is rewritten to t3365's rule.

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

**Byte equality needs a chain that forgets within its stated tail.** A reverb's decay never reaches exact
zero, so the byte-equal tests (t3361, t3363) use a delay with no feedback (as t2376 does) or a reverb with an
exact cutoff. A test with a real reverb checks within a tolerance, and its scenario says so.

t3361 (a seek into a track delay's tail rings as continuous play: the samples after the seek equal the
continuous render's), t3362 (a seek where the lane is silent before costs no pre-roll: no lane read before
head), t3363 (an export of `[a, b)` with `a` inside a track reverb's tail equals the whole export's `[a, b)`
byte for byte; GOLDEN_AUDIO unchanged), t3364 (a keyed compressor's key branch pre-rolls: ducking at the
seek point equals continuous play's), t3365 (the kept mix keeps the start after a seek: `cold_after_seek` 0
after a full pre-roll), t3366 (a rate sign flip into a stage's tail pre-rolls; a loop wrap stays a seam: no
pre-roll counted over three laps — t166's rule for stages), t4103 (the start's prefetch covers the
pre-roll window: on a cold cache the pre-roll reads no silence and counts no miss), t4104 (the trace record
of a stage pre-roll and its `kt-cli trace summary` line).
