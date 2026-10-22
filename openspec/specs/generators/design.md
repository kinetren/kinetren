# Generators — why it is this way

## What hurt and for whom

The editor could not put into the frame anything that is not in a file: neither a solid
colour nor a title. The owner: "there is not enough video generators, at least text of
various kinds, as in serious programs". The parsing of the pseudo-path `synth:` lived at that time
in two copies (kt-app and kt-cli) and already diverged by case — `synth:Ramp`
silently became a missing file. After the first hand-over (all values in the
media address string) the owner heard that every keystroke in the title field shut down the
decoder and rebuilt the renderers ("this sounds terrible and has no connection whatsoever"), and
later that "changing the heading in the second issue" physically had nowhere to be written:
two titles on one media could not carry different text. The single entry "Create generator"
(right click on the empty background of the bin) was not found by him.

## Decisions and why

- **The generator is a third kind of `MediaSource`, not a new layer of the plan.** The frame
  is born BEFORE `frame_plan`, so kt-gpu, `video_plan` and kt-fx are not touched,
  the CPU↔GPU conformance stays green without a line of WGSL, and the generator gets trim,
  transitions, the FX chain, level, copy-paste and undo for free (invariant 7 is respected
  by construction; capability `video-plan`).
- **The generator stays a medium with a cache and a coverage bar.** The variant "draw
  directly in the composite" was rejected by the owner: a title computes a distance transform and blurs
  alpha three times, noise takes millions of hashes per frame; "it generates too, it does
  work, as if it decodes". The bar does not lie.
- **Values belong to the event, the kind belongs to the medium.** The medium = address and cache bar;
  `ClipEvent.gen`/`gen_auto` = values and curves. The address of the source is derived from the event, the
  shape of the order `(address, tick, quality)` is the former one — "order == read",
  bars, prefetch and export chunks were not edited. "Edit everywhere at once" is a separate command with its own undo step.
- **Curves are resolved by the source at its own tick, not by the plan.** The frame must be
  a function of (state, tick, quality), otherwise the cache key stops determining the frame
  (invariant 4). The plan of compiled automation knows nothing about them (capability
  `automation`).
- **Retuning in place (`retune_gen`) instead of swapping the file.** A strict
  contract: `true` = applied and `info()` did not change; `false` = recreate.
  There is no intermediate state; the predicate must match the implementation.
- **One home for parsing pseudo-paths — the factory in kt-media.** A broken path is an error and does not
  go to FFmpeg. The dictionary of kinds is in kt-model; the dependency kt-media → kt-model
  is acyclic.
- **The determinism of the title is written BEFORE the code**: vendored Noto Sans with sha256 in
  the manifest, pins `=x.y.z` for harfrust/ab_glyph/fontdb, hinting off,
  no libass/freetype through the loader of kt-ffmpeg. The title's golden is separate —
  `GOLDEN_VIDEO` does not move (invariant 11).
