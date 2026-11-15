# Tasks: stream-round-trips

## 1. The walk's state on the device (rows 401, 621, 402, 405, 135)

- [x] 1.1 `ComputeHost::{keeps, run_keeping}`; kt-gpu keeps job buffers as `Held`, spares on drop; t2708
- [x] 1.2 `StreamMemo` live tier; t2709
- [x] 1.3 `Batch::keep`; the walk carries held values across batches, leaves them live at its end, reads back the shown frame and the checkpoints; `needs` sees the live tier; t2700/t2706
- [x] 1.4 A refresh's I-frame in the live tier (402, t2703); a region at a held position in the walk's batch (405, t2704)

## 2. The shown frame to the composite (row 620)

- [x] 2.1 `Planes::to_rgba` row by row (t2701); `fit_into` at its own size is a copy, skipped in `video_stream::run` (t2702)
- [ ] 2.2 Generators drawn by a compute shader: verdict
- [ ] 2.3 Row 622: the slow chain's composite into egui — verified

## 3. The export (row 490)

- [ ] 3.1 The pipelined export advances stream stages in frame order; pre-roll on the decode thread; t2707 (pipelined == serial with a Datamosh)

## 4. Class walk and cost

- [ ] 4.1 Every read-back-then-upload in kt-gpu and kt-engine — the list in `design.md`
- [ ] 4.2 Release: V-fps, time per history position, time to a complete paused frame, processor load on a pause; the trace
- [ ] 4.3 found-defects rows out, new rows in; What's new

## 5. Records

- [ ] 5.1 nextest of the touched crates; clippy
- [ ] 5.2 Commit, target-prune; archive
