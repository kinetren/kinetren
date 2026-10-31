# Registry of the owner's list `kkt-26.11.01`

Source — [`kkt-26.11.01.txt`](kkt-26.11.01.txt) (verbatim, the owner's message of 01.11.2026: the
orchestration rules, fifteen numbered remarks and ten more requests). Columns and states — as in
[`v26.10-index.md`](v26.10-index.md). The round is in `docs/user-feedback.md` ("2026-11-01 — the
orchestrator round").

The rows name the changes PROPOSED for the discussion of 01.11.2026. A change name in the Address is
the planned name, created by `/opsx:propose` after the owner's answers; until then the Stage says
"to discuss". The investigation behind each row (code, causes, specs) is summarised in the row's
Claim and was done read-only on master `94e36e9e`.

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| O1 | 1–5 | orchestrate executors; register requests; propose/apply after discussion; master only by merge; keep target small; remember the standing rules | partial: the rules are in the root `CLAUDE.md` ("Rules for agents"), the round in `user-feedback.md` | `CLAUDE.md`, memory `kinetren-orchestrator-role` | orchestration 2026-11-01 |
| K1 | 9 | the welcome window is ugly, does not resize, What's new is a 220 px column, one scroll for the whole window (`welcome.rs` draw_welcome) | no: defect → ui-reach | `welcome-window-and-settings` | to discuss |
| K2 | 10 | lesson 1 "play a part in a loop" counted only after a window closed: the step is re-checked on (model_rev, open windows), and region/loop do not bump model_rev | no: defect → ui-reach | `tour-lessons-fixes` | to discuss |
| K3 | 11 | lesson texts run together; no `<kbd>` (the Guide has it, lessons draw plain labels) | no: defect → ui-reach | `tour-lessons-fixes` | to discuss |
| K4 | 12 | only lesson 1 highlights (menu targets only); generators live in File and the bin menu only; there are no audio generators (sound only inside SMPTE/leader) | no: defect → ui-reach; feature, no note → generators (audio generators) | `tour-lessons-fixes`, `generators-placement-and-audio` | to discuss |
| K5 | 13 | "Show" does nothing for panels; for the morpher it opens the window itself, which also completes the step and teaches nothing | no: defect → ui-reach | `tour-lessons-fixes` | to discuss |
| K6 | 14 | lesson 3 promises a crossfade from an overlap; an overlap is a cut by the owner's decision (t163/t464), only "Transition with previous" makes one | question: should overlapping two video events make a crossfade by itself, or should the lesson teach the menu's "Transition with previous"? | `tour-lessons-fixes` | to discuss |
| K7 | 15 | lesson 4 does not say what changed after the effect, nor bring the cursor to where it shows | no: defect → ui-reach | `tour-lessons-fixes` | to discuss |
| K8 | 16 | jokes in the lessons panel while lesson 5 renders | no: feature, no note → ui-reach | `tour-lessons-fixes` | to discuss |
| K9 | 17 | crash in render: the "remaining" time panics when frames rendered > the floored total on an off-grid region; recovery keeps the sandbox path (`lesson-5`), never re-opens sources (the lesson's generators play), the log appends across runs in UTC without a per-run header, no crash window | no: defect → export, durability | `render-crash-and-recovery` | to discuss |
| K10 | 19 | a segment's shape parameters (zigzag) are editable only for "all segments"; one segment has only a drag handle, many segments nothing | no: defect → automation | `segment-shape-fields` | to discuss |
| K11 | 20 | where are the plugins — inside the dist-out binary? | done plugin-api-stage1: the built-in set is compiled into `kinetren` and `kt-cli` (no separate files); t975 | answered in the report of 01.11.2026 | plugin-api-stage1 |
| K12 | 22 | windows do not fill their area when enlarged (`window_body` caps the body at 0.62× the screen; fixed columns; 13 sites; many windows not resizable); the Guide's tables and "I want to…" are flat; the FX picker is two tiny panes | no: defect → ui-reach, ui-timeline | `windows-fill-their-area` | to discuss |
| K13 | 23 | the start checkbox is in the window and in View; app preferences are scattered in View — a Settings section in File | no: defect → ui-reach | `welcome-window-and-settings` | to discuss |
| K14 | 24 | Performance moves into Diagnostics, three graphs there | no: feature, no note → ui-timeline | `diagnostics-three-graphs` | to discuss |
| K15 | 25 | pitch tears across 15 short events (each event's node starts cold: zero ring, grain grid at the clip start, look-ahead past the end) | no: defect → audio-fx | `pitch-event-edges` | to discuss |
| C1 | 30 | CI died on master 94e36e9e: ffmpeg9 (a root-owned registry volume), flatpak (no eu-strip), t935 (no mimetype tool), t801 (shared output folder), windows t973c/t1019/t1048 (Linux-only hashes), t962 (VAAPI absent on Windows), t986 (`\\?\` paths) | no: defect → distribution | `ci-green-again` | to discuss |
| W1 | 35 | update What's new; agents write human-language lines into it on every merge | partial: the page got the October items and a kt26.11 section; the rule is in `CLAUDE.md` | `docs/guide/whats-new.md` | orchestration 2026-11-01 |
| V1 | 40 | a REAL VHS: a tape written and read back, heads, drums (good, dirty, four heads, a chipped one; one drum or two), a device-like UI; note for agents: imitation ≠ simulation | no: feature, no note → video-fx | `vhs-tape-simulation` (the note first) | to discuss |
| I1 | 45 | a real camera ISP: RGB → Bayer (RGGB, other layouts, +IR) → calibrations, denoise → YUV | no: feature, no note → video-fx | `camera-isp-simulation` (the note first) | to discuss |
| D1 | 50 | a real datamosh: a motion-compensated compression path broken carefully by parameters | no: feature, no note → video-fx | `datamosh-codec-path` (the note first) | to discuss |
| M1 | 55 | a convenient multicam, built on the morpher's source swap but not its UI; no features thrown anywhere | no: feature → F-TL-005 | `multicam` | to discuss |
| T1 | 60 | an AE-style point tracker, and properties good enough to link one thing to another | no: feature → F-MSK-005, F-AUT-008 | `point-tracker` | to discuss |
| S1 | 66 | convenient classic masks | no: feature → F-MSK-001, F-MSK-002 | `shape-masks` | to discuss |
| G1 | 71 | magic-wand masks | no: feature, no note → masks | `magic-wand-mask` (the note first) | to discuss |
| P1 | 76 | new feature-request prompts from ktprompts, docs/features and the web | partial: 20 drafts for the owner to pick from in `kkt-26.11.01-candidates.md` | `docs/ktprompts/kkt-26.11.01-candidates.md` | to discuss |
