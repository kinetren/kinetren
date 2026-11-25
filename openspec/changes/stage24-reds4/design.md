# Design: stage24-reds4

## D1. The step comes back (843)

`plan_graph_in`: with `in_scale > 1`, walk the raw plan past barriers and point-wise runs (they may lead at
S×); what stands there must be a geometry run. If it is a stop (a snapshot, a graph step, a merge) or the
end, an identity run is put in front: the same fused program a zoom of 1.0001 runs (supersampling, alpha,
margins), so the picture is the limit of the zoom's own and not a bilinear of another look. A chain that
consumes its step plans as it did, byte for byte.

Rejected: asking the step per moment — the decode quality is a property of the clip (R.46: one key per
frame, one coverage bar, one order), and a step that moved with the curve would decode a clip twice.

What drew in a release build: nothing (`fits == false`, t633's rule). In a debug build: the panic.

`clip_head_zoom` skips `StreamStage::front`: those slots run on the stream's input at its codec size.

The cell of the report: Datamosh in lesson 2's picker, over `[Geometry 1.18, Blur]` (found by trying every
candidate of the saved lessons 2 and 4 at every place: 2 856 others ask no step they do not consume). The
row's "lesson 4, a Blur" was the neighbouring line of the log.

## D2. A panic in one item (843)

Walked: every service lane and every lock held across work.

| Place | Before | Now |
|---|---|---|
| `fx_thumbs` scene cells | the scene's guard `unwrap`ped: poisoned for the session; the cell neither ready nor failed, asked again each frame | `bg::caught`: the cell fails, the half-made scene is dropped |
| `fx_thumbs` sample cells | asked again each frame | fails |
| `thumbs` (a thumb, a poster of the bin, a probe) | asked again each frame | `WorkerState.at`: found at the next run, published as a failure |
| `welcome` posters | `busy` for ever: the close waited its whole bound | `caught`, the queue goes on |
| `index` | the rest of the project's files unindexed | `caught` per file, its error line |
| `listen` | the rest of the task's files unheard | `caught` per file |
| `reel_preview`, `analysis` | the job is gone, its key in the negative cache | as it was |
| `media-watch`, `render-ahead` | next poke, next kick | as it was |
| `sched` | a pump that panicked waited for a kick | run again, at most 8 times in a row (t3265) |
| the compose queue | `catch_unwind` already | as it was |
| kt-gpu engines | `holds::lock` takes a poisoned lock | as it was |

## D3. A lesson's copy (842)

`later` (found-defects 833's small files: tmp and rename, no fsync, read-your-write) carries a lesson's copy:
`keep_sandbox` on Next lesson, the fresh copy in `ask_open`, and the read. The flags are cleared by the
gesture, so nothing asks. A Ctrl+S of the copy still writing is waited out (`save_then`), or its older text
would land over the newer one. A lesson's copy is never the last project and never in Recent (t1076): a
power cut costs a lesson's practice edits.

Walked, who else waits for an fsync with nothing shown: "Save and continue" and Ctrl+S then New wait for the
PERSON's file, rightly; what they show meanwhile is found-defects 917.

Before: t2305 ("opens when the copy is written") holds as it was written — no question, lesson 2 opens, the
edit is in the copy.
