---
id: F-RES-011
title: Memory and large projects: who evicts and by what rule
status: candidate
priority: B
cost: M
category: resilience/performance
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-cache, kt-engine, kt-app, kt-gpu]
invariants: [4, 5, 8, 9, 12]
ui_rules: [12, 19]
round: null
point: null
tests: []
depends_on: []
trackers: [mlt, shotcut, kde-bugzilla, kde-invent, pitivi, olive, otio]
checked: 2026-09-08
---

# F-RES-011 — Memory and large projects: who evicts and by what rule

## Essence

A class that is not visible on a demo project and that arrives in the third hour
of editing: the program takes up more and more, the machine goes into swap, and from
then on it is not the editor that slows down but the whole system. The editor sees one symptom
("everything froze") and cannot tell apart three different causes: whether memory is leaking,
whether it ran out once on 4K, or whether the cache is simply too small for this material.
The subject of the note is not "is there a leak" but the eviction rule, and whether
the user can see that exactly this limit was hit.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Dynamic RAM Preview max (MB) | a single number in the video settings: how much RAM to give to RAM preview. When it fills up, the preview simply stops. The DECODE cache is not controlled by this number and is shown nowhere [NOT VERIFIED] |
| Adobe Premiere | 26 | Preferences → Memory + Media Cache | the limit is set by SUBTRACTION: "how many GB to reserve for other applications", plus a switch "Optimize rendering for: Performance / Memory". A number in megabytes exists only for the DISK cache [NOT VERIFIED] |
| DaVinci Resolve | 21 | Preferences → Memory and GPU | the only one of the five that names TWO ceilings separately: system memory and GPU memory (plus the Fusion cache). The render cache is on disk and is visible as a coloured strip under the ruler [NOT VERIFIED] |
| Final Cut Pro | 12 | none | there are no memory settings at all; all management comes down to deleting the library's generated files. The bet is that the OS will sort it out itself [NOT VERIFIED] |
| After Effects | 26 | RAM Preview + "RAM reserved for other applications" | the closest analogue to ours: the GREEN bar under the timeline is literally the coverage of the RAM cache, and Purge → All Memory resets it by hand. But the limit itself is again set by subtraction, not as a number [NOT VERIFIED] |

What all five have in common: **the RAM ceiling is expressed either by subtraction ("leave it to others") or
not expressed at all**; only VEGAS (and only for RAM preview) and Resolve name it as a positive number. Coverage of the RAM cache is shown by exactly one —
After Effects.

Kdenlive, Shotcut, Pitivi, Olive and the MLT engine do not appear here as comparison rows: they are
sources of the count, not of the mechanism (`_editors.md`, section "Open-source software"). Their
design is analysed below as an explanation of where the count comes from.

## The trick behind it

