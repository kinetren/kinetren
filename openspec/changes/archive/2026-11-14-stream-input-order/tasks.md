# Tasks: stream-input-order

## 1. The front of a stream stage

- [x] 1.1 `StreamStage::{front, front_key, front_curved}`, `front_slots`, the front's key with curves and placement
- [x] 1.2 `ClipStream::with_frame` runs the front on the main stream through the injected executor; `ident` names the front; the front memo
- [x] 1.3 `layer_src` runs the chain after the stage without the front (`after_chain`)
- [x] 1.4 The door: `install_stream_front` in kt-app, kt-cli and kt-gpu's testkit; `kt_gpu::stream_front_engine`
- [x] 1.5 The preview prewarm reaches the front's device (`Reach.front`)
- [x] 1.6 t2470–t2472

## 2. Class walk and cost

- [x] 2.1 Every path that builds a stream input, every stage reading the decoded frame, the audio pairs — the list in `design.md`
- [x] 2.2 `kt-cli showcase --mosh-behind`; the history walk's cost with and without a front on release — in `design.md`
- [x] 2.3 Idle load on a pause with the front project

## 3. Records

- [x] 3.1 found-defects 484 out; 540–542 in; What's new line
- [x] 3.2 nextest of kt-engine, kt-gpu, kt-cli, kt-app; clippy
- [x] 3.3 Commit, target-prune; archive
