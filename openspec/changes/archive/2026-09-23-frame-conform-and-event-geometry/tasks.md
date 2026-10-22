# Tasks

## 0. Documentation before code

- [x] 0.1 Open notes `docs/features/time/retiming/frame-rate-conform.md` (conforming the source frame rate: nearest/duplication/blending, who has it where) and `docs/features/color/management/color-management-and-tags.md` (space and range tags, working space), status `researched`, verdict "deferred" with a reference to R.62; `python3 src/scripts/features-index.py --check` green, the README table updated by hand.
- [x] 0.2 In `openspec/specs/compositions/design.md` — the item "Revision: the reel's own raster and fps — the owner's decision in R.62", and in "Deferred" of the video plan — frame rate conforming and color; check — `openspec validate --specs`.

## 1. Layer geometry (A1)

- [x] 1.1 `kt-media`: `MediaInfo.sar` (num/den, default 1:1) and `display_size()`; `kt-ffmpeg` reads the frame's/stream's `sample_aspect_ratio`; `SyntheticSource` accepts `sar`. Test t711 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`, fixture with `-sar 2:1` in `src/fixtures/gen.sh`) and t711b without FFmpeg (`src/libs/kt-media/src/synthetic.rs`).
- [x] 1.2 `kt-fx`: `conform_geometry(native_display, canvas, rule, crop, zoom) -> LayerGeom` and the table of four rules; test t712 (`src/libs/kt-fx/tests/conform.rs`): vertical/horizontal/smaller/larger source × 4 rules, SAR 2:1, evenness and the 4096 ceiling, "as is" larger than the canvas.
- [x] 1.3 `kt-fx::geom` + `kt-gpu` `geom_fused.wgsl`: the edge primitive `Transparent` and the placement affine in the geo-program, as a pair; test t713 (`src/libs/kt-fx/tests/video_fx.rs` + `src/libs/kt-gpu/tests/parity.rs`): margins are transparent on both backends, tolerance class EXACT for K = 1, the identity affine is byte for byte equal to the old path.
- [x] 1.4 `kt-engine` `video_plan.rs`/`video.rs` + `kt-gpu/src/engine.rs`: `LayerSrc` carries `LayerGeom`; the order `(media, pts, Quality)` takes `decode` from the geometry; `fits` checks against the decode size; the composite places the layer into `rect` (CPU and GPU). Test t714 (`src/libs/kt-engine/tests/decode_scale.rs`): order == read on four rules, the step from the conform size; t714b (`src/libs/kt-gpu/tests/gpu_render.rs`) placement conformance.
- [x] 1.5 Goldens do not move: t715 (`src/libs/kt-engine/tests/golden_export.rs`) — `GOLDEN_VIDEO`, t11/t67/t184 green without regeneration; a vertical source in 16:9 gives a frame with transparent/background margins (a measurement of the geometry of the white rectangle, as in F-MED-004).
- [x] 1.6 Thumbnails: the `THUMB` frame → size by the source's view; the sidecar table is additive; the filmstrip draws margins in the row's color. Test t719 (`src/apps/kt-app/src/thumbs.rs` + `layout_tests.rs`): a vertical clip is not squashed, scrolling does not order a decode (t8 class).

## 2. The "Geometry" node (A2)

- [x] 2.1 `kt-model`: `FxInstance::Geometry { lead, rule, crop, pos, zoom, rot, anchor }`, the property passport (R.59), removal of `Pan`, validation "exactly one leading node per video event, the rule only on the leading one, the leading one is undeletable"; a drop creates a leading node. Test t716 (`src/libs/kt-model/tests/fx_chain.rs`): deleting the leading node — a refusal with a reason, reordering — ok, a second node — ordinary, `Pan` is absent from `FxInstance::all()`, the golden table of t328 covers `Geometry`.
- [x] 2.2 `kt-fx` chain scheduler: effects before the leading node — on the decode buffer, after — on the canvas; Pan3d/CamMotion — over the conformed one. Test t717 (`src/libs/kt-fx/tests/video_fx.rs` + `src/libs/kt-gpu/tests/parity.rs`): Blur → Geometry ≠ Geometry → Blur on a vertical source, both backends within tolerance; a 90° rotation of the vertical source fills 16:9.
- [x] 2.3 `kt-app` `fx_editor.rs`: the leading node is pinned by a flag, its cross gives an error (rule 16), the catalog contains "Geometry", the rule combo is localized in five catalogs (`i18n_tests.rs`, `font_tests.rs`). Test t716b (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 2.4 `kt-cli probe` prints the rule, the rectangle and the decode size; test t727 (`src/apps/kt-cli/tests/cli.rs`).

## 3. Project rule, window, placement (A3)

- [x] 3.1 `kt-model`: `ProjectSettings.conform`, `settings_explicit`; `Conform::Project` is resolved through the project in `frame_plan`; changing the project rule rewrites only those that follow it. Test t718 (`src/libs/kt-engine/tests/conform_rule.rs`): two events (Project / explicit Fill), changing the default changes the first and does not touch the second.
- [x] 3.2 `kt-app`: the "Project settings" window (raster with validation, fps from divisors of 90 000 + a field with a refusal at 29.97, frequency, rule, pitch engine, segment shape), one body with the preview combo, opens from the menu and from the address in the status line, returns focus (rule 14). Tests t720 (`src/apps/kt-app/src/reach_tests.rs` — reachability and verbs), t720b (`src/apps/kt-app/src/layout_tests.rs` — field validation, refusal of 29.97 in the error style, reopening).
- [x] 3.3 `kt-app` `place_media_at`: the first clip into an empty root with implicit settings sets the raster (view with SAR, even) and fps (if it divides 90 000) in one undo step together with the placement commands; explicit settings are not touched. Test t721 (`src/apps/kt-app/src/layout_tests.rs`): an empty project adopts 1440×1080/25; after the settings window — it does not adopt; 29.97 — the raster is adopted, fps is not, the hint says why.
- [x] 3.4 The mismatch message via `hint()` with the address "Event geometry / Project settings"; test t722 (`src/apps/kt-app/src/layout_tests.rs`): 4:3 in 16:9 — a hint; 16:9 in 16:9 — none.

## 4. Reels with their own settings (A4)

- [x] 4.1 `kt-model`: `Sequence.raster`, `Sequence.fps` (`None` — the project), the command and the reel property window (the same fields as the project), fps validation; the instance's native size = the reel's raster. Test t723 (`src/libs/kt-model/tests/nest.rs` + `src/libs/kt-engine/tests/nest_render.rs`): a square reel fitted into 16:9 with margins, "fill" crops, the reel's step comes from its raster, the memo key changes when the raster changes.
- [x] 4.2 Quantization of reel time to its grid when fps is explicit; at `None` — the former continuous path. Test t724 (`src/libs/kt-engine/tests/nest_render.rs`): a 12 fps reel in a 24 project — frames in pairs; without fps — the former bytes of t579/t580; GPU t724b (`src/libs/kt-gpu/tests/gpu_render.rs`).
- [x] 4.3 The law E "with equal settings": t725 (`src/libs/kt-engine/tests/nest_render.rs`) — a reel with its own raster is NOT equal to the expansion (negative), with `None` it is equal (t434/t435/t447 green).
- [x] 4.4 Generators: native = the raster of the container; tests t471/t484b updated to the new formula, t726 (`src/libs/kt-engine/tests/nest_render.rs`): a title in a 1080×1080 reel is computed in its raster.

## 5. Format and finish

- [x] 5.1 `kt-otio`: v11 by content (the project rule ≠ Fit, the explicit rule/parameters of the leading node, the reel settings, `settings_explicit`), migration `Pan` → leading node, a leading node with defaults is not written; test t728 (`src/libs/kt-otio/tests/convert.rs`): a v11 round trip, a file without the new features is byte-stable on the previous version (t433b), an old `Pan` is read into a node with the same numbers.
- [x] 5.2 Export at a resolution ≠ the project's applies the same proportions: t728b (`src/libs/kt-engine/tests/golden_export.rs`) — a 1280×720 export frame is similar to the 1920×1080 preview within tolerance.
- [x] 5.3 Live scenario `src/scripts/live/conform.py`: drop a vertical file into 16:9 — the preview with margins, a hint in the status bar, the settings window opens from the address, changing the project rule changes the picture; screenshots in `target/live-out`.
- [x] 5.4 Specs: update the capability scenarios from the delta, `openspec validate --strict`; F-MED-004 → "in-progress" with `round: R.62`.
- [x] 5.5 `cargo test --workspace`, `cargo clippy --all-targets`, `./src/scripts/finish.sh` — the version in the report; commit of the stage.
