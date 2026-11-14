# Proposal: stream-input-order

## Why

found-defects 484, read in the owner's project `problem_region_26_11_09` (kkt-26.11.10b K5): event 2149
carries camera_isp → datamosh → vhs_tape, but the stream stage's kernel read the RAW decoded frame
(`ClipStream::with_frame`), and the chain the executor ran still held Camera ISP — after the stage. So
Datamosh coded a picture no camera had made, and Camera ISP was laid over the mosh: the order the person
built was not the order run. The owner (11.11): "A stage either has its own input or the frame is
Loading" — the stream's input is the chain's running picture in front of it, not the decoded frame.

## What Changes

- The slots in front of a stream stage that change the picture (`front_slots`: not audio, not a layer
  strength, not an identity geometry, not the leading Geometry node) run on every frame of the main
  stream — the event's own source and its history in the event before — before the kernel reads it,
  with their curves at the position's material time, at the stream's size, on an executor the host
  injects (`kt_engine::video::install_stream_front`, kt-gpu `stream_front_engine`: the offline engine's
  sibling). The chain after the stage no longer runs them (`after_chain`: identity in their place, the
  indices masks and layer strengths name stay).
- The stream's addresses (`ClipStream::ident`) name the front (its slots and curves, and the position
  when curved): an edit of a slot in front invalidates the history; an edit after the stage does not.
- No device or a refused plan: the frame is missing (Loading), never the decoded picture in its place.
- A bounded memo of front outputs (64 MB) keeps a re-walk or a second reader from running the slots again.
- The preview's pipeline prewarm reaches the front's device when a project has a front.
- `kt-cli showcase --mosh-behind` puts Datamosh after each styled cut's first look (the bench of the cost).

## Capabilities

### Modified Capabilities
- `video-fx`: a stream stage reads the output of the slots in front of it.

## Impact

kt-engine (`video_stream.rs`, `video.rs` layer_src, `prewarm.rs`, `player.rs`), kt-gpu (`lib.rs`
`stream_front_engine`, `testkit`), kt-app/kt-cli (install the door), kt-cli showcase. No format change;
OTIO unaffected (the chain order was always written).
