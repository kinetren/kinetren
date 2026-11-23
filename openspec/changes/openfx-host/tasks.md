# Tasks: openfx-host

> DESIGN ONLY until the orchestrator's go; after `wasm-plugin-host`. Revision 2: the owner's D4 (frames through shared
> memory, no copy); Q1 (a), Q2 (a) decided. Tests: t3974–t3979.

## 0. Notes

- [x] 0.1 F-EXT-001 refreshed
- [x] 0.2 proposal (the owner's whole item and this design's reading), design (D1–D9 with the answers to items 2–4), deltas `plugins`, `ui-reach`; `openspec validate openfx-host --strict`
- [x] 0.3 the owner's answers folded in (D4: D10 shared frames; Q1 a, Q2 a)

## 1. Host process

- [ ] 1.1 `kinetren-ofx`: bundle scan, the host suites, the socket protocol carrying handles; t3976
- [ ] 1.3 shared frames (D10): exportable images and semaphores and host-memory buffers in kt-gpu through the hal, the
      pool, the child's imports (GL, CUDA, OpenCL), the counted fallback, the trace path; t3978, t3979
- [ ] 1.2 supervision: crash/hang → bypass, notice, restart; t3975

## 2. Plan

- [ ] 2.1 the OFX barrier node (shared input, semaphores, a GPU conversion pass only when the depth differs), temporal
      reads; t3974

## 3. Interface

- [ ] 3.1 picker marks, hints, notices, Library rows; i18n ×5; t3977, live `ofx_plugin`

## 4. Close

- [ ] 4.1 What's new; F-EXT-001 verdict; registry O1–O4 moved
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator), version in the report