**First: a video editor has no mode "do not cache".** Playback is physically impossible without look-ahead decoding,
so the question is never "how much memory to occupy" — it is "by which rule to release it".
MLT (the engine of Kdenlive and Shotcut) has such a rule, and it reads "never":
`mlt_pool` grows to its peak and never returns memory to the OS, so closing the
producers does not reduce the footprint, and `mlt_pool_purge()` does not help
(mlt #695, the reporter's own diagnosis, task closed as completed). One level higher this
is already visible as behaviour: a clip that has been read is not released from memory, the project size runs into the
amount of RAM, and the working workaround is to save the project and reopen it to force the program to let go of
memory (kde-invent #970). That is, the role of LRU there is played by a restart. Precisely for this
reason, complaints that "changing the cache setting changes nothing" do not occur in the trackers at all:
there is nothing to configure — the cache has no limit, no rule, no name.

**Second: in our case the direction of dependency is the reverse of the generally accepted one.** Not "how many
frames to pull in — that much memory we will occupy", but "how much memory is allocated —
that many frames fit". Three independent consumers are computed by ONE formula,
"not more than half of the video budget": the length of the decode span
(`span_key_budget`, `src/libs/kt-engine/src/worker.rs`), the look-ahead window of
the prefetch (`video_lookahead_ticks`, `src/libs/kt-engine/src/player.rs`) and
the size of the export batch (`export_chunk_frames`,
`src/libs/kt-engine/src/export.rs`). A consequence that is absent from every list of features: **our look-ahead window is a resolution function**. With
an unchanged budget of 512 MB it equals 9.7 s at 640×360, 1.07 s at 1080p and
0.27 s at 4K (arithmetic: a 4K frame in RGBA8 is 33.2 MB, 16 of them fit in the budget,
the window is half of that). This is literally the "in memory
range from the cursor" requested in kde-invent #1012, only not set by hand but computed.

**Third: the cost of a small cache for us shows up not as slowness but as a WRONG RESULT, and this has already happened twice.**
Items 116 and 195 of `docs/porting-notes.md`: a batch of export frames did not fit into the budget, LRU
evicted frames BEFORE composition — and the finished file came out black with a few
surviving frames (a measurement at 720p: 102 missing layers out of 120). The user notices slowness;
a black frame in a rendered file — does not. Therefore for us the cache limit is not a "performance setting" but part of
correctness: where others with a small cache get "slow", we got "wrong".

## Pros

- Invariant 4 removes a whole class of discrepancies: the "warmth" bar in the UI is
  `BurstCache::coverage()` of the same cache, not a separate bookkeeping.
  Eviction punches a hole in the bar exactly over its own `span` (tests 8, 28), so
  the state "warm on screen, not in memory" is structurally impossible.
- The audio and video budgets are SEPARATE, and `evict_over_budget` evicts from the
  pool that is exceeded: a video storm cannot knock out the sound, so memory overflow
  does not turn into silence (invariant 6).
- `quality` is part of the key: changing the preview quality does not "double" the same frame
  silently; it is visible as a separate set of keys within the same budget.
- Pin (the head of the loop region) is stronger than the budget, but acts only on entries that
  already exist — the single declared exception to the eviction rule, not a silent one.
- Every major consumer has a hard ceiling, and it is named by a number:
  512 MB video + 64 MB audio (cache), 192 MB (export), 256/128/64 MB VRAM
  (`UPLOAD_BUDGET`/`TRANSIENT_BUDGET`/`READBACK_BUDGET` in
  `src/libs/kt-gpu/src/engine.rs`), 384 thumbnails, 800 log lines, 200
  notices, 80 undo steps.
- Occupancy is computed FROM the real limit (`cache.max_video_bytes()`), not from a constant
  duplicated by hand — in v1 such a constant was off by a factor of 384.
- Two thresholds instead of silence: "eviction spike: N/s — the cache is cramped" (>500/s) and
  "N cache entries dropped before first read (prefetch-thrash)" — the latter fires on ANY non-zero delta.

## Cons

- **The limit cannot be configured from anywhere.** `Player::new`
  (`src/libs/kt-engine/src/player.rs:189`) takes `CacheConfig::default()`;
  there is no environment variable, no menu item, no settings file anywhere in `src/`.
  The warning "the cache is cramped" names the diagnosis and does not name the action —
  this is a direct violation of rule 19 of the UI checklist.
- Both warnings live in the performance panel, which has to be opened in advance — the same
  channel defect as "ui stall" in F-RES-010.
- Occupancy is shown only for the VIDEO pool. The audio pool (64 MB) and the three
  VRAM budgets of kt-gpu are shown nowhere and are not counted anywhere.
- Post-FX memoization (`BurstKind::PostFxVideo`) shares the video budget with pre-FX
  frames, but under invariant 4 it is invisible to coverage. "90 % occupancy" next to
  empty bars is correct behaviour that is not explained by anything.
- **Export is in-process (invariant 9), and that costs memory.** Its 192 MB
  plus a SECOND headless GPU engine (`export_video_gpu`,
  `src/apps/kt-app/src/app.rs:5215`) add up on top of the editor's memory.
  In Kdenlive rendering is a separate melt process, and the user can close the
  editor without killing the render (kde-invent #1424 asks for exactly this). For us this
  is impossible BY CONSTRUCTION, and it is a conscious price of invariant 9.
- Not everything in memory is cache, and the rest has no ceiling: the packet index of
  each source is held as a `Vec<i64>` in RAM
  (`src/libs/kt-media/src/ffmpeg.rs`, `packet_index`), a MediaWorker with an
  open decoder is started for EVERY media of the project unconditionally
  (`Player::new`, analysed in F-RES-010), and undo keeps 80 FULL clones of
  `Project` (`MAX_UNDO`, `src/libs/kt-model/src/model.rs:11`).
- The worker's negative caches (`failed_video`/`failed_audio`,
  `src/libs/kt-engine/src/worker.rs:194`) are the only sets without a ceiling in the decode path. Thumbnails do have such
  a ceiling, and it is named (`MAX_FAILED = 4096`, `src/apps/kt-app/src/thumbs.rs:115`).

## What users say

SIGNAL: memory and leaks — 18 tasks in mlt, `memory|leak|\bram\b|out of memory`; a share of 3.0% against 0.5% in kde-bugzilla

The full breakdown across the six exported trackers: mlt 18/601 = 3.0 %,
shotcut 20/1368 = 1.5 %, kde-invent 19/2217 = 0.9 %, kde-bugzilla
33/6346 = 0.5 %, pitivi 6/2672 = 0.2 %, olive 1/405 = 0.2 %, otio 0/680.

**The main point of these numbers is not the size of the topic but WHERE it sits.** First place
goes to mlt, which is not an editor at all: it is the engine on which both Kdenlive and
Shotcut stand. Their consumers of it have a lower share (1.5 % and
0.9/0.5 %); an editor with ITS OWN engine (olive) has 0.2 %; a format library
(otio) has zero. The engine-versus-home-made-engine gap is fifteenfold. The pain about memory lives not in the interface and not in the editing model, but in the layer
that holds the frames, and is inherited downward by everyone who stands on it. For us this
is a direct pointer to where to look: kt-cache and kt-engine, not kt-app.

**A caveat about the filter, without which the number would grow fourfold.** The filter declared at the input contained `oom` without word boundaries, and it catches `zoom` and `room`
exactly as `ram` without boundaries catches `frame`, `program` and `parameter`
(this second error has already been corrected in the lexicon of `_trackers.md` on 08.09.2026).
Checked: `memory|leak|\bram\b|oom|out of memory|swap` gives 139 / 53 / 64
for kde-bugzilla / kde-invent / pitivi, the corrected
`memory|leak|\bram\b|out of memory` — 33 / 19 / 6, that is, an overstatement of
4.2 / 2.8 / 10.7 times. `\bswap\b` across the whole export gives exactly one task,
and that one is about swapping values with the mouse wheel. All numbers below are by the
corrected filter.

**Leak versus a one-off peak — the distinction for which the note was opened.**

- Leak (grows over time):
  `leak|memory (is )?not (released|freed)|release ram|memory grows|persist in memory`
  — 53 tasks, a share of 1.7 % in mlt against 0.3 % in kde-bugzilla.
- One-off peak (opened something big — not enough):
  `out of memory|insufficient memory|(all|full).{0,10}\bram\b|\bram\b.{0,12}(usage|saturation|hog)|eat.{0,12}(\bram\b|gb of ram)|fills ram`
  — 14 tasks, a share of 0.2 % in mlt against 0.1 % in kde-bugzilla.

**The leak outweighs the peak almost 4 : 1.** That is, people complain not that it was not enough for 4K, but that
it stopped being enough after an hour. And this class lives long not because it hangs open, but because it is closed
unfixed: in kde-bugzilla (the only tracker in the export that gives the reason for closing) the share of GENUINE fixes among closed memory-topic tasks is 32 %,
against 47 % on average across the tracker, and WORKSFORME (9 tasks) is more than
FIXED (7). Among open tasks in the "leak" subtopic the median age is 2.7 years,
the maximum is 9.6: mlt #271 "About the release of memory after the closed
project" has been open since 23.11.2017 and is still open.

**Visibility of the RAM cache is ABSENT ENTIRELY among the sources of the count.** The filter `cache` gives 30
tasks across six trackers (0.4 % pitivi, 0.2 % kde-bugzilla, mlt and olive,
0.1 % kde-invent and shotcut), and on reading the headings one by one **not one is
about the cache in memory that the user sees and can limit**: every single one is
about the DISK directory (where to move it, what to clear it with, why it did not
clear, what is included in its overview) or about the internal code cache (the key by
PTS, the thumbnail image size). There is nothing for the user to observe there and nothing
to turn, so the only language of description available to him is "ate all
memory", and the only tool available is a restart.

> The problem is that after closing all producers, the process memory usage
> does not go back down.
> — mlt #695 "Deallocating memory from `mlt_pool`", 06.04.2021,
> https://github.com/mltframework/mlt/issues/695

> Repeat to create a project, close the project, the memory will continue to
> rise until the shutcut program crashes.
> — mlt #271, open since 23.11.2017 and still open,
> https://github.com/mltframework/mlt/issues/271

> It seems that whenever Kdenlive reads a video clip, it does not release
> that clip from memory.
> — kde-invent #970, 02.03.2021,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/970

> As far as I can tell, Kdenlive insists on loading every clip that it knows
> about into memory. Due to the high bitrate of some of the footage out
> there, memory is consumed pretty quickly.
> — kde-invent #1012 "Suggestion: "In memory area/range" on the timeline.",
> 31.03.2021, https://invent.kde.org/multimedia/kdenlive/-/work_items/1012

> If you work at least 15 minutes more by reviewing any fragments, adjusting
> the clips, changing the fades, then the RAM will end completely and SWAP
> will start to be consumed and after that everything will start to slow down
> terribly both Kdenlive and other applications.
> — kde-invent #736 "Pretty easy to eat 8 GB of RAM", 26.06.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/736

> I could close kdenlive and the render would continue however then I would
> loose access to the progress bar.
> — kde-invent #1424 "Release ram while rendering project", open since
> 07.05.2022 and still open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1424

> This memory usage is excessive. This project is not that large... and even
> then I still had heaps of memory left when the crash happened.
> — pitivi #623, 28.08.2009 (in the body — month-by-month growth: 380 MB at start,
> 1.8 GiB at 95 % of the render),
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/623

Verbatim quotes from kde-bugzilla are absent from this note for a technical reason, not
out of laziness: the REST export `/rest/bug` does not return the task description; in the dump the
`body` field is empty for all 6346 records. The headings are the author's own text and are quoted
as they are: "Memory leak when rendering anything longer than 5-10 minutes in 4K"
(#501578, 16.03.2025, UNCONFIRMED, https://bugs.kde.org/show_bug.cgi?id=501578)
and "Performance: Memory leak in GBs when removing files from project bin /
timeline - it doesn't actually free memory" (#516409, 20.02.2026, UNCONFIRMED,
https://bugs.kde.org/show_bug.cgi?id=516409).

Separately worth noting is kde-invent #1599 "Warn user during rendering when system is low
on memory" (15.12.2022, closed): a request for exactly the genre of warning
that we already have — only ours is about OUR cache and carries a number, while they asked
about the system.

## How it fits Kinotrain

ADDRESS: `src/libs/kt-cache/src/lib.rs` — the class "what was read is not released, the project ran into the amount of RAM" (kde-invent #970, #1012) is impossible for us: the cache has hard byte budgets, an LRU with pin and an interval-set of coverage that is updated by the same operations as the cache itself. The class "the cache is cramped, and there is nothing to enlarge it with" is POSSIBLE and is not caught by anything: `src/libs/kt-engine/src/player.rs:189` takes `CacheConfig::default()` (512 MB video + 64 MB audio), and nowhere in `src/` is there a single place where this number can be changed — even though `src/apps/kt-app/src/app.rs:5039` already prints "the cache is cramped" to the user.

### What already exists

- `src/libs/kt-cache/src/lib.rs` — the whole mechanism: `CacheConfig
  {max_audio_bytes, max_video_bytes}`, `Entry {payload, recency, pinned,
  had_hit, span}`, LRU over `BTreeMap<recency, key>`, `evict_over_budget`
  (goes from the oldest, skips pinned, evicts FROM THE EXCEEDED pool),
  the coverage `IntervalSet` with `insert_range`/`remove_range`/`query`.
  `epoch` is bumped on every insertion and eviction — the UI redraws
  the bars only when something changes.
- The counters are honest and separate: `CacheStats {audio_hits, audio_misses,
  video_hits, video_misses, evictions, unused_evictions}`. `unused_evictions`
  grows both on eviction and on OVERWRITE of an entry before its first read — a port of
  `hadHit` from v1, covered by test 37 (`t37_unused_evictions_counted_separately`).
- `src/apps/kt-app/src/app.rs` (`draw_diagnostics`, ~5077) — occupancy
  of the video cache is computed as `video_bytes() / max_video_bytes()`, the bar
  turns red above 90 %; next to it "no-read drops total: N". Threshold entries
  in the log are there too (~5038): `eviction_spike` at >500 evictions/s and
  `prefetch_thrash` on any non-zero delta of `unused_evictions`. The chart
  `perf.churn` draws three series: evictions/s, misses/s, no-read drops/s.
- The sizes of all "batches" are subordinated to the budget, not the other way round: `span_key_budget`
  (`worker.rs:327`), `video_lookahead_ticks` (`player.rs:959`),
  `export_chunk_frames` (`export.rs:66`) — everywhere "not more than half
  the budget". The reason is recorded in item 19c of porting-notes: a span must not be able
  to evict itself.
- Prefetch does not warm what will not sound: `lookahead_windows` folds the look-ahead window into the loop region (item 167, tests 167/167b) — a direct answer to
  prefetch-thrash.
- Post-FX memoization lives in the shared video budget and is invisible to coverage
  (test 65), and in EXPORT it is switched off entirely (`FramePlan.memoize_postfx`,
  item 195) — precisely because there it only evicted the not-yet-used frames of neighbouring tracks.
- VRAM is also under a ceiling: `TexturePool`/`UploadCache`/`ReadbackCache`
  (`src/libs/kt-gpu/src/pool.rs`) — three LRUs with budgets of 128/256/64 MB,
  "caches are the only holders of textures between frames".
- Everything that accumulates in the UI is bounded: the log 800 lines, notices 200, thumbnails
  `MAX_READY = 384`, the thumbnail negative cache `MAX_FAILED = 4096`, the thumbnail-worker
  sources `MAX_OPEN_SOURCES = 4`.

### What is missing

- **The cache limit is not exposed anywhere, yet a warning about it is already shown.** "The cache is cramped" without a place
  to enlarge it is a message to which one cannot react (rule 19).
- **4K is not covered by a regression test.** Test 116
  (`t116_export_has_no_black_frames_at_large_quality`,
  `src/libs/kt-engine/tests/frame_order.rs:184`) runs a matrix of
  320×180 … 1920×1080 × 1–5 layers. The arithmetic beyond its boundary: at 4K with seven
  simultaneous layers the batch degenerates into a single frame, which needs 232 MB
  in a 201 MB budget — and the assertion will let this through because of the caveat `|| n == 1`.
  At 1080p the same does not happen for any number of layers. That is, the class of item 116
  (a black render because of eviction before composition) returns at 4K at roughly
  3.5× fewer layers than at 1080p, and the test matrix does not reach there.
- **The `None` branch in `span_key_budget` (`worker.rs:334`) bypasses the budget**:
  it returns 64 keys at FULL resolution of the source, that is 2.1 GB at 4K into
  a 512 MB cache in one pass. Today it is unreachable (the player always sends
  `Some(q)`), but this assumption has no guard, neither by a test nor by a type.
- **There is no upper limit on the number of open sources.** `Player::new`
  starts a thread and an open decoder for every media of the project; thumbnails have a limit
  and it is named by a number, the player does not. This hole in itself belongs to
  F-RES-010 (there it is about responsiveness); here it is about the fact that each source brings
  its packet index into RAM along with it.
- **Occupancy is shown for one pool out of five** (the video cache; the audio cache and the three
  VRAM budgets — no), and nowhere is a total figure shown against which the user could check
  what they see in the task manager.
- The worker's negative caches without a ceiling (see "Cons") are the only place in the decode path
  where the structure grows monotonically over a session.

### What gets in the way

`CacheConfig` is an ordinary field of `BurstCache`, filled in `new`, and the cache itself
is handed to the workers as an `Arc` and created once in `Player::new`. There is no way to change
the limit on a live cache: recreating it means losing everything warm and orphaning the workers.
So either the setting applies from the next project opening (cheap, but looks like "does not work"), or
`config` becomes atomic and `evict_over_budget` reads it on every insertion —
that is a hot path under the same mutex.

The second obstacle is that there is nowhere to store the setting. The cache limit is a MACHINE property, and it has no place
in `.otio`: the same project travels between a machine with 4 GB and a machine with
64 GB. This is exactly the conclusion of point 2 of the first wave ("there is no user storage outside
the project", the catalogue's README) — the same one that F-AUT-004,
F-TXT-002, F-EXP-001 and F-UI-002 run into.

### Which invariants it touches

Invariant 4 (the cache is the single source of truth about coverage) — CONFIRMED and
is the reason the note has a subject at all: occupancy and the bars are read from one structure. Invariants 5 and 8 explain why
scrolling and zooming do not grow memory (they do not spawn decodes at all), and 12 — why
showing occupancy must not cost frames (the bars are throttled during playback).
Invariant 9 (in-process export) is the only one that works AGAINST here: it forbids moving the render into a separate process, that is, it forbids
the decision used by Kdenlive and its users. The UI checklist rules: 12 (background work does not interfere) and 19 (a dialog must say where
it can be changed later) — the second is literally violated today.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages of the round |
| Where the main work is | kt-app (machine setting + warning channel), kt-cache (limit taken from configuration instead of `default()`), kt-engine (t116 matrix up to 4K, guard of the `None` branch) |
| Project format | does NOT touch it and must not: the limit is per machine, it has no place in `.otio` |
| Tests that will have to be written | extend t116 to 3840×2160 and 6–8 layers (today the caveat `\|\| n == 1` hides a batch that does not fit); `span_key_budget(None, …)` must respect the budget; harness: the "cache is cramped" warning carries the address where the limit is changed |
| Depends on | user storage outside the project (conclusion 2 of the first wave), F-HLP-013 (log and bell), F-RES-010 (warning channel and the limit on the number of sources) |
| Breaks compatibility | no |

## Verdict

Take it, priority B. The engine half is closed more strongly than in any of the
sources of the count: we have a limit, an eviction rule, a counter of entries
dropped before their first read, and occupancy computed from the REAL limit — whereas
all 30 "cache" tasks across six trackers turned out to be about the disk directory or about the
internal code cache, and there is not a single one about the observable cache in memory,
because there is nothing to observe. And the topic sits in the ENGINE (mlt 3.0 % against 0.2 %
for an editor with its own engine) — that is, exactly in the layer that is described for us by invariant 4 and closed by tests. Not A because the user gets nothing from
this today except a diagnosis.

The open part is exactly one, and it is the reverse side of our own feature:
**the warning "the cache is cramped" is useless as long as the limit cannot be raised from the
interface.** Visible occupancy turns "the program is slow" into "the cache is small"
and thereby moves the diagnosis from the developer to the user — but moving the diagnosis without moving the lever
makes it a mockery. The second thing to close before a complaint, not after: the t116 matrix does not reach 4K, and it is precisely there that the
batch arithmetic again hits the budget — the "black render" class from item 116
is not cancelled, it has been pushed to a resolution that we do not check.

## Sources

- mlt #695 — https://github.com/mltframework/mlt/issues/695
  (task of 06.04.2021, accessed 2026-09-08)
- mlt #271 — https://github.com/mltframework/mlt/issues/271
  (task of 23.11.2017, open, accessed 2026-09-08)
- mlt #899 "Huge memory consumption of mlt-7.14.0 runs oom with 08:40
  minutes project on 64+8 GB laptop" —
  https://github.com/mltframework/mlt/issues/899 (10.04.2023), #1268 "4k
  video edit need large memory?" (08.07.2026), #66/#1202 (`[mlt_pool] out of
  memory`, pool corruption) — accessed 2026-09-08
- shotcut #483 "closing a project does not release all memory" (05.01.2018,
  open), #977, #880, #1908 —
  https://github.com/mltframework/shotcut/issues/483 and onward by number,
  accessed 2026-09-08
- kde-invent #970 — https://invent.kde.org/multimedia/kdenlive/-/work_items/970
  (task of 02.03.2021, accessed 2026-09-08)
- kde-invent #1012 — https://invent.kde.org/multimedia/kdenlive/-/work_items/1012
  (task of 31.03.2021, accessed 2026-09-08)
- kde-invent #736 — https://invent.kde.org/multimedia/kdenlive/-/work_items/736
  (task of 26.06.2020, accessed 2026-09-08)
- kde-invent #1424 — https://invent.kde.org/multimedia/kdenlive/-/work_items/1424
  (task of 07.05.2022, open, accessed 2026-09-08)
- kde-invent #1599, #392, #215, #1973 — same, accessed 2026-09-08
- pitivi #623 — https://gitlab.gnome.org/GNOME/pitivi/-/work_items/623
  (task of 28.08.2009, accessed 2026-09-08)
- pitivi #2459 "Memory not released after rendering" (open since 29.04.2020),
  #2673 — same, accessed 2026-09-08
- kde-bugzilla #501578, #516409, #478871, #479684, #506718, #425806, #396637
  (the last one — WONTFIX) — https://bugs.kde.org/show_bug.cgi?id=501578 and
  onward by number, accessed 2026-09-08. There are no task descriptions in the export:
  `/rest/bug` does not return them, so the headings are quoted.
- The count is reproducible:
  `python3 src/scripts/tracker-rank.py --query '<filter>'`. As of the date of
  writing, SIX registry trackers were exported in the cache — kde-bugzilla,
  kde-invent, pitivi, mlt, shotcut, olive and otio (the last gives zero on this topic); the other five (flowblade, openshot, losslesscut, blender-vse,
  ffmpeg-trac) were still downloading, and no shares for them are given here. The export arrived
  in the middle of the work: the numbers for kde-bugzilla/kde-invent/pitivi were taken
  twice and did not change; mlt and shotcut were added afterwards.
- The rows for the five core editors — from vendor documentation, not checked live
  (marked `[NOT VERIFIED]` in the table).
- Our code was read directly: `src/libs/kt-cache/src/lib.rs` in full,
  `src/libs/kt-engine/src/{player,worker,export}.rs`,
  `src/libs/kt-gpu/src/{pool,engine}.rs`,
  `src/apps/kt-app/src/{app,thumbs}.rs`,
  `src/libs/kt-model/src/model.rs`, `src/libs/kt-media/src/{lib,ffmpeg}.rs`,
  `src/libs/kt-engine/tests/frame_order.rs`; items 19, 37, 113, 116, 167,
  195 of `docs/porting-notes.md`.
