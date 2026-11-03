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
| 7 | 4K decode | The panel says decoding cannot keep up on 4K layers | decode speed | open | С.1, `preview-throughput` |
| 8 | one decoder per file | A picture-in-picture of the same file ping-pongs seeks in the preview; export is fixed | `kt-engine/src/worker.rs` | open | С.1, `preview-throughput` |
| 9 | readback stall | Full-quality preview stops on each frame's return from the GPU | `kt-gpu/src/engine.rs:1231` | open | С.1, `preview-throughput` |
| 10 | export composite and memory | Composite is now the bottleneck; export peaks at 7.3 GB of memory | `kt-engine/src/export_pipeline.rs` | open | С.1, follows the `compute-backends` decision |
| 11 | cold-cache underruns | The first play after opening starves audio (+51k underrun frames, on release too) | prefetch | unknown since `0e7772ff` | С.1, re-measure in `preview-throughput` |
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
