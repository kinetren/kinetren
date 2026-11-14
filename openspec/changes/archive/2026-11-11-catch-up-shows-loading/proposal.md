# Proposal: catch-up-shows-loading

## Why

The owner's run of stage 10 (`docs/ktprompts/kkt-26.11.10b.txt`, registry K1, K2, K5; found-defects
403). While a Datamosh history catches up, the playing preview shows the stage's latest HELD picture
(`StreamKernel::held`, `StreamPacing::hold`) — a picture of another position, possibly from the history
before the loop — and runs every later stage over it: the event's own VHS Tape at the moment asked, the
track stages, the reel's stages. He saw exactly that: "у меня vhs tape будто бы накладывается в этот
момент актуальный … либо есть кадр либо нет, зачем накладывать эффект если предыдущий не готов?". He
decided: "ну пусть loading показывает" (403) and "catchung up надпись ну очевидно можно в том же месте
где loading писать" (K2).

Mechanism: a stage of the plan is given an input that is not its own position (a stand-in), and the
stages after it run over the stand-in at the moment asked, so the composite is a picture no moment has.
The change removes the stand-in from the stream stage and walks the class through every stage that
could substitute an older input.

## What Changes

- **No held stream frame anywhere.** A frame whose stream stage lacks more than the composite's two
  positions is incomplete, playing or paused alike: no layer picture, no later stage runs over anything,
  the composite is not presented over the last full picture, and the preview shows its Loading look (the
  last full picture under the scrim after 180 ms). `StreamPacing::hold`, `VideoRenderer::set_stream_hold`,
  `stream_held`, `StreamKernel::held` (the host's question) and `GpuStages.stream_held` /
  `Composed.held` go. A kept composite of the exact moment still shows at once (`show_kept`), and an
  export never paced is unchanged.
- **The composite says how far the history is behind**: the plan records the most positions a stream
  stage of the frame still lacks (`VideoRenderer::stream_short`), `GpuStages.stream_short` and
  `Composed.catching_up` carry it to the app.
- **Loading names the catch-up where Loading is written**: the scrim's centre text reads
  "Loading — catching up: N" (N the positions the history still lacks) while a stream stage is behind,
  plain "Loading" otherwise; the corner marker and the held/s counter go (five catalogs). V-fps counts
  complete new pictures, as it does for every incomplete composite.
- **The trace says it**: the `loading` record carries the positions (`b`), `kt-cli trace loading` names
  the stream catch-up; the held flags stay readable for older traces only.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-plan`: the live preview's pacing has no held frame; a short frame is incomplete with its
  shortage reported; the held-frame requirement is replaced by "a stage never runs over a stand-in".
- `ui-timeline`: V-fps without a held count; the corner marker is replaced by the Loading text naming the
  catch-up.
- `telemetry`: the `loading` record carries the catch-up positions.

## Impact

- `kt-engine` (`video_stream.rs`, `video.rs`, `player.rs`, `compose_queue.rs`, `video_plan.rs`),
  `kt-plugin-api` (`StreamKernel::held` removed), `fx-video-codec` (its trait impl; `mosh::held` stays
  as the schedule's answer t2270 pins), `kt-app` (preview draw, perf, five catalogs), `kt-time`
  (trace docs), `kt-cli` (`trace loading`).
- No file format change; no OTIO export change.
