# Audio render — why it is this way

## What hurt and for whom

The old engine crackled on slowed&reverb for six independent reasons: the warp computed at the source's frequency (44.1 kHz → a burst skipped every ~230 ms), stateful FX without locks were pulled by four threads (a poisoned delay line), the resampler duplicated the last sample every 1024 frames (a 46.9 Hz crackle), every parameter edit zeroed the buffers, tails were cut at the event boundary, and any UI hiccup recreated the FX nodes. The owner came with this in the very first round, and a regression test was filed for each item (t1–t7).

The second wave of pain was playback in a loop and reverses: "the computer is noisy", "underruns are growing", "the picture freezes", "the sound from another event came from somewhere", "are two reverses really THAT heavy", "a region inside the start of event 21 — underruns are growing". One complaint was sent twice: the first pass fixed symptoms, because there was nothing to look at in the model — hence `kt-cli probe`, which is run FIRST on a complaint like "sound from another event".

The third — "the music drowns out the voice" (the most frequent live complaint about any editor in the reconnaissance), "I delivered and learned about the loudness from the venue", and a request for signed speed: "reverse playback in slowed", "I should be able to set 0.05 or −0.03".

## Decisions and why

- **The whole path is f32 48 kHz/2ch; the boundaries are floored to frames once.** The only way in which FX and the mix do not know about the file, and tick↔sample is exact (capability `time-model`). The BS.1770 coefficients are written out exactly for 48 kHz — a rare case where an invariant coincided with the standard to the letter.
- **FX state belongs to one thread, without mutexes; prefetch warms only the pre-FX source.** The chain's output is a streaming automaton: the tail depends on the entire history, so a kept output is a RECORDING of one play, not a function of the place. The owner chose to keep it anyway (cached-range-plays, kkt-26.11.07b, "вариант А"): a covered range keeps the sound it was first heard with, as a prerender does in VEGAS, and plays with no decode and no effect run. What that costs and how it stays right: the key is the scene hash of the events heard in the burst (an edit drops exactly what it determines), linear play and loop laps are kept apart, and leaving the store is a seek (the FX state did not run while the store played). Measured on his project: 911 of 950 bursts of a 20 s loop came from the store.
- **Chain diff by clip id and structure fingerprint; parameters are smoothed; buffers are cleared only in `reset`.** Otherwise a model edit would tear the reverb tail. The fingerprint knows only the kinds of slots, so the store MUST also convey the GEOMETRY (entry point, length, handles) — and it conveys it with a seam, not a seek.
- **Two kinds of break.** A real seek (the player does `reset`) — a pre-roll into the void; a position break during continuous output — a seam, where the resampler and the reverb live on. A pre-roll on every lap of the loop cost ≈1.25 s per chain and was the cause of "the computer is noisy"; misses of the pre-roll are not audible, so they are counted separately.
- **Underruns in frames.** Frames are comparable to the length of the event and the output delay; bursts are not comparable to anything. Starvation of the callback is shown to the user as milliseconds of silence.
- **Equal-power weights — the previous formula byte for byte; everything else is on top of it.** `GOLDEN_AUDIO` rests on it: transition curves, fades, overlapping transitions, reels — everything is arranged so that for back-to-back events with no options the weight is identically 1.0 and the reference does not move. An overlap without a transition (equal-power for two, 1/N for 3+) was reinstated as a legitimate state by the owner's direct request ("turn it on by default"); there is no exact sample-by-sample compensation for a varying 1/N, so when importing 3+ overlaps the loudness is not compensated — the owner's decision.
- **Reverse — "warp, then mirror; the axis is consumed amount."** A mirror BEFORE the warp would drive the speed curve backwards and desynchronize A/V; an axis with the resampler's margin would pull the sound off by 63 % of the event's length. One function computes the axis, and the same axis stands in the burst order.
- **Signed speed: the modulus goes into the resampler, the sign goes into the address map.** Rubato reads a virtual stream and knows nothing about direction; the run positions accumulate as integers, otherwise there would be a click exactly at the reversal point. The owner's decision: minus is the Vegas velocity envelope (applied to the material BEFORE the entry point), the sound stretches down to |rate| = 0.01, silence only at zero, and the tail rings on. A constant −1 stays on the flat path, so it equals the "Reverse" flag with a shift EXACTLY, not within a tolerance.
- **Order == read — one window function.** Beyond the window the source returns silence without an underrun, so a mismatch between the order and the read is a permanent miss that cannot be seen. Raw reverse transport ("like Vegas", linear and without warp) received its own order.
- **The price of a place: sweeping the track + binary search; a chunk past `gate_end` is skipped.** Three rounds in a row the symptom was explained by the build profile and the length of the region; the cause was found when they began to CUT: a measurement without FX gave the same price, and a measurement with distant events gave linear growth. Skipping the tail is byte-identical by construction: the gate was zeroing everything by itself anyway.
- **PDC — look-ahead, not delay.** The output is aligned with the input, there is no delay for the engine, the playhead and crossfades do not move; the price is that the source window is extended forward. "Warming" and "sounding" are separated: La is included in the warm-up, but not in the tail.
- **Ducking — a snapshot.** The result is visible, can be edited by hand, is undone with a single undo, and does not make the sound of the event a function of another track. The cost is stated in the dialog: if the music changed — call it again.
- **Loudness in f64, in one pass, without a master gain, without a "normalize" button.** A second pass for LUFS would mean a second full decode (an hour-long file — minutes). The master gain lives only in the player and does not get into the export — it is a CONTROL loudness; the preview number and the file's number match by construction. The target (−23/−14/−27) is chosen by a person.
- **Sync — by the peak pyramid, by transients, with refusal.** The data is already in memory for the waveforms and already on the unified 48 kHz grid; for different microphones the envelope shapes do not look alike, while the transients are shared. The command only moves: a recorder longer than the camera, and trimming it, is an error of the magnet, not of the synchronization.

