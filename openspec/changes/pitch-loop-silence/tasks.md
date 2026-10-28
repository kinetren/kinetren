# Tasks

Test numbers t906–t911 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done).

## 0. Records before code

- [x] 0.1 The owner's claim (ktprompts v26.10.18 item 7) is recorded in `docs/user-feedback.md`, round "The owner's list of 2026-10-27 and ktprompts v26.10.18"; this change's artifacts written. Verify `openspec validate pitch-loop-silence --strict` is green

## 1. Prove the cause before the fix

- [x] 1.1 Write t906 and t907 in `src/libs/kt-engine/tests/loop_fx.rs` (pitch +7 st on a 440 Hz sine, `render_looped_with`; both engines; regions 1 s, 6 000, 3 000 and 700 frames). Verify they are RED and record here the measured silent run after each wrap (predicted ≈ 4 608 frames + up to half a grain) and which regions are silent throughout (predicted: ≤ ≈ 5 100 frames). If the numbers differ, stop and revise design.md before stage 2
  - RED as predicted. t906: a silent run starting exactly at every wrap — 4 136 frames (WSOLA) and
    2 839 (vocoder). The prediction was ≈ 4 608 + up to half a grain; it is SHORTER because a +7 st
    grain reads its ring 1.5× faster and reaches live material before the end of the window — the
    same mechanism (the ring restarted empty at the seam), so design.md stands. t907: 19 of 19
    bursts near-silent at a 700-frame loop, 17 of 19 at 3 000, 10 of 19 at 6 000, both engines.
    t908 is green before the fix too — it is the determinism / slicing / `wanted ⊇ read` guard.
- [x] 1.2 Write t909 in `src/libs/kt-engine/tests/audio_lookahead.rs` (limiter, 220 Hz before the region end, 1 kHz after it). Verify it is RED with ≈ 240 frames of the 1 kHz tone after each wrap, and record the number here
  - RED: "478 frames differ, first at lap 1 + 1, last at lap 2 + 239" — 239 frames of material
    from past the region end after each of the two wraps (the 5 ms window). Measured as a difference
    to the same loop over a pure 220 Hz source, not by a Goertzel probe: on a 240-frame window the
    220 Hz leakage into the 1 kHz bin is ≈ −25 dB and would hide the answer.
- [x] 1.3 Run `kt-cli probe` on a looped region of a pitched clip in a project built from our fixtures (`src/fixtures/gen.sh` media) with `--time` on a release build. Verify the probe lists the event with its pitch slot among the effective events and record the `--time` render cost before the fix
  - A project of a 20 s audio event (synth sine 440, our synthetic fixture: `src/fixtures/gen.sh`
    media need the `ffmpeg` binary and add nothing for a sine) with pitch +7 st; the probe lists it
    among the effective events with its one FX slot. Release, region 5.0–5.5 s: linear 4 ms per
    0.5 s, looped (4 laps) 15 ms per 2 s.

## 2. Stream-keyed look-ahead nodes

- [x] 2.1 `PitchNode` (`src/libs/kt-fx/src/audio/nodes.rs`): ring, accumulator and grain grid keyed by the stream position set at a real seek and advanced by processed frames; the `(stream, clip offset)` list for the curve lookup; no `restart` on a seam (design D1, D4). Verify t337, t338, t339, t322 (the registry frame) and t910 are green
  - `PitchNode` stream-keyed: `written` is the stream, `clip_next` the expected clip position, a
    jump pushes `(stream, clip − stream)` into `deltas` (pruned behind the oldest grain still to be
    synthesized); `ratio_at` reads the curves at the clip position; `restart` only at a real start.
    t337, t338, t339, t322 and t910 green. Found on the way: the VOCODER's level depends on the phase
    its synthesis starts from (no vertical phase coherence) — cold renders at twelve positions give
    0.56 or 0.25 RMS; a seam lands on such a phase too, so a looped vocoder lap may be 6.5 dB lower
    than the first. A phase reset at the first grain after a seam only re-rolls the dice (tried:
    lap 1 fixed, lap 2 broke). Out of scope (design Non-Goals: the algorithms); t906's lap-level
    check is for WSOLA, the silence check for both; a separate change is proposed.
