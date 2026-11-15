# Design: stream-round-trips

## Context

stream-front-on-device kept the front of a stream stage on the compute device. The rest of the stream
path still crossed the bus: Datamosh's walk read its own state back at every batch end and uploaded it
for the next one (rows 401, 621), the shown frame went through two passes on the processor (row 620),
a refresh I-frame and a region at a held position cost jobs of their own (rows 402, 405), and an export
with a stream stage ran serially (row 490). During the work the orchestrator moved the owner's
kkt-26.11.15 N2 in front of it — "the real-time preview became much slower than kt26.11.10", the piece
WITHOUT Datamosh worrying him most — and its mechanisms (rows 660–662) are part of this change.

Mechanism (the class): **a value computed on one side of the bus is brought to the other side only to
be brought back, or a walk does work proportional to something other than what it walks.**

## Decisions

### D1. The compute door keeps buffers; the memo has a live tier

`ComputeHost::{keeps, run_keeping(job, keep)}`: the named buffers stay on the device as `Held`
(kt-gpu wraps the job's own buffer, no copy; dropped, it goes back to the device's spares, which the
next job of its size takes before the pool). `StreamMemo::{put_live, get_live, holds}`: entries a
kernel keeps on the compute device, bounded on their own (`LIVE_BYTES`, 128 MB, an LRU among
themselves, under the ledger as compute memory), never evicted by the pictures. `drop_unpinned` keeps
it (it is not the LRU); `clear` empties it.

Why a tier and not the same LRU: at 1080 lines the pictures filled the 256 MB memo and evicted the
clean encoder's state; every frame then encoded its GOP again — the showcase's 1080-line Datamosh ran
795 encodes for 196 positions.

### D2. The walk goes on from the device