## Rejected and why

- **Caching the audio FX output** — rejected until 2026-11-07 (a streaming automaton: the bytes depend on the read history); taken by the owner's decision with the read history named (cached-range-plays). Still rejected: a byte-exact hand-over of the FX state where the store ends — `Warp` holds rubato's resampler, which cannot be copied, and a node's `fork` makes a second owner of one stream; canonical blocks that reset live play too — a pitch steps where two grain grids meet.
- **Bypassing the warp in the tail for speed within the same round** — touches the byte path under `GOLDEN_AUDIO`; it was done as separate work (skipping a chunk past the gate), once it became clear that it is byte-identical by construction.
- **A live sidechain** — a second input on the effect, buses, a format change (XL); the practical benefit is covered by the envelope.
- **Output delay as the implementation of look-ahead** — the sound would drift from the picture by the window, and the tests would not see it.
- **Muting below 0.25 / "minus = a second input on the Reverse button"** — rejected by the owner: stretch down to 0.01, minus is a speed envelope with a reversal in the middle of the event.
- **A "normalize to −23" button** — useless to everyone except broadcasters, and it requires re-encoding an already recorded file.
- **f32 loudness accumulators "for the sake of speed"** — 0.37 LU of drift over ten minutes against a tolerance of 0.1; the path is offline, speed is not an argument.
- **Sync "by eye" at low confidence and a clamp to zero** — a pinned value looks synchronized; a shift with a flanger is worse than a refusal.
- **Drift compensation in sync** — requires retiming the video; pretending that a button cures it is worse than not having a button.
- **Refining the sync to the sample via MediaWorker** — 480 ticks = 1/8 of a frame, which is enough; decoding would bypass the playback queue.
- **Rendering a nested reel backwards** — a third result that belongs to no one: the inner reverb must sound forward inside the reel (capability `compositions`).

## Non-obvious details

- **A rebuild's swap costs what the edit changed** (cached-range-plays, t1935 red under a loaded
  workspace run). The audio render thread took the new renderer in by `sync_fx` over every event and
  asked each reel placement's pre-roll, which walks the reel's events: 1.2 M chain-tail walks for a
  reel of 3000 placed 400 times, 244–425 ms against a 341 ms ring. `sync_fx_since` skips events whose
  fingerprint is unchanged, the pre-roll is a field computed at build, the old renderer is dropped
  elsewhere: 0.6 ms. Builds never run on that thread; one asked on a pause holds the next start.

