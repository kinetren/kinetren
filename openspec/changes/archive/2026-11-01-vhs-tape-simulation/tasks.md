# Tasks: vhs-tape-simulation

## 1. The contract

- [x] 1.1 `kt-plugin-api` barrier: `Node::PassAt`, `check_graph` size tracking; `frame_values` /
      `gpu_values` defaults; the registry checks the value form; `ParamUnit::Um`/`Us` (D6, D7)
- [x] 1.2 hosts: kt-fx `apply_barrier` and kt-gpu `chain_ops` call the value forms; the engine renders
      `PassAt` at its size and counts it in the TooLarge check; kt-app `unit_suffix`
- [x] 1.3 t1187 (registry refusals, value forms keep the bytes); t972, t1051–t1054 stay green

## 2. The physics (CPU reference)

- [x] 2.1 crate `src/plugins/fx-video-tape`: standards table, fixed kernels designed once (D4), our
      f32 cos/sin/atan2 (D5), the line table (D3, D8), stages 1–8 and F (D2), threads over row bands
- [x] 2.2 the manifest: passport of physical quantities, presets, `AlphaRel::Destroys`; the assembler,
      the workspace, kt-app `VIDEO_ORDER`, catalogues ×5, t328 GOLDEN_ALL row, t329 class, t973 form,
      t1051 row
- [x] 2.3 t1180–t1185, t1188–t1192 (`tests/physics.rs`)

## 3. The GPU graph

- [x] 3.1 WGSL passes mirroring each stage, the generated constants, the graph (D2)
- [x] 3.2 t1186: measure CPU↔GPU, set `FrameTol::SIGNAL` at ×2; determinism on the device

## 4. The panel

- [x] 4.1 the deck face in the effects window (D11); t1193 with its PATH; reach/layout/i18n/font
      tests green
- [x] 4.2 a live scenario with screenshots on our fixtures

## 5. Close

- [x] 5.1 cost per 1080p frame on a release build (once), recorded in design D10
- [x] 5.2 What's new line, registry V1 closed, note F-FX-009 frozen as done
- [ ] 5.3 `cargo test --workspace`, `cargo clippy --all-targets`, `git merge master`,
      `./scripts/finish.sh` — version recorded here
