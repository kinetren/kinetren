# Tasks

Test numbers: t1113, t1114 (the rest of the t1105–t1114 block of kkt-26.11.01 item 15). Every stage
ends with a commit and `src/scripts/target-prune.py`.

## 0. Records

- [x] 0.1 The owner's second report in `docs/user-feedback.md`; this change's artifacts; the measurement on his shape (`zz_probe_owner`, not committed): joins 0.034 with `[slowed, pitch +5]` against 0.88–0.90 without pitch

## 1. Context through the whole chain

- [x] 1.1 Write t1113 (the owner's shape: copies, `[slowed, pitch]` directly and through a morpher, rate curve changing sign past the event, a reel as the media) and t1114 (a node before the pitch: nothing outside the event heard, its tail ends with the event; prefetch with a warp, forward and reversed). Verify RED before 1.2
  - RED: t1113 "Wsola chain: a join dips to 0.032 of the steady level"; t1114 "echo + pitch
    reversed false: sound outside the event".
- [x] 1.2 `chain_context` and the processor's `ctx`: drop the "no tail before" condition, a warp only when the speed at the start moves forward; `run_predecessors` requires no warp. Update t1109's expectations. Verify t1105–t1114, t334–t340, t906–t911, t17, t336, t973c green
  - Green: `cargo test -p kt-fx -p kt-engine` whole, t973c unchanged. The owner's shape
    (`zz_probe_owner`): `[slowed, pitch +5]` joins 0.034 → 0.894 (pitch alone 0.891, the body
    0.893); his file's events 2135–2138 now get the context.
- [x] 1.3 kt-fx/kt-engine CLAUDE.md lines; `cargo test --workspace`, `cargo clippy --all-targets`; commit

## 2. Finish

- [ ] 2.1 What's new line; `./scripts/finish.sh` with a fresh version stamp; version here
- [ ] 2.2 Archive; the "why" into `openspec/specs/audio-fx/design.md`
