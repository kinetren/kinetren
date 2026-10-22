# Tasks

## 1. One placement arithmetic (foundation)

- [x] 1.1 `kt-fx/src/geom.rs`: move the placement of the lead node into a public `lead_placement(lead, canvas, &EventGeometry) -> Placement { ox, oy, dw, dh, rot_deg, anchor_px, crop_view }`; the `FxInstance::Geometry` branch of the builder computes its own `Aff`/`Proj` FROM IT (no second formula remains). Check: `cargo test -p kt-fx -p kt-gpu` is green without regenerating the golden files (t712–t715, t224), and the new t761 covers the rectangle part.
- [x] 1.2 In the same file, the inverse pair of pure functions: `pos_for_drag`, `zoom_for_handle`, `rot_for_point`, `crop_for_handle`, `anchor_move`. `anchor_move` returns `(anchor, pos)` for which `lead_placement` gives THE SAME rectangle and angle; test t761 (`src/libs/kt-fx/tests/placement.rs`) compares the rectangles at rotation 0° and 30° and requires the exact return of the requested offset.
- [x] 1.3 `Player::source_view(media) -> Option<(f64, f64)>` on top of `SourceFacts.views` (kt-engine), so that the interface knows the view of the source without going to the decoder (invariant 5). Check: test t760 uses it for the frame; `cargo test -p kt-engine` is green.

## 2. The preview share is similar to the project frame

- [x] 2.1 `kt-app/src/app.rs`: the choice of share computes the nearest EVEN PAIR that minimizes |w/h − pw/ph| (a neighbourhood of ±2 on each axis; on a tie, the larger area); the function is pure and placed next to `preview_key_tick`. Test t765 (`src/apps/kt-app/src/app.rs`): 1/1…1/16 at 1920×1080 and 1080×1920 — the deviation of the ratio is no more than half a pixel on the short side; failing on the old independent rounding.
- [x] 2.2 Check "preview == export by frame": the "fit" frame in the SHARES of the canvas is the same for all shares (continuation of t765, `src/libs/kt-fx/tests/conform.rs`).

## 3. Frame over the picture: drawing and hit-testing

- [x] 3.1 `kt-app`: the control layer in `draw_preview` — the frame outline, 8 handles, the rotation handle, the anchor point, 4 crop handles; the geometry is taken from `lead_placement` and converted "canvas → screen" with one scale `img_rect`. The frame is drawn for ONE selected video event that covers the cursor; otherwise the reason is in the status line (rule 16). Test t760 (`src/apps/kt-app/src/layout_tests.rs`): the frame matched the analytical rectangle; for an audio event and under multi-selection there is no frame and the reason is named.
- [x] 3.2 Hit-testing and priority: the grab zone of each handle is ≥ 14 px; with a small frame the handle nearest to the pointer wins, the body does not intercept the handle gesture; the order of resolution is "handle → body → the previous preview resolution", with the controls off the preview path is the same as before. Test t763 (`src/apps/kt-app/src/layout_tests.rs`); the earlier preview tests (single click = play/pause, double click = detach) stay green.
- [x] 3.3 The display toggle in the preview and in its right-click menu; the state is the viewport (`dirty_ui`, not the content); the controls hide during playback and come back on pause; when idle, neither repaint nor allocations in the draw path. Tests t764 (`src/apps/kt-app/src/layout_tests.rs`) and t766 (`src/apps/kt-app/src/reach_tests.rs`: reachability of the toggle, the handle hint in the status line, behaviour in the detached window).

## 4. Gestures

- [x] 4.1 Moving by the body and scaling with eight handles: `Command::SetGeometry` through `apply_coalesced` with a gesture token; the number from the gesture equals the number in the FX window field. Test t760 (continuation): one history step per gesture, the values match the fields.
- [x] 4.2 Rotation with the rotation handle (angle from the anchor), with a step on Shift; test t760b (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 4.3 Anchor: the gesture edits the anchor and compensates the position in one step; the picture does not move at 0° and 30°. Test t761b (`src/apps/kt-app/src/layout_tests.rs`), failing on analytical compensation without the rectangle check.
- [x] 4.4 Crop with four handles: the frame follows the cropped view by the conform rule, the cropped part is transparent. Test t762 (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 4.5 A parameter with a CURVE: the gesture sets a keyframe at the cursor by the same rule as the FX window field (`CurveBind`), instead of rewriting the constant. Test t762b (`src/apps/kt-app/src/layout_tests.rs`).

## 5. The form fields of "Transform"

- [x] 5.1 `fx_editor.rs` + `widgets.rs`: the "Transform" section of the lead node — collapsible groups (frame / position / crop), X/Y pairs on one row, units (× / ° / %), a slider for zoom and rotation, a scheme of the frame with crop and anchor, an "reset geometry" action in one history step. The publication door at the captions is preserved (t752b stays green). Test t759 (`src/apps/kt-app/src/layout_tests.rs`).
- [x] 5.2 New captions and units — in all five catalogues; the font subsets are rebuilt (`python3 scripts/subset-fonts.py`). Tests t759b (`src/apps/kt-app/src/i18n_tests.rs`), `font_tests.rs`.

## 6. Live scenario and the finish

- [x] 6.1 `src/scripts/live/geomctl.py`: select the event, turn on the controls, pull the body, the angle, the rotation and the anchor; assert that the preview pixel changes on move/scale/rotation and does NOT change on moving the anchor; after Ctrl+S the file holds the new geometry values. Run: `./scripts/live-check.sh geomctl`.
- [x] 6.2 The capability specs are updated by the delta, `openspec validate --all --strict`; the "why" is written into `openspec/specs/ui-timeline/design.md` and `video-plan/design.md` (one placement function exposed outward, anchor compensation by the rectangle, the nearest even pair); in `docs/features/ui/viewer/on-screen-controls.md` — the verdict "done, CAPABILITY: ui-timeline" and a section on the deferred cameras/markup.
- [x] 6.3 `cargo test --workspace` (kt-app per crate because of the SIGBUS flake), `cargo clippy --all-targets`, `./scripts/live-check.sh` (all scenarios), `./src/scripts/finish.sh` — the version in the report; a commit of the stage; open the owner's projects in the built kt-app and say in the report what is visible.
