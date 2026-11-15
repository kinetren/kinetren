# Tasks: frame-work-by-edit

## 1. The frame by its events (rows 153, 154)
- [x] 1.1 `LayerIndex` per renderer: events by time, dissolve partners; track stages' range, index, revision, masks
- [x] 1.2 The plan, the order, the stream stages, `stream_frame_bytes`/`stream_checkpoints` ask it
- [x] 1.3 Stream second input by halving; `link_stages` copies nothing without a stage
- [x] 1.4 Reel coverage by the window, one-pass `subtract`, once per (release, window)

## 2. The edit by index (rows 156, 157)
- [x] 2.1 `with_sequence_index` around the builders
- [x] 2.2 `LaneSpans`, `Reel::lane_busy`: validation and `reconcile_reel`

## 3. Curves (rows 168, 169)
- [x] 3.1 `LayerChain` and the layer's `ChainMemo`
- [x] 3.2 `RowCurves` (transitions, cameras, generators), `MasksPlan`, `Mask::set_spec`

## 4. Keystroke and export (rows 190, 191, 192)
- [x] 4.1 `update_gen`: other addresses on the builder thread, no project copy when it holds the values
- [x] 4.2 `project_may_stream` for the export's choice
- [x] 4.3 Row 192 measured on a ×10 synthetic project (design D9)

## 5. Executor and simulations (rows 364, 365)
- [x] 5.1 `plan_key` streams the masks, run keys on the stack
- [x] 5.2 Camera ISP passes on the stack, VHS one vector

## 6. GPU submits and the first lap (rows 134, 118)
- [x] 6.1 Measured on the showcase; verdicts in found-defects

## 7. Guards and contracts
- [x] 7.1 `kt_time::cost` rows `FrameClip`, `ChainClone`, `ParamByName`
- [x] 7.2 Tests t2660–t2669, t2672 (count guards, equivalences); red on the scan form
- [x] 7.3 Spec deltas; crate CLAUDE.md rules; found-defects rows closed, new rows 640–
- [x] 7.4 Release measurements: frame, edit, idle (pause, hover, effects window)

## 8. Check
- [x] 8.1 nextest of the touched crates, clippy, target-prune
