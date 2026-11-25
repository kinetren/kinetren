# Tasks: memory-breakdown

> DESIGN ONLY until the orchestrator's go; no open question. Revision 3. Tests: t3962–t3965, t4232–t4234 (t3966 went to
> loudness-norms; t3967 spare). Areas: kt-time `mem.rs` (appends two tenants: the second of this and
> device-decode-sources 2.1 to merge takes the next numbers), kt-gpu `memory.rs`, SHELL (`app.rs`, Diagnostics).

## 0. Notes

- [x] 0.1 F-RES-011 refreshed; proposal, design (D0–D7), deltas `telemetry`, `ui-timeline` (the requirement replaced), `ui-reach`; strict
- [x] 0.2 B12 read again (the frames pair goes too), the ledger's device kinds, the folder instead of `ktindex.db`

## 1. Accounts

- [ ] 1.1 the two tenants, `on_device`; every `match`/array over `KINDS`; `kt-cli trace machine`; t3962
- [ ] 1.2 the decoders' account (kt-ffmpeg/kt-media), the audio state's (`fxstore.rs`); t4232
- [ ] 1.3 `Project::approx_bytes`, `Snapshot` estimates in steps; t4233
- [ ] 1.4 wgpu `counters`, `kt_gpu::memory::video`; t4234; `./scripts/windows-test.sh` (the feature builds under wine)

## 2. Interface

- [ ] 2.1 `mem_view.rs`; the section with its bars and lists; the disk by `known_sizes`; t3963, t3964; t918, t1145–t1148, t2398
- [ ] 2.2 the status bar, its hint and click; t2397 rewritten by B12 (the requirement replaced); layout's widths (`layout_tests.rs:1985`); t3965; i18n ×5

## 3. Close

- [ ] 3.1 live scenario `memory` (our fixture: the counter, its hint, the click, the three bars, a segment's hover), the load check: the heavy showcase paused, the section open, the mouse over it
- [ ] 3.2 What's new; F-RES-011 verdict; registry Q1–Q3 and B12
- [ ] 3.3 `src/scripts/finish.sh` (orchestrator), version in the report