- The axis of the mirror of a reversed event is `kt_fx::chain_source_used` (`src/libs/kt-fx/src/audio/mod.rs`), not the `.end` of the window `chain_source_window_ext`: the end includes `WARP_SOURCE_MARGIN_FRAMES` = 8192 frames of resampler margin. Both quantities live separately in `SrcWindow`; the same axis MUST stand in the mirror branch of `wanted_audio_bursts_for` (`src/libs/kt-engine/src/audio.rs`).
- `FxStateStore::sync_clip` (`src/libs/kt-engine/src/fxstore.rs`) in the reuse branch calls `ClipProcessor::set_src_in` (`src/libs/kt-fx/src/audio/mod.rs`) — the anchor is reassigned by a seam. `reset()` does not help here: `hard_seek` and `seam` compute the position from the same field. There, `lookahead_frames()` is also checked against the static `chain_lookahead`, and `opts()` against the project settings — a mismatch rebuilds the processor.
- `ClipProcessor::read` (`src/libs/kt-fx/src/audio/mod.rs`) distinguishes breaks by `next_local == i64::MIN`: only the state after `reset()` looks like that. Any other jump is a `seam`: `Warp::seek_source` (`src/libs/kt-fx/src/audio/warp.rs`) discards the ready resampler buffer, but not the sinc state and not the reverb. A seam costs ≈0.3 ms per chain — so the price of the place depends on the length of the region, and it must be measured in the repeat mode of `kt-cli probe … --time`, not by a linear run.
- `PullKind::{Output, Warmup}` (`src/libs/kt-fx/src/lib.rs`) is the role of an access to the source; `Warmup` misses go to `RenderStats.warmup_misses`, not to `underruns` (`src/libs/kt-engine/src/audio.rs`). `MIN_PREROLL_FRAMES` = 4800 is there too — all look-ahead windows (limiter ≤ 960, pitch 4608) are smaller than it, otherwise after a seek the ring of the look-ahead node would be empty.
- `overlap_weight` (`src/libs/kt-engine/src/audio.rs`) takes the "first two" from the set of covering events in ASCENDING index order — `GOLDEN_AUDIO` rests on this order (f32 addition is not associative). The sets are built by `LaneCover` there; the maps lie in an array indexed by track number: a hash per frame cost 40 % on small projects. The `overlaps` flag closes the path for an event that intersects no other.
- `ClipProcessor::produce` (`src/libs/kt-fx/src/audio/mod.rs`) skips a chunk lying entirely beyond `gate_end = clip_len + tail_ext`, without the resampler and without the read. The warp state "lags" in this case, and that is safe: one can return to the material only by a `local` jump, and `read` catches it and reassigns the source by a seam or a seek.
- PDC: `produce` keeps two positions of the chunk — the output `q` and the input `p = q + La` (`src/libs/kt-fx/src/audio/mod.rs`); the source, the warp, and the tail gate live in the INPUT domain, and slot node k receives `q + ahead[k]`. The accepted price: with a live speed curve the warp map is sampled at the input position — the curve's phase is shifted by La (≤ 20 ms), otherwise the consumption would diverge from `source_offset`. A `LayerMix` on top of a look-ahead node delays the dry side by a ring over the "eaten" window; with a zero delta there is no ring.
- The look-ahead formula of a kind lives in one home (`nodes::limiter_la_frames`, `PITCH_LOOKAHEAD_FRAMES` in `src/libs/kt-fx/src/audio/nodes.rs`) and is called both by the node and by the static `chain_lookahead`; the OLA accumulator of the pitch shifter MUST hold a chunk of 1024 PLUS the full synthesis window of 2048 — on a ring of 2048 the result depended on the slicing of the buffers.
- Signed speed: `Warp::new`/`set_rate` (`src/libs/kt-fx/src/audio/warp.rs`) take `rate.abs()`, the address is given by `chain_addr_knots` (`src/libs/kt-fx/src/audio/mod.rs`) as runs of constant direction, and `fill_source_signed` (`src/libs/kt-engine/src/audio.rs`) reads along them. The hold (|rate| < `RATE_AUDIO_FLOOR` = 0.01, `src/libs/kt-model`) is decided PER CHUNK by the same midpoint rule as the automation targets; it MUST NOT be accepted per frame — the resampler cursor would not survive. A rejection by rubato on a change of ratio is counted (`Warp::rejected`); before, it was swallowed by `let _ =`, and the curve silently got stuck.
- `has_warp` looks at the MODULUS of the speed (`src/libs/kt-engine/src/audio.rs`): a constant −1 gives no stretch and stays on the flat path — which is why t616 compares BYTES.
- The raw path `render_frames_raw` plays everything through `render_plain_clip` (`src/libs/kt-engine/src/audio.rs`), and its order is `wanted_audio_bursts_raw`; the Playback prefetch with `dir < 0` goes through it. Over an embedding, the raw path MUST wrap the fetcher: the reel's address in an ordinary fetcher is silence plus underruns.
- Audio attaches the transition windows via `reel.transitions()` in `AudioRenderer::build_reel` (`src/libs/kt-engine/src/audio.rs`), not via the project root: otherwise a transition inside a nested reel would silently not crossfade. The window of an overlapping transition is marked `fade_*_excl`, and `overlap_at` mutes the built-in equal-power — otherwise there is double fading.
- For reversed FX events the transition handles are disabled (`head_ext`/`tail_ext` = 0 in `render_fx_clip`, `src/libs/kt-engine/src/audio.rs`): the mirror anchor of the source is tied to the end of the window, and extension would shift the body. The fade weights still apply.
- The reel's sub-mix: `NestStream` (`src/libs/kt-engine/src/fxstore.rs`) is keyed by the instance's `ClipId`; the producer in `src/libs/kt-engine/src/audio.rs` has exactly three branches (in the ring / catch up forward / canonical move from the block boundary minus the pre-roll); a fourth branch is a new kind of nondeterminism. The gate of the move is decided by the NON-MONOTONICITY of the read (reverse or signed speed), not by the warp. The order and the read are one `nest_read_window`.
- `LoudnessMeter` (`src/libs/kt-fx/src/audio/loudness.rs`) keeps the whole chain in f64, not "this particular sum": guarding it separately would mean guarding forever. The ordinary `Biquad` in f32 remains in the effects — there it sounds, rather than measures. The true peak is computed by a polyphase sum only around the samples comparable to the largest; without the gate an hour-long file cost 4.4 s on every opening of the project (`build_observed`, `src/libs/kt-media/src/index.rs`).
- The export feeds the meter with the finished chunk buffer in `render_raw_audio` (`src/libs/kt-engine/src/export.rs`); the master gain exists only in `Player` (`src/libs/kt-engine/src/player.rs`), and t407 guards against a field with such a meaning appearing unnoticed in `ProjectSettings`.
- Ducking: `duck_spans` (`src/libs/kt-engine/src/ducking.rs`) renders a clone of the project with a single track, RMS with a window of 960 / step of 480; the window is not broken at a chunk boundary; pauses < 300 ms are merged, spans < 150 ms are discarded — otherwise the curve "hiccups" between words. `duck_curve_commands` (`src/libs/kt-model/src/ops.rs`) places the keys in ticks of the MATERIAL via `clip_material_at` and REPLACES the existing `db` curve rather than layering onto it.
- Sync: `offset_from_pyramids` (`src/libs/kt-media/src/sync.rs`) compares `a[i]` with `b[i + lag]`; the first block is compared with itself (otherwise a false "onset" on the zero block gives a correlation of 0.98 for two unrelated noises); the starting level of the pyramid is chosen BY LENGTH, not as "the topmost" — at the top there are one or two blocks, and a full search would never happen. `sync_by_audio_commands` (`src/libs/kt-model/src/ops.rs`) goes only through `clip_timeline_at`/`clip_material_at` — its own arithmetic would drift on a reversal.
- The `loudness` and `sync_offset` tables in the sidecar (`src/libs/kt-media/src/index.rs`) store integers ×100 (the round-trip is byte-exact) and were added without bumping `SCHEMA_VERSION` (a bump would wipe the batch index, peaks, and thumbnails of all media); for `sync_offset` the key is a PAIR, and a uniform `DELETE ... WHERE media_id` does not clear it (capability `media-index`).
- Invariant 5 in the wording of R.49: the offline path (ducking, index, export) opens ITS OWN short-lived source under three conditions — it is shared with no one, it promises no tempo, and its task is finite and cancellable (`src/apps/kt-app/src/analysis.rs`, `bg.rs`). The playback cache is warm only around the cursor; there is no envelope of a whole track there.
- **At an armed loop wrap the INPUT wraps first** (pitch-loop-silence, 2026-10-29). The chain's
  input runs `La` ahead of the output, so the last `La` frames of a lap read material PAST the
  region end, which never sounds in a loop: the limiter played it after the wrap (t909, 239 frames
  per wrap). `ClipProcessor::set_loop_span` folds the input and every node's content position into
  the region and re-seats the source there; `read` takes the output's jump as expected. The
  renderer sets the span for an event that covers the region end and folds a track stage's input
  window the same way (t911). Rejected: re-seating on the output jump and refilling the ring in
  place — the nodes before the look-ahead node would process that window twice.
