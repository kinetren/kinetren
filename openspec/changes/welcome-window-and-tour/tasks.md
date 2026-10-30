# Tasks

Test numbers t984–t989 are reserved by this change. The update line of the What's new panel reads
the state of `update-check` (stage 2 of that change); everything else does not wait for it. Every
stage ends green and is committed at once (English message, first line = the stage and what was
done).

## 0. Records before code

- [x] 0.1 The owner's words (item 8 of `kkt.txt`; the answers "a Guide window", "a welcome screen", "highlight the guide on the welcome screen"; the request of 2026-10-28 about the intro window) are recorded in `docs/user-feedback.md`, round of 2026-10-27/28; this proposal recorded
- [x] 0.2 Feature notes: F-HLP-028 carries the dated section "Revised by the owner, 2026-10-28" and the status `accepted`; F-HLP-027, F-HLP-023 and F-HLP-029 carry dated revision paragraphs naming this change
- [x] 0.2b The README summary-table row of F-HLP-028 (status `rejected` → `accepted`) updated by hand from `features-index.py --table`. Verify `python3 src/scripts/features-index.py --check` and `openspec validate welcome-window-and-tour --strict` green; commit stage 0
  - Done in 1314b9b (the row reads `accepted`); both checks green on 2026-10-30.
- [x] 0.3 The open question 1 of design.md (the accent frame) is answered by the owner and recorded in `docs/user-feedback.md`; until then the frame is not built
  - Answered 2026-10-28 ("3. yes, it may draw"), recorded in `docs/user-feedback.md` (commit 5eb5b34): a static outline drawn by the target itself, no layer, no pulsation. Built in stage 4.

## 1. Probes before code

- [x] 1.1 Probe: can egui 0.36 open a `menu_button` popup of the menu bar programmatically (memory popup id of the File menu) — a throwaway harness test in `src/apps/kt-app/src/reach_tests.rs` that opens File by code and finds "Create a generator…" in the accessibility tree of the next frame. Record the answer here; if no, adjust the t988 PATH to the fallback of design (Risks) before stage 4
- [x] 1.2 Probe: render one poster frame of a fixture project through the reel-preview offline path at 320×180 on release, timing it (`cargo build --release -p kt-app`; a `#[ignore]` timing test); record the time here
  - Measured 2026-10-30 on release (`cargo test --release -p kt-app -- --ignored probe_poster_render_time --nocapture`, load average ~30 from parallel sessions): one 320×180 poster of the lesson project = 14.3 ms cold, 11.6 ms warm — against a Ctrl+S it is noise, and it runs on its own thread, paused during playback. The probe CAUGHT A DEFECT: the first posters were flat black — a generator's values live on its event, so the plan asks for per-event generator sources (`GEN_TAG` addresses from `kt_engine::gen_sources`), which export opens itself and `render_poster` did not. Fixed; t985's poster half now asserts the poster is not flat (red on the old code). The reel strip (`render_reel_preview`) has the same shape and likely the same gap for generator events inside a reel — not touched here, a follow-up.

## 2. Records in the user store

- [x] 2.1 `user_store.rs`: `recent.json` (bound 12, newest first, missing kept), `templates.json`, the tour progress and the start choice, each with the `.broken` and future-version rules; seed from `last_project` once; posters folder ownership. Write t985 and t987 first (red), then green
  - `StartStore` (four files, `RECORDS_VERSION` 1). Red proofs: a bound of 14 instead of 12 fails t985; a "Start over" that also resets the start choice fails t987. The records are written tmp + rename WITHOUT fsync: with `write_atomic` t985 took 90 s on a busy disk (two fsyncs per write), and the same would stall the UI thread on every open; a lost recent list costs nothing. The poster half of t985 (the real render) comes with 3.3.
- [x] 2.2 Commit stage 2 (`cargo test -p kt-app`)

## 3. The welcome window

- [x] 3.1 `welcome.rs`: `StartFacts`, `primary()` and t984 (every case of the spec, incl. FFmpeg missing and recovery); green
  - `welcome::primary`/`window_tile` and t984 (every case of the spec plus the edges: no recent and the tour never started → the tour; started and stopped → continue it). The start order is `welcome::plan_start`, which `run` executes. Red proof: dropping the recovery case fails t984.
