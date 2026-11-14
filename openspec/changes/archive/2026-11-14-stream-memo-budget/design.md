# Design: stream-memo-budget

## The mechanisms (the classes)

1. **A bounded cache over a cycle longer than itself.** An LRU evicts exactly the entry the next lap
   reads first, so a loop one entry too long keeps nothing (row 517). The fix is an admission rule over
   the current lap, as the present memo has (memory-ledger D5).
2. **A want counted at the wrong grain.** A sizing that multiplies "moments of the region" by "the
   size of what is on screen now" asks for what the region does not hold (row 518) and changes when the
   playhead crosses an event of another size; a sizing that does not descend into reels misses what
   they keep (row 512). The fix counts each holder's own entries at its own size, through the tree.
3. **A shared default inherited by a job it was not sized for.** `VideoRenderer::new` gives the
   preview's memo; an offline job that keeps it fills a budget sized for the player's loop (row 519).
4. **A hold honoured by the outer loop only.** A job's frame loop waits at `should_stop`, but the work
   inside one frame asks a check that answers only the cancel (row 514); a job whose purpose is to run
   during playback was put on a lane the playback holds (row 513).
5. **A negative proof by a sleep** (row 515): "nothing happened in 300 ms" proves nothing under load.

## Decisions

- **D1. The stream memo keeps a part of a loop it cannot hold.** `keep_loop(true)` starts a fresh lap
  count (what an earlier loop kept becomes stale); `lap()` is the host's mark that the loop came round.
  An entry carries the lap of its last use. While a loop is kept, a new non-transient, non-pinned entry
  put into a memo full of entries the current or the last lap used enters a **window** (probation). The
  unpinned victim is, in order: the window's oldest past a quarter of the bound; a kept entry no recent
  lap used; the kept part's least recent (so the window grows into the kept part up to its quarter);
  the window's oldest; the new entry itself. Unlike the present memo, a refused entry would be lost to
  the composite that reads it right after the look-ahead computed it (the advance and the composite are
  two readers), so a window replaces refusal; its quarter matches the look-ahead's reach
  (`stream_ahead_frames` keeps `cap / 4`). Two laps of tolerance because at a lap's start every entry
  was last used in the previous lap. `keep_loop(false)` turns the window back into plain entries.
- **D2. The player marks a lap** when the moment shown goes back while playing inside a loop (a wrap or
  a seek back); a seek counts as a lap, which only ages the protection by one lap.
- **D3. The want is per stage.** `VideoRenderer::stream_want(from, to, q)` walks the region's moments
  once, through reels (`clip_inner_tick`, `clip_decode_quality`), collecting each stage's distinct
  positions (keyed by the renderer and the clip, so a reel's copy is its own stage), its picture (half
  its RGBA frame at `stream_quality`) and the checkpoints of its walk to its last position in the
  region. The memo wants `lap_bytes + lap_bytes/8 + 2·largest picture + checkpoint tiers + base`,
  within a quarter of the room — the same formula as before for one stage at the region's size.
- **D4. The player sizes once per (build, quality, loop).** Not per pass at the moment on screen: a
  stage of another size crossed while playing re-sizes nothing. Sizing now runs for every project, so
  a project that lost its stream stages gives the grant back (before, the memo kept it).
