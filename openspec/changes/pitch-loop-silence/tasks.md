# Tasks

Test numbers t906–t911 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done).

## 0. Records before code

- [x] 0.1 The owner's claim (ktprompts v26.10.18 item 7) is recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this change's artifacts written. Verify `openspec validate pitch-loop-silence --strict` is green

## 1. Prove the cause before the fix

- [ ] 1.1 Write t906 and t907 in `src/libs/kt-engine/tests/loop_fx.rs` (pitch +7 st on a 440 Hz sine, `render_looped_with`; both engines; regions 1 s, 6 000, 3 000 and 700 frames). Verify they are RED and record here the measured silent run after each wrap (predicted ≈ 4 608 frames + up to half a grain) and which regions are silent throughout (predicted: ≤ ≈ 5 100 frames). If the numbers differ, stop and revise design.md before stage 2
- [ ] 1.2 Write t909 in `src/libs/kt-engine/tests/audio_lookahead.rs` (limiter, 220 Hz before the region end, 1 kHz after it). Verify it is RED with ≈ 240 frames of the 1 kHz tone after each wrap, and record the number here
- [ ] 1.3 Run `kt-cli probe` on a looped region of a pitched clip in a project built from our fixtures (`src/fixtures/gen.sh` media) with `--time` on a release build. Verify the probe lists the event with its pitch slot among the effective events and record the `--time` render cost before the fix

## 2. Stream-keyed look-ahead nodes

- [ ] 2.1 `PitchNode` (`src/libs/kt-fx/src/audio/nodes.rs`): ring, accumulator and grain grid keyed by the stream position set at a real seek and advanced by processed frames; the `(stream, clip offset)` list for the curve lookup; no `restart` on a seam (design D1, D4). Verify t337, t338, t339, t322 (the registry frame) and t910 are green
- [ ] 2.2 Commit stage 2 (`cargo test -p kt-fx -p kt-engine`)

## 3. The input wraps first

- [ ] 3.1 `ClipProcessor` (`src/libs/kt-fx/src/audio/mod.rs`): an optional clip-local loop span on `read`, the input-side fold in `produce` (chunk split at the fold), the expected output jump recognised without a second re-seat (design D2). Verify t906, t907 and t909 are green, and t166, t167, t14 are green unchanged
- [ ] 3.2 `src/libs/kt-engine/src/audio.rs`: `render_looped_with` passes the region to every event that covers the region end; the source window and the prefetch request fold the extension (design D3); the track stages get the same span. Verify t908 (bytes, slicing, `wanted ⊇ read`), t911, t336 and t167b are green
- [ ] 3.3 Re-run the probe of 1.3 on the release build. Verify the render cost of the looped region did not grow by more than 5 % and record both numbers here
- [ ] 3.4 Update `src/libs/kt-fx/CLAUDE.md` "Rules of a lookahead node" (rings are stream-keyed; at a loop wrap the input wraps first). Verify `cargo test --workspace --no-fail-fast` and `cargo clippy --all-targets` (no new warnings); commit stage 3

## 4. Finish

- [ ] 4.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` here; ask the owner to repeat item 7 (a loop and a small loop with pitch) on that build and record his answer in `docs/user-feedback.md`
- [ ] 4.2 After archiving: append the "why" (the empty ring at every seam, the input that read past the region end, D1 stream keys, D2 input-first wrap) to `openspec/specs/audio-fx/design.md` and `openspec/specs/audio-render/design.md`
