# Kinetren

> Until 2026-10-21 the program was called Kinotrain (full rename — change
> `kinetren-rename`). `.kt` files with the former root key `kinotrain` open only through
> the import door; the `kinotrain` data directory moves to `kinetren` on the first start.

A non-linear video editor. The code base is a cargo workspace in **`src/`**.

The rewrite from C++/Qt (v1) is complete with full parity; the old code is removed from the working tree and is available in the git tag **`cpp-reference-final`** (behaviour cross-check: `git show cpp-reference-final:<file>`). **Current contracts and the reasons behind decisions — OpenSpec: `openspec/specs/<cap>/{spec,design}.md`**, overview map — `docs/architecture.md`.

## Build and tests (Rust)

```bash
cd src
cargo test                 # fast synthetic tests (no FFmpeg/audio hardware)
cargo test                 # conformance on real fixtures is on by default
                           # (feature ffmpeg of crate kt-media; needs the ffmpeg binary
                           #  to generate fixtures, src/fixtures/gen.sh)
cargo clippy --all-targets
cargo run -p kt-app        # GUI
cargo run -p kt-cli -- …   # headless render/export
cargo run -p kt-cli -- probe <project.kt> <from_s> <to_s> [--time]
                           # "what sounds at this spot" (R.43): effective events
                           # AFTER morpher layers, the prefetch order, foreign media in it
                           # and the engine build cost. The first thing to ask
                           # on a complaint "sound/picture here comes from another event".
                           # --time: does the audio render keep up with real time
                           # (judge SPEED only on a RELEASE build — debug
                           # is 9 times slower, item 301)
cargo run -p kt-cli -- preview <project.kt> <from_s> <to_s> [--size WxH]
                           # plays a range through the real player against a wall-clock
                           # output device: V-fps and V-skipped as the performance panel
                           # counts them, and a frame's time stage by stage. On a RELEASE
                           # build, with the footage in /dev/shm: on a busy disk two runs
                           # differ by hundreds of incomplete composites
python3 scripts/subset-fonts.py  # CJK/Arabic font subsets from the translation catalogs
./scripts/live-check.sh    # live scenarios on a virtual screen (Xvfb+openbox):
                           # the real kt-app, real gestures, screenshots in
                           # target/live-out; finish.sh runs them itself
./scripts/release-build.sh # release tarball: dist profile (strip+LTO) in an
                           # Ubuntu 22.04 container → runs on glibc >= 2.35; inside
                           # "ldd without libav*" and the GLIBC_ ceiling are checked
./scripts/release-build.sh --target windows  # (make windows) the portable Windows zip,
                           # cross-built with mingw in the `windows` stage of the same
                           # Dockerfile; inside, the PE imports are checked (no av*/sw*,
                           # no mingw runtime DLL); no FFmpeg DLL is ever packed
./scripts/target-prune.py  # drop the STALE artifacts of target/debug (cargo names what the
                           # tree needs; the rest goes). A fresh build is ~14 GB, but every
                           # core edit leaves new copies of ~96 test binaries: one agent day
                           # grew a worktree's target to 142 GB. finish.sh runs it; a
                           # session runs it after every committed stage too
./scripts/windows-test.sh  # (make windows-test) the workspace tests for Windows under
                           # wine; the windows-latest CI job runs the same archive
```

System requirements for the full build (kt-media/kt-app): FFmpeg dev packages (libavcodec/format/util/swscale/swresample), `clang` (bindgen), `libasound2-dev` (cpal/ALSA), the `ffmpeg` binary in PATH (export and fixture generation). The crates kt-time/kt-model/kt-fx/kt-cache and the kt-engine core build and test **without** any of this.


## Workspace map

