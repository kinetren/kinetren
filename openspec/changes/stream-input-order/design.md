# Design: stream-input-order

## Context

found-defects 484. A `Stream` slot (Datamosh) is a SOURCE stage: `layer_src` runs its kernel in the
plan's common code and hands the executor the result as the layer's pre-FX frame
(`datamosh-codec-path` D1–D6). The kernel read its frames through `ClipStream::with_frame`, which
returned the decoded frame from the cache; the executor then ran the WHOLE chain — the stream slot is
transparent to `plan_chain` — so a slot standing in front of the stream ran after it. In the owner's
event 2149 (camera_isp → datamosh → vhs_tape) Datamosh coded the raw footage and Camera ISP was laid
over the mosh.

Mechanism (the class): **a stage that reads the decoded frame instead of the chain's running picture
in front of it.** The stream stage is the one stage that reads its input outside the executor, so it
is the one place where the chain's order could be lost between the plan and the executor.

## Decisions

### D1. The front runs on the stream's frames, on an injected executor

The video effects run only on the GPU (invariant 7, compute-kernels-render: no CPU kernel exists). The
plan code has no executor in hand (the stream runs while the plan is built, on the compose thread, the
prefetch thread, the export's workers, a strip's job). So kt-engine takes one more door, as
`kt_plugin_api::compute::install` does: `install_stream_front(fn() -> Option<Arc<dyn VideoGpuExec>>)`,
opened at the first frame that needs it. kt-gpu's `stream_front_engine` is the offline engine's
SIBLING — its device, pipelines and ring, its own frame lock — so a walk's front runs wait on no
composite's lock and the preview's display device is untouched. kt-app, kt-cli and the testkit
install it.

Each main-stream frame the kernel reads (`with_frame`, `Side::Main`) goes through `chain_plan` with
the front slots, their curves applied at the position's material time (the same time `rows_at` gives
the stage's own rows), `u` at the position, at the stream's size, read back as bytes. Rejected: the
front at the layer's decode size and then scaled to the stream size — the stream is coded at its own
size; an effect in front of a codec acts on what the codec is fed.

### D2. What the front is

`front_slots(chain[..slot])`: slots that change the picture — not audio, not a layer strength, not an
identity geometry (`has_video_fx`), and not the LEADING Geometry node. The leading node places the
event's picture on the canvas through `lead` (conform, crop, transform); it is the event's placement,
not an effect on its source, and it stays after the stage (where the stream's output is fitted to the
layer). A Pan or a CamMotion slot in front is an effect and runs in front.

The front runs on the WHOLE main stream: the event's own frames and the history in the event before
(the cut mosh reads the last `before` frames of the event before). The event's chain is the device
that records the stream — every frame the codec is fed goes through it (t2470 holds both shots baked).

### D3. The chain after the stage

`after_chain` puts the identity Geometry slot (`lead: false`, defaults) in place of each front slot of
the layer's chain when the stage ran. The planner treats it as transparent; the indices that masks
(`ClipMaskPlan`) and layer strengths (`LayerMix.from`) name stay valid. Rejected: slicing the chain —
every index-bearing structure would have to be shifted.

### D4. The history names what feeds it

The codec memo's keys are built from `StreamLayout::ident` alone (so "what is still needed" reads no
frame). `ident` on the main side now mixes `front_key` — a hash of the front slots and their RAW
curves — and, when a front slot has curves, the position and the event's placement (start, end,
source in, reverse), since a curve's value at a position depends on them. An edit of a slot in front
changes every main address and the history is computed again; an edit of a slot after the stage
changes nothing of the stream (t2471). The post-FX memo already mixes the stream's key into
`fx_hash`, and the kept composites key by the plan — both follow.

### D5. No front executor → the frame is missing

With no device or a refused plan `with_frame` answers `false`: the kernel reports the source missing
and the frame stays Loading. The owner (11.11): "A stage either has its own input or the frame is
Loading." The decoded frame never stands in.

### D6. A memo of the front's outputs

The walk reads a position once, but a re-walk after the codec memo dropped it, the composer and the
prefetch reading the same position, or a strip on the same history read it again. A bounded LRU
(64 MB, keyed by `ident`, which names the front) keeps those from running the slots again; the bytes
do not depend on it (invariant 11). It is outside the memory ledger (found-defects 542).

### D7. Prewarm

