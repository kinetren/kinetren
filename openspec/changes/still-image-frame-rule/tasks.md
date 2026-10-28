# Tasks

Test numbers t936–t941 are reserved by this change. Every stage ends with a commit: the message is in
English and its first line names the stage and what was done.

## 0. Records before code

- [x] 0.1 Proposal recorded: the owner's item 9 of ktprompts v26.10.18 is in `docs/user-feedback.md` (round of 2026-10-27), and this change's proposal, delta, design and tasks are written. Verify `openspec validate still-image-frame-rule --strict` is green

## 1. Prove the cause, tests first

- [x] 1.1 Confirm design D1's claim that nothing in the engine reads `MediaInfo::still`: `grep -rn "\.still\b" src/libs src/apps` lists only `kt-media/src/ffmpeg.rs` and `kt-cli/src/showcase.rs`. Then write t936 in `src/libs/kt-engine/tests/conform_rule.rs` against the REAL `SourceFacts::observe` (no hand-built facts), covering a square still, a SAR 2:1 still, a video, a generator, a title and the placeholder. Verify it is RED for the still (no view), record the failure line here, and verify t718 stays green
  - The grep holds: `.still` is read only by the FFmpeg source itself (kt-media `ffmpeg.rs`,
    kt-ffmpeg `source_impl.rs`), its conformance test and `kt-cli showcase` — never the placeholder's.
    t936 runs the REAL `observe` over the real infos (FFmpeg still and video, generator, title,
    placeholder) and asserts `view_of` and `lead_view`; it was written after `view_of` existed, so its
    red proof is the behaviour test t937 below. t718 green.