- **D5. Offline frame jobs own their memo.** `offline_renderer` builds the strip's and the poster's
  renderer with a memo of `EXPORT_STREAM_MEMO_BYTES` (an offline pass, as the export's). The view stays
  unset there (class-walk-revision D3).
- **D6. The pre-roll stops on a hold and waits between batches.** `preroll_under` runs the pre-roll
  under a stop check `cancelled || held`; a stage stops between history positions keeping what it
  computed; the job then waits at `should_stop` — outside any device job, so a waiting thread never
  holds a ring piece — and runs the pre-roll again from the memo. `BgCtl::is_cancelled` reads the token
  and the flag (the poster service's ctl carries only the token).
- **D7. The insurance autosave runs on an Answer-class lane** (`autosave-insurance`): the durability
  rule "the gate yields after two intervals" means the write happens while playing. It costs one
  serialization a minute on a pool worker, never the audio or the UI thread; Answer is the class of a
  save. A Background autosave posted on a pause and then held by a playback for another interval is
  posted on the insurance lane too: the body is taken by the first lane that starts it
  (`AutosaveBody`), the other finds it gone. The UI wakes for that move.
- **D8. t2238's proof is the scheduler's order.** After the request a job of a LOWER class (Offline) is
  posted and run to its end: a worker picking it passed the queued thumbnail lane (classes are picked in
  order); the Visible class's `started` counter did not move meanwhile. No clock.

## Class walk (what was checked, what was found)

**Memos and caches with an admission or a sizing rule**

| Holder | Sizing by the loop | Admission over a lap | Verdict |
|---|---|---|---|
| `StreamMemo` (preview) | was region × on-screen size, no reels | none | fixed: D1, D3 (rows 512, 517, 518) |
| `StreamMemo` (export, `EXPORT_STREAM_MEMO_BYTES`) | fixed; a linear pass shows every position once | not needed: no laps | ok |
| `PresentMemo` (kept composites) | loop moments × presented size — all composites are at that size | D5 rule | ok |
| `BurstCache` frames (`size_frames_cache`) | `loop_frame_bytes` sums the layers at their own decode sizes | audio loop head pinned; video LRU over a loop beyond its grant keeps nothing | **row 551** |
| `MixStore` (kept final mix) | fixed 64 MB (2.9 min), not sized by the loop | none: a loop longer than 2.9 min keeps nothing, each lap renders all | **row 550** |
| `ReadbackCache` | — | rows 521, 523 already | open elsewhere |
| thumbs / fx-thumbs LRU | count caps | never evicts what the last frames drew (t2237) | ok |
| reel preview LRU | count cap | never evicts what an event of the view waits for | ok |
| texture pools (Transient/Upload), compute pool | per-frame working sets | not loop-held | n/a |
| stream look-ahead reach `stream_ahead_frames` | `cap / 4` per two pictures | — | consistent with the window's quarter |
| `kt-cli stream-cost` | sized as the old player | — | fixed: sizes by `stream_want`, marks laps |

**Offline renderers that inherit the default (preview) memo** — every `VideoRenderer::new*` outside tests:

| Site | Verdict |
|---|---|
| `reel_preview.rs` strip and poster | fixed: `offline_renderer` (row 519) |
| `export.rs`, `export_pipeline.rs` | own memo already |
| `export.rs` `serial` probe, `export_pipeline.rs:775`, `kt-cli` build-cost timing | no frame composed |
| `player.rs` `preview_video` | is the preview |
| `kt-cli` `probe`, `probe_prefetch`, `probe_video_cost`, `pause-scan`, `preview` | model the preview: the shared memo is right |
| `kt-cli gpu-share` offline thread | fixed: own memo, as an offline job now has (its mosh thread had one) |
| `kt-cli stream-cost` | measures the preview's memo on purpose |

**Jobs under a pause vote** — every Visible/Background lane:

| Lane | Inner work between hold checks | Verdict |
|---|---|---|
| `autosave` | one write; its rule promises a write while playing | fixed: D7 (row 513) |
| `reel-preview` strip | frames: `should_stop` per frame; pre-roll inside a frame: cancel only | fixed: D6 (row 514); envelope per chunk (t2454) |
| `posters` (welcome) | `ctx.paused()` per pump; frame through `compose_offline_frame` | fixed by D6 |
| `thumbs`, `fx-thumbs` | `ctx.paused()` per pump, one picture per step | ok; t2238's proof fixed (row 515) |
| `index` | `ctx.checkpoint()` inside a file, between files | ok |
| `media-watch` | `ctx.cancelled()` per short step | ok |
| `analysis` | `should_stop` per step | ok |
| the player's far look-ahead (`with_class(Background)`) | the player's own work, its stop is `left()` | not under the vote by design |

## Rejected

- **Refusing a new stream entry outright** (the present memo's rule): the composite reads the entry the
  advance just computed, so a refusal makes it walk again from the nearest kept picture (D1).
- **Keying the lap by the memo's own clock** (the present memo's stamps): a stream position makes a
  kernel-dependent number of gets and puts, so a lap has no fixed length in uses; the host's mark is exact.