```
src/apps/kt-app     eframe GUI: timeline, preview, cpal output, translation catalogs i18n/
src/apps/kt-cli     headless render (determinism, golden tests, CI)
src/libs/kt-time    90 kHz ticks, ranges, fps/audio-burst grids (no dependencies)
src/libs/kt-model   timeline model, commands, undo
src/libs/kt-otio    OpenTimelineIO (.otio) exchange DOORS: import/export, schemas+passthrough,
                     metadata["kt"]; the PROJECT file is our own `.kt` (kt-model::project_file, R.70)
src/libs/kt-media   MediaSource trait, bridge to kt-ffmpeg, SyntheticSource, sidecar index DB
src/libs/kt-text    title rasterization: harfrust+ab_glyph, vendored Noto Sans (R.52)
src/libs/kt-cache   burst cache + coverage (the only truth about "warmth")
src/libs/kt-fx      AudioFx/VideoFx: slowedreverb, gain, video FX
src/libs/kt-engine  render graph, MediaWorker scheduler, transport, prefetch, export
src/libs/kt-plugin-api  the plugin contract (plugin-api-stage1): manifest + param passport, the
                     FX/transition/generator kernels, the registry by family; depends on kt-time and serde
src/libs/kt-plugins-builtin  the assembler: the closed list of built-in plugin ids and `install()`;
                     a normal dependency of kt-app and kt-cli only, a dev-dependency of core tests
src/plugins/*       the effects a person picks, one crate per family: fx-audio-{dynamics,modulation,
                     tone}, fx-video-{color,distort,stylize,blur,tape}, transitions-{basic,styled,volume},
                     generators-{basic,test}; a plugin's id is its file tag. An edit here rebuilds
                     the family, the assembler and the binaries, no core crate (t977)
```

The core keeps what is not an effect a person picks: the Geometry node, CamMotion, LayerMix, the
crossfade and the title. Every picked effect is a plugin. Since `plugin-api-barriers` this includes
the barriers: each declares its kernel and a GPU pass graph.

Every libs/ crate and apps/kt-app has its own `CLAUDE.md` with invariants and non-obvious decisions. Read it before editing the crate.

UI parity with the old Qt editor: U1–U4 done — markers/region/loop/punch-out/copy-paste chains/volume rail/stereo waveforms/filmstrip/meters+fader/media bin/FX editor with automation/render dialog/autosave. E5 ported all 10 video FX, H1 — the morpher system (morph tracks, placement, FX batches, bake; tests 35–36). I1 brought the perf panel to the v1 level and beyond: a 5-series FFmpeg activity graph (Total/Open/Read/VDec/ADec from the live kt-ffmpeg::iostats counters), no-read drops (prefetch thrash) in the cache churn, cache occupancy against the real limit, "pinned legends" with callouts, "×N" log dedup, threshold warnings. Functional parity with v1 is CLOSED (in v1 a third of the panel metrics were dead zeros — we do not reproduce them, see test 37).

## Time model

- The unit is the **tick = 1/90000 s** (`kt_time::Tick`). 90 kHz divides every frame rate (24/25/30/60) and the audio-burst step.
- Audio: **48 kHz stereo f32 interleaved** right after decoding and all the way to the output. Audio burst = 1024 frames = 1920 ticks ≈ 21.3 ms. Tick↔sample: `samples = ticks * 48000 / 90000` — exact (15/8).
- Video: a frame on the project fps grid; the burst index is aligned to the `90000/fps` step.
- The project format holds only ticks, no milliseconds.

## Invariants (breaking any of them is a regression; each has a test)

The full text of each invariant, its scenarios and mandatory tests are in `openspec/specs/<cap>/spec.md`;
reasons and history are in the `design.md` next to it. Here — one line each, to remember.