- **A split of a pitched event is one stream** (pitch-event-edges, 2026-11-01). Even with full rings,
  two pitch streams have independent grain grids, WSOLA offsets and vocoder phases, so a cut between
  them steps (0.46 against 0.004 inside the uncut event). `run_predecessors` marks the event a clip
  continues (same track, abutting, the next material frame, same chain, both read across edges, no
  reverse/overlap/transition, no tail); at its frame 0 on continuous play `FxStateStore::hand_on`
  swaps the processors — the predecessor's warm one goes on, rebased. Ascending clip order in every
  burst makes it slicing-independent; fifteen pieces equal the uncut event byte for byte (t1105). A
  tail-less context chain entered at its first frame or after it ended is a seek, not a seam (its
  ring holds where it stopped), and an event that starts inside a loop region does not fold its input
  (it is not what sounds after the wrap). Rejected: one processor per run keyed by its first clip (an
  edit that moves a cut would destroy states, invariant 3); cloning the state (two owners of one
  stream).
- **A split under "rings on" rings only at its end** (pitch-tail-choice, 2026-11-03). A ringing
  predecessor hands nothing on (it still sounds when the next piece starts), and the next piece's
  context already carries the echo of the join's material — so `split_continued` (the run relation
  with the tail allowed) tells the left piece not to ring (`ClipProcessor::set_ring_end`); the split
  is the "cut" split up to its last piece's end (t1332).
