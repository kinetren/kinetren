# Tasks: wasm-plugin-host

> DESIGN ONLY until the owner answers Q1–Q3 and the orchestrator's go. Tests: t3957–t3961.

## 0. Notes

- [x] 0.1 F-EXT-003 refreshed (since 2026-11-20)
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `plugins`, `ui-reach`, `project-format`; `openspec validate wasm-plugin-host --strict`
- [ ] 0.3 the owner's answers folded in

## 1. Host

- [ ] 1.1 `kt-plugin-wasm`: manifest through the one check, module cache, the import surface; t3961
- [ ] 1.2 kernels across the boundary (audio, generator, transition; video WGSL on the GPU path); t3957
- [ ] 1.3 traps and fuel, the faulted kind, the notice; t3958, t3959

## 2. SDK and template

- [ ] 2.1 `kt-plugin-sdk`, `export_plugin!`, `src/plugins/template-wasm` built for wasm32 in CI

## 3. Interface

- [ ] 3.1 "Add a plugin…", Library rows (after `download-library`), picker mark; i18n ×5; t3960, live `wasm_plugin`

## 4. Close

- [ ] 4.1 What's new; F-EXT-003 verdict; registry P1–P5 moved (P6 to its own design)
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