- [x] 1.2 `src/fixtures/gen.sh`: `still_square.png` and `square.mp4` (design D3). Then write t937 and t941 in `src/libs/kt-engine/tests/stills.rs` (four rules plus the "project" follow; placeholder in 4:3; generator identity). Verify t937 is RED (every rule gives the canvas) and t941 is green before the fix
  - Fixtures 176×176, not 180×180: with 180 the VIDEO's last 4 columns came back unwritten (alpha
    0) — a decoder defect of its own (swscale's fast YUV→RGBA path at a width not a multiple of 8),
    proposed as a separate change; with 176 the still-vs-video comparison is clean. t937 RED with the
    old branch restored: "fit: the still's picture spans columns (0, 320), expected (70, 250) … fill and
    stretch gave the same still frame — the rule does nothing" — the owner's complaint verbatim. The
    picture span is read by ALPHA (fit's margins are transparent). t941 green before and after.
- [x] 1.3 Write t940 in `src/apps/kt-cli/tests/cli.rs` (probe's conform line for the still and for the placeholder). Verify it is RED: the probe prints `view=180x180` today, but the renderer uses none. The test asserts that the probe line equals what the renderer reports, so the mismatch shows
  - t940 compares the probe's conform decode with the renderer's own decode order: RED with the old
    branch — "the probe's conform decode 180x180 is not what the renderer orders: media 1
    (still_square.png): 30 frames at 320x180".

## 2. One fact, one accessor

- [x] 2.1 `SourceFacts::observe`: store the view only for a source with proportions (`fps.is_none() && !still` is shapeless). Placeholder `still: false` (D1). Add `SourceFacts::view_of` and the `lead_view` helper (D2), and delete the renderer's `else if procedural { None }` branch. Verify t936, t937 and t941 are green, and t796, t796b, t796d, t471, t712, t718 and t722 are too
  - `observe`: `shapeless = has_video && fps.is_none() && !still` — no view stored; the placeholder
    `still: false`; `forget` also drops the view; `SourceFacts::view_of` and `lead_view(project, facts,
    clip, addr)` (a reel's own raster, else the source's view). The renderer's `else if procedural
    { None }` is gone. t936, t937, t941, t796, t796b, t718 green.
- [x] 2.2 `Player::source_view` and the `kt-cli probe` conform section call `view_of`/`lead_view`. Verify t940 is green and t290 (the probe test in `cli.rs`) is still green
  - `Player::source_view` → `view_of`, `kt-cli probe` → `lead_view`. t940, t290 green.
  - t762 (crop narrows the on-screen frame) went red: its scene was a GENERATOR, and the overlay
    had sized the generator's frame by its 4096×4096 reserve — a view the render never used. Now
    the overlay and the render agree (no view → the frame is the canvas), so the crop of a generator
    honestly does not narrow it. The test moved to a source WITH proportions (`harness_geom_ctl_file`:
    the three-clip scene's `synth:1` is not a pattern and opens as a placeholder, so the media is
    re-pointed to `synth:ramp` through the app's own media sync); the assertion is unchanged, and the
    test now also asserts its scene has a view. t760–t766 green.
- [x] 2.3 Write t938 in `src/libs/kt-gpu/tests/gpu_render.rs` (still under fit on CPU and GPU over a coloured lower layer; stretch equals the earlier frame byte-for-byte on the CPU). Verify it is green with the tolerance class of t713, and t713/t714b/t715 and `GOLDEN_VIDEO` are green without regeneration
  - t938 on synthetic sources with the facts of a still (no FFmpeg in kt-gpu tests): fit — CPU vs GPU
    within `FrameTol::BILINEAR` (the class t714b uses for the lead node; t713's CPU side has no GPU
    pair), the margin pixel equal on both and showing the lower layer; stretch — byte-for-byte the
    frame without a view. Ran on the adapter (`KT_REQUIRE_GPU=1`).
- [x] 2.4 Commit stages 1–2 (`cargo test --workspace --no-fail-fast`, `cargo clippy --all-targets` with no new warnings)
  - Workspace: all green except t640 and t663 (gen_event, a 60 s timeout under the full-run load) —
    both green alone; t663 joins the known load-dependent list. Clippy: the two new warnings (a needless
    borrow in `kt-cli probe`, a tuple type in t936) fixed; the rest pre-exist.

## 3. What the person sees

- [x] 3.1 Write t939 in `src/apps/kt-app/src/layout_tests.rs`: harness, square still on a 320×180 canvas, switch the rule through the FX window's row, and compare the on-screen frame rectangle with the preview pixels after each switch. Verify it is green and `src/scripts/specs-user-path.py --check` (t799) accepts its PATH line
  - t939 opens a real square still (`harness_still_of`, the R.67 still harness generalized to a fixture)
    in a 320×180 project, clicks the Geometry node and then each rule word of the FX window's row. After
    each it waits for the complete frame of THAT edit (`preview_project` = the player's version) and
    compares the opaque box of `preview_buf` with `geom_placement` clamped to the canvas (±2 px). Fit —
    side margins, fill — covers with a frame taller than the canvas, stretch — the frame is the canvas,
    as is — a centred square smaller than fit. RED with the still counted shapeless again: "fit: not a
    picture with side margins (0, 0, 640, 360)" — the frame and the pixels agreed, both on the whole
    canvas, which is exactly the owner's "the rule does nothing".
- [x] 3.2 Extend the live scenario `src/scripts/live/stills.py` with the square still in a 16:9 project. Take one `kt_live.step` screenshot per rule (fit, fill, stretch, as is), and measure the picture's width on screen as `conform.py` does. Verify `./scripts/live-check.sh stills` is green
  - `stills.py` part 4–5: a 360×360 red picture in a 1280×720 project; double click → the FX window,
    `fx.slot.0` (the Geometry node), then each word by its new probe `geo.conform.<rule>` (published by
    the rule row in `fx_editor.rs`). One screenshot per rule (`square-fill`, `-stretch`, `-as-is`,
    `-fit`). The width is measured as `conform.py` does (a red run on the centre line), but from the
    preview centre to the right and in the column there — the FX window covers the left of the
    preview, and the panel centre is not the picture centre (the meters): the frame's centre and width
    come from stretch's right edge and height. Green: fill 896 of 896, fit 502 of 896 (9/16), as is
    250×252 in an 896×504 frame.
  - **FOUND BY IT — design D5:** the first run had "as is" filling the whole height of the ½ preview,
    the size of fit. `conform_dest` put "as is" at the view in CANVAS pixels; the preview canvas is
    640×360 for a 1280×720 project, so the preview drew it twice the size the file gets. `LeadCtx`
    now carries `raster` (the project, or a reel's own raster; `in_raster`, `dest`), `conform_dest_in`
    / `conform_geometry_in` scale "as is" by canvas ÷ raster and skip the factor when they are equal
    (exports and goldens keep their bytes); the renderer (`build_reel`, `nest_scales`), the overlay
    (`Player::raster`), the probe and the GPU plan key carry it. t765 extended from fit to all four
    rules in the raster: RED without the factor — "view (1080.0, 1920.0) AsIs @0.5: the frame in canvas
    fractions diverged — (1.125, 3.556) against (0.5625, 1.778)"; its tolerance became one canvas
    pixel RELATIVE to the frame (fill runs past the canvas, and at 1/16 its rounding grows with the
    overflow). A scenario for it in this change's spec.
- [x] 3.3 Run the kt-app UI guards (`reach_tests.rs`, `layout_tests.rs`). Verify they are green
  - Workspace after the raster fix: all green except t640 (reel preview from the sidecar, a timeout
    on its background wait; red once alone at 32 s, then green 5 of 5 alone at 0.23 s — on the known
    load-dependent list, and it failed in the run before this fix too). reach_tests and layout_tests
    otherwise green; t938 and the kt-gpu parity suite green on the adapter (`KT_REQUIRE_GPU=1`);
    `specs-user-path.py --check`, `live-sleeps.py --check`, clippy without new warnings.
- [x] 3.4 Commit stage 3
  - Committed with 4.1.

## 4. Records and finish

- [x] 4.1 Append to `openspec/specs/video-plan/design.md` the "why": "procedural" (no grid) and "shapeless" (no proportions) are two facts, and R.67 had silently merged them. Name the three readers that had their own copy. Update the comment at `video.rs` ~932 that sends a still to "no proportions". Verify `openspec validate --specs` is green
  - Two bullets in `openspec/specs/video-plan/design.md` "Decisions and why": the two facts and the
    three readers that had their own copy (renderer lead, `Player::source_view`, `kt-cli probe`), and
    "as is" in the composite's raster. The `video.rs` comment at `observe` already names both facts
    (written in 2.1). `openspec validate --strict` green.
- [ ] 4.2 Run `./scripts/finish.sh` from `src/`. Verify it is green, and record the version `vYY.MM.DD-hash` and the screenshots of 3.2 here