The preview prewarms the display device; the front runs on the offline device. `Reach.front` (a
picture slot in front of a stream slot anywhere in the project) has the player prewarm the front's
engine too, on a thread of its own (opening a device takes up to seconds).

## Class walk

Every path that builds a stream input — all go through `ClipStream` (the only host `StreamInput`):

| Path | Where | Finding |
|---|---|---|
| The composite's stage run | `layer_src` → `stream::run` | fixed (front in `with_frame`, after-chain) |
| The probe plan (`kt-cli probe`, structured-telemetry) | `stream::key_only` | keys only: `ident` names the front |
| The order / coverage bars | `layers_at`, `video_decode_qualities` | order decoded frames at the stream size: correct, the front runs on them |
| The prefetch's advance, a run's walk | `stream_advance`, `stream_advance_run` | same `ClipStream` → fixed |
| Export pre-roll and frames | `stream_preroll`, export pipeline | fixed; t2472 (pieces = whole, ≠ old order) |
| Reel strips, posters, the picker | `frame_plan` on the side engine | fixed through the same door (kt-app installs it) |
| Checkpoints, memo sizing | `stream_checkpoints`, `stream_frame_bytes` | positions and bytes unchanged |
| The history's second input | `with_frame(Side::Second)` | **same class, open**: another track read as decoded (row 540) |
| The event before's frames | `with_frame(Side::Main, k < cut)` | through this event's front (D2) |

Every other stage that could read the decoded frame instead of the running picture:

| Stage | Finding |
|---|---|
| Barriers, point-wise runs, fused runs | `plan_chain` sequential over the running picture; a fused run keeps chain order (t2191) |
| Geometry series | one pass of the inverse maps in chain order; point-wise stages inside a series are absorbed in place (R.44) |
| Layer strength (`LayerMix`) | mixes with the snapshot at its `from` index — by design; a `from` inside the front now names an identity (the stream's output) |
| Slot masks | the dry snapshot at the slot's index; a mask of a FRONT slot is not applied in the front run (row 541) |
| Pan3d in front | `chain_plan` gets no camera: skipped in front (row 541) |
| Frame blending | blends decoded pairs BEFORE the chain by design; never on a stream layer |
| Reel / track stages | their chain runs on the sub-composite: the running picture |
| Generators | the source itself |

The paired audio side of the simulated devices: Datamosh's sound (`fx-video-codec` `sound.rs`), VHS
Tape's and Camera ISP's pairs are ordinary `AudioState`s in the audio chain, which runs its nodes in
order on the running block; the sound half reads only rows and timing from a `StreamLayout` whose
`ident` is 0 — no decoded input is read. Nothing to change.

## Cost

Measured on release (`kt-cli stream-cost … 11.2 13.2 --laps 1`, 1920×1080 stream) over the showcase
`--heavy` built twice: `--mosh` (Datamosh first: no front — the walk the old code ran for ANY order)
and `--mosh-behind` (duotone → datamosh → glow on the same cuts: the owner's order). Two runs each, on a
machine other sessions shared, so the walls move by 3×; the counters do not.

| | no front (`--mosh`) | front (`--mosh-behind`) |
|---|---|---|
| positions decoded / encoded, jobs | 196 / 212, 126 | 196 / 212, 126 (the walk is the same) |
| front runs for 50 frames | 0 | 62 (each position once: 50 + 12 history before the cut; the memo answers the rest) |
| advancing thread's processor time outside jobs, a frame | 3.6 / 3.3 ms | 45.7 (cold pipelines) / 19.2 ms |
| advance wall, a frame | 89 / 274 ms | 702 (cold) / 156 ms |

So the front costs one composite of the front slots per history position, at the stream's size, read
back: ≈ 16 ms of the advancing thread a position at 1080 lines warm, on top of the walk; the codec
side is unchanged. The first run compiled the front's pipelines on the offline device (7.4 s for the
first frame) — what D7's prewarm takes off the first frame in the editor. The round trip
(device → bytes → the compute device) is the cost to remove next: found-defects 543.

Idle: the front runs only inside a stage's walk, which runs only while a history position is
missing; no timer, thread or poll was added (the prewarm thread runs once per changed project reach).
A pause over a walked history asks nothing (t2471: zero positions ordered once the memo holds them).

## Risks

- The front's device is the offline device; on a machine where only the display device works, the
  front has no executor and a stream with a front stays Loading (it said so before only for no device
  at all). Same as a strip or a poster today.
- A software adapter: the front adds one composite per history position to a walk that was compute
  only.
