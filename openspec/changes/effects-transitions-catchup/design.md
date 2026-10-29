# Design

## Context

See proposal.md — Why. What exists and shapes the plan:

- **Our catalogue** (observed): `FxInstance::all()` (`src/libs/kt-model/src/project.rs:6166`) —
  46 rows: 18 audio, the layer marker `layer_mix`, 27 video; `TransitionKind::ALL` (`:743`) — 33;
  `GeneratorKind::tag()` (`src/libs/kt-model/src/genmedia.rs:729`) — 15 kinds (19 menu rows). The
  full id lists are in F-FX-007 ("Our catalogue, from the code").
- **Forms a look can take without a new kernel**: built-in variants (`FxVariant`,
  `src/apps/kt-app/src/fx_registry.rs`, R.54 — "a variant is exactly an `FxInstance`", no new
  command); chain presets that carry curves (`FxPresetStore`, `src/apps/kt-app/src/user_store.rs`,
  factory file `src/apps/kt-app/assets/fx-presets-factory.json`, F-AUT-004); segment shapes with a
  zigzag (R.61, `SegShape`, `project.rs`); the formula layer (R.67).
- **Kernel families a new kind plugs into** (`src/libs/kt-fx/CLAUDE.md`, "Video effects of R.47"):
  geo through the closed primitive set (`GeomPrim`, `src/libs/kt-fx/src/geom.rs:470`; a new Warp MODE
  is a core edit of `geom.rs` + `geom_fused.wgsl` in one commit — `plugin-api-stage1` D5 keeps the
  fused map closed to plugins), point-wise through one f64 tap function, barriers with the K× rule.
- **Earlier refusals the waves meet** (`openspec/specs/video-fx/design.md`, "Rejected and why"): "a
  directional (linear) blur, a grid Warper, the shapes/tracking Censor, sepia/invert as separate kinds"
  and a node graph; its "Deferred" allows "a directional blur — as the RadialBlur mode".
  `openspec/specs/transitions/design.md`: a transition progress curve, a gallery of live previews,
  stylised audio transitions — each "not chosen by the owner".
- **Dependencies outside FX**: blend modes (F-CMP-003, in progress), masks (F-MSK-001…003,
  candidates), retime (F-SPD-002), links (F-AUT-008), the offline analysis subsystem (first-wave
  conclusion 1, `docs/features/README.md`; loudness already runs in the background, R.49).
- **Web reconnaissance was available** (WebSearch/WebFetch, 2026-10-28): CapCut's own help page of
  2026-02-03 gave the desktop/web/mobile effect tabs and named looks; the text-animation slots and the
  speed-curve presets come from third-party pages and are marked `[NOT VERIFIED]` where they disagree.
  The core editors' counts were not re-checked (helpx.adobe.com and reddit are closed from this
  environment, `docs/features/_editors.md`) and are marked `[NOT VERIFIED]`.

## Goals / Non-Goals

**Goals:**
- A ranked, checkable plan: every wave is a future change with its kinds, their classes, the plugin
  declarations they need and the earlier decisions they touch.
- Guards that make the catalogue note fail the build when it stops describing the code.