1. **Audio inside the engine is always f32 48 kHz/2ch.** FX and the mix never see the file's sample rate — `audio-render`.
2. **FX state is touched only by the audio render thread.** No mutexes around FX, no access from prefetch/meters; prefetch fills only the pre-FX cache — `audio-render`.
3. **A model edit does not destroy FX state.** Chains are diffed by clip id: the same chains → the same state objects, parameters → `set_params`, buffers are zeroed only on seek — `audio-render`.
4. **The cache is the only truth about coverage.** Coverage bars = `BurstCache::coverage()`; quality and the decode scale step are part of the key and part of the ORDER (prefetch, composite, export and the bars all ask the same thing) — `video-plan`, `media-index`.
5. **A decoder instance is owned by exactly one thread.** **Live playback** owns no sources: playback, prefetch and thumbnails go through the MediaWorker priority queue with generation cancellation, and playback never blocks. **The offline path** opens its OWN short-lived source under three conditions: it **does not share** it with anyone, it **promises no tempo** and its job is **finite and cancellable**. Breaking any of them — back to the queue — `media-index`.
6. **The cpal callback is the master clock.** The playhead advances by the frames the callback HANDED OUT; starvation gives silence but does not stop time (except before the first frame); no allocations and no locks in the callback — `transport`.
7. **There is one composition code path.** `frame_plan` is shared by preview and export; the GPU (`dyn VideoGpuExec`) is its one executor, held to stored frames by the `FrameTol` tolerances; with no device the preview has no picture and says so, and an export refuses; geometry series are fused in one pass, the chain plan is built by `plan_chain` — `video-plan`, `video-fx`.
8. **Timeline scroll/zoom produces no decode requests.** Waveforms come from the peak pyramid, thumbnails from the sidecar, reel previews from the cache keyed by `content_hash` with a disk tier — `media-index`, `compositions`.
9. **Export is fully in-process.** libavformat/libavcodec of the system FFmpeg, an encoder cascade with a real open check; no temporary files, no external binary. The one exception: our own binary, started once, meets a libva driver first, so a driver that faults cannot end the render — `export`.
10. **The binary does not link against libav*.** FFmpeg loaded at run time, the major version from `avcodec_version()`, one binary for FFmpeg 6/7/8/9: Linux by SONAME (`ldd … | grep -c libav` == 0), Windows from ONE folder found by the search plan or downloaded on a click (the PE imports name no av*/sw* DLL and no mingw runtime) — `media-index`.
11. **Determinism.** Audio — byte for byte, always; video — byte for byte between runs on one device, across machines — within tolerances of the stored frames; no wall clock in offline paths; no platform libm on a frozen path — `kt_plugin_api::math` (t1066) — `video-plan`, `export`.
12. **Video display is not quantized by the audio callback, the UI does not heat the system.** The display position is interpolated (`video_playhead()`), repaint only while something moves, no allocations in draw paths — `transport`, `ui-timeline`.

## Rules for agents

- **Before any UI/behaviour change — `docs/user-feedback.md`**: the history of the user's requirements in his own words (what he asked for, which decisions he made, what he repeated). A limit, mode or look set at his direct request is not changed without a new request. After every round of remarks the list is appended THERE, before work starts.
- Before editing a crate — its `CLAUDE.md` and `openspec show <cap> --type spec` for the capabilities named in its first line; the `design.md` next to it — reasons and what was rejected. Before architectural changes — `docs/architecture.md`. Work is done as an OpenSpec change (`/opsx:propose` → `/opsx:apply` → `/opsx:archive`); `openspec` needs Node ≥ 20 (`~/.nvm/versions/node/v24.21.0/bin`).
- **Before implementing a new FEATURE (not a defect) — `docs/features/`**: reconnaissance of other editors (what Vegas/Premiere/Resolve/FCP/AE can do, which non-obvious detail makes it work, what our verdict is). No note for the feature — create it first, then write code; the rules are in `docs/features/README.md`, the check is `python3 src/scripts/features-index.py --check`.
- A newly found defect/subtlety → document it right away: a scenario with a test goes into the capability's `spec.md`, the "why" into its `design.md` (or the crate's CLAUDE.md if it is a code-writing rule), and close it with a regression test where possible.
- **An old file stopped loading — the fix goes into IMPORT, not into the reader** (owner's
  decision, R.67): `File → Import older Kinetren project…` → `kt-otio legacy_import.rs`,
  a table of fixes with the version each was added for; dropping support for a version removes
  its fixes. The reader does not learn new old forms; the plain "Open" rejects such a file naming
  its version and the import menu item. Spec — `project-format`.
