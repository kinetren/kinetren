# Tasks

## 0. Feature notes and agent rules (before code)

- [x] 0.1 `docs/features/automation/keyframes/formula-as-segment-and-layer.md` (F-AUT-010):
  the combination of keys, segment shapes, a formula on a segment and a formula layer in Vegas/Premiere/
  Resolve/FCP/AE, "the trick behind the trick", verdict with the reference `CAPABILITY: openspec/specs/automation/`.
  Check: `python3 src/scripts/features-index.py --check` is green.
- [x] 0.2 `docs/features/automation/keyframes/shape-parameters-by-mouse.md` (F-AUT-011):
  shape parameter handles on the graph (tension/amplitude/periods under the mouse) in five
  editors; verdict with `CAPABILITY: openspec/specs/ui-timeline/`. Check: `--check`.
- [x] 0.3 `docs/features/help/hints/formula-input-help.md` (F-HLP-034): reference and completion
  in the formula field (AE expression menu, Resolve Fusion, Blender drivers) without pop-ups; the verdict form
  `TEST: t789 …`; `ui_rules: [10]`. Check: `--check`.
- [x] 0.4 Rules in `CLAUDE.md` (root, kt-otio, kt-app): "repairs of old files — only in the
  import, with the version, they die out with the version"; "a new UI scenario has a PATH line, a live
  scenario has a screenshot per step, the list goes into the report"; "the key only from a gesture, the widget does not
  write by itself". Check: t799 (task 7.2) reads the threshold from the same words; `docs/user-feedback.md`
  R.67 is already recorded.

## 1. Import of old versions (item 1)

- [x] 1.1 `kt-otio`: `legacy_import.rs` — table of repairs with the version, the first
  `v5_null_morph_targets`; `import_older(text, base)`; `load_project` wraps the error of reading a file of a version < current into `Error::NeedsImport { version }` with a text naming
  the import. Test t783b (`src/libs/kt-otio/tests/convert.rs`) on the form `untitled2_v26.09.26`
  (a synthetic copy of the structure: three `null`, two objects, one list); RED before the fix
  (today — `untagged enum Repr`).
- [x] 1.2 `kt-app`: item `File → Import older Kinotrain project…` (verb registry, catalogs ×5),
  the "Open" error window with the version and the name of the item, the import opens the document
  with the previous path and an asterisk. Test t783 (`reach_tests.rs`); live scenario
  `src/scripts/live/import_old.py` (a copy of the v5 structure in the fixture, PATH steps
  with screenshots). Then open `sources/26.09.26/untitled2_v26.09.26.otio` itself through the import
  in Xvfb and say in the report what is visible.

## 2. Curve model: segment formula, layer, `t`, overrides, key only by gesture (items 3–6, 11, 12)

- [x] 2.1 `kt-model`: `SegShape::Formula { expr }` (variables `s`, `a`, `b`, `t`; a parse cache;
  a loud refusal in validation and in the command), `Copy` is removed — places with a copy of the shape use
  `clone()`. Test t787 (`src/libs/kt-model/tests/fx_auto.rs`); `GOLDEN_*` do not move
  (t11, t67, t695b).
- [x] 2.2 `kt-model`: `ParamCurve.layer: Option<String>` with variables `k`, `t`, `src`;
  order keys → layer; the resolver flattens the layer and segment formulas into keys on the frame
  grid in the same pass as the links; a `Formula` source is read as a layer over empty
  keys (t707 green); a project without formulas — 0 access count (t705, t710). Test
  t787c (`fx_auto.rs`).
- [x] 2.3 `kt-otio`: the envelope of the shape `formula` and the field `layer`, v13 BY CONTENT, round trip
  byte for byte, refusal of the old reader by version. Tests t787b, t787d
  (`src/libs/kt-otio/tests/convert.rs`); t144 idempotence green.
- [x] 2.4 `kt-model`: `formula_preview` counts `t` from the start of the material (receives
  `source_in`), the window is only the range of the sample. Test t784 (`src/libs/kt-model/tests/
  formula.rs`); RED before the fix (the preview gives 0 at the left edge of the window).