- **The shaper at once (the owner's decision).** Noto Sans has no legacy `kern` —
  "just ab_glyph" would give zero kerning.
- **System fonts at once, with substitution.** A family miss = the vendored
  font byte-for-byte + a visible notification; the goldens only on the vendored ones.
- **Backward compatibility is not needed — but reading old files does not break.**
  The old build gets a noticeable stub, there are no converters of legacy titles;
  a foreign `generator_kind` — passthrough (byte idempotence of foreign files costs more than
  typing; capability `project-format`).
- **All title sizes are relative** (the size of the type is a fraction of the frame height, the rest are fractions of em): the frame is born at any Quality, the decode step S×Quality is free.
- **Animation — numbers of a kind, not modes.** Five hard-wired modes became presets
  that set values and curves; time is not part of kt-text.
- **Presets set everything, including position and anchor** (the owner's decision): a "crawl"
  not shifted off the edge is useless. The price — one undo step and an honest
  "matches none" after a manual edit.
- **The generator's form is the first crumb of the event window**, not a separate window:
  the owner wants to see the source next to everything else that is edited on the event.
- **One list of kinds for all entries** (main menu, bin, picker): a new kind
  cannot appear in one menu and not in another.

## Rejected and why

- A title as a new variant of `PlanLayer` — twice as expensive (CPU kernel + WGSL mirror) and
  exactly the class of divergence where preview and export show different things.
- Drawing a title/outline in egui or "over the finished frame" in kt-app — a back door around
  `frame_plan`; kt-cli would get different bytes.
- A sixth FFmpeg library (avfilter → drawtext/libass/freetype/fontconfig) — versions and
  settings are user-specific, the bytes of the frame would become a function of the machine.
- Generators outside the media pipeline (directly in the composite) — rejected by the owner:
  for complex kinds the warm-up is real.
- Parameters in the media address string (the first implementation) — two titles on one
  media are indistinguishable, animation is impossible.
- One "shape" kind with a switch — dead controls (a segment has no rounding, an ellipse has a pair of ends); the passport is handed out per variant.
- Five enum variants for engineering tables — tables have no numbers of their own, it would be
  five copies of the code.
- A second transparency switch on `Solid` — semi-transparency is already expressed by the event's level.
- A per-character machine that cuts the line — the shaper computes the shape over the whole sequence, the glyphs would mutate on every frame.
- The domain "fraction of the event length" for title animation — already discarded with a reason
  (trimming stretched the animation); "reaches the end" is a property of the title.
- Our own grapheme segmenter — a separate subsystem; the shaper gives the clusters for free.
- A flag "which preset was pressed" — it would lie after the first edit of the field.
- `Err` for "no sound" in a generator — false underruns in the negative cache.
- A hash of the bytes or a pointer as the identity of a frame on the GPU — a hot path and ABA;
  the serial number of the cache record is taken.
- A separate `animate_text`/modes in the model — removed; four numbers remain.

## Non-obvious details

- The media path carries ONLY the kind (`gen_kind_path`, `src/libs/kt-model/src/genmedia.rs`),
  but the variant's sort, whose name is a function of the value (table `tb=grey`), remains in
  the path. Parsing of the full old path `parse_gen_path_full` there serves
  migration — do not create a second one.
- The predicate `is_pseudo_path` lives in `src/libs/kt-model/src/genmedia.rs`, and the only doors are
  `open_pseudo`/`open_gen`/`open_indexed` in `src/libs/kt-media/src/factory.rs`. Background services filter pseudo-paths:
  peaks/loudness are not measured, the watchdog does not paint offline, thumbnails are built
  by the factory source bypassing the sidecar DB (its key is the file's metadata).
- `can_retune_gen` (`src/libs/kt-media/src/factory.rs`) MUST match
  `GeneratorSource::retune_gen` (`src/libs/kt-media/src/generator.rs`) on all pairs
  of kinds; switching the tone on in SMPTE changes `has_audio` — the set of clips — and
  honestly rolls back to recreating the source.
- The retune request is a separate field of the queue, not `RequestKind`
  (`src/libs/kt-engine/src/worker.rs`, `retune_gen`): a seek in the middle of a parameter drag would
  cancel it by generation cancellation. It is processed first in the worker loop, the cache invalidation is THERE TOO,
  the race "the old frame was written back after the cleanup" is removed by construction; the list of addresses of the ancestor reels travels inside the message.
- `content_version` is separated from `project_version`
  (`src/libs/kt-engine/src/player.rs`, `update_gen`): prefetch wakes up,
  the renderers are not rebuilt; the same version goes into the kt-app preview key —
  without it the preview held the old frame until the cursor moved (found by a live scenario;
  the harness asked the player's cache directly).
- `Command::touches_render_plan` (`src/libs/kt-model/src/model.rs`) is an
  exhaustive match: a new command will not compile until it is stated whether it moves
  the plan. `SetMediaPath`/`SetClipGen`/`SetClipGenAuto` answer "no".
- `GenState::kind_at` (`src/libs/kt-model/src/genmedia.rs`) returns
  `Cow::Borrowed` without curves — zero cost for a non-animated one; `animated()`
  disables the cache by Quality on a par with `depends_on_time`.
- `GEN_NATIVE` = 4096 (`src/libs/kt-media/src/generator.rs`) equals
  `kt_fx::plan::MAX_INTER_DIM` — the link is by a comment; the "Max (by sources)"
  pseudo-path skips it, otherwise the project would get a native size of 4096.
- `kind_has_alpha` (`src/libs/kt-media/src/generator.rs`) is an exhaustive method; a `matches!` with a single variant would miss a new transparent kind
  as a black plate in the composite.
- The `PxScale` trap in `src/libs/kt-text/src/lib.rs`: it is the height
  (ascent−descent), not the em; the scale — by one function via `units_per_em`.
  The metrics are taken by ab_glyph from the same bytes as harfrust.
- The resolution of a system family — `resolve_face` (`src/libs/kt-text/src/lib.rs`):
  the path and sha256 of the file go into the identity of the frame; kt-cli draws with the same
  resolution. `fontdb` is pinned, because which FILE is selected under the family name depends on the traversal order; the list of families is `OnceLock`.
- The outline is an exact Euclidean distance transform in integer squares i64, the "outside" is obtained by the order of the layers (fill from the top); the shadow is box-blur ×3 of ONE channel
  (without a "green halo"); the compositing of layers is premultiplied u16 with one
  un-premultiplication at the output (`src/libs/kt-text/src/lib.rs`). The output
  outward is straight RGBA8; the engine multiplies it itself.
- `reveal`/`reveal_soft`/`offset` — a one-line modulation in the silhouette loop;
  at `reveal >= 1 && reveal_soft <= 0` the EXACTLY former branch is taken, otherwise the golden
  of the title would shift from the very appearance of the animation
  (`src/libs/kt-text/src/lib.rs`). The 1/64 px quantum is set by the caller
  (`text_params`, `src/libs/kt-media/src/generator.rs`).
- A known limitation: with a soft reveal the outline "clicks" in the middle — the EDT is computed over the binarized silhouette (`src/libs/kt-text/src/lib.rs`).
- The formulas of the presets `text_anim_curves`/`text_anim_values`
  (`src/libs/kt-model/src/genmedia.rs`) call both the migration of old files and the shape; `active_preset` (`src/apps/kt-app/src/gen_ui.rs`) checks BOTH
  halves — by the values alone an empty title matches "typewriter".
- Noise: the same `lowbias32` as the Noise effect of kt-fx, but the grain is a cell of the base
  grid 1920×1080 and the pts of the material (`src/libs/kt-media/src/generator.rs`);
  the tone and the beep are one `tone_1k` with gates, the phase from the global sample number.
- Shapes: distance fields in integer 1/256 px (`isqrt`, `coverage`), the chroma plane of the tables — by columns and rows before the loop; float only in the preamble
  (`src/libs/kt-media/src/generator.rs`). A loop with `sqrt`/`atan2` is a different rounding profile and speed.
- Tables: the origin of the bars is marked "NOT VERIFIED" (EBU 3213 is paid,
  75 % of the scale = 191); the export does not declare colorimetry
  (`src/libs/kt-ffmpeg/src/muxer_impl.rs`) — the table makes this visible.
- The timecode and the countdown show the time of the MATERIAL — the source cannot see the timeline's clock
  by construction (`src/libs/kt-media/src/generator.rs`); a hint in the form
  says this directly.
- The 3×5 pixel font and `Canvas` — common `src/libs/kt-media/src/pixfont.rs`
  (placeholder, timecode, countdown); this is NOT the rasterization of user text.
- Everything that shows the frame of a generator event (filmstrip, bar, picker gallery)
  asks `clip_source_addr`/`gen_source_addr`
  (`src/libs/kt-engine/src/video.rs`); `c.media` knows only the kind — by it
  the title is empty. The list of sources for export/kt-cli — `gen_sources` there.
- Sync is two snapshots (`src/apps/kt-app/src/app.rs`): `sync_media_sources`
  (the set of media) and `sync_gen_sources` (values by event). A new path of adding media EITHER pre-registers the snapshot and raises the source itself, OR
  relies on the sync — not both (a double worker).
- Changing the kind — `SetMediaPath`; the model re-dresses the events (`fit_gen_to_media`,
  `src/libs/kt-model/src/model.rs`); this is a separate step of history.
- The colours of the title are direct fields, not `ParamSpec`; the common `color_row` via `get_param`
  returned zero and made the title transparent black. `color_row_rgba`
  (`src/apps/kt-app/src/widgets.rs`) reads the fields directly; the components —
  `to_srgba_unmultiplied()`, because `egui::Color32` stores a premultiplied
  colour.
- Percentages — recalculated only in `widgets::param_value`
  (`src/apps/kt-app/src/widgets.rs`); storage, file and curves — a fraction.
- The label scope `ParamScope` (`src/libs/kt-model/src/project.rs`): `size` is the grain for Noise and the size of the type for the title; the global map of names was lying.
- The poster of a card for media without a duration is not taken from zero: in the countdown at zero not a single letter is typed; "no duration" and "not yet measured" are
  different states of the thumbnail service (`src/apps/kt-app/src/thumbs.rs`).
- The identity of a frame on the GPU is the serial number of the cache record
  (`src/libs/kt-cache/src/lib.rs`, `get_video_stamped`), not the address: editing the colour
  with a motionless cursor gave back the old texture (capability `video-plan`).
- A trap of the harness: the title's form is the tallest, the "+ curve" menu opens below the bottom edge of the harness window and does not get into the accessibility tree — the curves are probed
  in the compact view (`src/apps/kt-app/src/layout_tests.rs`, t666).
- Editing the dictionary of kinds — the dictionary + factory + form + picker + five catalogues
  `src/apps/kt-app/i18n/*.toml` in ONE commit; the menu group is a function of the kind
  (`GenGroup::of`, `src/libs/kt-model/src/genmedia.rs`), there is no second list.

## Deferred

- Arabic script/bidi for titles (the shaper is in place), word wrap by width, italics.
- Burn-in of the timecode at the delivery stage.
- The generator does not know the speed of the event — physics/expressions must reach
  it as numbers (capability `automation`).
- Colorimetry in export (the tables make the gap visible).