- [x] 3.2 The window `kt_welcome` (registry entry in `WINDOWS`, form contract t779), template tiles, recent cards, the tour tile, the Guide link, What's new from `docs/guide/whats-new.md` (new file, English, `## kt26.10`), "Show at start"; `run` start order of design D4; View checkbox; Help menu. Write t986 with its PATH in `reach_tests.rs`; verify t986, t757, t779, t83, t20 green and `src/scripts/specs-user-path.py --check` (t799) green
  - `kt_welcome` in `WINDOWS` and the t779 table (closed by "Close", an action label); Help menu (Guide…, Start the tour, Start the tour over, Welcome window…, What's new) and the View checkbox; `docs/guide/whats-new.md` (## kt26.10), linked from getting-started. t986 with its PATH; t986, t757, t779, t83, t20 green, `specs-user-path.py --check` green. Found on the way: an auto-sized window with a `right_to_left` row and unwrapped guide labels grew to 2010 points — t986 now checks the window fits the screen. Red proof: a template click that makes the default frame fails t986. The ru Help title is "Помощь": "Справка" is the formula help button, and t789/t789b query it by label.
- [x] 3.3 The poster job on save/close (design D5); verify t985's poster half and that no file appears in the project folder; the release timing of 1.2 recorded against a Ctrl+S
  - The poster service (`welcome::request_poster`, outlives every app; coalesced by file; paused while playing) renders through `reel_preview::render_poster` (the offline path's shared `compose_offline_frame`, t602 intact) into `<store>/posters/<xxh3>.png`; queued by `replace_with` (close) and on a successful save; `on_exit` waits ≤ 3 s. t985's poster half (`t985_posters_render_into_the_store_never_into_the_project_folder`) renders a real poster and finds nothing new in the project folder; red when the poster is put next to the project. The release timing is recorded under 1.2.
- [x] 3.4 "Save as template…" in the Project settings window; verify t987 and t780 green
  - "Save as template…" in Project settings: the name is typed IN PLACE of the button (no new window, R.63 item 12); `templates.json`; the template tile's context menu deletes it. t987 and t780 green.
- [ ] 3.5 The update line from the update-check state (when that change is applied); a harness case in t986 with a state file naming a newer release
  - NOT DONE on this branch (2026-10-30): `update-check` lives on `wine-frame-and-dark-theme` and is not applied here, so there is no state reader or `decide` to call. Per D10 the line is simply absent until then; t986 does not assert it. To do after the branches meet: read `<data_dir>/update-check` through that change's reader and add the harness case.
- [x] 3.6 Catalogs `[welcome]`, `[help]` ×5, `python3 scripts/subset-fonts.py`; verify t100, t125, t128, t171 and `font_tests.rs` green; commit stage 3
  - Sections `[welcome]`, `[help]`, `[guide]`, `[guide.index]`, `[lessons]` and the new keys of `[menu.view]`/`[settings]` ×5; `subset-fonts.py` rebuilt the SC subset. No font of ours has U+2192/U+2190: catalog menu paths use `›` (Arabic `‹`), the guide pages get Phosphor arrows (`icons::ARROW_RIGHT/LEFT`) at parse time. t100, t125, t128, t171, `font_tests.rs` green.

## 4. The lessons

- [x] 4.1 Move `demo_project` from `src/apps/kt-cli/src/main.rs` into the shared builder in its generator-only form; kt-cli `demo` calls it. Verify t151 (`src/apps/kt-cli/tests/cli.rs`) green and that the lesson project renders with `kt-cli render` without a missing source
  - `kt_model::demo::demo_project(DemoSources::{Synth, Files, Generators})` — one body; kt-cli `demo` calls it and gained `--generators` (the lesson form; docs/guide/export.md row updated). t151 (`tests/cli.rs`) green; `kt-cli demo x.kt --generators && kt-cli render x.kt x.wav` renders 105 s without `--allow-missing`, i.e. no missing source.
- [x] 4.2 `lessons.rs`: the lesson table, `Predicate`, `Target`, `done()`; the side panel; "Show"; progress; "Start over". Write t988 (both scenarios, incl. `no_teaching_layer_over_the_ui` over the closed list of layers above `Order::Middle`) with its PATH; verify t988, t83, t20 (no repaint at rest with the panel open) green
  - `lessons.rs`: five lessons (D7 as recommended), the closed `Predicate`/`Target` enums, `done()`; the right `Panel` (drawn before the timeline); "Show" opens a registry window or the File menu by code (probe 1.1) and the target outlines itself (`widgets::lesson_outline`, 0.3); a step is re-evaluated only when (model revision, open windows, render result) changes; the sandbox `<store>/lessons/lesson-<n>.kt` is never in Recent. t988 (both scenarios) with PATH lines; the outline scenario is added to the spec delta. Red proofs: a title predicate that never closes fails t988; a `lesson-callout` Foreground layer fails `no_teaching_layer_over_the_ui` (by the source scan and at run time). t83, t20 green.
- [x] 4.3 Catalog `[lessons]` ×5 within the budget; verify t100, t128; commit stage 4
  - `[lessons]` ×5, every paragraph ≤ 160 characters (the guard checks it per language); t100, t128 green.

## 5. The Guide

- [x] 5.1 `guide.rs`: the compiled page list (guard: equals `docs/guide/*.md`), the subset renderer with `<kbd>`, the `[guide.index]` entries with anchors, search; the window `kt_guide` in `WINDOWS`. Write t989 with its PATH; verify t989, t757, t779 and `readme-check.py --check` (t826) green
  - `guide.rs`: `PAGES` compiled from `docs/guide/` (t989 compares with the directory), the subset parser (headings, paragraphs, one-level lists, bold, italic, inline code, `<kbd>`, tables, links, fenced code — `export.md` has one; an image → its caption) failing with file:line on anything else, the `[guide.index]` entries with GitHub-slug anchors, search over the entries and the headings — a heading is also found by the words of its SECTION ("loop" finds "Markers and the region", as the spec's PATH expects; the heading alone has no "loop"). `kt_guide` in `WINDOWS` (closed by its title cross). t989 with PATH; t757, t779, `readme-check.py --check` (t826) green. Red proof: heading-only search fails t989.
- [x] 5.2 Commit stage 5
  - Committed with stages 3–4 (the What's new panel of stage 3 renders through the guide parser).

## 6. Live and records

- [x] 6.1 Live scenario `src/scripts/live/welcome.py` (own process, own `XDG_DATA_HOME`): first start → the window with "Start the tour" primary → a template → Help → Welcome window… → a recent card → Help → Guide… → search "loop" → Help → Start the tour → step 1 closes by adding a title; `kt_live.step` screenshot per step. Verify `./scripts/live-check.sh welcome` green
  - `live/welcome.py` green (`KT_LIVE_DISPLAY=:173 ./scripts/live-check.sh welcome`, 12 steps): first start (tour primary) → 9:16 → Project settings → Save as template… → Ctrl+S (the project heads Recent, nothing next to it) → Help → Welcome window… → the Guide link → the recent card → Help → Guide… "loop" → Help → Start the tour → Show (File menu, outlined item) → Create a generator… → Titles and countdown → Text closes step 1 → Start over (asks, Don't save) → Help → Start the tour over → Help → What's new → Show at start off → Close → View → Welcome window at start on. Verdicts read the store files and probes. Screenshots `target/live-out/welcome/NN-step-*.png`. The first run was red on the scenario's own mistakes only (XTest lower-cased the typed capital; the poster check counted the screenshots; the Guide link is above the fold when the window opens at What's new).
- [x] 6.2 `src/apps/kt-app/CLAUDE.md`: the start order and the three new modules; commit stage 6
  - Section "The start, the Guide and the tour": `plan_start`, `replace_with` carrying `start`, the memory default store, no-fsync records, the poster service, the auto-size trap, lessons on state, the sandbox, the Guide's arrows.

## 7. Finish

- [ ] 7.1 Run `./scripts/finish.sh` from `src/`; record the version `vYY.MM.DD-hash` and the live screenshots of 6.1 in the report
- [ ] 7.2 After archiving: F-HLP-027/028 verdict lines name `CAPABILITY: openspec/specs/ui-reach/`; append the "why" (D1, D2 revisions, D3 in-viewport, D5 posters outside the tree, D8 the lesson sandbox) to `openspec/specs/ui-reach/design.md` and `openspec/specs/durability/design.md`