- [x] 2.2 Commit stage 2 (`cargo test -p kt-fx -p kt-engine`)
  - Committed together with stage 3 (one commit): the stage-1 tests are red until the input-side
    fold too, and `cargo test` must be green on every commit.

## 3. The input wraps first

- [x] 3.1 `ClipProcessor` (`src/libs/kt-fx/src/audio/mod.rs`): an optional clip-local loop span on `read`, the input-side fold in `produce` (chunk split at the fold), the expected output jump recognised without a second re-seat (design D2). Verify t906, t907 and t909 are green, and t166, t167, t14 are green unchanged
  - `ClipProcessor::set_loop_span`, `fold`, `reseat_source`, `in_next`; `produce` splits a chunk at
    every point where the input or a node's content position crosses the span end; `read` takes the
    output's jump to the span start as expected when the input is already there; an input folded to
    before the event's material is silence. t906, t907, t909 green; t166, t167, t14 green unchanged.
- [x] 3.2 `src/libs/kt-engine/src/audio.rs`: `render_looped_with` passes the region to every event that covers the region end; the source window and the prefetch request fold the extension (design D3); the track stages get the same span. Verify t908 (bytes, slicing, `wanted ⊇ read`), t911, t336 and t167b are green
  - `AudioRenderer::loop_region` (set by `render_looped_with` for its calls): `render_fx_clip` gives
    an event covering the region end its clip-local span; `render_stages` sums a track's events over
    the FOLDED input window. The source window needs no fold: the folded input stays inside the
    material, and the region head is in the request (t908 `wanted ⊇ read` green). t908, t911, t336,
    t167b green.
- [x] 3.3 Re-run the probe of 1.3 on the release build. Verify the render cost of the looped region did not grow by more than 5 % and record both numbers here
  - Release, the same project: linear 4 ms per 0.5 s, looped 15 ms per 2 s (and 15 / 60 ms on a 2 s
    region) — unchanged.
- [x] 3.4 Update `src/libs/kt-fx/CLAUDE.md` "Rules of a lookahead node" (rings are stream-keyed; at a loop wrap the input wraps first). Verify `cargo test --workspace --no-fail-fast` and `cargo clippy --all-targets` (no new warnings); commit stage 3
  - The look-ahead node rules gain "the rings are stream-keyed and survive every seam" and "at an
    armed loop wrap the INPUT wraps first". `cargo test --workspace --no-fail-fast`: 1167 passed, red
    only t815 (the restore-record write timed at 3.7 s under load; green alone). Clippy: one new
    `type_complexity` in t916's table (slowed-reverb-is-an-event) factored; nothing new from this change.

## 4. Finish

- [ ] 4.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` here; ask the owner to repeat item 7 (a loop and a small loop with pitch) on that build and record his answer in `docs/user-feedback.md`
  - finish.sh → **kt26.10.29-c3462989** (2026-10-29, on c346298). Cargo and SIMD green. Live: 49
    scenarios, `fade` red in the 4-worker run and green alone (a known timing flake), `nestdead` red twice
    — a race in the scenario, not the program: it read the probe dump right after the right-click,
    before the menu's frame (red about one run in two on the same binary, green with any extra wait).
    The scenario now waits for `nest.enter` before listing the items; 3 of 3 green alone.
    OPEN: the owner's repeat of item 7 on this build (asked in the report of 2026-10-29); the task
    stays open until his answer is in `docs/user-feedback.md`.
- [ ] 4.2 After archiving: append the "why" (the empty ring at every seam, the input that read past the region end, D1 stream keys, D2 input-first wrap) to `openspec/specs/audio-fx/design.md` and `openspec/specs/audio-render/design.md`
