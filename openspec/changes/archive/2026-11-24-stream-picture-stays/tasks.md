# Tasks: stream-picture-stays

> Owner Q1: b) share now (kkt-26.11.20 Q51). Tests reserved: t3812–t3823. Base: orch `5e7a8ea9`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D6, class walk D5), deltas `video-plan`, `export`; `openspec validate stream-picture-stays --strict`

## 1. The crossing in the trace (evidence, not a gate)

- [x] 1.1 the `StreamCross` record and the lap sum in `kt-cli trace … loop`, the memo's device bytes (`StreamHeld`, the card's memory risk); t3812, t3813
- [x] 1.2 Q1 decided by the owner: b) share now

## 2. The held layer input

- [x] 2.1 `LayerInput::Held`, `StreamKernel::render_held` (Datamosh: the walk's picture at `n` kept live, never read back); no `accepts` — an executor on another device reads the planes back itself
- [x] 2.2 the planes → RGBA pass, integer, equal to the processor's (`planes_to_rgba`, one formula); t3814–t3816
- [x] 2.3 the pictures a walk ahead computes stay held in the memo where the host holds pictures (`ComputeHost::holds_pictures`); t3823
- [x] 2.4 the compute generator's twin — not this change's shape (design "The compute generator's hold"): a held `VideoFrame` in the burst cache with a fallback at every processor reader; found-defects 909

## 3. One device per adapter (after Q1)

- [x] 3.1 the window's device is the process's one context (`compute::share_display`), the compute host over it where `share_refusal` allows; t3817, t3818
- [x] 3.2 the device loss over both; t3819 (UI frame time during a walk on the heavy showcase: not measured on a card)
- [x] 3.3 t3821 (`kt-cli preview --one-device`), t3822 (a paused mosh on one device at rest)

## 4. Export

- [x] 4.1 `export_engines` on a card on the compute host's device while it holds pictures; kt-cli holds them on a card; t3820
- [x] 4.2 found-defects 663 closed; `cargo test` of the touched crates, clippy, target-prune; commit per stage
