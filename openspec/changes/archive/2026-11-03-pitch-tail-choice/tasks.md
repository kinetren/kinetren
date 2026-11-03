# Tasks

## 1. The contract: fork and the echo row

- [x] 1.1 `kt-plugin-api`: `AudioState::fork` (default `None`), `CurveRead::clone_box` + `Clone for Box<dyn CurveRead>`, `Clone` on the `dsp` primitives, `Decl.echo_row`; builds (`cargo check --workspace --all-targets`)
- [x] 1.2 Every audio plugin state (`src/plugins/fx-audio-*`) and kt-fx's `OwnedCurve` derive `Clone` and implement `fork`; t1330 (every built-in kind forks and continues byte for byte)
- [x] 1.3 `pitch_shift` passport: the choice row `echo_before` = cut | ring (`omit_default`), `Decl.echo_row`; t973/t973c and t322 stay green (the default writes nothing, sounds the same)

## 2. The processor: rings on

- [x] 2.1 `ClipProcessor`: the effective choice (`ring`: context applies, the `ctx` slot's echo row is on, a node before `ctx` rings); the node loop factored so the main stream and the twin run the same code (`Pass::run`)
- [x] 2.2 D2 measured and dropped: restarting the ringing nodes before the pitch at the in-point put the vocoder at 0.53 (+7) / 0.29 (−7) at every event start (WSOLA 0.93 / 0.85); the start stays the "cut" start (t1331 and t1333 hold the body byte for byte)
- [x] 2.3 D3: the fork at the input's out-point, the twin fed silence, the gate merge, the promotion and `ringing`; reset/seek/seam rules; t1331, t1334, t1339
- [x] 2.4 D5 and joins: `split_continued` + `set_ring_end` (a piece the next piece continues does not ring) — t1332; t1333, t1335 (one piece == pieces, `wanted ⊇ read`, the loop in pieces); t1105–t1114, t322, t973c green. Measured: the deepest 64-frame window of the first 1 024 frames past the out-point / the echo's level — WSOLA +7 0.97, −7 0.87, vocoder +7 0.82, −7 0.32; nothing of the next material (byte-equal over a loud and a silent continuation). The owner's five copies on a 440 Hz tone, deepest join window: "rings on" 0.87 (WSOLA) / 0.46 (vocoder), "cut" 0.90 / 0.76 — the previous copy's reverb and last pitch window over the next copy of the same tone

## 3. The file

- [x] 3.1 `project_file`: format 3 by content (`Project::has_ringing_echo` over every reel's events, tracks and morpher patches), reads 1–3, refusal above; the OTIO envelope v15 by content; t1336 (kt-model + kt-otio); t1264 moved to "above the ceiling"; t811/t812/t1260/t1265 green

## 4. The interface

- [x] 4.1 `fx_editor.rs`: the pitch panel's echo line (combo + the two-part hint through the status line), one base edit; catalogs ×5 in the owner's words (the hint split in two keys under the 160-char limit); `python3 scripts/subset-fonts.py`; t1337 (PATH), t100, t125, t131 green
- [x] 4.2 Live scenario `src/scripts/live/pitchtail.py` on our fixture (a step screenshot per action, the saved `.kt` checked): t1338 green on `KT_LIVE_DISPLAY=:257` (:231 was another session's), four steps

## 5. Docs and wrap-up

- [x] 5.1 Specs merged on archive; `openspec/specs/audio-fx/design.md` and `audio-render/design.md` get the why (D2–D5); kt-fx and kt-plugin-api CLAUDE.md get the rules (done)
- [x] 5.2 `docs/guide/whats-new.md` `## kt26.11` line; `docs/ktprompts/kkt-26.11.01-index.md` K15 row moved; `docs/features/audio/processing/pitch-shift.md` updated; `features-index.py --check`, `ktprompts-index.py --check`, `specs-user-path.py --check`
- [x] 5.3 `cargo clippy --all-targets` (no new warning), `cargo test --workspace` green (t815/t640 red only under the machine's load, green alone); archive the change; `git merge master`; `./scripts/finish.sh` and the version in the report

`finish.sh`: kt26.11.03-3d5a0137 — live scenarios green (pitchtail: four steps); the workshop's only red
was t640 ("sidecar write frames failed: sqlite: database is locked" under the machine's load), green alone.
