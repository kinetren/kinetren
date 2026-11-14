# Design: class-walk-revision

## The mechanism of the miss (the class)

A class walk enumerated the places that HAVE the mechanism (every thread, every tenant, every kernel)
and stopped there. The places that DEPEND on what the change moved were not enumerated: a caller that
waits for a result now delivered later, a consumer that reads a value now sized elsewhere, a second
copy of a constant, a path that bypasses the new scheduler/ledger/arbiter, a cache or memo keyed
without what now determines it, a flag one consumer does not honour, a tool that measures the old
behaviour. Their old assumption survived each change.

The revision walks those for each change of 2026-11-09 … 2026-11-11: grep of the change's functions,
types, constants, flags and trace kinds across `src/`, each hit read, compared with the change's own
list. Four read-only walks ran in parallel (subagents), every miss they returned was re-read in the
code before it was fixed or recorded; one claim was not confirmed and is not counted (a flush race in
kept-mix-settle's output thread: whether it is new could not be established).

## Decisions

- **D1. Fix what is the same defect, small and outside the parallel sessions' areas.**
  `scheduler-followups` (sched lanes, project open, loop background composing) and `gpu-throughput`
  (kt-gpu arbiter, rings, pacing, compute host dispatch) run in parallel: their misses are rows only.
  The run-prewarm fix touches `kt-gpu/src/engine.rs` `prewarm_point_runs` only (not the arbiter).
- **D2. One helper for the room.** `kt_time::mem::room()` = available + the growth the growing kinds
  hold above their bases (the quantity `growth_budget` halves). Both self-shrinking sizings use it;
  the ledger's grant stays the bound.
- **D3. The stream view is per renderer.** A flag on `VideoRenderer` set by `set_stream_view` and
  passed through `PlanCtx.stream_view` (root and track stages inherit it, a reel's sub-plan gets
  `false`). The memo keeps one view (the player's); nested reels and the strip/poster renderers read
  everything back, as before stream-history-on-device. A per-reel view in its own time is a possible
  later refinement, not needed for correctness.
- **D4. The close pauses the player first.** `on_exit` calls `Player::pause()` before it waits: the
  window is gone, nothing plays any more, and both waits it makes (autosave 5 s, posters 3 s) are on
  Background lanes the playing vote holds.
- **D5. A landed save compares the view state.** `Saving` carries the `UiState` it wrote; `dirty_ui`
  clears only when the state now equals it (`UiState: PartialEq`). `dirty_ui` setters that change no
  `UiState` field were cleared by the old synchronous save too, so nothing regresses.
- **D6. The reel envelope is a function** (`reel_envelope`) whose per-chunk progress asks
  `should_stop` (the gate wait) — the test drives it with a held `BgCtl` and no GPU.
- **D7. The preview's single click** waits `preview_click_delay_ms(key_clock)` = egui's widened
  double-click window (never under 300 ms), measured from `key_clock.frame_at`, as test-hygiene D3/D4
  measure every other window.
- **D8. Row 463** — t2230 frees one held worker, so the scheduler's pick (Visible before Background)
  is what the order shows; the rest are freed after. 40 of 40 stress iterations green.

## The table

Legend: ✔ fixed here (test); № a found-defects row; "known" a miss found before and already fixed.

| Change | Mechanism (one line) | Places checked (callers, consumers, siblings) | Misses |
|---|---|---|---|
| one-scheduler | background services moved onto `kt_time::sched` lanes; a playing player's vote pauses Visible/Background; Ctrl+S, autosave and media opens became async jobs answered later; parked threads wake on `Wake` | every `save()`/`save_then()` caller (toolbar ×2, SaveOverLock, Unsaved, `save_as`, lessons), `guard_unsaved`, `on_exit`, `Drop for KtApp`, `replace_with`; `take_media` users (`add_media_clip_at`, `place_media_at`, `sync_media_sources`, `poll_returned_sources`); every Player setter vs `Wake::notify`; every `set_playing` site; all lanes; every long job's checkpoints (`BgCtl::should_stop`) — analysis, reel frames, reel envelope, posters, pre-rolls; `StallWatch`; `autosave_due` | known: "Next lesson" (t2305), "Save and continue" (t2306), gesture after Ctrl+S (t2307), parked audio thread flagged a stall (t2303). ✔ a view edit during the write lost its asterisk (t2452); ✔ closing while playing waited out the autosave and poster jobs the vote held (t2453); ✔ the reel envelope ignored the vote (t2454); № 513 the insurance autosave is dead during playback; № 514 the strip/poster stream pre-roll checks cancel only |
| fused-pointwise-runs | a run of ≥2 point-wise stages runs as one generated pass; fused or parts chosen by a timing trial, relying on equal bytes | `PlanOp::PointRun` producers/consumers, `FxOp::passes`, `slot_units`, `bind_runs`/`run_pieces`, `PassLabel::Run`, prewarm reach (clips, tracks, morphers, reels), callers on the main, slow-route and export devices and `kt-cli fuse-cost`, picker cells and reel previews on the offline engine, `MAX_RUN_STAGES` (one source), keys of ReadbackCache / kept composites / reel disk tier | ✔ the prewarm passed a run as one key, skipped past 32 stages; the frame asks pieces (t2450); № 510 fused == parts is bounded, not enforced, on hardware while no kept/export key names the form |
| stream-layout-per-build | a structure constant per build (the mosh plan) compiled once per build and stream size; packed planes, device conversion; the preview memo sized by the loop and the machine | `StreamKernel` implementors (Datamosh only), `plan_cell`/reset in `link_stages`, `stream_memo()` users (`VideoRenderer::new` default, player, `stream-cost`), `set_stream_memo` callers (export, pipeline, gpushare), `stream_ahead_frames`, the two export memo literals | ✔ two copies of the export memo size → `EXPORT_STREAM_MEMO_BYTES`; ✔ `kt-cli stream-cost` sized without the checkpoints the player passes; № 518 the want counts the whole loop region at the size of the frame on screen; № 519 strip and poster jobs fill the preview's memo |
| gpu-arbiter | every GPU/compute submission admitted by class into a ring per queue with a bound before the display; long frames cut into measured pieces; a stop check for stream histories; event-driven present wait and export pause | every `queue.submit` in `src/` (export submit, offline execute, the cut, the present — not admitted by design, `read_target` test path, warm.rs, compute calibration/prepare/run); every class source (compose now/near, prefetch near, export offline + pipeline threads, media workers by priority, lanes via `sched`: fx-thumbs, thumbs, reel-preview, posters; render dialog; kt-cli untagged on displayless rings); ring registration (`GpuFxEngine`, `ComputeDevice`); stop scopes (compose, prefetch, reel/poster pre-roll) and their consumers (`mosh.rs`, `walk.rs`); export pause notify paths | № 511 the compute calibration submit is not admitted (gpu-throughput's file); rows 480–482, 486 already hold the ring/pacing/query misses |
| memory-ledger | caches sized by their own constants put under one ledger; growth granted from half of (available + held) in the order Kept → History → Frames; the present memo sized by the loop; exports claim the growth; preview decoders take the render's thread rule | every `mem::open` (kt-cache ×3, kt-gpu compute + pools ×4, stream memo), `Claim` holders (export, pipeline, render dialog), probe injection (kt-app, kt-cli), `PresentMemo`, ReadbackCache, UploadCache, the PRESENT/UPLOAD/TRANSIENT/READBACK constants, `BATCH_BYTES`, every `set_decode_threads` caller (app, kt-cli preview, export, render dialog), `growth_budget` | known: the frames cache on a constant (frames-cache-growth). ✔ the stream memo's want clamped by the bare available shrank as it filled (t2458); ✔ `kt-cli pause-scan` opened decoders with a thread per core (the D11 rule now shared with `preview`); № 517 the stream memo's LRU keeps nothing of a lap it cannot hold |
| test-hygiene | input judged by frame starts; every press a frame reads runs in order; egui's click windows widen by the busy frame; scenarios read consequences | `Controls::pressed`, every `key_pressed(` outside controls.rs (fields' Enter/Esc on focus loss — one per field, right), `num_presses` users, `KeyClock::widen_clicks`, the preview's deferred click, maskctl keys, the search window keys, sleep-based tests | ✔ the preview's single click waited a fixed 300 ms from when its frame ran (t2457); ✔ the search arrows read one press a frame (t2457); ✔ the mask pen took one point a frame (t2456); № 515 `thumbs.rs` "no thumbnail while playing" is a sleep-based negative proof |
| local-cost-queue | a per-element value constant over a larger unit, or a per-unit allocation, became a table, memo or kept scratch, bytes unchanged | Camera ISP flag bits and every `Node::PassAt` consumer, bind cache and staged writes at all frame entries, flush before every submit, the camera `ix` map vs linear lookups elsewhere (`param_idx`, tape `values_of`), `FrozenSinc` (only sinc builder), static window tables (WSOLA, tape FIR, mic NS, vocoder — row 359), the 128-byte uniform literal (predates) | known: the weakest record kept. № 520 the compute host builds a uniform and a bind group per dispatch (gpu-throughput's file); № 521 ReadbackCache's LRU is a linear scan per frame |
| stream-history-on-device | a Datamosh walk is one compute job per batch; only positions at/after the host's view come back; checkpoints pinned; `Piece::cut` submits and goes on | `walk.rs` stop/view/checkpoints, `ClipStream::with_view`/`shown_from`, every `set_stream_view` caller, every `StreamMemo` owner (global — every `VideoRenderer::new`, nested subs included; export's own; gpushare), pre-roll callers (export, reel preview, gpushare), `CHECKPOINT`/`MAX_BATCH`/`BATCH_BYTES`, `stream_checkpoints` | known: the ring wait called "intended" (rows 480–481). ✔ the view (an outer tick) applied in every renderer sharing the memo — reels, strips, posters read nothing back (t2451); № 512 `stream_checkpoints` skips nested renderers, so the memo's checkpoint tier is sized without them |
| kept-mix-settle | a kept mix chunk after a replay/seek/reset is produced by warmed effects or not kept; the cpal stream opens on Play on its own thread and closes 1 s after | every `fx_reset`/`reset_all` site in the render loop (seek, direction, loop flip, source come/go), `video_playhead_lap`/`display_extrapolation` while closed, every `AudioShared` reader, the flush writers, `Sink::fill`, `output_loop`, `CLOSE_AFTER_REST` (one copy), the stall bell | none confirmed. № 516 `kt-cli preview` models an always-open device, so the open-on-Play latency is unmeasured |
| frames-cache-growth | the decoded-frame cache asks the ledger: twice the look-ahead, or the loop + an eighth + the window; its cap read at use | every `max_video_bytes`/`CacheConfig` reader (worker, prefetch ×3, player + `set_stream_cap` incl. nests, export ×2, pipeline, reel preview ×2, app ×2), kt-cli (`probe --time`, `probe_prefetch` — row 440, streamcost, gpushare), one asker per kind, the sizing key | ✔ the loop count stopped at the bare available and the cache shrank itself after an edit (`room()`, same fix as memory-ledger's, t2458 holds the shared helper) |
| catch-up-shows-loading | a stage got a stand-in input (a held stream frame, a pair's other half, a partial offline composite), the later stages ran over it and the result was kept or shown as complete; now incomplete, "catching up: N" | held-frame symbols gone from src, scripts, catalogs, live scenarios; how the shortage travels (`stream_short` reset, sub-plans share it, player → compose queue → app); `show_kept`; the three keepers (`PresentMemo`, slow route `keep`, `note_kept`); app present; plan completeness (`resolve_plan`, pair side); export both paths; `fx_thumbs`; poster; reel strip; kt-cli `preview`, `pause-scan`, probe, gpushare; ReadbackCache | ✔ the strip never opened generator sources, so every generator cell went empty after D6 (t2455); ✔ the bench, `kt-cli preview` and `pause-scan` named no cause for a catching-up frame (t2459); № 522 an export writes a frame with a pair side missing (the layer dropped) instead of the retry D5 claimed; № 523 ReadbackCache keeps incomplete composites; № 524 strip rows with empty generator cells already on disk stay until the reel's revision moves; rows 460–462 stand |

Also fixed: found-defects 463 (t2230's race, test code only).

## Not in this change

Rows 510–524 (`docs/found-defects.md`) with their evidence; the owner's word on 513 (insurance
autosave while playing) and 522 (export policy for a missing pair side).
