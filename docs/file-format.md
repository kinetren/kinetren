# Exchange format: OpenTimelineIO (.otio)

**Since R.70 this is the description of the EXCHANGE file, not the project.** The project file is its own `.kt`
(`src/libs/kt-model/src/project_file.rs`: a line-by-line deterministic JSON of the model, the `kinetren` marker and the version at the root; a file with the former marker `kinotrain` —
before the rename to Kinetren on 2026-10-21 — is rejected by "Open", and is repaired by import); OTIO remained as two doors —
`File → Import older Kinetren project…` and `File → Export → OpenTimelineIO…`
(`kt-cli import` / `kt-cli export-otio`). The contract — `openspec/specs/project-format`.

The exchange format is OTIO (JSON, the reference is the OpenTimelineIO library 0.18.x).
The implementation is the crate `src/libs/kt-otio` (pure Rust, see its CLAUDE.md).
The decisions of round R.24 — docs/user-feedback.md; the catalogue of subtleties — items 135–153.

The old `.ktproj` is ONLY for import ("File / Import older Kinetren project…"
or `kt-cli import`): overlaps are converted into transitions/split (item 143),
the result lives as `.kt`, the source is not changed. There is no writer of ktproj in the codebase
any more.

## The OTIO layer (compatibility with other programs)

- A file = one `Timeline.1`; tracks — `Track.1` (`kind: Video|Audio`) inside
  `tracks: Stack.1`. **The order of video tracks is inverted relative to our
  lanes**: OTIO composites later tracks over earlier ones, while our lane 0 is the top,
  therefore the first video track of the file is the bottom layer. Audio — in direct order
  after the video.
- A clip — `Clip.2`: `source_range` (in-point + duration),
  `media_references["DEFAULT_MEDIA"] = ExternalReference.1` with `target_url`
  (a path relative to the project file; not made relative — an absolute
  `file://` URL; `synth:<pattern>` — test sources, verbatim).
- A generator (R.52) — media with the path `gen:<kind>[:<key>=<value>]*` (canon:
  lower case, defaults are not written, colour as one token `c=rrggbb`;
  the vocabulary — `kt_model::GeneratorKind`). In the file its clip carries
  `GeneratorReference.1`: `generator_kind = "kt.<kind>"`, `parameters` — the fields of the kind,
  `available_range` is MANDATORY (0 .. source_in+duration, rate
  90000 — a generator has no duration, but the format requires it to be named).
  A FOREIGN `generator_kind` (not `kt.*`) is not typed: passthrough +
  a noticeable placeholder. The generator does NOT raise the file version: an old build
  sees a placeholder instead of the frame, and the node carries it verbatim — there is no silent corruption.
  An unused generator in the bin travels by path in `kt.bin_media`.
  **R.53 (items 419-424)** added the kinds `transparent`, `rect`, `ellipse`,
  `line`, `leader`, `table` (a variety — the field `table`), `window`, and the title animation
  fields `anim`/`anim_secs`. It still does NOT raise the file version, for the same reason:
  an old build shows an unfamiliar `generator_kind` as a placeholder and carries it verbatim, while serde
  ignores unfamiliar fields of `parameters`. Files WITH TITLES acquire the keys
  `anim`/`anim_secs` when re-saved (all fields are serialised) — idempotence holds,
  byte-stability of exactly such files is not promised.
- Gaps — `Gap.1`. **A trailing gap sets the editing length** and is written:
  a track shorter than the end of the timeline is padded with it (R.25 — otherwise a foreign edit
  became shorter after our save). The length is read in
  `Project.timeline_end`, the render runs up to it with black/silence.
  An empty track that was not in the source file is not written.
- A transition — `Transition.1{ in_offset, out_offset, transition_type }` between
  neighbouring clips; the same object as the FIRST/LAST child of a track or next to
  a `Gap` is a fade-in/fade-out (R.25): the reference substitutes a Gap for the missing neighbour, and we render the fade to transparency/silence.
  Outward `transition_type` is meaningful (R.33, item 222): the crossfade —
  `SMPTE_Dissolve`, wipes/masks — `SMPTE_Wipe`, everything else — `Custom`;
  a foreign type is imported as a crossfade (notice) and is written back
  untouched. Our data is in `Transition.metadata.kt`:
  `kind` (the kind with parameters, internally tagged), `audio` (the curve
  of the audio crossfade, not written under equal-power), `auto` (the curves
  of the kind's parameters, ticks from the start of the window), `invert` (the inversion of R.51,
  written only as true). Returning to an ordinary crossfade CLEARS the envelope.
  An OVERLAPPED transition (R.51, item 399) cannot be expressed as a child of a track (its pair
  lies in neighbouring Tracks of the layer map) — it travels as a list
  `overlap_transitions` in the envelope of its reel: `[{a, b, kind?, audio?,
  auto?, invert?}]`, there are no offsets (the window is the overlap zone itself), the id
  is assigned on reading.
- Markers — `Marker.2` on the root Stack: `marked_range` (a range marker,
  items 141-142; a point marker has zero duration), `color` (ours are written as GREEN
  by default, a foreign colour is carried round-trip).
