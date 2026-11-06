## Context

`gpu-render-hardening` left a render on a software adapter at 8.7 GB and 4 devices on the showcase. The
CPU build is not coming back; it stays a yardstick.

## Decisions

### D1. The end of a file is met once

The packet scan at opening stops where the file stops, so `video_keyframes` holds what is really there,
whatever the header says. Three things followed from reading only the header:

- `prepare_for` sought for any target past the 4 s forward bound. Past the real end every key was such a
  target, and each seek reopened the decoder and decoded the last GOP again. It now decodes on when no
  keyframe lies between the cursor and the target: a seek would land behind the cursor. This also holds
  in the middle of a long GOP.
- A seek is clamped to the last keyframe the scan found: an MP4's index names samples past the cut, and
  a seek to one finds no packet.
- The audio sought once per burst past the end, and on a Matroska file with no cues each seek walks the
  clusters. Past the fifo's end with the stream ended it answers at once.

`cursor_for` asks the same rule (`video_reach`), or a second decoder opens for the same end of file. A
packet the file ended inside of (`AV_PKT_FLAG_CORRUPT`) is not sent to the decoder or indexed: the
decoder refused it, and the read failed.

Counters on the 12 s fixtures: t1760 301 reopens to 1, t1762 130 seeks to 1, t1763 62 reopens to 1. A
40 s event over the end of a cut 4K-class VP9: not done in 280 s before, 33 s on one device at load 60.

### D2. A span's pictures convert on threads

The conversion to RGBA took four times the decoding on the showcase (64 s against 17 s), on the source's
own thread. `read_video_span_sizes` now takes a reference to the picture and gives it to one of four
converter threads, each with its own scaler contexts, and emits in key order. Not swscale's own
slices: the x86 block overrun of `sws_to_rgba` lands on the next row, which another slice's thread
would be writing. A render asks it (`memory::rgba_threads`), a tight render does not, and a preview's
sources stay on one: the count is taken when a source opens.

### D3. The tape's convolutions are written out

The per-pass table told the story: 8 taps of a constant bound took 2.7 ms, 25 taps of a bound chosen by
the standard took 31 ms. llvmpipe unrolls a loop whose bound it can see, up to 32 iterations; then the
index is a constant and the table folds. Otherwise every tap goes through execution masks and a table
read lane by lane. So the passes that convolve come once per standard, with their taps written out by
`wgsl_taps`, the 65- and 81-tap ones too, past the unroll limit. The dropout compensator moved to a pass
of its own (`s7a`): it was a branch and a second load in each of 25 taps.

Their stage textures are `R32Uint` (`BarrierKernel::word_passes`): 5 to 15 % of a pass, less than it
looked. No float changed: t1640 and t1641 hold the shaders to the models at 0.0005, and the stored
frames did not move.

| pass, one PAL frame | before | after |
|---|---|---|
| s1 record | 11.0 ms | 5.1 ms |
| s4 write | 11.5 ms | 5.7 ms |
| s5 read | 23.4 ms | 22.5 ms |
| s7 playback | 32.3 ms | 10.4 ms |
| s8 chroma | 12.8 ms | 7.7 ms |

Minima of repeats on one device, load 30 to 60. s5 stays: its 162 coefficients are per row, read per
pixel, and it is the next thing to look at.

### D4. A fused pass of its own per program shape

The fused pass interprets a program: a loop over a run-time count, a table of slots indexed by a varying
cursor. Nearly every frame runs it, because a camera puts a Proj on each layer. `geom_specialized` writes
the same arms with their slots known, per shape (the uniform's header), built the first time a shape
runs, once per device. Taps and Folds keep the interpreter. 14 to 7 ms on the warp shape, 10 to 4.7 ms on
the camera's. t1766 holds the bytes to the interpreter's.

Rejected: a pass per shape compiled at engine creation (the shapes are not known), a text cache on disk.
Tried and dropped: replacing `/ 255u` in `over` by shifts, measured no gain: the 8.7 ms composite is not
the division.

### D5. A pipelined chunk is counted by what its frames ask

`export_chunk_frames` divides half the budget by one frame of the project's largest decode size times
its most layers. That is right for the serial loop, whose cache is one budget. On the showcase it gives
one frame: 4092×2160, three layers. So every chunk was one frame, the sources advanced in step, and a
chunk waited for its slowest source. The threads of D2 had no span to work on. `pipe_chunks` walks the
frames and cuts at half a budget of what they really ask. Workers with no frame fell from 130 s to 44 s,
pictures decoded from 9072 to 7448, reopens from 84 to 51.

### D6. What did not help

- `LP_NUM_THREADS` at 8 per device against the default: 6 % less CPU on the demo, nothing on the showcase's
  wall. Left alone.
- The decoder threads: 4, 8 and auto within noise on the showcase, and auto takes 2.5 GB more. The row's
  known difference is not the gap.
- No clear on a full-frame pass: 0.1 ms a pass. Taken because it is free.
- The readback memo on a render's devices: 64 MB each and an 8 MB copy per frame. Removed from a render.
- `SyntheticSource` built a frame byte by byte through `.cycle()`, 10 ms at 1080p. That was the demo's
  "decode", 95 % of its wall.
