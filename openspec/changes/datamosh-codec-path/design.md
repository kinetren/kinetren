# Design

## Context

- The physics (F-FX-011): a datamosh is a motion-compensated decoder applying P-frame vectors
  and residuals to the WRONG reference. Our model must contain motion estimation, a prediction
  loop over a reference buffer, a quantised DCT residual and a GOP; the breakages are changes to
  what the decoder is told.
- The engine: a video kernel is a function of one frame (plugins D4). Two memos rest on it — the
  CPU post-FX memo (`postfx_key_hash`, `src/libs/kt-engine/src/video.rs`) and the GPU readback
  and upload caches (`plan_key`, `src/libs/kt-gpu/src/engine.rs`).
- The one temporal thing the plan already does is FRAME BLENDING (R.37 C5): `clip_frame_keys`
  decides the pair ONCE, `layers_at` orders both frames, and `layer_src` blends them in the
  plan's common code, so both executors receive one already-blended pre-FX frame and the GPU
  key carries the blend (`LayerBlend`). CamMotion reads no neighbour: it is a closed form of the
  clip's progress `u`. Audio has the other precedent: a stateful chain pre-rolls `max(tail,
  100 ms)` at a continuity break so its state is a deterministic function of the input.
- Random access everywhere: the preview seeks, prefetch warms ahead, export renders in chunks
  sized by memory (`export_chunk_frames`), the CPU and GPU execute the same plan (invariant 7),
  and the bytes must not depend on where the computation started (invariant 11).

## Goals / Non-Goals

- Goal: a real codec model whose breakages are parameters; frame N is the same bytes however
  it is reached (seek, play, export in one go or in chunks, range export).
- Goal: a bounded cost per frame after a seek, and O(one encode + one decode) per frame in
  sequential playback and export.
- Non-goal: a bit-exact model of a standard codec (H.264, MPEG-4). We model the MECHANISM —
  the same quantities, a simpler syntax (no entropy coding, no B-frames, integer-pel vectors).
- Non-goal: GPU execution of the codec; curves on the codec rows; the stage on a reel or a track.

## Decisions

### D1. The form: a video effect with a SOURCE STAGE, not a transition

- A transition kernel is a pure function of two pictures at ONE moment and of a progress. A mosh
  needs a STREAM — the frames before N — on both sides. A transition also requires an overlap,
  while the classic mosh happens at a HARD CUT, where there is none.
- So the mosh is an effect on the INCOMING event, and its stream may START IN THE EVENT BEFORE
  (row `before`, frames): the stream is the previous adjacent event's last frames, then this
  event's. The cut between them is an I-frame — exactly what a real encoder's scene-cut
  detection puts there — and "drop the I-frame at the cut" is the classic mosh.
- The second input (vector transplant) is the event active at the same moment on the track
  above or below (row `vectors_from`). An event-reference row (a pinned `ClipId`) was rejected for
  now: kt-app has no picker for it, and "the track above" is how people stack a mosh source.
- The stage acts on the SOURCE wherever its slot stands in the chain: a codec encodes footage,
  not a graded picture. The picker template puts it first. The host removes the slot from the
  chain the executors see, so kt-fx and kt-gpu never meet it.

### D2. A new planner class `Stream` and a `StreamKernel`, not "video state"

- The registry keeps refusing `state`, `tail` and `lookahead` for video. A stream kernel declares
  none of them: it declares a HISTORY — which stream positions frame N reads — as a pure function
  of its rows and of the stream's layout (where it starts, where the cut is, where the second
  input's events start). F-FX-007 named exactly this way out: "not to admit state but to read
  the PAST SOURCE … a new plugin declaration".
- The kernel trait (`kt_plugin_api::stream`):
  - `lines(p)` — the encoded height; the host orders the stream's frames at
    `(even(lines × aspect), lines)`, the aspect of the layer's decode size;
  - `before(p)`, `vectors_from(p)` — the stream's layout requests;
  - `needs(p, layout, n, memo)` — the positions whose frames are still needed to produce frame n
    given what the memo holds (empty when the memo already has frame n);
  - `render(p, layout, n, input, memo, out)` — the frame, or `None` when a frame of the history
    is missing; everything computed before the gap stays in the memo.
- Refusals (t1240): a `Stream` class without a stream kernel; a stream plugin with any GPU side; a
  stream kernel on a plugin of another class. A built-in stream plugin is exempt from "a built-in
  must keep the GPU default": D4.

### D3. Temporal state: replay from the anchor, with a content-keyed memo

**How frame N is computed.** The stream positions are the project grid's frame indices. The
I-frame SCHEDULE is content-free: an I-frame at the stream start, at the cut (when the stream
starts in the event before) and every `gop` frames from the cut. The breakages mark some of
them not intact (dropped, swallowed by a bloom). The ANCHOR of N is the last intact I-frame at or
before N. Frame N is the decoder's loop run from the anchor to N over the packets the encoder
produced, with the breakages applied — a pure function of the rows and of the source frames in
[anchor, N] (and of the second input's frames from its own GOP starts).

**The bound.** The rows are bounded (gop ≤ 60, drops ≤ 4, bloom ≤ 60), and the decoder has an
INTRA REFRESH: a broken chain longer than `MAX_CHAIN` = 300 frames meets a forced intact I-frame
every 300 frames counted from its anchor. It is a fixed grid, not a sliding window — a sliding
"N − 300" would give every frame a different chain, so consecutive frames would not continue each
other and no memo entry would ever be reused. The cold cost of a frame is therefore at most 300
encodes + decodes at the encoded height.

**The memo.** `StreamMemo` is a bounded LRU of opaque entries keyed by `u64`, defined in the API
and owned by the HOST. The codec keeps two kinds of entries:
- CLEAN (the encoder): the closed-loop reconstruction and the packet (vectors, residual) of
  position k, keyed by a chain hash of the encoder rows and the source ADDRESSES (media, pts,
  codec size, the source's revision) from k's clean GOP start to k;
- DECODED (the broken decoder): the picture at k, keyed by the clean key, the breakage rows and
  the decoder's own chain hash from the anchor.

A key is computed from addresses alone, so the walk "what is still needed" reads no frame. The
address is stable for a file; a procedural source (generator, title) adds its revision, which
the renderer hashes from the media item when it is built. The memo is invisible to determinism:
an entry is exactly what the replay computes from the same inputs, with the same arithmetic.

Rejected:
- **A sliding history window** (output N reads only N − W … N): every frame then starts its own
  chain, the old texture "slides back" as the window moves, and nothing can be memoised.
- **Storing the decoder state in the renderer and continuing it** (the audio path): the renderer
  is rebuilt on every model edit and the preview, prefetch and export run their own renderers;
  a continued state would make the bytes depend on what was rendered before (invariant 11).
- **The memo in `BurstCache` as a new payload kind:** the codec's entries are planes and packets,
  not frames; the cache's coverage and recency would start counting things that are not the
  source. The memo is a cache by the root rule ("caches are allowed where they pay": keyed by
  what determines the result, bounded, invisible to determinism), but not the coverage cache.

**Who owns which memo.** The preview (composer, prefetch, picker thumbnails) shares one process
memo (`kt_engine::stream_memo()`, 256 MB), so the prefetch's question "what does the codec
still need" and the composer's answer see the same entries. Each export owns a private memo
(64 MB): another thread's preview cannot evict what an export is about to read.

### D4. CPU only — and why that does not break invariant 7

The codec is a serial loop: frame k needs frame k−1's reconstruction, motion search needs
data-dependent branching, and the residual is integer-rounded. A GPU twin would be a second text
of the same arithmetic held to tolerances, and a mosh AMPLIFIES differences frame after frame —
a one-LSB difference at the anchor is a visible difference 100 frames later. So the stage runs in
the plan's common code on the CPU, like frame blending, and BOTH executors receive the same
decoded bytes as the layer's pre-FX frame. The chain after it (geometry, grading) runs on
whichever backend runs the plan. The layer's `content` and `fx_hash` carry the decoded frame's
key, so neither the GPU upload/readback caches nor the CPU post-FX memo return another mosh.

Cost (encoded height 360, 640×368 padded, 920 macroblocks): a diamond search plus a ±1
refinement is ~30 SADs per block; the DCT is 8×8 separable in f64. Measured in tasks 6.x.

### D5. The order: own frame in steady state, the history only when the memo lacks it

- `layers_at` asks the stream frames `needs(n)` names, at the codec size, mapped to (media, pts)
  through the stream's layout: positions before the cut read the event before, positions of the
  second input read the event on the other track at that moment.
- In sequential playback `needs(n) = {n}`: the frame the layer would have ordered anyway, at a
  smaller size. After a seek it is the history up to the memo or the anchor. The prefetch window
  ahead of the playhead names the frames it will play anyway.
- The layer's decode quality for the coverage bars (`video_decode_qualities`) is the codec size:
  the bars show what the order warms (invariant 4).
- A stream layer orders NO frame at its ordinary decode quality: the stage replaces the source,
  so a frame there would be a decode nobody reads.

### D6. Export pre-roll

Export warms its chunk with `wanted_frames` WITHOUT history (the ordinary per-frame needs), then
before each frame asks `stream_needs` against its private memo; when a history is missing (the
first frame of a range inside a mosh, or an entry evicted), it decodes the missing frames in
batches bounded by the export cache budget and advances the codec over each batch
(`stream_advance`), until the frame's needs are empty. Progress is monotonic: every batch leaves
its results in the memo. So a chunk boundary changes nothing (the memo carries over), and a range
export equals the same frames of a full export.

### D7. The codec model

- Planes: RGB → Y'CbCr (BT.601 full range, integer), Cb/Cr 4:2:0 by 2×2 averaging; padded to a
  multiple of 16 by edge replication. Back: chroma upsampled by nearest neighbour (the colour
  bleed of a mosh is block-shaped by nature), integer inverse.
- Motion estimation on luma against the encoder's reconstruction: candidates (0,0) and the left
  neighbour's vector, then a small diamond to convergence (≤ 16 steps), then a ±1 refinement;
  ties by SAD, then by |v|, then lexicographically. Integer-pel vectors, clamped to the search
  range; the reference is read with clamped coordinates (unrestricted vectors).
- Residual: per 8×8 block, forward DCT-II in f64 (basis from the frozen `fcos`), quantised with
  the step `2^((qp − 4) / 6)` (the H.264 law), rounding half away from zero; the decoder's
  inverse rounds to an integer. The residual is stored in the spatial domain (i16).
- Intra: an I-frame is a P-frame predicted from flat 128 — one code path.
- Decoder: `recon = clamp(pred(ref, f(v)) + g × residual)`; `f` = swap, scale, rotate, rounded to
  integer pel; `g` = the residual row (Q8 integer); a lost macroblock takes a vector drawn from
  `lowbias32(seed, k, mb)` within the search range and no residual.
- Determinism: integer arithmetic plus IEEE f64 with no fused ops and no platform libm (t1066).

### D8. The output and alpha

The decoded picture is converted to RGBA at the codec size and fitted to the layer's decode size
with the shared bilinear (`kt_fx::video::fit_into`); the chain sees the size it saw before. A
codec has no alpha: the stage's output is opaque, as the codecs it models.

## Risks / Trade-offs

- Cold seek cost: up to 300 encodes and decodes at the encoded height. Mitigation: the memo,
  the bound, the encoded height row (144–720).
- Memory: the shared memo is 256 MB at most; an entry at 360 lines is about 1.4 MB.
- A procedural source's revision must reach the key, or an edit of a generator under a mosh
  would show the old picture: the renderer hashes the media item into the address.
- The rows are not automatable (Structural): a curve on `gop` would make the anchor of N depend
  on the curve at every earlier frame. Deferred, not forgotten.

## Migration Plan

None: a new plugin id; older builds read the slot as `Missing` and keep its bytes (plugins D3).
