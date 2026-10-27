# Tasks

Test numbers t872–t878 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done, no `Co-Authored-By` or other agent lines).
Stage only this change's files: the checkout may hold another session's uncommitted edits.

## 0. Records before code

- [x] 0.1 Append "Version check (2026-10-27)" to `docs/user-feedback.md` before "What the user repeated more than once": the owner's request and the three answers (View checkbox, "kt26.10 (outdated)", normal releases only). Verify the entry names `update-check` and has no Cyrillic
- [x] 0.2 Write the feature note `docs/features/resilience/environment/update-available-notice.md` (F-RES-013, reconnaissance of the five editors, `CAPABILITY: openspec/specs/distribution/`) and update the `resilience/` row and the summary table in `docs/features/README.md`. Verify `python3 src/scripts/features-index.py --check` is green
- [x] 0.3 Commit stage 0 with the change's artifacts. Verify `openspec validate update-check --strict` is green first

## 1. Tests first

- [ ] 1.1 Create `src/apps/kt-app/src/update_check.rs` with only the public signatures of design D2–D5/D9 (`decide`, `due_in`/`record_in`, `resolve_consent`, `Checker`) returning placeholders, and write t872, t873, t874, t877 in it as the spec scenarios say. Verify they compile and are RED, and record the first failure line here
- [ ] 1.2 Write t875 and t878 in `src/apps/kt-app/src/reach_tests.rs` (harness: inject the checker's outcome, no network), each with the `- **PATH**` of the spec. Verify both are RED and t799 accepts their PATH lines

## 2. The checker

- [ ] 2.1 Implement `decide` (D2: asset regex `kinetren-(\d\d\.\d\d\.\d\d)-([0-9a-f]{8})-`, prerelease/draft, nogit) and the `release-build.sh` naming guard. Verify t872 is green
- [ ] 2.2 Implement the state file (D3 UTC day, D4 `day=`/`latest=`, tmp + rename, garbage → due, a failure writes nothing). Verify t873 is green
- [ ] 2.3 Implement `resolve_consent` and the process-wide `install`/`global`, the lazy thread, the agent of D10 and the `Outcome` slot with one `request_repaint` (D5, D6, D9). Verify t874 and t877 are green and `on_exit` never waits (t874's 50 ms bound)

## 3. The program

- [ ] 3.1 `app.rs`: install the checker in `run` from the stored `update_check` value; take it in every `KtApp` constructor; start it on the first frame; take the outcome in `draw_all`, post the one Info notice (D8); append the title word and keep `title_outdated` (D7); store the choice in `save`. Verify `cargo build -p kt-app`
- [ ] 3.2 `toolbar.rs`: paint the word in the bell's warning colour after the title with its hover hint, crumbs after it; the View checkbox right after the telemetry one, disabled with the reason when blocked; unchecking clears the word in the same frame. Verify t875 and t878 are green
- [ ] 3.3 Catalogs: an `[update]` section in all five `i18n/*.toml` (checkbox, hover text, notice with two `{}`, title word with its parentheses); run `python3 scripts/subset-fonts.py` for the new CJK/Arabic glyphs. Verify t876 (new checks in `i18n_tests.rs` and `font_tests.rs`) and the existing catalog guards (t100, t125, t128, t171) are green
- [ ] 3.4 Run the UI guards of the ui-reach and ui-timeline specs (`reach_tests.rs` incl. t757 — no new window in `WINDOWS`, t799; `layout_tests.rs`). Verify green, naming any failure that belongs to another open change
- [ ] 3.5 Commit stages 1–3 (`cargo test --workspace --no-fail-fast`, `cargo clippy -p kt-app --all-targets` with no new warnings)

## 4. Live and records

- [ ] 4.1 Live scenario `src/scripts/live/update.py`: a local HTTP server answers a release JSON with a newer asset; the program starts with `KT_UPDATE_URL` and the rig's own `XDG_DATA_HOME`; `kt_live.step` screenshots: the start (no window, bell shows 1, title marked), the hover hint on the word, the notices window, View with the checkbox, the title after unchecking. Verify `./scripts/live-check.sh update` is green
- [ ] 4.2 `src/apps/kt-app/CLAUDE.md` first line names `distribution`; `docs/features/README.md` rule 13 no longer says telemetry is the only network path (name `update_check.rs`). Verify `features-index.py --check` is green
- [ ] 4.3 Commit stage 4

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/` (and `--release` once, to see the release build start without a request when the checkbox is off and with one when on, via `scripts/ui-probe.py close_window`). Verify it is green and record the version and the live screenshots here
- [ ] 5.2 After archiving: set the note F-RES-013 to `done` with its frozen verdict and `CAPABILITY:`, and append the "why" (D2 asset-not-tag, D4 file-not-storage, D6 no exit wait, the owner's decisions) to `openspec/specs/distribution/design.md`
