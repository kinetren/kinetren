## Why

The owner's first trace of the render-ahead pipeline (kt26.11.17-2abc3af9, `fails/26.11.18/new`, his Ryzen
7700X iGPU) shows the loop of VHS Tape + Camera ISP + Datamosh complete only from lap 16, the card busy
32–46 % while frames were missing, and "6 kept of 68 composed", "29 kept of 309" moments ahead a lap
(`kkt-26.11.18` T2: "мне кажется процессор/gpu вообще не напригался, отрисовалось оно раза с 15го").

His trace names two mechanisms:

1. **A moment short of its stream history was skipped for the whole lap.** All 2316 moments the producer
   did not keep were `stream_run SHORT` (the Datamosh history 19–64 positions behind). The history walk
   reached most of them seconds later, but a skipped moment was not tried again until the playhead wrapped.
2. **Every piece of device work waited for an empty ring.** The arbiter cut pieces at the whole bound (one
   frame period) and admitted a piece while the queue plus its claim fit the bound: a piece that size fits
   only an empty ring. The history position — Camera ISP in front of Datamosh at the codec's size, 59 ms of
   device — took a 117–124 ms span; ~200 s of admission waits in a 160 s run. The device stood idle while
   each next piece was noticed, encoded and submitted.

## What Changes

- The render-ahead producer keeps an incomplete moment WAITING for what it lacked: a moment short of `k`
  positions is tried again once the walks computed `k` less what a composite computes itself, any other
  incomplete moment once a walk or a decode moved; a new lap tries it again. The display composes a waiting
  moment on demand as it did a skipped one. A moment the memo refused stays skipped for the lap.
- The arbiter cuts a piece at HALF the bound and no admission claims more: one piece is queued behind the
  one that runs. What stands in front of the display's present stays the bound (the owner's raise of
  render-ahead D5 is kept).
- The trace: a render-ahead job says its outcome (kept, incomplete, short of the history, refused);
  `kt-cli trace loop` prints that split per lap and the ring admissions' waits.

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-plan`: the producer's incomplete moments wait for what they lacked; ring pieces are half the bound.
- `telemetry`: `trace loop` says why moments ahead were not kept and how long work waited for its ring.

## Impact

`kt-engine` `render_ahead.rs`, `video.rs` (`stream_compose_steps`); `kt-gpu` `arbiter.rs` (`piece_limit`);
`kt-cli` `trace_query.rs`. No file format, no picture change (cuts move no bytes, t2253).
