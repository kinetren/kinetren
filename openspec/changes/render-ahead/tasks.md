# Tasks

## 1. Proposal and base measurement

- [x] 1.1 Proposal, design, spec deltas (video-plan, transport, telemetry); tests t2920–t2939 reserved in the shared notes
- [x] 1.2 Base (stage 18, 7ad1ad71) release `kt-cli preview` of the VHS loop with `--trace`

## 2. The bound (R5) and utilisation in the trace

- [ ] 2.1 `arbiter`: `bound()` = one frame period; `room` and `AHEAD_PIECES` deleted; a lower piece is cut at the bound; t2251, t2430, t2431, t2433, t2441 at the new bound; t2517 → t2923; kt-gpu CLAUDE.md
- [ ] 2.2 `Ev::GpuFrame` written by the pass timer's harvest; `trace loop` per-lap device share and "while frames were missing"; `trace gpu` frames; t2926
- [ ] 2.3 `cargo test -p kt-gpu -p kt-time -p kt-cli`, clippy; commit; target-prune

## 3. The producer (R2)

- [ ] 3.1 `VideoGpuExec::{ahead, as_any, poll_done}` and `Ahead`; kt-gpu's on-device keep into a same-device keeper's present memo with a done flag, the bytes form otherwise; t2922
- [ ] 3.2 `render_ahead.rs`: one near lane, the order (frame on screen + lead, the lap or the decoded window), skips for the lap, the measured depth (t2921), the executor choice (display sibling on a card, the side engine on software); `loop_fill.rs` deleted
- [ ] 3.3 Player: the producer in place of the fill (kick on play, seek and every playing prefetch pass, `ahead_gpu_setter`), `compose_ahead`; compose thread presents only kept moments while playing (`KT_RENDER_AHEAD=off` for an A/B)
- [ ] 3.4 `FAR_STALLS` → the cache's decoded count (`BurstCache::video_inserted`); t2924
- [ ] 3.5 `tests/render_ahead.rs`: t2334–t2336 moved, t2920, t2925; t2432 green; `cargo test -p kt-engine` (nextest), clippy
- [ ] 3.6 kt-cli `preview` lines, kt-app setter; build kt-app; commit; target-prune

## 4. Docs

- [ ] 4.1 kt-engine and kt-gpu CLAUDE.md; the "owner's latency limit" wording corrected in `openspec/specs/video-plan/design.md` and the loop-laps-regression design
- [ ] 4.2 found-defects rows touched (663, 692, 693 state); What's new line; `kkt-26.11.17-index.md` row S4

## 5. Measured and checked

- [ ] 5.1 Release bench on llvmpipe, base against this branch, interleaved: `kt-cli preview … --loop`, `trace loop`, `trace gpu`; lap-1 V-fps and device share while frames were missing into design "Measurements"; what needs a card stated
- [ ] 5.2 Idle cost on the heavy showcase (pause, pointer over a static place, the effects window open): live `idle` scenario
- [ ] 5.3 Live scenario `render_ahead` (loop playback), t2927
- [ ] 5.4 `cargo test` of the touched crates under nextest, clippy; commit; target-prune
- [ ] 5.5 Archive: specs merged, the "why" into `openspec/specs/{video-plan,telemetry}/design.md`