- [x] 2.5 `kt-model` + `kt-engine`: validation of overrides accepts a formula, a layer,
  shapes and a link inside the reel; `apply_overrides` resolves sources in the reel's domain before
  `retime_curve`. Tests t792 (`src/libs/kt-model/tests/published.rs`) — the owner's project structure
  (two instances one above the other, the curve on the upper one): a ramp 0→1→0 gives
  0.5 in the middle at the target; t792b (`src/libs/kt-engine/tests/nest_param.rs`) — the frame
  of the instance changes by the formula `t/10`. RED before the fix (`CurveSourceUnsupported`).
- [x] 2.6 `kt-app/widgets.rs`: the field with a live curve writes only from a gesture
  (`dragged`/focus+`changed`/reset), the display is clamped by the passport before the widget. Test
  t786 (`layout_tests.rs`): 120 frames of play with a zigzag beyond the passport — on the previous code
  120 keys, after — 0; a manual edit — exactly one.

## 3. Automation widget: drawing, handle, block, link door, reference, widths (items 3, 4, 6)

- [x] 3.1 `automation.rs`: the line is built by sampling `curve_value` per pixel (buffer in
  `AutoView`, invariant 12); for a layer — two lines (`k` solid, the result dashed). Test
  t784b (`layout_tests.rs`): the line's pixels against `curve_value` on a zigzag/step/
  segment formula; RED before the fix (a deviation ≥ 10 px at the zigzag peak).
- [x] 3.2 `automation.rs`: the shape parameter handle at the middle of the segment (vertical — the main
  parameter, Shift+horizontal — periods, the number next to it for the duration of the drag, a 14 px grab, one history
  step). Test t785b (`layout_tests.rs`).
- [x] 3.3 `widgets::curve_block` — one "curve as parameter" block (header with the name and
  color of the selected curve; the shape of all segments with parameters; the layer field; "Driven…";
  "unlink"; delete); called from `fx_editor.rs`, `gen_ui.rs`, `pubparam.rs`
  (the head of the instance); the choice of the curve — by name, line and field (`CurveBind` sets
  `active`). Tests t785 (two curves — the zigzag lands on the selected one; RED before the fix),
  t791 (the block at the instance is the same and `t*2` is not erased; RED before the fix). Live scenario `curves.py`.
- [x] 3.4 The link door "Driven…": a menu "this event / other events of the reel" → properties by
  the passports of the carriers; `CurveSource::Link { path }`; the model's refusal — by a status-line string.
  Test t788 (`reach_tests.rs`); catalogs ×5.
- [x] 3.5 Reference: a group in the block from the evaluator's registry (`Fun::ALL` with arity, variables
  with their purpose), a hint in the status line at the field, the "no name" error with the nearest ones,
  Tab completes in place without a list (`Order` above Middle does not appear). Tests t789
  (`layout_tests.rs`), t789b (`reach_tests.rs`); t83 (no pop-ups) green.
- [x] 3.6 Widths: `param_value`/`plain_row` — by the width of the longest value of the passport,
  the formula/layer field — 220 px, the shape choice — by the width of the longest name. Test t790
  (`layout_tests.rs`) at window widths 900 and 1800; RED before the fix on the shape choice/
  formula field.
- [x] 3.7 Labels of the block, the reference, the link door, the errors — into all five catalogs, font
  subsets rebuilt (`python3 scripts/subset-fonts.py`). Tests `i18n_tests.rs`,
  `font_tests.rs`.

## 4. Frame in the preview and the head of the strip (items 8, 9)

- [x] 4.1 `geomctl.rs`/`user_store.rs`/`app.rs`: `geom_controls` in the user storage
  (default on, "Transform" mode), an icon button in the corner of the preview (only on
  pause, without animation) + the previous menu item; the form's scheme fits `union(frame, rectangle)`.
  Test t794 (`layout_tests.rs`): first launch — the frame is there; switched off, restarted —
  it is not; the scheme at 2× contains all corners of the frame; t764 (idle without repaint) green. Live
  scenario `geomctl.py` extended with PATH steps.
