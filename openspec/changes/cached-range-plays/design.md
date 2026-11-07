# Design: cached-range-plays

## The mechanism

**A range the bars show as covered is worked on again every time it plays.** "Covered" meant only
that its SOURCES are decoded. Between the decoded sources and what a person sees and hears there
are two more products, and neither was kept for the preview:

1. **The composite.** The `video-plan` spec promised a memo of the composed frame keyed by the plan
   key. It lived in `ReadbackCache`, on the readback path. preview-throughput D2 moved the preview
   to `present` (the display target, no readback), and `present` neither asked nor filled any memo:
   since then every frame of every lap of a covered range ran its whole chain on the device.
2. **The final mix.** Invariant 2 forbade keeping the output of audio effects ("a streaming
   automaton, a second lap sounds different"). So every burst of every lap ran the effects, the
   track stages and the producers of the nested reels.

The bars asked only the burst cache, so they showed the first product and promised the last.

## His run, measured (relinked copy, release build, this machine: Ryzen, llvmpipe)

| | before | after |
|---|---|---|
| `kt-cli preview haski 4643.95 4644.36 --size 640x360 --loop 20` V-fps | 13.6 (8 s) | 50.5 average over 20 s, the first lap included; 60 grid fps on later laps |
| device time per shown frame | 72 ms | 3.3 ms |
| composites in 20 s | one per shown frame | 48 (the first two laps), then 986 kept composites |
| audio bursts with effects in 20 s | all (≈940) | 39, then 911 from the kept mix |
| process CPU | 23 cores (llvmpipe) | 2.5 cores |

His load report (711–645 s before its end) for the same play on his Radeon: 16 UI frames a second,
16 composites, 0 decodes, the audio render at 6–7 %: the decoded frames were held, the composite
was the cost. What could not be proved here: the device time on his card (llvmpipe stands in for
it); the shape of the fix — composing once instead of once a lap — does not depend on the device.

## The crackle: every reason the audio render can fall behind, and which fired

| reason | on his project |
|---|---|
| the effects cost more than real time | did not fire on the range: `probe --time` 5 % (linear), 4 % (loop); his report 6–7 % of a core. Audio effect speed is `audio-fx-cost`. |
| the renderer is rebuilt on the audio render thread | **fired** whenever he edited during playback: `AudioRenderer::new` 412 ms here against a 341 ms ring; his report shows 12–16 bursts a second instead of 47 at those seconds (113 s, 79 s before the end), audio-render 66–75 %, prefetch 75–87 % |
| prefetch competition (a source missing at its deadline) | did not fire: 0 render underruns in an 8 s loop, 0 decodes in his report |
| the ring (341 ms, an 8 ms wake) | did not fire: 0 callback underruns in the 8 s loop |
| the loop wrap (a seam) | not the click: the largest steps sit at one position INSIDE the region (0.126 s into it) on every lap and in a linear play of the same material |
| the sound itself | **what he hears on the range**: VHS Tape (sound)'s dropouts, 123 a second as set in his panel, are single-sample steps of full scale (0.287 at a peak of 0.286). The same play without that effect: largest step 0.106, with Slowed alone 0.106, with Camera ISP (sound) 0.051 |

So the crackle on the 400 ms range is not the render falling behind; the render does fall behind
on every edit during playback. The rebuild now runs on a builder thread (D4). Whether the dropout
clicks of the VHS Tape sound are the device's honest sound at that setting is a question for its own
change, listed under "found, not fixed" in the report.

## The class walked: every place where a covered range may be worked on again

| place | what playback asks | what the bars ask | found | done |
|---|---|---|---|---|
| decoded video frames | `layers_at(t, q)`: (source, pts, decode quality incl. the step) | `coverage` by the clip's decode quality | the same keys; 0 decodes on replay | nothing |
| composites with effects | `present` | — | composed every lap (D2 bypassed the memo) | D1: kept by the plan key |
| nested reels (picture) | a sub-plan inside the plan; its key is recursive over the sub-tree | `nest_coverage` (the children's frames) | composed every lap with the parent | covered by D1: the parent's key holds the reel's |
| nested reels (sound) | `NestFetcher` re-renders the sub-mix every lap | the children's bursts | worked every lap | covered by D2: the sub-mix is inside the kept final mix |
| morpher layers | the effective clips, built once per renderer | the pieces' qualities | built once per edit, not per frame | nothing |
| transitions | the plan; the kind and its numbers in `put_mix` | — | composed every lap | covered by D1 |
| generators and titles | the source at its salted address, rasterized into the cache | the address's coverage | rasterized once; composed every lap | covered by D1 |
| camera tracks | resolved per frame, in the plan key | — | composed every lap | covered by D1 |
| audio pre-FX | `wanted_audio_bursts` | `coverage(Audio)` | read once | D3: not ordered for what the store holds |
| audio post-FX | every burst | — | rendered every lap | D2 |
| playback forward | both stores | — | | D1, D2 |
| playback backward | video: the same plan keys; audio: the raw dry path | — | video replays; the dry reverse is not kept (no effect runs in it) | nothing |
| loop | video: the same keys; audio: lap ≥ 2 differs from linear play | — | | D2: `MixCtx::Lap` |
| scrub / a paused frame | `present` at the moment | — | a repaint of the same moment composed again | covered by D1 |
| the first frame after Play | video: the moment; audio: a start waited for the ring's pre-FX bursts | — | the start waited for sources the kept mix does not need | D3 |
| export | composes and renders every moment once | — | must not replay anything (invariant 11) | untouched: it never meets either store |
| the bars | — | decoded sources only | promised what needs work | D5: a bright layer for what plays without work |
| setting a loop region | `set_loop_region` builds a whole `AudioRenderer` on the UI thread to pin the head | — | 412 ms of the UI on his project per region change | found, not fixed (another mechanism: a model build on the UI thread; `ui-frame-cost`) |

## Decisions

### D1. The executor keeps what it presented (`PresentMemo`, kt-gpu)

`present` computes `plan_key` first. A key it holds is shown by the display pass (and the glow's)
from the kept integer composite: no upload, no chain, no composite; `GpuStages.replayed` says so
and the player counts `composite_replays`. A complete composed frame is kept whole (the result
texture moves into the memo instead of back to the pool). 512 MB of device memory, least recently
shown first; an evicted texture goes back to the pool.

The key is the existing one, which already holds what determines a frame: the content serials of
the decoded frames, the chain with its curves, the camera, masks, transition, step, blend. A
parameter change moves the keys of exactly the moments it touches (t1934). A frame decoded again
gets a new serial and composes again — honest: the bars drop it too.

Rejected: keeping the display texture (Rgba8Unorm) — the glow and a later display pass read the
integer composite; a copy of the result per frame — the result texture itself is free to keep.

### D2. The final mix of a covered range is kept (`kt_engine::mix`, the owner's "вариант А")

Invariant 2 is reworded: the FX state is still touched by the audio render thread alone; the final
mix that thread rendered (after the effects, the track stages and the nested reels, before the
master gain) is kept by the same thread and handed out again instead of running the effects.

- **Key = what determines the result.** `AudioClip.fp` fingerprints each event: its model event's
  Debug (chain, curves, generator values, gain), the track level folded into its gain, the
  transition windows and handles, the reel instance's content hash, its run predecessor by id.
  `AudioRenderer::scene_hash(range)` hashes the fingerprints of every event active in the range —
  body, handles and tail — widened by the track stages' longest tail and look-ahead, plus what the
  reel shares (the chain options, overlaying, the stages). An edit drops exactly the bursts whose
  scene changed (`retain_valid`, t1931); a superset is safe, a subset would replay a stale mix.
- **Unit = a timeline burst** with the valid frame range inside it, so a region off the burst grid
  is kept exactly (t1933, t14 unchanged).
- **Linear play and laps are kept apart** (`MixCtx`): the first lap of a loop is linear play from
  where it began; the second lap carries the first lap's ringing at its head (R.28: the reverb rings
  through the wrap) and is the one later laps replay.
- **The first recording stays**: a frame held is not overwritten; a chunk with an underrun or a
  pre-roll miss is not kept.
- **Leaving the store is a seek** (`MixPlay`): the processors did not run while the store played;
  the first chunk the store cannot hand out whole resets them, so its first read pre-rolls exactly
  as after a seek to it. The play after the boundary equals a live render that started there, byte
  for byte (t1932); the junction's step stays within the material's own.
- **Determinism** (invariant 11) is untouched where it is stated: offline paths (export, `render`,
  probe) never meet the store; the preview is a function of what was played, as before (a loop's
  second lap always differed from its first).
- 64 MB (2.9 minutes), least recently heard first; cleared when a source comes or goes.

Rejected: a byte-exact hand-over of the FX state at the boundary (snapshots): `Warp` holds rubato's
`SincFixedOut`, which cannot be copied, and the nodes' `fork` would make a second owner of one
stream; canonical blocks (reset at every block boundary in live play too) — a pitch stream steps
where two grain grids meet (pitch-event-edges: 0.46). The seek at the boundary is the rule the
engine already has for "playing from here".

### D3. What the store holds needs no source

The playback look-ahead, the start's wait for its sound (cold-start-audio) and the background
prefetch skip windows the store holds: a covered range starts with no decode and plays with no
decode.

### D4. An edit does not stop the sound

During playback the render thread hands the rebuild to a builder thread and keeps playing with the
old renderer; the new one arrives, `sync_fx` carries the state (invariant 3), the store drops what it
renders differently. On a pause the rebuild stays inline (nothing sounds).

### D5. The bars tell "decoded" from "plays without work"

The top strips (cyan audio, green video) and the event's line keep their colour where the sources
are decoded and are drawn bright where the final mix or the kept composite plays without work:
`Player::final_audio_runs` (the store's published runs: laps inside an armed region, linear play
outside it) and `Player::final_video_runs` (moments presented whose key the executor still holds
and the current plan still asks; after an edit at most 32 moments are checked per refresh, so an
edit never waits on it).

## Load at rest

The store and the memo add no work at rest: the publish runs inside the render loop's existing
wake (250 ms parked on a pause); a pending re-check after an edit asks a finite run of frames.
Measured on the heavy showcase in the live scenario `cached_range.py` and on his copy (the report).
