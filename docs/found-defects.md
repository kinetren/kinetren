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
| 14 | Noise default | Added Noise shows nothing until the amount is raised | barrier noise kernel | open | owner's decision: a fix moves bytes |
| 15 | VAAPI/QSV | Intel and AMD never encode in hardware; picking `h264_vaapi` by hand crashes the render | change `hw-encode-context`, 14 of 21 tasks done | open | С.1 |
| 16 | loop freeze on wine | Play with a loop region freezes the picture until a click | change `loop-play-latency-hold` | open, unverified | С.3, wine probe first |
| 17 | `.kt` MIME | A name-only guess can open `.kt` as Kotlin | `src/packaging/kinetren-mime.xml` | open | owner's decision |
| 18 | seam outside a wrap | A clip dragged during playback does not refill the pitch look-ahead | kt-engine audio | deliberate | only on a complaint |
| 19 | Arabic wrapped lines | Wrapped Arabic labels have imperfect run order | `MAX_VALUE_CHARS`, kt-app | known limit | later |
| 20 | mask after Geometry | A mask follows only the leading Geometry node | masks spec | recorded in the spec | later |
| 21 | Datamosh placement | CPU only, cannot sit on a reel or a track | `datamosh-codec-path/design.md` | by design | not a defect |
| 22 | Windows supervisor | If the crash supervisor is killed, the editor keeps running | `render-crash-and-recovery/design.md` | open | later |
| 23 | frame toggle glyph | The toggle's glyph sits in every preview crop | kt-app preview | unknown | С.3, check first |
| 37 | VHS speed | The VHS CPU path runs at about 10 fps, no SIMD twin | `fx-video-tape` | open | С.1, follows the `compute-backends` decision |
| 42 | VHS thread cap | VHS Tape uses at most 16 threads, so half of a 32-thread machine idles | `src/plugins/fx-video-tape/src/sim.rs:126` | reported by `compute-backends` | С.1 |
| 43 | Datamosh motion search | One thread, 67 ms per 1080p frame, multiplied by the frame chain | `src/plugins/fx-video-codec/src/codec.rs:279` | reported by `compute-backends` | С.1, moves to compute |
| 46 | sound of a device effect on a track chain | VHS and Datamosh sound on a track's chain may count fields and frames from the wrong time: `TrackProcessor` never calls `place` | `src/libs/kt-fx/src/audio/mod.rs` | not checked | С.3, check first |
| 47 | `vhs_tape` red 2 of 5, once a Ctrl+S that did not save | If the Ctrl+S is the program's, a person loses edits | `src/scripts/live/kt_live.py:1407` | cause unknown | current stage, read `finish.sh`'s result first |
| 48 | Datamosh sound at a cut | The audio half assumes a long previous event, so after a short one the break may miss the picture's | `src/plugins/fx-video-codec/src/sound.rs` | open | С.3 |
| 49 | unknown rows dropped silently | An older build drops rows it does not know and loses their settings on saving | `src/libs/kt-model/src/plugin_slot.rs` | open | С.3 |
| 50 | morpher hides the sound half | The sound half follows the layer, is not visible in its strip and cannot be switched off there | morpher | deliberate, reported | owner informed |
| 51 | format 4 | A project using a device's sound settings does not open in older builds; choosing a look sets them | project format | deliberate, in What's new | owner informed |
| 52 | window preview below 25 fps | With no real graphics card, `queue.submit` takes 30 ms in a window and 0.5 ms without one, so 17-20 fps there | `kt-gpu/src/engine.rs`, `present` | cause unknown; no real card measured | С.1 |
| 53 | dropped-frame counter on the CPU path | The panel shows 0-14 skipped frames with real losses; V-fps is right | `kt-app` `draw_preview` | open | С.1 |
| 54 | one-off composite stalls | 1.7-1.9 s at one cut and once 21.9 s, on the old path only; at most 25 ms since | composite | cause unknown | watch |
| 55 | RGBA conversion, one thread per file | The next limit for 4K at native size | `kt-ffmpeg` `to_rgba` | not reached in the measurements | С.1 |
| 56 | a start on a really cold disk | The OS page cache could not be dropped here; if sound misses the 500 ms wait, the silence stays as before | start of playback | not measured | watch |
| 44 | no ARM build | The program cannot run on ARM at all; the owner wants it | `.github`, `scripts` | open | С.2 |