- [x] 4.2 `fx_editor.rs`/`widgets::HeadSlot`: the head of the instance — `📌 <reel name>` via
  `elided_name`, the slot color, the selected one of the same width, the full name — as a hint. Test t793
  (`layout_tests.rs`); live scenario `forms.py` extended with a step.

## 5. Reel previews survive to disk (item 10)

- [x] 5.1 `kt-media/index.rs`: `put_reel_frames` and `put_reel_peaks` in separate
  transactions, the flag "peaks are present" without a bump of `schema_version`, reading the strip without peaks.
  Test t795b (`src/libs/kt-media/tests/index_db.rs`).
- [x] 5.2 `reel_preview.rs`: writing of frames right after the frame stage; `Cancelled`
  separately from failure, a repeated request at the first pause. Test t795 (`layout_tests.rs`):
  cancel after frames → a row on disk; a second opening of the project in the form of the owner's file
  (two reels one above the other with a set, a missing still) — zero renders; RED before the
  fix. Then open `paintitblack_v26.09.26.otio` itself in Xvfb twice and say in the
  report how many renders.

## 6. Stills and media paths (item 13)

- [x] 6.1 `fixtures/gen.sh`: `still.jpg`, `still_alpha.png` (ffmpeg lavfi); `kt-ffmpeg`:
  recognition of a single picture, one frame at any tick, "infinite" duration,
  silence, native → a scale step. Test t796 (`src/libs/kt-media/tests/conformance.rs`).
- [x] 6.2 `kt-engine`: the plan and the export with a still (JPEG and PNG with alpha over a background),
  determinism. Test t796b (`src/libs/kt-engine/tests/stills.rs`).
- [x] 6.3 `kt-app`: the preview of a still without "Loading…" (complete within 180 ms); the placeholder
  of a missing file — a full frame, no scrim, the name in the frame, returning the file replaces it. Tests
  t796c, t796d (`layout_tests.rs`); RED before the fix on a missing file. Live scenario
  `src/scripts/live/stills.py` (PATH steps).
- [x] 6.4 The path of added media is made absolute when added (kt-app, kt-cli);
  relativity — only on writing. Tests t797 (`layout_tests.rs`), t797b
  (`src/libs/kt-otio/tests/format.rs`); t147 green.

## 7. rendereg3, PATH guard, screenshots in the report (items 2, 7)

- [x] 7.1 `git worktree add ../kt-tested-48 tested-48`, build of kt-app there; both binaries in Xvfb
  on `untitled_v26.09.26.otio` at 1380/1390/1400 s, screenshots of the preview, the difference
  (`compare`). If there is a difference → the cause in design.md, a regression test t798 (the crate by
  the cause), the fix; if not → screenshots and words in the report, a question to the owner about the media. Check:
  files `target/live-out/rendereg3/*.png` and a paragraph in the report.
- [x] 7.2 `src/scripts/specs-user-path.py --check` (the PATH line for UI scenarios with a test
  ≥ t783 in kt-app), test t799 (`reach_tests.rs`) calls it; `kt_live.py::step(name)` —
  a screenshot per step into `target/live-out/<scenario>/`; `finish.sh` prints the list after the
  version; `live-check.sh` knows `curves`, `stills`, `import_old`. Check: t799 is red
  when the PATH line of a new scenario is removed, green on the round's specs.

## 8. Specs and finish

- [x] 8.1 The capability specs updated by the delta, `openspec validate --all --strict`; in
  `design.md` of the capability the "why" is recorded (the segment formula as a shape, the layer after keys,
  `t` from the material, the key only by gesture, import instead of the reader, a still as a source, frames
  before peaks); the verdicts F-AUT-008/F-MED-003 and the new notes are updated with the status and
  `CAPABILITY:` after archiving.
- [x] 8.2 `cargo test --workspace` (kt-app per crate with the SIGBUS flake), `cargo clippy
  --all-targets`, `./scripts/live-check.sh` (all scenarios, in a separate run without build load),
  `./src/scripts/finish.sh` — the version and the list of screenshots in the report; a commit
  of the stage; a report on all thirteen items of the owner with what is visible in the screenshots.
