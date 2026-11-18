## Why

The owner's list `kkt-26.11.18b` (his answer of 18.11.2026 to found-defects 734):

- **U1** — "на 144p получаются красивые изломы но они мыльные, а есть ли возможность сделать их не мыльными?
  … не пиксилизацию включить а чтобы именно расчет был такой хитрый чтобы оно в разрешение проекта
  превращалось если я хочу". At a low encoded height Datamosh's break has the look he wants (a coarse block
  grid moving as one) but its picture is the codec's 144-line picture stretched over the frame.
- **U2** — "да - тут бы конечно как-то учитывать настройки превью проекта чтобы поменьше работы было".
  Row 734: the effects in front of a Datamosh (his Camera ISP) run at the codec's size; with a codec coding
  more lines than the 640×360 preview a history position cost 59 ms of his iGPU against ~14 ms at 360p.

## What Changes

- Datamosh gets an opt-in structural row `sharp` (off by default). On, the codec still runs at `lines`
  and decides the break there (the GOP and every breakage, the block grid, the motion vectors, which blocks
  are coded); a CARRIER decoder executes those decisions at the layer's size: each block of the codec grid
  is the same part of the large picture, its vector scaled by the size ratio, and a coded block adds the
  full-size residual of the same prediction (the source frame minus the previous source frame moved by the
  block's own vector). An intact stream reproduces the source; every difference is a break.
- The stream's INPUT size (the frames the host decodes and the slots in front of the stage run at) follows
  the layer's size: the layer's size with `sharp`; otherwise the codec's size, but never above the
  layer's. In the preview the layer's size is the preview's, in an export the project's. The codec
  resamples its input to `lines` when the two differ; with the input at the codec's size the bytes are
  the former ones.
- The paired sound needs no change: `sharp` moves no node of the stream's schedule.

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-fx`: Datamosh's carrier (`sharp`); the stream's input size follows the layer's size.

## Impact

`kt-plugin-api` `stream.rs` (`StreamKernel::carried`, `stream::coded_size`); `fx-video-codec` (`mosh.rs`,
`walk.rs`, `codec.rs`, `lib.rs`, shaders `resample`, `coded`, `carry`); `kt-engine` `video_stream.rs`
(`stream_quality`); kt-app catalogs (the row's name). File format: a new row omitted at its default.
