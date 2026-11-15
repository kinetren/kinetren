# Design: no-partial-picture

## Context

`kkt-26.11.14` M1 answers found-defects 461. The mechanism behind the owner's example (the first picture
of a fresh preview) is: **something that is not complete is shown as if it were the thing, and work is
spent making it.** Two halves: what reaches the eye, and what the machine does for a result it throws
away. Both are walked across every place a picture or a level is made or drawn (the table below).

## D1. The executor knows before the device

`FramePlan::starved()` walks the plan's layers, reels' and track stages' sub-plans included, and is true
when any `LayerInput::Frame` is `None` — exactly the starvation `resolve_plan` turns into
`complete = false`. kt-gpu asks it first:

- `present`: after the kept-composite check (a kept moment still shows at once) and before the upload
  frame begins. An incomplete plan that only the resolve finds (a frame of a foreign size, a reel's
  sub-plan short of a frame below a fitted reel) stops after the resolve: no arena, no encode, no display
  pass, no glow. `last_cost` stays zero for a frame that ran nothing.
- `execute` (bytes) gets a `partial` flag: `execute_into` passes `false` — an incomplete frame leaves `out`
  empty, nothing uploaded, encoded or read back; `execute_once` passes `true` — the export's last resort
  (it writes a frame its input cannot give after its retries, and names it in its report, gpu-leftovers
  D5). `execute_measured` (tests' cost probe) keeps the bytes as before.
- The `submit`/`collect` ring is the export's; unchanged.

Rejected: skipping `frame_plan` in `compose_at` when `missing > 0`. `frame_plan` advances the stream
stages' history (a paced walk is work toward the frame, not drawing), and `layers_at`'s list is the
ORDER, which may name a key the plan does not need: a moment could stay incomplete forever. The plan is
the truth about starvation; asking it costs a walk over a few layers.

Rejected: keeping uploads of the present layers for the next attempt. The upload cache already keys
frames by serial; an incomplete attempt pre-uploading them only moved the same bytes earlier and paid the
encode on top.

## D2. The app draws only complete composites

`show_incomplete` is removed from `PresentOptions` (a flag left in the contract invites the exception
back). `draw_preview` takes a composite onto the texture or the display target only when it is complete;
the frame's place (`img_rect`) is computed before the picture branch and the scrim block moved after it,
so Loading (and "Loading — catching up: N") stands in the frame's place over the empty area. A held last
full picture after a seek/edit stays (round 14 items 14–15, the flicker "black frame ↔ Loading"): it is
complete, dimmed under the scrim, and the owner's M1 concerns partial pictures; the fresh project starts
with no texture (`replace_with` builds a new `KtApp`), so nothing of another project can be held.

## D3. The scrub meter

`Player::sample_levels_at` renders one burst through a `CacheFetcher` for the meter under a moving cursor
on a pause. A burst not in the cache read as silence, so a fresh position showed the level of what was
cached — a partial mix — for up to `SCRUB_SHOW_MS`. It now returns `None` when the fetcher missed; the
meter then releases its target as on a still pause (`MeterState::unknown`), never holding another
position's level, and the next probe 120 ms later measures again.

## The class walk

Mechanism: a result that is not complete is shown as the thing, or work is spent producing one that is
thrown away. Every place a picture or a level is made or drawn:

| Place | What was found | Verdict |
|---|---|---|
| Preview, fresh (open, new project) | first composite presented incomplete under the scrim (`!shown_any`) | fixed: empty place under Loading (D2), t2533, live t2534 |
| Preview after a seek, an edit, a quality/size change | the last full picture held under the scrim; an incomplete composite never drawn over it, but encoded (display) or read back (bytes) and thrown away | drawing kept (complete, round 14); the wasted work fixed (D1), t2530 |
| Preview during a stream catch-up | incomplete composites with "catching up: N", each resolved and encoded | wasted work fixed (D1) |
| Preview paused, retrying an incomplete frame | each retry (on news or the growing timer) composed the whole frame | now stops at `starved` (D1) |
| Preview playing | an incomplete frame is a skipped frame; V-fps counted complete only already | unchanged; its encode is gone (D1) |
| Preview while scrubbing | the same ask path | as above |
| Detached preview window | `draw_preview` itself, same texture and target | follows D2 |
| Slow-chain route (offline device, bytes) | an incomplete composite read back, dropped by the app | fixed: no readback (D1) |
| Loop fill (`compose_fill`) | asks only all-cached moments; a stream-short one composed and read back, then skipped | readback gone (D1) |
| Kept composites, present memo, readback memo | complete only (`insert(.., complete)`, `memo_keep` on complete) | not the class |
| Stream front memo (`video_stream.rs`) | kept on `Ok(true)` of full length only | not the class |
| Reel strips and posters (`compose_offline_frame`) | kept complete only (catch-up-shows-loading D6), but an incomplete one was composed and read back | readback gone (D1) |
| Reel strip disk tier | written from kept (complete) cells; versioned rows (gpu-leftovers) | not the class |
| Welcome posters | `render_poster` → the same offline path; loaded from a finished PNG | follows D1 |
| Timeline filmstrip | exact keys, sidecar thumbnails of whole decoded frames | not the class; see 573 (a near thumbnail of the bin under the same key) |
| Bin card scrub | draws the last hovered moment's thumbnail while the asked one loads, no Loading look | a held complete picture of another moment: row 570 (a look decision) |
| Thumbs sidecar | written after a successful decode only | not the class |
| Waveforms (timeline, bin, reel peaks) | drawn only from a finished pyramid (`build_until` → `None` when stopped, `mark_complete` after a full build) | not the class |
| Meters while playing | the output's levels | not the class |
| Meters on scrub/seek | a partial mix's level (misses read as silence) | fixed (D3), t2532 |
| Scopes | none in the program (graphs.rs are performance graphs) | n/a |
| Effects editor, ISP panel, deck face, camera/EQ/mask/geometry controls, transition/morpher editors, generator UI, track window | no picture of their own (overlays on the preview or vector drawing); the effects window's base thumbnail feeds `fx_thumbs` | not the class here; `fx_thumbs` is `chain-everywhere`'s |
| Render dialog | a progress graph, no preview picture | n/a |
| Export | composes an incomplete frame again up to twice; writes one only when the input cannot give it, and names the lost layer | kept (the person is told); the retries' full composes are row 572 |
| `kt-cli pause-scan`, `preview` bench, `gpushare`, `firstframe` | gpushare counted latency of every present; the bench and pause-scan re-ask an incomplete moment each pass | gpushare fixed; the re-asks now stop at `starved` (row 571) |
| Effects picker / gallery cells | excluded: `chain-everywhere` | for it: the gallery's base frame comes through `request_near` (a nearest stored thumbnail, found-defects 12) — a frame of another moment near a cut |

## Measured

Before = the stage-12 build (`orch-26-11-01` 8fd5f2be), after = this branch; both debug, llvmpipe, the
showcase `kt-cli showcase --heavy`, the machine shared with other sessions' test runs (timings swing by
several times between runs; the counters are the measure).

- **Fresh open of the heavy showcase** (live `open_heavy`, six runs each side over two rounds): the old
  build drew a picture under Loading before the first full one on one run of three (29 looks — the
  partial first picture), the new build on none. With the sidecars warm (rounds 3–4) the open to the
  picture took 1.3/1.4 s before and 1.3/1.2 s after, 2 composites both, 103/114 against 95/118 device
  submits — the same work; the incomplete composites the old build encoded are the ones that now stop
  at `starved`, and on this machine the open's first frame is complete after two asks.
- **The pause after the open**, 10 s: the same counts both sides (88–104 UI frames, all event passes,
  equal before and after: the scenario's last Escape on the File menu, not the preview).
- **Pause at rest** (live `idle`, the heavy showcase): before — quiet after 6 s, a pause 1 % of a core,
  nothing ran, an open effects window 1 %; after — quiet after 3 s, a pause 1 %, nothing ran, an open
  effects window 1 %. Nothing at rest changed: a rested preview asks no composite either way.
- Live `datamosh`, `gpufx`, `heavy_loop` green on the new build (Loading over the held picture during a
  catch-up, the device-drawn picture, a covered loop).