## Internal

| # | Item | Where | State | Verdict |
|---|---|---|---|---|
| 24 | t815's 2 s limit fails under disk or CPU load | `kt-app/src/layout_tests.rs:18065` | open, verified | С.2 |
| 25 | `formula` live scenario red under load | `src/scripts/live/formula.py` | open | С.2 |
| 26 | live scenarios red in a batch, green alone; displays collide | `scripts/live-check.sh` | open | С.2 |
| 27 | sidecar "database is locked" (t640, t1171, t411, t1413) | `kt-media` | fixed by `ece65c8f`, to confirm on the merged tree | confirm in the stage's `finish.sh` |
| 28 | load-flaky tests: t663, t888, t947, t951, t952, t476, t795, t986, t939, t20 | various | open | С.2 |
| 29 | two parallel fixture generations share one temp name | `src/fixtures/gen.sh` | open | С.2 |
| 30 | `BadWindow` in ui-probe, the `welcome` scenario | live | open | С.2 |
| 31 | a plugin edit costs 158 s of test build | workspace | open | С.2 |
| 32 | CI wall 21 min; `ci-speedup` proposed only | `.github/workflows/ci.yml` | open | С.2 |
| 33 | `font_tests::sources()` misses code after the first `#[cfg(test)]` | kt-app `font_tests.rs` | open | С.2 |
| 34 | `cargo fmt --all` reformats about 60 files | tree | open | С.2, one commit |
| 35 | old clippy warnings | kt-gpu, kt-cli, kt-app | unknown | С.2 |
| 36 | a copied `target/` builds another worktree's text | `CARGO_MANIFEST_DIR` in build.rs | open | С.2 |
| 38 | Flatpak steps after compose never ran on a real runner | CI | unknown | needs the owner's push |
| 41 | `pkill -f` kills the caller's own shell | agent scripts | trap | goes into the briefs |
| 57 | t836, t362 red once under load; t1524 has a thin margin (70 % against 64 %) | `telemetry.rs`, `project_lock.rs` | open | С.2 |
| 58 | t939 runs `gen.sh` on every call | `kt-app/src/layout_tests.rs:17714` | open | С.2 |
| 59 | `loop` live scenario allows one stale dump read | `src/scripts/live/loop.py:54` | open | С.2 |
| 60 | `docs/features/README.md` drifts from `--table`, `--check` does not see it | docs | open | С.2 |
| 61 | dead code warnings; two clippy notes | `kt-app/src/reel_preview.rs:188`, `:265`; `kt-cli/src/main.rs` | open | С.2 |
| 62 | two chains in a window share probe names | `kt-app/src/deck_face.rs` | open | С.2 |
| 45 | Color Grade GPU and CPU add in a different order: 140 channels per 1080p frame differ by 1 | `src/plugins/fx-video-color/src/color_grade.wgsl` | reported by `compute-backends` | not a defect once the math moves to compute |

## Changes left unfinished

- **`r72-timeline-transport-defects`: 19 tasks, none done.** Seventeen timeline defects the owner
  meets every day, ordered by him on 2026-10-30. Verdict: first in С.3.
- `hw-encode-context`: row 15. `loop-play-latency-hold`: row 16.
- `fx-lens-and-camera` (2 of 14 done), `flatpak-flathub` (11 tasks open): after С.3.
- Every task done, or only the archive steps left: `ffmpeg-download-window-info`,
  `menu-chords-out-of-text`, `morpher-event-grid-cost`, `split-menu-label`, `still-image-frame-rule`,
  `selection-copy-target`, `shared-window-shell`, `slowed-reverb-is-an-event`,
  `render-window-v1-parity`, `i18n-gaps` (manual check 4.2 left). Verdict: one cleaning executor
  checks and archives them.
