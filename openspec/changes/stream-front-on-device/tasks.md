# Tasks: stream-front-on-device

## 1. The front on the compute device (row 543)

- [x] 1.1 `compute::{Held, Init::Held}`, `stream::{OnDevice, StreamInput::on_device}` in kt-plugin-api
- [x] 1.2 kt-gpu: `held.rs` (buffer + spares under the ledger), `Init::Held` in the compute host, `GpuFxEngine::execute_held`, `front_on_compute`, `stream_front_engine` on the compute device, testkit
- [x] 1.3 kt-engine: `HeldOut`, `ClipStream::on_device`, the front memo's two forms, `with_front_bytes` / `KT_FRONT_HELD`
- [x] 1.4 Datamosh's walk converts a held frame on the device (`Batch::convert_held`)
- [x] 1.5 t2620–t2622; `kt-cli stream-cost` counts held runs

## 2. Rows 561, 562

- [x] 2.1 Row 561 tried (a ready geometry shape moving the forms, process-wide and then per device): t2334 red both ways — reverted, the row stays open with the verdict in design D6
- [x] 2.2 The calibration's result apart from its lock; a skipped job's records on a stale one carry no position; t2623

## 3. Class walk and cost

- [x] 3.1 Every place a GPU result is read back to be uploaded again — the list in `design.md`; callers walk
- [x] 3.2 The advancing thread's time per position, held vs bytes, on release (`stream-cost`, showcase `--heavy --mosh-behind`); idle load on a pause; the trace
- [x] 3.3 found-defects 543/561/562 out, new rows in; What's new

## 4. Records

- [x] 4.1 nextest of kt-gpu, kt-engine, fx-video-codec, kt-plugin-api, kt-cli, kt-app; clippy
- [x] 4.2 Commit, target-prune; archive