**Non-Goals:**
- Implementing any effect, transition, generator or preset.
- Masks, real 3D, physics, AI effects (see the note's verdicts).
- Changing the picker's form (the gallery with thumbnails, R.54) — the waves feed it.

## Decisions

### D1. The product is the note plus guards; the waves are planned here
`docs/features/README.md` rule 8 keeps implementation order out of notes. So F-FX-007 carries the
reconnaissance, the per-group verdicts and the RANKED list of wave names with their dependencies (a
verdict "take, in this order"); this design carries what each wave builds. After archiving, the "why"
of the ranking moves to `openspec/specs/video-fx/design.md`.

### D2. The ranked waves
Classes: G = geo (closed primitives), P = point-wise (tap), B = barrier (own pass), V = variant or
preset over an existing kind (no new kernel), A = audio node. Families are `plugin-api-stage1` D6
crates.

**W1 — Lens and camera** (`src/plugins/fx-video-distort`, `src/plugins/fx-video-color`,
`src/plugins/fx-video-stylize`; capability `video-fx`; depends on F-FX-002, F-FX-001, F-FX-008;
opened as the change fx-lens-and-camera):
- `lens_housing` (P): the dark circular housing and soft corners of a fisheye, measured in the
  coordinates of its own stage like `vignette`; pairs with `lens_distort` in a factory preset
  "Fisheye lens".
- `lens_dirt` (B): highlights extracted with `glow`'s threshold, multiplied by a procedural dirt
  texture (seeded noise pinned to the stage grid, as `noise` pins grain), added back — invisible on
  dark frames, as real dirt is (F-FX-007, trick 3).
- `lens_drops` (G): refraction by drops is an output→input displacement, i.e. invertible by
  construction — a new `drops` MODE of the Warp primitive (a core edit of `geom.rs` +
  `geom_fused.wgsl`, the one core change of the wave), seeded drop positions and radii.
- `lens_flare` (B): ghosts along the line through a light position and the frame centre; the
  position is a parameter (animatable), not detected.
- Camera shake and zoom punch (V): factory chain presets on `geometry` curves with zigzag segments
  and eased keys ("Zoom Shake", "Handheld"; "Punch in" is already a factory preset).
- (The live re-check of 2026-10-30, F-FX-008, moved `lens_dirt` and `lens_flare` next to `glow` in
  `fx-video-stylize`: a family crate cannot borrow another family's threshold code.)

**W2 — Glitch, film and soft looks** (`src/plugins/fx-video-stylize`, `src/plugins/fx-video-blur`;
`video-fx`; depends on F-FX-003, F-CLR-007):
- `glitch_blocks` (B): block displacement and per-channel offset, a pure function of `burst_index`
  and a seed.
- `film_damage` (B): scratches and dust specks; `gate_weave` (G): a per-frame affine jitter from the
  burst index; `halation` (B): highlights blurred and tinted, the `glow` pattern.
- `light_leak` (P): moving procedural light blobs, a pure function of `u` — the effect form of the
  existing `light_leaks` transition.
- `lens_blur` (B): a disc (bokeh) kernel with highlight weighting, under the K× rule.
- A directional mode of `radial_blur`. Revision: a directional blur returns as a MODE of
  `radial_blur`, not as a kind — the video-fx "Deferred" line already names this form, and the
  rejection of a separate kind stands.
- Sepia, "Movie Grain", "Cinematic" (V): variants of `duotone`, `noise`, `color_grade`. Revision:
  sepia and invert stay variants of existing kinds, as the video-fx rejection of "sepia/invert as
  separate kinds" requires — no new kind is created for them.

**W5 — Transitions catch-up** (`src/plugins/transitions-basic`, `src/plugins/transitions-styled`;
capability `transitions`; depends on F-CMP-002):
- `luma` with a user image as the matte (the passport already has a media TYPE); more `iris` shapes
  (heart, star, hexagon — appended at the end of the shape list); warp transitions (`lens_warp`,
  `ripple_warp`, `twirl_warp`) on the same uniform road as the volume kinds; a motion-blur amount on
  `push`/`slide` whose default 0 gives the former bytes.

**W3 — Text animation library** (`src/plugins/generators-title`; capability `generators`; depends on
F-TXT-006, F-TL-012):
- In / Out / Loop as three preset slots on the title (curve presets over the title's passport, the
  R.53 "presets set everything" form); per-word targets for the owner's "blur on one word of a
  ticker". The first task of that change is an investigation: a per-span style inside the title
  versus a word-box mask (the second waits for F-MSK-001).

**W7 — Voice presets** (`src/plugins/fx-audio-modulation`; capability `audio-fx`; depends on
F-AUD-010, F-AUT-004):
- Presets over existing ids: telephone (`filter` band-pass + `distortion`), megaphone (`filter` +
  `distortion` + `eq`), radio (`filter` + `bitcrusher`), chipmunk / deep (`pitch_shift`), small hall
  (`delay` + `eq`).
- One new node: `ring_mod` (A, a carrier frequency and mix; no state beyond the carrier phase,
  which is a function of the clip position as the LFO rule requires) for "robot".

**W4 — Speed-ramp presets and beat sync** (no new kernel: the presets live in the factory preset
file, the beat analysis is a background job of the program; capabilities `automation` and
`audio-fx`; depends on F-SPD-002, F-AUT-008):
- Rate-curve presets for `slowed_reverb`: Montage, Hero, Bullet, Jump Cut, Flash In, Flash Out
  (signed rate and freeze, R.57, make all six expressible).
- Beat markers from an offline analysis job (the loudness pattern: background, cancellable, result in
  the sidecar), then "snap the ramp to the beats".
- Audio-reactive zoom: a link (F-AUT-008) from an analysis envelope to `geometry` scale.
- Jag edit: a rate preset plus frame interpolation (F-SPD-003).

**W6 — Temporal and relief** (`src/plugins/fx-video-stylize`, `src/plugins/fx-video-distort`;
capabilities `video-fx` and `plugins`; depends on F-FX-005, F-CMP-003):
- `echo` / `trails` and the owner's "buffer + blend levels": a NEW plugin declaration — a temporal
  window of N past INPUT frames (pre-FX frames at t−1 … t−N from the cache, invariant 4) — so the
  kernel stays a pure function and the post-FX memo stays honest. This extends `plugin-api-stage1`'s
  refusal table and is designed in that wave's change.
- `droste` (G): the homothety / recursive zoom as a log-polar `droste` MODE of the Warp primitive
  (core edit, like W1's drops).
- `relief` (B): normals from luminance and one light — bump mapping.

### D3. The rank
W1 first: it is the owner's own FX item 1, and four kernels plus presets cover the lens group that
we lack entirely. W2 next: the most CapCut tabs (Retro, Glitch, Cinematic, Blur) for six kernels. W5:
transitions are half of the complaint, and the gap is short because R.51 already took the trend set.
W3 is ranked after them only because it starts with an investigation; it is the owner's complaint F2
("the text generator is poor") and may be pulled forward on his word. W7 is cheap but audio was not
the complaint. W4 waits for the analysis subsystem; W6 for an API extension.

### D4. The guards live in one kt-cli test file
`src/apps/kt-cli/tests/fx_catalogue.rs` (kt-cli already links every crate and already hosts the
README guard t826). It parses the note's id lists, its verdict table, its owner-items table and its
Verdict list, and this design's waves; a market cell counts as live-checked only through a numbered cite
`(src N)` of a Sources item without `[NOT VERIFIED]` — "was this row checked" is otherwise prose the
guard cannot read; it reads `id:` lines of every note under `docs/features/`
and checks `openspec/specs/<cap>/spec.md` existence. t978 today compares with `FxInstance::all()` /
`TransitionKind::ALL` / `GeneratorKind`; when `plugin-api-stage1` lands, it compares with the
assembler's closed list plus the core kinds (one edit of the test, named in task 2.1). t981 keeps its
closed list of rejected names inside the test (the t643 "frozen list" technique): parsing prose of
design files for "what was rejected" would be fragile, and a reviewer sees the list change in a diff.
After archiving, t979/t981 read the waves from `openspec/specs/video-fx/design.md` where the waves
move.

## Risks / Trade-offs

- [The `[NOT VERIFIED]` market rows are wrong in detail] → the verdicts rest on our gaps (which are
  observed in the code), not on exact counts; each wave's change re-checks its own group live.
- [The guards make every new built-in kind also edit the note] → that is the point (R.47's
  completeness lock, applied to the reconnaissance); the edit is one id in one list.
- [A wave grows into "67 niche plugins"] → a wave's change is sized by kernels (each needs a CPU
  reference, a WGSL mirror and golden rows); presets and variants are counted separately.

## Migration Plan

Docs and a test only; nothing to migrate.

## Open Questions

1. **Is the rank right?** Recommendation: as in D3 — W1, W2, W5, then W3 (pulled forward if the owner
   puts the text generator first), W7, W4, W6.
2. **AI effects** (portrait, sky replacement, style transfer) are rejected on invariant 11.
   Recommendation: keep rejected; if the owner wants them, they are an offline "bake into media" job
   (the F-EXT-001 fallback), never a chain effect.
3. **Per-word FX in a title**: recommendation — a per-span style inside the title (blur, tint, offset
   on a word range) first; a word-box mask only after F-MSK-001.
4. **The temporal window in the plugin API (W6)**: recommendation — admit it as a declaration over
   INPUT frames with a hard maximum N (e.g. 32) so the cost is bounded; refuse any kernel that wants its
   own previous OUTPUT.
