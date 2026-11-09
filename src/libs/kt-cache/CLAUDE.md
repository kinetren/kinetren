# kt-cache

> The crate's contracts and mandatory tests — `openspec/specs/{media-index}/spec.md`; the reasons for the decisions — `design.md` next to it.

Cache of bursts + coverage. Without FFmpeg/audio hardware.

## Invariants
- The cache is the ONLY truth about coverage (invariant No. 4 of the root CLAUDE.md). No parallel "warmth" structures — the old ms buckets (defect g) lied in four ways.
- Quality is part of the key; the audio key has no quality. The decode scale step (R.46) changes NOTHING here: it enters through the quality itself (a clip frame at step 2 lies under the key 2×Quality). The consequence for the UI: the bars MUST ask for coverage by the clip's DECODE QUALITY, otherwise a warmed clip looks cold (t317).
- Coverage is an interval set, updated on insert/evict; a query is O(log n), without sampling.
- Pin (a loop region) excludes records from eviction, but not from coverage.
- **The content serial is unique in the process, not per cache** (t1607): a GPU engine outlives a
  cache and keys its textures and its frames by the serial alone. Never take it from a per-cache count.
- **An entry evicted and decoded again takes back its serial** (structured-telemetry, t2152): an
  unchanged source gives the same bytes for the same key (invariant 11), and the executor's kept
  composites are keyed by the serial. `Inner::past` remembers evicted serials (bounded);
  `invalidate_source` forgets a source's, and an overwrite of a present entry gets a new serial. A path
  that puts NEW bytes at an old address must invalidate the source first.
- Evictions go into the trace with the key's parts and the insert that pushed them out (`trace_evict`).
- **The cache reports to the memory ledger** (memory-ledger): two `Frames` accounts (video, audio) with
  the configured budgets as bases; every path that changes `video_bytes`/`audio_bytes` ends in
  `report`. Holds stay stronger than any cap.
- **A hold is a counted pin for a job that reads ahead of its use** (export-pipeline-determinism):
  `hold_video` answers "present" and holds in one step, `insert_video_held` holds with the insert, each
  hold needs its `release_video`. LRU's order is not the order of need: a read refreshes an entry that
  will never be needed again, and it outlives one decoded and not yet read. Never make a job's
  correctness rest on the budget being large enough.
- `refresh(keys)` marks present keys recent in the given order WITHOUT a read (no `had_hit`, no hit, no epoch): the prefetch passes its window far → near so the nearest frame outlives the rest (video-prefetch-thrash). `contains` stays a pure probe — refreshing in ascending order is exactly what inverted the LRU in item 19.
- **Thrash = dropped unread while STILL WANTED.** `refresh` numbers the passes; an entry carries the last pass that wanted it (or the one current at its insertion). Dropped unread at the current pass → `unused_evictions` (the panel's "prefetch thrash"); older → `abandoned_evictions` (a seek left it, the picture skipped it — waste, printed by `kt-cli probe`). Audio and a never-refreshed cache keep the old count (t37).
- The cache holds PRE-FX data; coverage bars are built ONLY from it. Audio post-FX is not in this cache: the final mix is kept by the audio render thread (`kt_engine::mix`, cached-range-plays), and the bars learn it from there. VIDEO post-FX (R.7 item 2) — a separate kind `PostFxVideo{quality, fx_hash}` (idx = timeline tick): pure memoisation of expensive chains; it is invisible to the bars, prefetch and pin, because their requests are kind-exact; it lives in the common video budget, and invalidate_source also removes it. Test 65.

## Tests
- Test 8: eviction shrinks coverage by exactly the evicted records; quality does not cross-satisfy.
- Test 14 (jointly with kt-engine): the pin of the head of a loop region.
- Test 1417: `refresh` keeps the window ahead of what was read. Test 1417b: an abandoned entry is not thrash.
- Test 37: no-read drops (`unused_evictions`) — eviction/overwriting of a record BEFORE its first read; records that were read (had_hit) do not get into the counter.
