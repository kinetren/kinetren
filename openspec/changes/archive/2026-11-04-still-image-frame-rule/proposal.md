# Proposal

## Why

The owner (ktprompts v26.10.18, item 9, recorded in `docs/user-feedback.md`, round of
2026-10-27): a square still on a 16:9 timeline is squashed, and in its Geometry not one
frame-rule option (fit / fill / stretch / as is) changes anything, while the same options work
for video. "Duplicated code again? Why does it work for video and not for stills?"

The conform code is not duplicated: there is one `conform_dest`. The still never reaches
it with its proportions. `SourceFacts::observe` (`src/libs/kt-engine/src/video.rs:779-787`)
defines "procedural" as "has video and no fps". That rule was written for generators,
titles and the placeholder, which are drawn at the requested quality. R.67 then gave a still
`fps: None` too (`src/libs/kt-ffmpeg/src/source_impl.rs:285`), so a still is now procedural.
The renderer drops the view of every procedural source (`video.rs:945-946`, the R.62 comment "it has
no proportions"). With no view, `conform_dest` returns the canvas for every rule
(`src/libs/kt-fx/src/conform.rs:50-52`), the decoder stretches the picture to 16:9
(`clip_base_quality`, `video.rs:591-606`), and the Geometry node is an identity. Two other
readers of the same fact do NOT drop the view: the Geometry overlay
(`src/apps/kt-app/src/geomctl.rs:274-279` via `Player::source_view`) and `kt-cli probe`
(`src/apps/kt-cli/src/main.rs:415`). So the frame drawn over the picture and the probe's
"conform" line both show a Fit rectangle that the render does not use. That breaks the
R.64 rule that the placement arithmetic exists only once.

## What Changes

- "Has no frame grid of its own" (procedural timing: no floor to the frame grid, no end of
  material) and "has no proportions of its own" become two separate facts of
  `SourceFacts::observe`, both decided in that one place. A single picture keeps the first
  (R.67 stays as it is) and loses the second: it is conformed by the event's frame rule
  exactly like a video of the same size and pixel aspect. Generators, titles and the
  missing-file placeholder are drawn at the requested quality and stay without
  proportions: frame = canvas, as the generators spec says.
- The renderer, the Geometry overlay (`Player::source_view`) and `kt-cli probe` read the view
  through ONE function of `SourceFacts`. There is no second "is it procedural" branch at a
  call site.
- New fixtures in `fixtures/gen.sh`: a square still and a square one-second video made from the same
  lavfi pattern, so the four rules can be compared still against video.
- No change to the kernels (`kt-fx/geom.rs` ↔ `kt-gpu/geom_fused.wgsl`): the still now gets
  the same lead geometry as a video. The new GPU scenario still names both files, because
  the pair rule applies to anything that checks the lead node on both backends.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-plan`: a requirement that the source's proportions are one fact. A single picture
  is conformed by the frame rule like a video. Only a source drawn at the requested quality
  (generator, title, placeholder) has no proportions. The plan, the on-screen geometry frame and
  `kt-cli probe` read the same fact.

## Impact

- Code: `src/libs/kt-engine/src/video.rs` (`SourceFacts::observe`, a `view_of` accessor, the
  renderer's lead at ~935-953), `src/libs/kt-engine/src/player.rs` (`source_view`),
  `src/apps/kt-cli/src/main.rs` (probe conform section), and possibly
  `src/libs/kt-media/src/placeholder.rs` (decision D1). Nothing changes in kt-fx or kt-gpu kernels.
- Fixtures: `src/fixtures/gen.sh` gains `still_square.png` and `square.mp4`.
- Goldens: unchanged. Every existing still fixture (`still.jpg`, `still_alpha.png`,
  320×180) matches the 320×180 test canvas, so Fit = Stretch for it. t796/t796b stay
  byte-for-byte.
- Project format: untouched. Any project that already has a non-square still on a canvas of
  other proportions will look different after the fix, because it now follows its frame rule
  (the project default is Fit). That is the fix itself, see design "Risks".
- Docs: none. This is a defect, not a feature (R.62 and R.67 are done), so `docs/features/` is not touched.
