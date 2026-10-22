# Tasks

All `openspec …` commands — through Node 24: `export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"`.
The order — per design.md D4: first all 17 capabilities, then truncation of the sources,
then configuration and freezing. Until section 4 the old chronicles remain intact.

## 1. Preparation and inventory

- [x] 1.1 Take the baseline: `openspec validate --all --strict` and `python3 src/scripts/features-index.py --check` green, the output saved in the scratchpad; check — both exit codes 0
- [x] 1.2 Build the map "tNNN → capability" from the table "Crate → mandatory tests" of the root `CLAUDE.md` by the table D2 of design.md (a script in the scratchpad: extract all `t\d{2,3}[a-g]?` by the lines of the table, attach a capability); check — each tNNN of the table is attached to exactly one capability, the list of unattached ones is empty or explicitly sorted by hand
- [x] 1.3 Build the map "source → capability" for the three chronicles: the chapters of `docs/porting-notes.md` (by `## `/`### `), the sections of the rounds of `docs/user-feedback.md` (`## Round R.N`), the notes of `docs/features/*` with status `done`/`accepted`/`already-have`/"done/accepted/already-have/rejected" — by the table D2; check — the map is saved in the scratchpad, each of the 17 capabilities has at least one source, each "our" note has a capability
- [x] 1.4 Prepare the file format: `spec.md` = `# <Name>` / `## Purpose` / `## Requirements` with `### Requirement:` and `#### Scenario:` (WHEN/THEN + last line `- **TEST** tNNN (`path/tests/file.rs`)`); `design.md` = sections "What was and who was hurt", "Decisions and why", "Rejected and why", "Non-obvious details (src/… paths)", "Deferred" — check: a trial capability from 2.1 passes `openspec validate --specs --strict`

## 2. Capability: spec.md + design.md (one by one, each checked separately)

The common check of each task of the section: `openspec validate --specs --strict` green;
`openspec show <cap> --type spec --json` returns all requirements; each scenario
carries a `TEST` line; all tNNN attached to the capability in 1.2 occur in its `spec.md`
(the cross-check script from the scratchpad); `design.md` has no chronology ("in R.N first…") and
comparisons with C++ without the rule they lead to.