- FX — standard `Effect.1` in the `effects` of the clip: `effect_name = "kt:<kind>"`,
  `enabled = !bypass`, parameters (the typed `FxInstance`) — in
  `metadata.kt` of the effect. A reverse of the clip — `LinearTimeWarp.1{time_scalar: -1}`
  FIRST in the chain (FX are applied after the reverse). A foreign scalar ≠ ±1 is not
  rendered (notice + passthrough).
- Time — `RationalTime{value, rate}`. Our files write rate=90000 (ticks, integer
  values are exact in double); foreign rates are quantised to the tick grid.

### Passthrough

Content that is not understood is preserved and written back: unknown fields of known schemas stay in their places; objects of unknown schemas or leaf versions are kept verbatim (on the timeline they appear as a grey placeholder: it takes up space and can be deleted, but cannot be moved); foreign `metadata` namespaces are left untouched. A version of the SPINE (Timeline/Stack/Track) newer than the supported one is an honest refusal. The byte guarantee is idempotence: `save(load(save(x))) == save(x)`; the first save of a foreign file normalizes its formatting (as the reference does). Binding of leftovers: clips by ClipId, tracks/gaps/transitions by position at load time (a v1 limitation).

## The `metadata.kt` layer (our data; version — `kt.v`, currently 10)

The file has ONE version (on the Timeline), and it is set BY CONTENT: a file without new features is written with the lower version and stays byte-stable; a file newer than the supported one is an honest refusal `KtMetaTooNew` (an old build would silently corrupt the edit). What raises the version:

| v | What appeared |
|---|---|
| 2 | cameras and camera tracks; FX automation (R.29–R.30) |
| 3 | curves in material ticks; switches; transition kind and audio curve (R.33) |
| 4 | morpher LAYER and named areas (R.35) |
| 5 | curves and strength of the morpher layer (R.38) |
| 6 | nested sequences (R.50) |
| 7 | R.51 transitions: `invert` and overlapping ones (`overlap_transitions`) |
| 8 | an event left of zero inside a reel (R.58) |
| 9 | a generator on an event; public parameters of a composition and overrides (R.59) |
| 10 | segment shape on a keyframe and the project's default shape (R.61) |

**Segment shape (R.61).** In the model the shape lives on the KEYFRAME and describes the section after it; in the file it is written BY CONTENT. A curve whose keys all carry one shape from the pair "third order with the previous smoothing / linear" is written with the PRIOR field `ease` (`true`/`false`) and does not bump the version — the bytes of such files did not change. Any other shape travels in the `shape` field on its own keyframe and raises the file to v10; the reader expands `ease` into a uniform shape, and `shape` overrides it. The warp speed (`rate`) accepts only linear: on a piecewise-linear profile there is a closed stretch map.

On `Timeline.metadata.kt`:

| Key | What |
|---|---|
| `v` | envelope version; newer than the supported one → load refusal |
| `settings` | `{fps, width, height, sample_rate}` |
| `transport` | `{region: [start, end] in ticks, loop_enabled}` — outside undo |
| `ui` | `UiState` (viewport/cursor/lane_order/meters_w; `lane_names` — only morph: the names of v/a tracks live in `Track.name`; bin fields — as separate keys below) |
| `bin_media` | media NOT on the timeline: `[{id, path}]` (paths as target_url) |
| `bin_groups` / `bin_layout` / `bin_free` | layout of the media bin |
| `end` | editorial length in ticks, if it is longer than the material (trailing Gap) |
| `lanes` | number of our tracks `[video, audio]` — empty ones are not written to the file |
| `layers` | layer overlap: `{video: [n…], audio: [n…]}` — how many consecutive `Track`s each track occupies (overlap inside one track is inexpressible in OTIO), plus `off` when the option is switched off |
| `morph_lanes`, `morphers` | morpher system (the engine sees only effective_clips) — morph tracks are NOT OTIO tracks; morphers are sorted by MorphId |
| `sequences` | sidecars of nested reels (R.50): tracks/order/morphers/cameras/layer map of each; the body of a reel is inlined as a `Stack` into each instance |
| `overlap_transitions` | overlapping transitions of the ROOT reel (R.51, item 399): `[{a, b, kind?, audio?, auto?, invert?}]`; for nested reels the same field is in their sidecar |

On `Clip.metadata.kt`: `{id, media, link_group?, gain?}` — our numeric identifiers (stability of undo/morphers/links) and the clip gain (there is no standard place for it in OTIO). A foreign file without kt gets an id deterministically (item 153).

On `Effect.metadata.kt`: the full `FxInstance` (internally tagged, `{"kind": "...", ...parameters flat}`).

The schema of the reusable types (`FxInstance`, `MorpherEvent`, `UiState`, `BinGroup`) is the serde-derive of the live kt-model model; its drift is caught by the byte goldens `src/fixtures/demo-legacy-import.otio` (test 148) and the kt-otio tests (tests 144–154). Conformance with the reference python library — test 149; a cross-render against melt (MLT) on foreign samples — test 158 (`src/fixtures/otio-samples`, both gated on the presence of the tools).

## v1 limitations (documented, not implemented)

A `SerializableCollection` root (several timelines) is a refusal; `.otioz`/`.otiod` bundles are not supported; the label colour is not editable in the UI (round-trip only); import notices are shown in English (cataloguing is a future round).