- **A loop lap of a chain with a tail before the pitch starts as a seek there** (loop-pitch-tail-dip,
  2026-11-03). pitch-event-edges D4 kept tail chains on the seam at a re-entry (R.28: a reverb rings
  through the circle), and the pitch's look-ahead ring then held where the previous lap stopped:
  0.000 of the play from there for ≈ 75 ms with "rings on", ≈ 0.45 with past-the-end material with
  "cut". For a chain without the context the seam is exact (a linear tail plus the new input); with an
  edge-reading pitch the ring is the window it reads AHEAD, not storage of the past. So a lap entry
  (`ClipProcessor::lap_entry`, the re-entry rule of `fresh_entry` with a tail) seeks, and the processor
  that played the previous lap, standing in its tail, moves to `FxStateStore`'s ringing laps
  (`start_lap`) and is read on for the same output frames, summed at the tail zone's weight — as two
  copies of the event one after another sound. At most `MAX_LAPS` = 4 ring at once; a retired lap is
  the spare the next one starts with (no allocation in a steady loop); a seek drops them. Rejected: the
  input-first fold for an event whose TAIL covers the region end (the new lap's first grains would
  read the old tail instead of the material before the in-point, and nothing happens when the tail
  rang out before the wrap); a seek alone (cuts a ringing feedback echo at every wrap). t1440, t1441.

## Deferred

