## Context

Datamosh (F-FX-011) is a `Stream` stage: the host orders the event's frames at the stream's size,
runs the slots in front of the stage on them (stream-input-order) and hands them to the kernel, which
encodes them with its clean encoder at `lines` and decodes the broken stream there. The picture is the
broken decoder's, `lines` high, fitted by the host to the layer. Until now the stream's size WAS the
codec's: `stream_quality` = `lines` high, the layer's aspect, whatever the preview's or the project's
size.

Two consequences the owner named (kkt-26.11.18b):

- U1: at 144 lines the break looks right and the picture is soft.
- U2 (row 734): the front runs at `lines`. At 720 lines in a 640×360 preview Camera ISP in front cost
  59 ms of his iGPU a history position, ~14 ms at 360p.

## Goals / Non-Goals

Goals: a sharp break at the layer's size that is the SAME break as at `lines`; a preview whose stream
work follows the preview's size; the export's soft stream byte for byte as before when `lines` is not
above the project.

Non-goals: a codec that decides at the preview's size (rejected below); changing the default look.

## Decisions

### D1. The codec decides, a carrier executes (U1)

A P-frame is decisions (per block: a vector, coded or skipped; the GOP and the breakages say which
packet reaches the decoder and against which reference) plus pixels. With `sharp` on, the decisions are
the codec's at `lines`, unchanged; the broken decoder at `lines` is not run. A CARRIER decoder executes
the same chain (`Plan::chain`: the same nodes, references, packets) at the input size S:

- `Base` — grey; `Hold` — the reference; `Intra{p}`, `Refresh{p}` — the source frame at p at S.
- `Inter{p}` at k — per carrier pixel x (each plane, chroma at half):
  `block = min(x·Cw/(Sw·bs), bw−1)` (the codec grid's block covering x, by the visible sizes);
  `v' = round(v·Sw/Cw)` for the APPLIED vector (after `vectors_from`, swap/scale/rotate, loss — the
  vectors the broken decoder would have used), `o' = round(o·Sw/Cw)` for the packet's OWN vector;
  `pred = ref(x + v')`; if the block is coded in this plane,
  `out = pred + ((src_p(x) − src_{p−1}(x + o'))·gain) >> 8`, else `out = pred`; clamped.
  A block is coded in a plane when the packet's residual there — after the loss's drop — is not all
  zero.

Why the residual is the source's own difference and not the codec's residual upscaled: upscaling the
144-line residual brings the soft picture back through the leak. `src_p − MC(src_{p−1}, o')` is what an
encoder at S with the same decisions codes losslessly, so an intact chain gives `out = src_p` exactly
(the reference is the previous source by induction; a skipped block is one the codec found static, and
its prediction keeps the previous picture moved by its vector). The quantiser keeps its role through the
skip decision: at a high QP more blocks are skipped and the old texture persists, as at `lines`.

Rejected: a sharpening or detail-transfer filter over the soft picture (an imitation, root rule);
running the full codec at S (the search at S is the expensive part, and a fine grid is a different look —
the reason the owner picks 144); upscaling the codec's residual (soft again).

### D2. The stream's input size follows the layer (U2)

`stream_quality` now returns the size the host decodes and the front runs at:
- `sharp` (`StreamKernel::carried`) — the layer's base size (`clip_base_quality`);
- else the codec's size (`stream::coded_size(lines, base)`, the former formula), but the layer's base
  size when `lines` is above the layer's height.

In the preview the layer's base size follows the preview's quality, in an export the project's: the
preview does the work it shows, the export the full settings. The kernel derives the codec's size from
`lines` and the input's aspect (`coded_size`, the same formula: at the input = the codec's size it is
the identity) and resamples the input to it (D3). With `lines` at or below the layer's height and
`sharp` off — every existing project whose `lines` is not above the project's height — the export's input
is the codec's size and its bytes are the former ones. With `lines` above the project's height the export
now codes a source resampled from the project's size, where it decoded at `lines`: the extra lines were
never shown; the change is in What's new's line.

Rejected: the codec at the preview's size in the preview (row 734's first idea): a coarser grid in the
preview than in the export, so the preview's break would not be the export's.

### D3. The resample is part of the codec's convert, integer and the same on both paths

`codec::resample_rgba` (CPU reference) and `wgsl/resample.wgsl`: per axis, a box over the source
footprint when shrinking (`[⌊o·s/d⌋, max(⌊(o+1)·s/d⌋, lo+1))`), bilinear with 8-bit weights when
growing (centre-aligned, `p = ⌊(2o+1)·s·128/d⌋ − 128`, clamped), the identity at equal sizes; the 2-D
weight is the product, rounded once. The batched walk resamples on the device before its convert; the
reference walk on the processor before its encode job; t2970 holds the walk to the reference.

### D4. Keys, needs, output

- `enc_hash` mixes the codec's size and the input's size; `break_hash` the `sharp` flag: a carrier
  picture never answers for a soft one, nor a picture at another input size.
- `needs` with `sharp` adds, for every node computed, the source frames the carrier reads: `p` for an
  intra or refresh node, `p` and `p−1` for an inter node.
- The output: the carrier's picture at the input size, or the codec's at the codec's size; the host fits
  it into the layer (`run`).
- A region of interest with `sharp`: the intact picture outside is the source at S.

### D5. The sound stays in step

`sharp` is a foreign row of `datamosh_sound`: the audio half never reads it, and the carrier changes no
node of the schedule, so the dropped keyframes, bloom runs, frozen references and lost blocks fall at the
same positions in picture and sound (t2975).

## The class walked (root rule: a problem is a class)

The mechanism of U2: "a stage's working size is set by its own parameter, not by what the picture will be
shown at". Checked:
- the Datamosh front and history — changed (D2);
- the second input's chain (`second_front`) — read at the same input size, follows with D2;
- the stream memo's want (`stream_want`, `stream_frame_bytes`) and the order (`layers_at`,
  `video_decode_qualities`) — they ask `stream_quality`, so they follow;
- the other stages with a working size of their own: VHS Tape simulates at its own lines (F-FX-009) and
  Camera ISP at the layer's size — both run inside the chain at the layer's decode size already, which is
  the preview's in the preview; generators render at the layer's size; the reel previews have their own
  quality. No other stage orders frames at a size of its own.

## Risks / Trade-offs

- The carrier's memory: its pictures are at the layer's size (a 1080p export holds 1080p history
  pictures where it held 144-line ones). The memo's want counts the stream at `stream_quality`, which is
  that size.
- Preview and export decide from slightly different inputs (the codec's input is resampled from the
  preview's or the project's size): the same grid, the same breakages, vectors that may differ where the
  motion is ambiguous.
