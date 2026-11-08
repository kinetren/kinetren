# Proposal: decode-request-storm

## Why

The owner, 2026-11-08, release build kt26.11.08-360b50e6 on his own project (`docs/ktprompts/kkt-26.11.08.txt`,
line 14, registry rows H6 and H8): "а loop-picture-throughput может связан с тяжелым эффектом? типо я поменял
там что-то и оно пошло считать? не? new2 папку там посмотри.. ну типо стало лучше конечно но все равно я думаю
что на моем компьютере всё что могло бы - работало пошустрее... гораздо.."

His journal (`fails/26.11.08/new2/load-report.txt`): at 06:49:13 he selected Datamosh in an event's chain; for
the next minute the journal shows, PAUSED, `decode_requests` ~1570 a second, `composites` 4, `decoded` 13–108,
and every second "prefetch thrash: N cache entries dropped before the first read" with N equal to `decoded`;
then four loops of 0.22 s in a row ended "V-fps 0.0, new frames 0", with ~20 000 requests a second.

Partly yes, the heavy effect: a stream stage decodes its history at the codec's size. But a queue that asks
1570 times a second for 50 frames delivered, all thrown away unread, is a defect whatever the effect costs.

## What Changes

- The stream stage's ORDER in the live preview names only the earliest frames its history lacks, as many as
  fit a quarter of the video cache (`StreamPacing`); the whole history was ordered at once.
- The prefetch thread ADVANCES the stream stages (the memo) over what the decoder brought, at the frame on
  screen and, while playing, ahead of it; a pass with a pending stage runs again even on a pause.
- A preview composite runs a stage only when its frame lacks at most two positions; a longer history is the
  prefetch thread's, so the UI thread never computes one (the composite took 9 s on his relinked project).
  While playing, a stage still behind shows the latest frame it holds (a late frame, as a late decode shows
  its last one); such a composite is not kept as the moment's own.
- The MediaWorker reads a run of keys BEHIND its cursor forward from its start, not one key per seek from
  its end; a key already queued, or in the span being decoded, is neither queued again nor counted as a
  request.
- The reel strip and the project poster pre-roll a stream stage in batches their small caches keep
  (`stream_preroll`, the export's, now public and sized by the cache it is given).
- `kt-cli pause-scan --from/--to` and its request/decoded counts per point.

## Impact

- `kt-engine` (worker, player, video, video_stream, export), `kt-plugin-api` (`StreamKernel::held`,
  `StreamMemo::bytes_cap`), `fx-video-codec` (`held`), `kt-app` (`reel_preview`), `kt-cli` (`pause-scan`).
- No format change, no new dependency. Export bytes unchanged: the pacing is the live preview's only
  (`VideoRenderer::stream_pacing` is `None` everywhere else), and the export's pre-roll batch is the same
  number (its cache's half budget). Invariants 4 and 5 hold: the order is a prefix of what the stage reads,
  in the order it reads it; a request still lives in the one worker's queue.
- Tests t2040–t2045.