- Bypassing the resampler in the warp tail beyond skipping a chunk past the gate (one item of spending `problems1` remains).
- A live EBU mode of the meter (M/S/I/LRA/dBTP in the panel) and "normalize to target" with a choice of target.
- Sync by timecode (`tmcd` in kt-ffmpeg, a field in `MediaInfo`) and drift compensation for long duplicates.
- **Revision of R.63: ducking SNAPSHOT → LIVE SIDECHAIN in the next change** (the owner's decision). "Ducking works in the program, for example via two compressors" — that is, a compressor with a control input, not an envelope recorded in advance. In R.63 only what does not touch the engine was done: the name according to the industry, a section in the audio track's window instead of its own window, the instrument's parameters in `user_store`; the curve snapshot remained a snapshot, and the caveat "if the voice was changed — call it again" stayed with it. The next change is `sidechain-ducking-compressor` (note `docs/features/audio/mixing/buses-and-track-fx.md`).
- A master limiter (the master fader exists only in the preview).
- Formant correction and a pitch engine per event instead of a project setting.
- The extended warp mode (a curve below 0.25) limits the anchor to 2× — the anti-aliasing cut on fast sections is coarser; this was accepted deliberately.
- **R.62 B, tracks:** envelopes of a TRACK (a curve along the timeline tick, not along the event's material) — its key lives on a different axis, and introducing it together with the stage would mean changing two contracts at once. Buses and sends: a track today has exactly one output — the mix. Skipping the DECODE of a muted track: the prefetch order stays whole deliberately (invariant 4 — "order == read"), the cost is as in Vegas, and unmuting MUST sound without a cold start.

## A build costs what the project holds, not its square (builder-cost-guard, 2026-11-08)

- **What hurt**: `AudioRenderer::new` took 393–488 ms on the owner's project (1 ms on the heavy showcase): `sign_knots` rebuilt a shaped speed curve's segment list per sampled point (found-defects row 151). Every edit pays it, in every consumer. Now 11–12 ms (release, relinked copy), the sound byte-identical.
- **Decision**: a COUNT guards growth, never a clock (t1671): `kt_time::cost` (a thread-local walk meter) and the bytes allocated on the building thread, at generated projects ×1/×4/×16 along six axes (t2062–t2067). A clock would pass on a fast machine and say nothing about the next quadratic.
- **Non-obvious**: a transition's engine events are found by `PieceEdges` (the first event of an original id at an edge — what the scan answered); a reel's revision is computed once per build (`revisions_scope`).
- **t1935's "a rebuild outlasts the ring"** now comes from `hold_builds_for_tests`: its scene was slow only through the per-placement hash.

The whole walk, the axes and the numbers: `openspec/changes/archive/2026-11-08-builder-cost-guard/design.md`.

## An edit derives what it touched (edit-without-rebuild, 2026-11-09)

- The player's builder keeps each event's derived audio clip and video layer from the last build
  (`incremental::BuildMemo`). The key hashes the model event's Debug text (the input of `fp`) plus
  every context value the builder passes in. A hit is a clone, never a recomputation, so a complete
  key makes the memo invisible to the bytes. t2004 holds a whole incremental build's state text, its
  sound and its scene hashes equal to a fresh build's across eight edits and an undo.
- A per-event revision kept by the model was rejected: every command would have to bump the right
  events, and a missed bump is a stale event no test sees; a content hash cannot miss.
- `AudioRenderer` is shared by the render thread, prefetch and the probe, so it holds no render-time
  `Cell`: `src_ends` is a `OnceLock`. The armed loop region is a thread-local keyed by the renderer, so
  a nested reel's renderer and another thread see `None`, as when each had its own renderer.
- `fp` is hashed through a `fmt::Write` adapter, with no `String` and the same digest: the kept final
  mix keeps its keys.
- Each reel instance is resolved once per build (`incremental::resolve` inside `revisions_scope`):
  the audio build, the video build, the decode step and the generator sources each resolved every reel
  (row 155).
- What an edit still walks off the UI thread: the root's resolve (7.6 ms on haski) and the hash of
  every event; an incremental build of an edited version is 20.5 ms against 31.6 fresh (row 192).