`Batch::keep` names buffers that stay; the walk keeps the decoder's picture and each side's clean
state (`HeldPic`, `HeldClean` — the packet only when a later position repeats it) at every batch end
and goes on from them; at its end they go into the live tier under the keys a memo entry would have.
The chain search prefers a held picture on the way (no upload of the memo's copy) and the memo's
picture at the frame asked (no readback). What comes back to the host: the frame shown, the
checkpoints, a region's clean picture. A host that keeps nothing reads the same values back and the
walk keeps them as transient memo entries, as before. `mosh::needs` asks `holds`. t2350 (batched ==
per-position) runs on the held path; t2700 holds the bytes moved; t2709 the tier; t2708 the door.

Rejected: binding a held buffer to a job without the device copy — the copy is a few microseconds a
plane on the device and keeps "a job's buffers are its own" (a step may write any buffer it binds).

### D3. A refresh's I-frame is kept beside the walk (row 402)

A refresh the walk encodes goes into the live tier under its key; a re-walk to it (the refresh is
self-contained) encodes nothing. t2703 (red without it: 1 encode).

### D4. A region at a held position is one job (row 405)

With a region, a walk whose last position did not encode the clean picture at `n` (a dropped I-frame
holds the reference) encodes it in its own batch; the blend finds it in the memo. t2704 (red without:
2 jobs).

### D5. The shown frame on the processor (row 620)

The codec's picture must reach the composite's device (the display's, the offline or an export's —
never the compute host's, design D1 of stream-front-on-device), so the readback stays. Its processor
part shrank: `Planes::to_rgba` runs row by row over slices (t2701 holds it to the per-pixel one), and at
the decode size `video_stream::run` hands the stage's RGBA as the layer — `fit_into` there is a copy to
the byte (t2702: each sample lands within an ulp of the pixel, its neighbour's weight times 255 far
below half a level), so it is skipped. A held layer source remains possible only where an export's
engine is built on the compute device: row 663.

### D6. A stream stage takes the pipelined export (row 490)

The decode thread pre-rolls the range's history (`stream_preroll`, bounded batches) before its first
chunk; the compose thread builds the plan of each frame in frame order and hands it to a worker
(`Job::Planned`), so the history is walked once, in order, and a frame never depends on whether the
memo still holds its position when a worker gets to it. `ExportOptions.serial` no longer flips on a
stream stage. t2707: one device and three workers, a range from before the stream and one inside its
history, chunks of four frames — the serial bytes.

Rejected: advancing the history on the compose thread and letting workers plan — a worker behind the
memo's bound would re-walk from a checkpoint and could need a source frame its chunk released.

### D7. The slow route shows its kept picture (row 622, verified)

The slow-chain device's composite was written into the display's present memo AND handed to the
interface as bytes, which uploaded it again into egui's texture (`tex.set`, read in `app.rs`). After
the keep the player now presents it from the memo (`present_kept`), the frame answers `on_display`,
and the interface shows the display target. t2151 reads the slow device's runs instead of
`on_display`; t2705 asserts the display path.

### D8. No far walk without a stage on screen (row 660 — kkt-26.11.15 N2, the plain piece)

From his traces (`kt-cli trace … during`, the same place played linearly in both runs, no stream
stage on screen): kt26.11.10 ran 10 prefetch passes a second; kt26.11.15 four passes of 26.5 s each,
the prefetch thread at 100 % of a core, the decode look-ahead never asked, 3507 incomplete composites,
V-fps 21.6 → 8.7. Inside a pass the time lay between the stream memo's sizing and the frames cache's,
with no record: `stream_far_frames` (gpu-throughput D3) divides the memo by the frame size of the
stage ON SCREEN, 0 there, so the divisor was 2 bytes and the walk 33.5 million frames of a loop that
looks each frame up. Playing, the state moves every pass, so the stall counter that bounds a far walk
was reset every pass. Now no stage on screen walks nothing far (a stage starting ahead is reached by the
near walk, as in kt26.11.10), and every look-ahead walk is at most 60 s of frames. Reproduced here on
the showcase with Datamosh (a pass of 36 s before, the longest 1.8 s after). t2710.

Introduced by `a528e2a3` (gpu-throughput 3-4, stage 11).

### D9. A compute job waits for its own submission (row 661)

His trace: a compute job's buffers came back 311 ms after its last dispatch ended (median; p95 1.3 s;
kt26.11.10: 6 ms). `run` mapped each read buffer and polled "the most recent submission", which is any
piece another thread put on the device after the job (the frame job of the compose thread, the front's
frames, an export's job). Now every read is mapped first and one wait names THIS job's submission index.

### D10. The frame on screen's history at the display's class (row 662, partly)

His trace: near-class Datamosh jobs cut into 6228 pieces waited 188 s for admission against 74 s of
their wall, while the device stood idle 56.6 s with nothing queued; kt26.11.10 ran each job as one
piece. The arbiter's rule (a lower class leaves the display its slack) is right for the look-ahead,
but the walk to the frame ON SCREEN is what the display itself waits for (Loading until it is walked).
`stream_advance_ahead` now walks the frame on screen at `now` when it runs at `near`, and the frames
after it at `near`. t2711. The look-ahead's pieces keep the rule; whether they still trail is for his
next trace's `trace gpu` admission share (row 662 stays open for it).

## Class walk — every read-back-then-upload in kt-gpu and kt-engine

| # | Path | Readback | Re-upload | Devices | Verdict |
|---|---|---|---|---|---|
| 0 | Front of a stream stage → codec | — | — | compute | none since stream-front-on-device |
| 1 | Codec walk state between batches and walks | `walk.rs` `flush` (recon, residual, vectors) | `walk.rs` `dev`, `upload_planes`, `upload_packet` | compute → compute | **fixed** (D1–D2): held; only the shown frame and the checkpoints come back |
| 2 | Refresh I-frame on a re-walk | — (re-encoded) | the source frame | compute | **fixed** (D3) |
| 3 | Region at a held position | `clean_at` job's readback | its source upload | compute | **fixed** (D4): in the walk's batch |
| 4 | Codec output → composite | `walk.rs` shown frame (8-bit planes) | `video_stream::run` → layer → `UploadCache` | compute → display / offline / export | required while the devices differ; processor part cut (D5); row 663 |
| 5 | Generators drawn by a compute shader | `generator.rs` `host.run(reads)` | the frame as a layer | compute → composite | required, as 4; row 663 |
| 6 | Preview slow chain | `player.rs` `execute_into` (offline) | display `keep` (`write_texture`); egui `tex.set` | offline → display | the egui upload **fixed** (D7); the keep is required (another device) |
| 7 | Loop fill | `loop_fill.rs` `compose_fill` | display `keep` | side → display | required (the side engine's point: off the display's queue) |
| 8 | Reel strips, posters, effect cells | `reel_preview.rs`, `fx_thumbs.rs` `execute_into` | CPU `fit_into` → egui; disk | side → UI | required (UI texture, disk tier) |
| 9 | Export | `execute_once` / `collect` | encoder (software or VAAPI transfer) | export → CPU | required |
| 10 | Display readback `read_target` | `engine.rs` `read_target` | — | display | tests and the harness only, no re-upload |
| 11 | Timestamps | `pass_time.rs`, compute stamps | — | — | read only |
| 12 | Nested reels | — | — | one device | none (`ChainSrc::Nest`) |

## Rows closed by verdict

- 135: residuals already travel at 16 bits a sample and pictures at 8 (stream-layout-per-build,
  gpu-pass-cost); since D2 a residual never comes back but for a region; t2706 bounds the readback
  per frame by the shown frame's 8-bit planes plus the checkpoints.
- 404: fixed by gpu-throughput D6 (a timestamp set per job in flight); t2437 holds it.
- 145, 173: measured (Cost). A paused 1080-line frame assembles; the first visit of a frame walks its
  history from its anchor — the broken decoder at `n` is a function of every position since it — at the
  device's speed, bounded on revisits by the checkpoints and now by the live tier.

## Cost

Release build, llvmpipe (the only adapter here), footage in `/dev/shm`, the machine shared with other
sessions (load 5 … 60; runs read at the lower loads).

Datamosh at 1080 lines (`showcase --heavy --mosh-behind --mosh-lines 1080`, `kt-cli stream-cost
… 11.2 13.2`):

| per lap of 50 frames | before | after |
|---|---|---|
| advance, a frame | 1041.8 ms | 432–533 ms |
| jobs' wall, a frame | 945.8 ms | 250–411 ms |
| clean encodes for 196 positions | 795 | 201–212 |
| uploaded, a frame | 104.6 MB | 23.0 MB (the decoded sources without a front) |
| read back, a frame | 19.9 MB | 11.8–12.6 MB (the shown frames) |
| positions a second | 3.8 | 7.4–12.3 |

The plain piece (kkt-26.11.15 N2) — his traces: kt26.11.10 21.6 V-fps, every composite waiting 42.4 ms
(p50) for the display's present; kt26.11.15 8.7 V-fps with the prefetch thread dead in its far walk,
the present wait 16.4 ms. With D8 the look-ahead is back and the present waits a third of
kt26.11.10's: the expected result is faster than kt26.11.10, to be read in his next trace. Here
(`kt-cli preview` of the showcase with Datamosh, two binaries started together on their own copies,
640×360): 23.2/23.4 V-fps kt26.11.10 against 22.9/22.4 — both at the llvmpipe ceiling of that piece;
at 1280×720 alone: 15.4/18.6 against 14.7/20.1, within the machine's noise. llvmpipe cannot show the
present-wait difference of his card.