- **A model change the OTIO export cannot express goes into "What OTIO export loses"** (owner,
  CORE section of `ktprompts_v26.10`: "any agent that adds any new feature that breaks otio must
  record it in a separate list"): a row in that section of `docs/ktprompts/v26.10-index.md` — since
  which change, what the model has, what the `.otio` export does with it (drops, flattens, writes
  into `metadata["kt"]`). The project lives in `.kt`; OTIO is only a door, supported last.
- **The owner's claims files are indexed** (R.71): every `docs/ktprompts/<name>.txt` (verbatim) has
  `<name>-index.md` — a row per numbered item with its source line, a state from the closed set
  (`done` R.NN + a live test / `partial` / `no` → capability or note / `question` / `future`) and a
  stage. A new claims file gets its registry before any of its items is taken; a round that closes
  an item moves its row. Guarded by `src/scripts/ktprompts-index.py --check` (t823) and `finish.sh`.
- **A new UI scenario (test ≥ t783 in kt-app) carries a `- **PATH**` line** — the mouse steps and
  what the person sees after each; guarded by `src/scripts/specs-user-path.py --check` (t799)
  and `finish.sh`. A live scenario takes a screenshot per step (`kt_live.step`), `finish.sh`
  prints their list after the version, and a report on UI work carries them. Runs use OUR
  fixtures, not the owner's projects ("item 1 but not on my projects", R.67).
- **Commit every complex stage AUTOMATICALLY** (owner's requirement, 2026-09-22): a tasks.md section / round stage is finished and the check is green → commit at once, without being asked; the message in English (owner, 2026-10-21), the first line — the stage and what was done. Uncommitted work at the end of a session is a process defect.
- **`master` changes only by a merge from a branch** (owner, 2026-11-01: "I forbid any agent to
  make edits in master directly without a merge from a branch"). Every session works on its own
  branch in its own worktree (`.claude/worktrees/<name>`); `master` receives that branch by a merge
  (fast-forward when it can), and the merge itself is done or ordered by the orchestrating session
  or the owner. A commit made on `master` itself is a process defect.
- **The disk is short** (owner, 2026-10-30 and 2026-11-01). A worktree's `target` is ~14 GB fresh
  and grows with every core edit. So:
  - `scripts/target-prune.py` runs after every committed stage;
  - a worktree whose branch is merged is removed (`git worktree remove`);
  - no `--deep`-style rebuilds just to measure.
- **A merged feature or visible fix writes its line into What's new in the same merge** (owner,
  2026-11-01): `docs/guide/whats-new.md`, the section of the current month (`## ktYY.MM`, newest
  first). The line is in human language: what a person can now do or no longer suffers from,
  not a change name, not a test number. Internal work a person cannot notice gets no line.
- **An IMITATION of a physical effect is not a SIMULATION of it** (owner, 2026-11-01).
  - An imitation reproduces the LOOK by image operations (the current VHS: an RGB shift, scan
    lines, jitter).
  - A simulation models the PROCESS and its causes. The VHS one writes a signal to a tape and
    reads it back with heads on a drum, so heads, wear and tracking are its parameters. The ISP
    one goes from a sensor's mosaic to YUV.
  - A request for a "real" effect is a simulation. Its note in `docs/features/` names the
    physical chain step by step and which parameters are physical quantities. Its interface is
    a device-like panel, not a column of sliders.
  - An imitation is never offered as the answer to such a request, nor renamed into one.
  - **A simulated device that records picture AND sound simulates both** (owner, 2026-11-03:
    "агенты … не подумали что к тому же vhs tape должен быть парный audio-фильтр, к datamosh
    тоже"). The tape, the codec, the phone: the audio half is a paired effect on the linked
    audio, driven by the SAME device state (one tape, one deck, one stream), so a dropout, a
    head clog or a broken keyframe is seen and heard at the same moment. A new simulation
    effect is not done until its pair exists or its note says why the device has no sound.
- **Caches are allowed where they pay** (owner, 2026-11-01): a memo or a disk tier is a normal
  tool, not a last resort. It must be keyed by what determines the result (content hashes,
  revisions — never a guess that can go stale silently), bounded in size, and invisible to
  determinism (invariant 11: the same bytes with and without it).
- **No co-authorship in commits** (owner, 2026-10-21): agents MUST NOT add `Co-Authored-By:` or any other AI/agent attribution lines to commit messages, tags or PR texts — "these are junk lines". Commits are authored as the repository's configured identity (`log <log@kinetren.com>`, set in the local git config); do not override it.
- `cargo test` must be green on every commit; all tests named in the `TEST` lines of the specs (running tNNN numbering) are regression tests and must not be weakened/removed without an explicit decision of the user.
- Do not add a dependency on FFmpeg/audio hardware to kt-time/kt-model/kt-fx/kt-cache/the kt-engine core. GPU (wgpu) lives ONLY in kt-gpu; kt-engine knows only the `VideoGpuExec` trait (the executor is injected from kt-app/kt-cli).
- **The dev profile builds with `opt-level = 1`, dependencies with 2** (item 301). `[profile.dev.package."*"]` covers ONLY dependencies: our own crates were built unoptimized, and on a dense edit the audio render in dev went past 100 % of real time (112 % against 10 % in release) — the ring drained, callback underruns grew. With level 1 the same spot takes 18 %. Overflow checks and `debug_assert!` do NOT depend on the level and stay on. **Still judge SPEED on release**: measure with `cargo build --release -p kt-app` and `kt-cli probe … --time`.
- **Work ends with `./scripts/finish.sh` and the version in the report** (requirement item 7 round 12). `cargo test` builds a SEPARATE test binary and does NOT rebuild `target/debug/kinetren` (the kt-app package binary; that is the program's on-disk name since 2026-10-22) — the one the user runs: twice because of this he got a stale program and took defects as unfixed. The script rebuilds kt-app/kt-cli, prints the version `vYY.MM.DD-hash` and cleans old tarballs out of `target/dist-out`; `--release` adds a tarball, `--deep` removes the regenerable `target/release` and `target/dist-container`. **One finish.sh at a time on the machine** (owner, 2026-11-01): a machine-wide lock (`$XDG_RUNTIME_DIR/kinetren-finish.lock`, held by `flock -o` around the run) makes a second run wait and say who holds it; three parallel runs made each several times slower. `KT_FINISH_NO_LOCK=1` skips it. **When an orchestrating session runs the work in stages,
  `finish.sh` runs once per STAGE on the merged tree, by the orchestrator** (owner, 2026-11-03:
  "finish.sh надо прогнать после каждой стадии, а не на каждом окончании отдельного запроса"): an
  executor ends with the tests of its specs, `cargo test` of what it touched, clippy, its own live
  scenario and target-prune — not finish.sh. The orchestrator's instructions: `docs/orchestrator.md`.
- **The dev profile keeps `debug = 1`** (change ci-speedup, D8): line tables and function names,
  as the release profiles. Full debug info took the target from 14 GB to 27 GB, and an edit that
  relinks the test binaries waited on the disk. `"line-tables-only"` is not enough: a crash report
  needs the function's module path (t1072).
- The program version is `kt-version` (build.rs, commit date + hash). The manifests keep `0.1.0`: semver does not accept `v26.08.27-…`. The debug title shows the full version, release — the short `ktYY.MM`.

## Editing a crate — which tests are mandatory

The "crate → mandatory tests" table moved into the specs: `openspec show <cap> --type spec`
gives the capability's requirements and scenarios, each scenario has a `TEST tNNN (path)` line.
Capabilities by behaviour: time-model, transport, audio-render, video-plan, video-fx, audio-fx,
transitions, automation, morpher, compositions, generators, media-index, export,
project-format, durability, ui-timeline, ui-reach, telemetry, plugins. Every crate `CLAUDE.md` states in its first
line which capabilities cover it. **Any UI change** — kt-app `reach_tests.rs`
(`ui-reach`) and `layout_tests.rs` (`ui-timeline`); any text in the UI — `font_tests.rs`/`i18n_tests.rs`.

All together: `cd src && cargo test --workspace` (plus `cargo clippy`).
