# Design: render-honest

## Context

render-speed (2026-11-16) made every export write its stage profile, and found two things it left open:
the device count of a software export suits one kind of stretch only (row 710), and the owner's "render
slower than the preview" is his preview's size, not a defect we can reproduce (row 711). The review of
2026-11-17 (R8) asked the render window to say so.

## Decisions

### D1. The count is opened by memory and used by time

`kt_gpu::export_engines` keeps opening the ceiling (`memory::engines_for`: cores and memory bound what can
exist). Opening is not the cost: an idle llvmpipe device costs no processor. The cost is a device that is
given frames while the bottleneck is elsewhere (decode, convert, encode) and its per-core threads take the
processor from them — the 31.8 s against 12.5 s of row 710.

The dispatcher (the compose thread of `export_pipeline::run`) hands frames round robin to the first `k`
workers, and `DeviceRule` picks `k`:

- a window is `max(8, 4k)` frames dispatched, measured after `2k` frames of settling (a worker's job slot
  and its two done slots fill first: the first frames given to a fresh worker are not throughput);
- the rate is frames per second of wall, the person's pause excluded;
- start at one device; try one more while the last step won by more than `GAIN` (10 %);
- a step that does not win goes back; a tie keeps the fewer devices;
- every `HOLD` (4) windows at a count, try one more, then one fewer: a light stretch after a heavy one
  takes devices away, a heavy one after a light one adds them. Going down is kept unless it loses more
  than `GAIN`, going up only if it wins more than `GAIN`, so the two do not flap on noise.

The numbers are structural (the queue depths, a margin above a window's noise), not measured on this
machine: the rule is the same everywhere and decides from what the run measures. A card has one device and
no rule runs.

The writer used to read frame `i` from worker `i % W`. The dispatcher now sends the worker of each frame to
the writer on an order channel before the job; frames are still dispatched in order and every worker is
FIFO, so the writer meets them in order. The frames do not depend on which device composed them (t1573),
and t2901 drives a scripted sequence of counts and compares the bytes with one device.

Invariant 11: the clock steers only WHICH device composes a frame, never what is composed or written. The
crate's rule "the clock is read only into `ExportProfile`" gains this one exception.

Rejected: a fixed rule by frame size or by stage shares (render-speed D1 found no rule right both ways);
measuring each device's own frame time (on llvmpipe a device's time grows with the others' load — only the
whole pipeline's rate says whether one more device helps).

### D2. The preview keeps what a moment cost

`VideoGpuExec::last_device_ns` answers the device time of the executor's latest measured frame (kt-gpu's
pass timer: the sum of the passes, or submit to done without timestamps); it is read after each complete
composite that was not a replay of a kept one, and the player keeps it per aligned moment with the
composite's size (`PreviewCosts`, bounded, the newest per moment). The timer's frame is the one harvested
by then — the same or a neighbour a frame or two back: over a range the mean is what matters.

`Player::preview_cost(a, b)` sums device time and pixels over the moments in the range. The prediction is
`frames × (Σ ns / Σ pixels) × render pixels`, on one device: it predicts the composing part, not decode or
encode, and the journal line (D4) keeps it honest. The newest measurement per moment wins whatever the
project version: an edit after playing the range makes the prediction stale until the range plays again,
which is better than none; the dialog says how many frames were measured.

### D3. The window states the size difference always

The render window's file group shows, under the size estimate: the preview's size against the render's and
the pixel ratio when they differ ("Preview 640×360 · render 1920×1080: 9.0× the pixels of a frame"), and
the predicted time with the measured share, or "not measured — play the range once for an estimate". The
preview's size is the player's preview quality at the moment the window draws; nothing is computed per
frame beyond a range walk of the ledger, and the window at rest asks no repaint (t2813).

### D4. Every render journals predicted against actual

`RenderJob` carries the prediction made at its start; at the result the window writes
"render time: predicted X s (from N of M frames of the preview at W×H), actual Y s (×R)" or "render time:
no prediction, actual Y s" into the telemetry journal.

## Class walk (the root rule "a problem is a class")

The mechanism of row 711: a cost differs between two paths, and the interface shows one of them. Checked:
- the render window (this change): now states the size and the predicted time;
- the reel strip and posters (offline renders at their own small size, never compared with anything by the
  person): nothing to state;
- the loop fill (composes at the preview's size, the same cost as the preview): none;
- `kt-cli export` (prints its own real-time factor and stages since render-speed): none.
The mechanism of row 710: a count of workers fixed where the right count depends on the stretch. Checked:
the export's frame workers on one device (`frame_workers`: they convert, not compose; a convert is cheap
and bounded by the cores — left), the loop fill's width (cpu-idle-work already narrows it on a software
device by what composes), decode threads per source (bounded by memory, render-speed measured no gain past
four — left).
