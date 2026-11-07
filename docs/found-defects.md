# Found and not fixed

What any agent saw and left, in one list (owner, 2026-11-04: "а ты вообще читаешь что там агенты
находят какие-то баги которые не трогают? Как бы нам это надо собирать и обдумывать тоже..").
The procedure is in `docs/orchestrator.md`, "What executors find and leave". A row gets its verdict
from the orchestrator at the latest at the end of a stage; a fixed row leaves this file in the merge
that fixed it.

Stages: С.1 performance, С.2 build and CI, С.3 usability (`docs/orchestrator.md`, "Current plan").

First harvest: 2026-11-04, from the executors' transcripts and hand-over notes since 2026-11-01. The
state was checked by reading the code and the log on `orch-26-11-01`, not by running anything;
"verified" means the code was read. Three of the longest transcripts were not fully read, so the
list may be short of items from them.

## A person suffers

| # | Item | What a person suffers | Where | State | Verdict |
|---|---|---|---|---|---|
| 1 | region-copy-of-selected | With one event selected and a region touching it, copy takes the whole event, not the part under the region | `kt-app/src/app.rs:4760` | open, verified | С.3 |
| 2 | fader wheel | The wheel moves the master fader ±1 dB per frame, so one notch jumps several dB; Shift+wheel for 0.2 dB never works | `kt-app/src/meters.rs:844` | open, verified | С.3 |
| 3 | area hint | The status hint promises dragging the area's edges, and no such drag exists | `kt-app/i18n/en.toml:124` | open, verified | С.3, with 2 |
| 4 | transparent tail | Video of a width not divisible by 8 has its last columns unwritten; the heap fix t1450 did not cover them | `kt-ffmpeg/src/source_impl.rs` | likely open, no test | С.3 |
| 5 | vocoder start phase | A looped vocoder lap can be 6.5 dB quieter than the first | kt-fx vocoder pitch | open | С.3 |
| 6 | loudness rounding | The same file shows -18.7466 or -18.75 depending on the sidecar | `kt-app/src/util.rs:210`, `:254` | open, verified | С.3 |
| 10 | export composite and memory | Composite is now the bottleneck; export peaks at 7.3 GB of memory | `kt-engine/src/export_pipeline.rs` | open | С.1, follows the `compute-backends` decision |
| 12 | gallery near a cut | A missing thumbnail is filled from the nearest stored one, so near a cut it shows the other shot | `request_near` | open | later |
| 13 | sound generator | An audio generator event draws no waveform | generators | unknown | С.3, check first |
| 14 | Noise default | Added Noise shows nothing until the amount is raised | barrier noise kernel | owner decided 2026-11-07: a visible default | С.3 |
| 16 | loop freeze on wine | Play with a loop region freezes the picture until a click | change `loop-play-latency-hold` | open, unverified | С.3, wine probe first |
| 17 | `.kt` MIME | A name-only guess can open `.kt` as Kotlin | `src/packaging/kinetren-mime.xml` | owner decided 2026-11-07: the file becomes `.ktren`, a breaking change | С.3 |
| 18 | seam outside a wrap | A clip dragged during playback does not refill the pitch look-ahead | kt-engine audio | deliberate | only on a complaint |
| 19 | Arabic wrapped lines | Wrapped Arabic labels have imperfect run order | `MAX_VALUE_CHARS`, kt-app | known limit | later |
| 20 | mask after Geometry | A mask follows only the leading Geometry node | masks spec | recorded in the spec | later |
| 21 | Datamosh placement | CPU only, cannot sit on a reel or a track | `datamosh-codec-path/design.md` | by design | not a defect |
| 22 | Windows supervisor | If the crash supervisor is killed, the editor keeps running | `render-crash-and-recovery/design.md` | open | later |
| 23 | frame toggle glyph | The toggle's glyph sits in every preview crop | kt-app preview | unknown | С.3, check first |
| 37 | VHS speed | The VHS has no CPU path any more: 108 ms per 1080p frame on llvmpipe, 95 on the deleted one | `fx-video-tape` | closed by `compute-kernels-render` | — |
| 42 | VHS thread cap | Gone with the CPU path; the cap survives only in the test model | `src/plugins/fx-video-tape/tests/model/sim.rs` | closed by `compute-kernels-render` | — |
| 43 | Datamosh motion search | Encoder, `predict` and `reconstruct` run as compute shaders, buffers pooled | `src/plugins/fx-video-codec/src/codec.rs` | closed by `compute-kernels-render` | — |
| 46 | sound of a device effect on a track chain | VHS and Datamosh sound on a track's chain may count fields and frames from the wrong time: `TrackProcessor` never calls `place` | `src/libs/kt-fx/src/audio/mod.rs` | not checked | С.3, check first |
| 48 | Datamosh sound at a cut | The audio half assumes a long previous event, so after a short one the break may miss the picture's | `src/plugins/fx-video-codec/src/sound.rs` | open | С.3 |
| 49 | unknown rows dropped silently | An older build drops rows it does not know and loses their settings on saving | `src/libs/kt-model/src/plugin_slot.rs` | open | С.3 |
| 50 | morpher hides the sound half | The sound half follows the layer, is not visible in its strip and cannot be switched off there | morpher | deliberate, reported | owner informed |
| 51 | format 4 | A project using a device's sound settings does not open in older builds; choosing a look sets them | project format | deliberate, in What's new | owner informed |
| 52 | window preview below 25 fps | With no real graphics card, `queue.submit` takes 30 ms in a window and 0.5 ms without one, so 17-20 fps there | `kt-gpu/src/engine.rs`, `present` | cause unknown; no real card measured | С.1 |
| 53 | dropped-frame counter on the CPU path | The panel shows 0-14 skipped frames with real losses; V-fps is right | `kt-app` `draw_preview` | open | С.1 |
| 54 | one-off composite stalls | 1.7-1.9 s at one cut and once 21.9 s, on the old path only; at most 25 ms since | composite | cause unknown | watch |
| 55 | RGBA conversion, one thread per file | The next limit for 4K at native size | `kt-ffmpeg` `to_rgba` | not reached in the measurements | С.1 |
| 56 | a start on a really cold disk | The OS page cache could not be dropped here; if sound misses the 500 ms wait, the silence stays as before | start of playback | not measured | watch |
| 68 | hardware encoding unproven | No real Intel or AMD driver ran; the T16 crash is isolated, its cause not proven; a driver that fails after the three trial frames still ends the render; only FFmpeg 6.1.1 ran | `kt-ffmpeg` `hw_trial.rs` | open | needs a machine with a card |
| 72 | render time on a busy machine | Equal binaries started together on one project differ by up to 51 s, and runs stand idle 30-80 s outside the pipeline, both versions; cause not found | `kt-engine` export | open | С.1 |
| 78 | the top of the ruler takes the panel divider | A drag on the top pixels moves the panel, not the marker's end; visible live, t142 passes in the harness | `kt-app` timeline | open | С.3 |
| 80 | a cut does not move `geometry_auto` curves | `clip_slice` leaves them in place; read in the code, to check on an event with speed | `kt-model` | not checked | С.3, check first |
| 81 | a long name runs over the event's edge | Clipped only when a level chip is drawn; read in the code | `kt-app` timeline | not checked | С.3 |
| 76 | event longer than its file holds the last frame silently | Zero lost layers, nothing in the log; a person who did not mean a freeze is not told | `kt-engine` export | owner decided 2026-11-07: told in the render window BEFORE the start | С.3 |
| 64 | first frame of Datamosh | The first frame after a start took 6.6 s on llvmpipe under load about 20: the compute device opens and the shaders compile, no disk cache of shaders; new with `compute-kernels` | `src/libs/kt-gpu/src/compute.rs` `open`, `pipe` | open | С.1, next compute stage |
| 65 | Datamosh not seen on a real device | The compute path ran only on Vulkan through llvmpipe: no real card, DX12, Metal or ARM yet; the panel's message without a device was not looked at in the live program | `fx-video-codec` | open | С.2 CI and the ARM build |
| 85 | render memory unproven at 4K | The memory formula is extrapolated from 1080p; the Windows branch of reading memory was compiled alone, not run; real card, DX12, Metal, ARM not run either | `kt-engine` export | not measured | С.1, on the first 4K render |
| 86 | frame blending on a stretch and `fit_into` still on the CPU | Not moved, no measurement whether it pays | `kt-engine` | not measured | С.1 |
| 90 | Esc in an open drop-down closes the whole window | Not only the list; a shared behaviour of windows, worked around in the `controls` scenario | `kt-app` window shell | open | С.3 |
| 91 | Guide speaks our style's keys only | Guide, lessons, `zoom_how` and the recovery messages name the Kinetren keys whatever style is chosen; only `shortcuts.md` says which style it describes | `docs/guide`, `kt-app` | open | С.3 |
| 92 | keys of other programs from secondary sources | Adobe's help pages answered 403, so the Premiere and After Effects tables are from secondary sources; the wheel of Resolve and Final Cut is from forums and stays editable. Translations of the new window into Spanish, Chinese and Arabic were not read by a native speaker | `kt-app` controls | open, reported | С.3 |
| 93 | preview on the VHS span unsteady under load | With `render-speed-no-card` 4.1-7.8 fps against 2.1-4.1 in three runs of five, but 55-84 incomplete composites in two, none before; cause not found | `kt-app` preview, `kt-gpu` | open | С.1 |
| 94 | render from the window: a second decoder of a file takes the reset settings | Opened during a render it gets auto threads and 1 RGBA thread; read in `render_dialog.rs`, not measured | `kt-app/src/render_dialog.rs` | open | С.1 |
| 95 | a truncated file is not named to a person | The render goes on now, the log is silent that the file is shorter than its header | `kt-engine` export | open | С.3, with row 76 |
| 96 | next render costs on a software device | VHS pass s5 22 ms (162 coefficients per pixel); a composite with alpha ~9 ms against 1.5 without, cause not found; upload makes two copies, ~12 ms a frame. Memory stays above the old CPU build: 8.5 against 7.3 GB, 3.8 against 2.5 | `kt-gpu`, `fx-video-tape` | open | С.1 |
| 97 | the large generated shaders not compiled by FXC | t1651 runs only in CI on Windows; a refusal there gives no engine at all, so the first push decides | `kt-gpu` | awaits the owner's push | С.2 |
| 105 | an interface stall of 10-30 s | With the mouse moving no frames come and the processor is at 0 %; seen twice in a live run, once before `idle-cpu`; cause not found | `kt-app` | open | first among the load requests |
| 106 | `Failed to acquire RwLock read after 10s` | Seven `kt-app` tests panicked once on `cb37b930` under load, no trace; not seen again in 4 full runs, the last with a backtrace on | `kt-app` tests | open | watch |
| 107 | a hover still lays events out | 58 of 111 frames over a composition; the limit in `idle.py` only prints now | `kt-app` timeline, `hover-static-load` | open | with `hover-static-load` |
| 108 | nothing load-related proved on a real card | The adapter choice (t1837) and all load numbers ran on llvmpipe; whether his project has the 7 releases the circle needs is unknown. Windows has counters, no per-thread times | `kt-gpu`, `load_watch.rs` | awaits his `load-report.txt` | С.1 |
| 110 | a real edit rebuilds the whole audio engine | 447 ms on two threads per edit on his 1974 events; a slider drag pays it per step | `kt-engine` player, `input-without-work` | open | own change |
| 111 | the first meter read after an edit builds the engine on the UI thread | seen by `input-without-work` | `kt-app` meters | open | with 110 |
| 112 | hover frames lost 3-10 s at 0 % processor in live `idle` | the stall of row 105 again; once a context-menu "Effects…" opened no window | `kt-app`, live | open | with 105 |
| 113 | a field under the fold publishes its rectangle | a live click may land outside the window | `kt-app` widgets field_probe | open | with `window-content-reach` |
| 114 | thumbnail and effect-cell caps go round | over 384 thumbnails or 256 effect cells in a frame are evicted and ordered again in a circle | `kt-app` fx_thumbs | open | first |
| 115 | a paired device's empty command still writes the other half | when the video and audio halves differ | `kt-app` fx editor, paired effects | open | first |
| 116 | VHS Tape audio dropouts are full-scale one-sample clicks | 123 dropouts/s on his project, jump 0.287 at peak 0.286; the owner chose (b): a dropout is a level dip with a short soft envelope, no digital click; deferred: "сейчас мы не трогаем vhs-и всякие фильтры по существу" | `fx-video-tape` audio half | decided, deferred | after the performance work |
| 117 | setting a loop region rebuilds the audio engine on the UI thread | 412 ms freeze per region change on his project | `kt-engine`, `kt-app` | open | with 110 |
| 118 | the first loop pass composes every frame in full | 72 ms a frame on llvmpipe | `kt-engine` video | open | first |
| 119 | a frame evicted and decoded again misses its kept composite | it is composed again; the bar shows it honestly | `kt-cache` | open | first |
| 120 | a `<project>.export.otio` appears next to the project after a start | seen by `ui-frame-cost` on his relinked copy; origin not found | `kt-app`, `kt-otio` | open | first |
| 121 | the composite stays on the UI thread during playback | ~5 ms a frame on llvmpipe, mostly the driver; `ui-never-waits` (E8) holds the move, unmerged | `kt-app`, `ui-never-waits` | open | with E8 |
| 122 | the load watch wakes once a second at rest | ~0.4 % of a core; the owner's bar is zero work at rest | `kt-app` load_watch.rs | open | first |
| 123 | the journal's path filter cuts a path with spaces to one word | seen by `telemetry-journal` | `kt-app` telemetry journal | open | first |
| 124 | adding an effect from the gallery writes no journal line | only `SetFx` and a slot click are logged | `kt-app` telemetry journal | open | first |
| 125 | ten `eprintln!` sites write to nowhere | not folded into the journal | `kt-app` | open | first |
| 126 | t1914 (frame cost with windows open) measures wall-clock time and splits under load | 8.5 ms against 4.2 ms at load ~60; green alone; a clock limit outside t1671's list | `kt-app` frame_cost_tests | open | first |
| 127 | t946b (morph grid cost, a count) failed once in the full suite at load ~50-60 | 318720 sub-samples against a bound of 82944; green alone; sent to `cached-range-plays` | `kt-engine` morph_grid_cost | closed: 44b89773, the cost is read with the frame under one lock hold (t1939) | — |
| 44 | no ARM build | The program cannot run on ARM at all; the owner wants it | `.github`, `scripts` | open | С.2 |
| 47 | Ctrl+S that does not save, under load | `fxpreset`, 2 runs of 2 at load 20–50 in the last full run, and once each in `envclip`, `corner`, `audiofx` earlier: the project is dirty, the key is read, nothing is written for 300 s, the FX window is open, no question on screen. Green alone. One cause is fixed (a chord read as a bare S, t1672); this one is not found. If it is the program's, a person loses edits. Unproved guess: a text field holds the focus, hotkeys are gated, and the S goes into it | `src/scripts/live/fxpreset.py:78`, `kt-app/src/app.rs` `handle_keys` | open, cause unknown | С.3, first: print the focused widget in the failure |
| 98 | Ctrl+S stands on the disk | The save is written on the UI thread and synced twice: on a busy disk the whole interface stands for seconds (6.7 s measured at load 76, 44 s for the same write in t815's old bound) | `kt-app/src/app.rs` `save`, `write_project_guarded` | open, measured | С.3: the autosave's thread is there to reuse |
| 99 | Two quick Space presses in one slow frame | `handle_keys` keeps one press per frame, so two presses read together are one toggle; the hold threshold and the double press run on the frame's clock too | `kt-app/src/app.rs` `handle_keys` | open, read in the code | С.3 |
| 100 | Esc right after a window opens | Esc closes the window on top of the LAST frame: pressed before the new window was drawn once, it goes down the cascade and clears the region or the selection instead | `kt-app` `window_shell::escape_action` | open, seen live on the bell | С.3 |

## Internal

| # | Item | Where | State | Verdict |
|---|---|---|---|---|
| 31 | a plugin edit rebuilds every test target of the core crates (no early cut-off, ~120 binaries); `debug = 1` halved it, the cascade stays | workspace | reduced by `ci-speedup` | С.2 |
| 32 | CI wall 13.0 min in the only log (21 min has none); `ci-speedup` estimates ~9, to confirm on a warm master run | `.github/workflows/ci.yml` | applied, awaits the logs | С.2 |
| 33 | `font_tests::sources()` misses code after the first `#[cfg(test)]` | kt-app `font_tests.rs` | open | С.2 |
| 34 | `cargo fmt --all` reformats about 60 files | tree | open | С.2, one commit |
| 35 | old clippy warnings | kt-gpu, kt-cli, kt-app | unknown | С.2 |
| 36 | a copied `target/` builds another worktree's text | `CARGO_MANIFEST_DIR` in build.rs | open | С.2 |
| 38 | Flatpak steps after compose never ran on a real runner | CI | unknown | needs the owner's push |
| 41 | `pkill -f` kills the caller's own shell | agent scripts | trap | goes into the briefs |
| 60 | `docs/features/README.md` drifts from `--table`, `--check` does not see it | docs | open | С.2 |
| 61 | dead code warnings; two clippy notes | `kt-app/src/reel_preview.rs:188`, `:265`; `kt-cli/src/main.rs` | open | С.2 |
| 62 | two chains in a window share probe names | `kt-app/src/deck_face.rs` | open | С.2 |
| 63 | TEST lines point at the wrong file: `automation` t645 names `auto_plan.rs`; the other specs were not checked | `openspec/specs/*/spec.md` | open | С.2 |
| 74 | `hwencode` live scenario | The second process overwrites the first's `01-step` screenshot, the scenario sets the folder | `src/scripts/live/hwencode.py` | open | С.2 |
| 83 | `openspec validate --specs` did not finish in 60 s | cause not read | tooling | open | С.2 |
| 101 | live first pass under load | On the merged tree 70 of 78 first time at load 25–55, 6 more alone; `space_hold` and `readme_shots` stayed red on the rerun. `space_hold` is green alone with the build stopped: it runs on the wall clock (hold 300 ms, double press 400 ms), and a frame past those cannot pass; `readme_shots` lost `win.fx` at the step of the VHS deck, a double click again. The last reds are reads in scenarios and frames slower than a gesture's own window, none shown to be the program, except row 47 | `src/scripts/live` | open | С.2 |
| 102 | tmpfs for the tests' temp files | Would cut a loaded run's fsyncs at once. `sidecar-fsync-stall` rejected it as hiding a stall a person's disk has; t1460 now holds that stall with a shim, so the reason is weaker, but the disk would leave every other test too | test run | owner decided 2026-11-07: tests go to tmpfs; live Ctrl+S runs stay on a disk until row 47 is read | С.2 |
| 77 | a recovered span leaves no trace | After row 71 the decoder failure of a recovered span is written nowhere: right for a person, but a decoder fault cannot be traced later | `kt-engine/src/export.rs` | open | С.2 |
| 69 | Flathub linter depends on our site answering | `https://kinetren.com` slow: the job fails with `appid-url-not-reachable` | `src/scripts/flatpak-build.sh` | open | С.2 |
| 70 | live scenarios are most of `finish.sh` | `readme_shots` fell from 593 s to about 380 s on a busy machine; the whole run on 4 workers, under a parallel build, is the longest stage still | `src/scripts/live` | reduced by `test-stability` | С.2 |
| 45 | Color Grade GPU and CPU add in a different order: 140 channels per 1080p frame differ by 1 | `src/plugins/fx-video-color/src/color_grade.wgsl` | reported by `compute-backends` | not a defect once the math moves to compute |
| 103 | windows has no picture tests | FXC compiles every shader in `windows-test` (t1650, t1651); the picture itself runs only in `windows-picture` on lavapipe, which has not had its first run: `continue-on-error` until it has | `.github/workflows/ci.yml` | open, needs a push | С.1 |
| 104 | physics tests read models | VHS and camera physics tests read `tests/model/`; t1640 and t1641 hold the models to the shaders live, so a pass edited alone is red | `src/plugins/fx-video-tape/tests/model/sim.rs`, `src/plugins/fx-video-camera/tests/model/cpu.rs` | by design | — |
| 87 | rules with no test after the CPU paths went | t1055 (an unknown barrier holds rule K) lives only in the spec text; t68 (equal bytes on a dirty pool, by kind) is only `t183` on the GPU now. t328, t1051, t1053, t1187, t1210, t443, t582, t66-t66d, t198, t1530-t1536 went with the code they checked | `openspec/specs` | t1055 and t68 open, the rest closed with their code | С.1 |
| 89 | `nextest` hides what a shared process would show | `park_offline_engine` hung `cargo test` for good and `nextest` stayed green, because each test had its own process. The rule says `cargo test`, `finish.sh` and CI run `nextest`, so a process-wide lock is seen only by a person's manual `cargo test` | `scripts/finish.sh`, `CLAUDE.md` | fixed for this lock, the gap stays | С.2 |

## Changes left unfinished

- `r72-timeline-transport-defects`: archived 2026-11-06, only the split of "Marker over the region"
  waits for the owner.
- `loop-play-latency-hold`: row 16.
- `fx-lens-and-camera` (2 of 14 done), `flatpak-flathub` (11 tasks open): after С.3.
- Every task done, or only the archive steps left: ARCHIVED 2026-11-04 (specs and design records merged,
  F-TL-013 and F-EXP-005 `done`): `ffmpeg-download-window-info`, `menu-chords-out-of-text`,
  `morpher-event-grid-cost`, `split-menu-label`, `still-image-frame-rule`, `selection-copy-target`,
  `shared-window-shell`, `slowed-reverb-is-an-event`, `render-window-v1-parity`, `i18n-gaps` (manual
  check 4.2 run on a throwaway HOME through Gio, not by `make install`).
