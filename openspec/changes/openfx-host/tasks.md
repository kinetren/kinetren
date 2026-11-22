# Tasks: openfx-host

> DESIGN ONLY until the owner answers Q1–Q2 and the orchestrator's go; after `wasm-plugin-host`. Tests: t3974–t3977
> (t3978–t3979 spare).

## 0. Notes

- [x] 0.1 F-EXT-001 refreshed
- [x] 0.2 proposal (the owner's whole item and this design's reading), design (D1–D9 with the answers to items 2–4), deltas `plugins`, `ui-reach`; `openspec validate openfx-host --strict`
- [ ] 0.3 the owner's answers folded in

## 1. Host process

- [ ] 1.1 `kinetren-ofx`: bundle scan, the host suites, shared-memory frames, the pipe protocol; t3976
- [ ] 1.2 supervision: crash/hang → bypass, notice, restart; t3975

## 2. Plan

- [ ] 2.1 the OFX barrier node (readback, float conversion, upload), temporal reads, the persistent cache key; t3974

## 3. Interface

- [ ] 3.1 picker marks, hints, notices, Library rows; i18n ×5; t3977, live `ofx_plugin`

## 4. Close

- [ ] 4.1 What's new; F-EXT-001 verdict; registry O1–O4 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
