# Tasks: stream-picture-stays

> Owner Q1 first. Tests reserved: t3812–t3823. Base: orch `3420bf58`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D6, class walk D5), deltas `video-plan`, `export`; `openspec validate stream-picture-stays --strict`

## 1. Measure (Q1 a)

- [ ] 1.1 the `StreamCross` record and the lap sum in `kt-cli trace … loop`; t3812, t3813
- [ ] 1.2 the owner's trace (or the heavy showcase on his machine class) read; the crossing per frame written into design D1; Q1 decided

## 2. The held layer input

- [ ] 2.1 `LayerInput::Held`, `StreamKernel::render_held` (Datamosh), the compute generator's twin; `accepts`
- [ ] 2.2 the planes → RGBA pass, integer, equal to the processor's; t3814–t3816

## 3. One device per adapter (after Q1)

- [ ] 3.1 `GpuContext` opens the window's device; `run` hands it to eframe (`WgpuSetup::Existing`); the compute host over it; t3817, t3818
- [ ] 3.2 the device loss over both; t3819; UI frame time during a walk measured on the heavy showcase
- [ ] 3.3 t3821, t3822 (rest measured with the track window and a Datamosh loop paused)

## 4. Export

- [ ] 4.1 `export_engines` on a card on the compute host's device; t3820
- [ ] 4.2 found-defects 663 closed; `cargo test` of the touched crates, clippy, target-prune; commit per stage
