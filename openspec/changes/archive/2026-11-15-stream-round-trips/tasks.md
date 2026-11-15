# Tasks: stream-round-trips

## 1. The walk's state on the device (rows 401, 621, 402, 405, 135)

- [x] 1.1 `ComputeHost::{keeps, run_keeping}`; kt-gpu keeps job buffers as `Held`, spares on drop; t2708
- [x] 1.2 `StreamMemo` live tier; t2709
- [x] 1.3 `Batch::keep`; the walk carries held values across batches, leaves them live at its end, reads back the shown frame and the checkpoints; `needs` sees the live tier; t2700/t2706
- [x] 1.4 A refresh's I-frame in the live tier (402, t2703); a region at a held position in the walk's batch (405, t2704)

## 2. The shown frame to the composite (rows 620, 622)

- [x] 2.1 `Planes::to_rgba` row by row (t2701); `fit_into` at its own size is a copy, skipped in `video_stream::run` (t2702)
- [x] 2.2 Generators drawn by a compute shader: required across devices, row 663
- [x] 2.3 Row 622 verified real; the slow route shows its kept picture (t2705)

## 3. The export (row 490)

- [x] 3.1 The pipelined export pre-rolls on the decode thread and plans stream frames in order on the compose thread; t2707

## 4. kkt-26.11.15 N2 (rows 660–662)

- [x] 4.1 Diagnosis from the owner's traces, old vs new, the plain piece first
- [x] 4.2 No far walk without a stage on screen, walks bounded (660, t2710)
- [x] 4.3 A compute job waits for its own submission (661)
- [x] 4.4 The frame on screen's history at the display's class (662 partly, t2711)

## 5. Class walk and cost

- [x] 5.1 Every read-back-then-upload in kt-gpu and kt-engine — the list in `design.md`
- [x] 5.2 Release: stream-cost, pause-scan, preview, old vs new; idle on a pause
- [x] 5.3 found-defects rows out, new rows in; What's new

## 6. Records

- [x] 6.1 nextest of the touched crates; clippy
- [x] 6.2 Commit, target-prune; archive