- [x] 2.1 `time-model` — from "Time model" of CLAUDE.md, kt-time; the common check + a decision on the open question of design.md: if the requirements are < 3 — write in design.md of the capability the note "merge into transport in 2.2" and perform the merge there
- [x] 2.2 `transport` — inv. 6/12, R.43 (the clock under starvation, t284), R.57 (the signed scale, t603/t604/t616–t619, t626–t629), kt-engine/CLAUDE.md, user-feedback R.43/R.57; the common check
- [x] 2.3 `audio-render` — inv. 1–3, the defect catalogue slowed&reverb (a–d → rules without C++ paths), cost.rs t292–t295, R.48 (PDC t334–t341), loudness t403–t407, sync t413–t419, features audio/processing (8 "done") and loudness/audio-waveform-sync; the common check
- [x] 2.4 `video-plan` — inv. 4/7/11, R.44–46 (the decode scale step t310–t317, the geometry merge t296–t309, memory t66), kt-gpu/CLAUDE.md, features transform-concatenation; the common check
- [x] 2.5 `video-fx` — R.47 registry (t322/t328/t329/t330–t333), R.54 thumbnails (t543–t551), features fx/* (4 "done"), color/basic-color-fx, preset-gallery; the common check
- [x] 2.6 `audio-fx` — registry of 16 nodes (t319–t327), features audio/processing along the chain of nodes; the common check
- [x] 2.7 `transitions` — R.33 S4, R.51 (t223–t226, t232, t459–t468), the rs+wgsl pair, features transition-vocabulary, same-track-transitions; the common check
- [x] 2.8 `automation` — R.33–34, §466–467, props t641–t649, fx_auto t190/t212–t215/t248, features keyframe-domain, presets-carrying-automation, expressions-and-drivers (accepted — the interface only), plugin-abi-and-sandbox (accepted — the interface only); the common check
- [x] 2.9 `morpher` — H1 (t35–t36), morph_slice (t233–t249, t555–t556), item 300 (source swap carries curves, t291), user-feedback about Adjustment Events; the common check
- [x] 2.10 `compositions` — R.50/R.55/R.56/R.58 (t423–t458, t552–t602, t631–t640), §459–464 (including the deferred growth of .otio → the section "Deferred"), §469–470 and nest_param t671–t690, features nested-sequences, inline-nest-expansion, composition-public-parameters; the common check
- [x] 2.11 `generators` — R.52–53, §468 (t469–t495, t656–t670), kt-text/CLAUDE.md (font/pins/FONT-MANIFEST), features titles/* (5 "done"), solid-gradient-shapes; the common check
- [x] 2.12 `media-index` — inv. 5/8/10, docs/media-index.md, kt-media/kt-ffmpeg (t10/t21/t34/t37/t70/t208), R.49 wave B (t398–t412, t420–t421), kt-cache (t8/t14/t27/t28/t37/t65); the common check
- [x] 2.13 `export` — inv. 9, golden_export (11, 25, 33, 67, 479), export_loudness t406–t407, features hardware-encoder-selection, preview-vs-export-mismatch; the common check
- [x] 2.14 `project-format` — docs/file-format.md, kt-otio/CLAUDE.md (t144–t154, t164–t165, t178–t180, t189, t192, t212, t222, t240–t251, t431–t437, t465, t473–t474, t494, t593, t606, t638, t664–t665, t681), §464; the common check
- [x] 2.15 `durability` — R.49 (t351–t391, journal t354, lock t362), features resilience/* (6 "done"), user-feedback "you did not check" (the rule of finish.sh + live scenarios); the common check
- [x] 2.16 `ui-timeline` — kt-app/CLAUDE.md (repaint/display clock/usability checklist/localization/fonts), layout_tests and the font/i18n tests from the table of CLAUDE.md, the table "repeated more than once" of user-feedback (limits "set on a complaint"); the common check
- [x] 2.17 `ui-reach` — R.38/R.40 (t252–t255, t273), wave V of R.49 (t392–t396), features help/* (4 "already-have", accelerator-visibility "done"); the common check
- [x] 2.18 Summary cross-check of the section: the script from 1.2 confirms that EVERY tNNN of the CLAUDE.md table occurs in exactly one `openspec/specs/*/spec.md`; the list of 17 (or 16 after 2.1) directories matches D2; `openspec list --specs --json` shows all of them

## 3. Truncation of the sources (only after 2.18)

Premise (a recorded assumption): only notes with status
`done`/`accepted`/`already-have`/"done/accepted/already-have/rejected" are truncated — their "How it fits" holds a decision or the
reason for rejection. Notes with status "candidate" keep the section: this is not yet a decision; the
round that takes them will truncate them under the new README rule.

- [x] 3.1 Before truncating each note, compare its "How it fits" with the `spec.md`/`design.md` of the capability: all `src/…` paths and tNNN from the section either exist in the capability or are carried over there (risk 1 of design.md); check — the script in the scratchpad prints for each note the list of uncovered paths/tests, the list is empty
- [x] 3.2 Truncate "How it fits Kinotrain" in the notes "done" (30 of them) to ≤ 5 lines: one line `CAPABILITY: openspec/specs/<cap>/` + the essence of the decision; do not touch the "Verdict" (rule 7 of the README — the note is frozen, but cutting out of "ours" is explicitly allowed by this change); check — `wc -l` of the section ≤ 5 for each, the `CAPABILITY:` line is present
- [x] 3.3 The same for the notes "accepted" (3), "already-have" (7) and "rejected" (6; for the rejected ones the reason remains in the "Verdict" and is duplicated in the capability's `design.md`, section "Rejected and why"); check — as in 3.2
- [x] 3.4 `docs/features/README.md`: rewrite rule 5 — "How it fits" must carry a line `CAPABILITY: openspec/specs/<cap>/` (an existing directory), and for the statuses `done`/`accepted`/`already-have`/`rejected` be ≤ 5 lines; for `candidate`/"candidate/researched" the `src/…` paths remain mandatory; extend rule 8: "design lives in an OpenSpec change"; check — the text of the rules is updated, the coverage table of the README is not broken
- [x] 3.5 `src/scripts/features-index.py --check`: implement rule 3.4 (check that the capability directory exists; the check "src/ is mandatory" only for non-frozen statuses; a line limit for frozen ones); check — `--check` is green on the tree after 3.2–3.3 and red on a probe note without `CAPABILITY:` (the probe is deleted)

## 4. OpenSpec configuration and freezing of the chronicles

- [x] 4.1 `openspec/config.yaml`: `context` — the time model and the 12 invariants, one line each with a reference to the capability, the Node-24 note; `rules.proposal` — "before Capabilities read `openspec/specs/<cap>/design.md` of each affected capability"; `rules.design` — "a decision that contradicts the capability's `design.md` is formalized as an item `Revision: <decision> — <reason>`"; `rules.tasks` — "the work ends with `src/scripts/finish.sh` and the version in the report"; check — `openspec instructions proposal --change <any> --json` returns `context` and `rules` without warnings about unknown artifacts
- [x] 4.2 Root `CLAUDE.md`: replace the table "Crate → mandatory tests" with a paragraph "contracts and mandatory tests — `openspec/specs/<cap>/spec.md`, the why — `design.md` next to it; when you edit a crate → `openspec show <cap> --type spec`"; keep invariants 1–12 at one or two lines each with the indication of the capability, remove from them the history of items (item NNN); check — the file has no `t\d{3}` outside the pointer, each invariant references an existing capability directory
- [x] 4.3 `docs/porting-notes.md`: add to the header the block "FROZEN at §470 (2026-09-21). Current rules and tests — `openspec/specs/`; new defects — in an OpenSpec change"; do not change the body; check — `git diff --stat` of the file shows only the header
- [x] 4.4 `docs/architecture.md`: resolve the open question of design.md — if after the transfer only what is in the capability's `design.md` remains in it, replace the file with a table of contents "capability → spec/design" (one line per capability); otherwise keep it with a pointer; check — the file references all directories `openspec/specs/*`
- [x] 4.5 Do not touch the crates' `CLAUDE.md` (out of scope), but add to each one line-pointer "the contracts of the crate — `openspec/specs/<cap1>,<cap2>`"; check — each of the 10 crate CLAUDE.md files has the line, the directories exist

## 5. Verification as a whole

- [x] 5.1 `openspec validate --all --strict` green; `python3 src/scripts/features-index.py --check` green; `git diff --stat` contains no files under `src/` except `src/scripts/features-index.py` and `src/**/CLAUDE.md`; `cd src && cargo test -p kt-time` (a cheap smoke test that the tree is whole)
- [x] 5.2 A live check of rule D1: create a temporary change `probe-rules`, run `openspec instructions proposal --change probe-rules --json` and `… design …`, make sure that `rules` contain the obligation to read the capability's `design.md`; delete the temporary change; check — `openspec list --json` does not show `probe-rules`
- [x] 5.3 Summary for the report: the number of requirements and scenarios per capability, the number of truncated notes, the number of lines removed from "How it fits", the list of tNNN without a capability (must be empty)
