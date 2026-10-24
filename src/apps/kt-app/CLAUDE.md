# kt-app (eframe/egui GUI)

> Crate contracts and mandatory tests — `openspec/specs/{ui-timeline,ui-reach,durability,telemetry}/spec.md`, the reasons for decisions — `design.md` next to them.

UI invariants earned through the rounds 4–5 (details and test numbers — items 38–94). Violating any of them is a performance or UX regression.

## Redraw and frame cost (items 38–40, 53, 57)

- **`animating` = "something is MOVING", not "something is lit".** Any new animated state must decay/converge and release the repaint in idle (250 ms). Lesson: `MeterState::is_active()` based on the condition "the bar is above the minimum" got stuck forever after the first sound → endless 60 fps while idle, the WHOLE system was heating up (t38, t20-repaint).
- **The key of the preview frame is the tick, ALIGNED to the fps grid** (`preview_key_tick`), never a raw one. A raw tick changed every UI frame → a full recomposite + re-upload of the texture to the GPU 60 times per second (up to ~500 MB/s at 1080p) — "visually no 30 fps" with "FPS=60" (t39).
- **No allocations/clones of structures in draw paths per frame.** Snapshots for a frame are `Arc` clones of the caches by revision (coverage/names/paths, model_rev / cache.epoch). The preview composition buffer is reused (`compose_preview_into`); the glow mip is rebuilt every 4th new frame.
- **Thousands of painter primitives = a mesh.** The waveform was 2 `line_segment` per PIXEL; the glow of the meters — ~310 tessellated ellipses per frame. Both are a single `Mesh` (t40).
- **`cache.epoch()` changes on EVERY insertion** — during playback this is a storm. Any UI cache invalidated by epoch must be throttled during playback (coverage bars — no more often than 4 Hz; otherwise 60 polls/s under the cache mutex, contention with the decoders).
- **Honest metrics before the fix**: V-fps (the NEW frames shown) and "V-skipped cells" in the performance panel. It is they that diagnosed the 24 fps quantum ceiling from the user's machine. "FPS" is the UI frames — it says NOTHING about the video.

## Preview key on pause (R.53 item 415, R.62 t722b)

- The preview composite is recomputed when ANY of the following changes: the key tick, the quality, the player's `content_version` (generator values, source substitution) and the player's `project_version` (any edit of the model). Without the last one, a change of the conform rule in the settings window on pause was written to the file, while the old frame hung on the screen — the live scenario `conform` found it, the harness was silent. A new input that changes the picture outside these four must bump one of the versions, rather than create a fifth flag.

## The clock of video display (items 55–56 — the main lesson of round 5)

- **The playhead moves in QUANTA of the audio callback** (PipeWire often has 2048 samples = 42.7 ms). Any consumer of the position for DISPLAY must go through `Player::video_playhead()` (the interpolation `display_extrapolation` + the smoothed latency), rather than read `transport.playhead()` during playback. Without this the video physically cannot show more than 1000/quantum frames per second — exactly 24 were observed, regardless of quality/backend/economy mode (t56).
- **The raw `output_latency` jitters between callbacks by ±a frame** — read `latency_smooth` (an EMA in the render thread), plus a monotonic clamp of the key during playback (a rollback ≤2 cells is ignored; loop-wrap/seek pass through).

## GPU/platform (items 39, 53; video FX — items 181-187)

- The wgpu default `PowerPreference::HighPerformance` picks the discrete card → on hybrid graphics the PRIME copy of every frame slows down the WHOLE system. The default is LowPower, `WGPU_POWER_PREF` overrides it; `WGPU_BACKEND=gl` — a bypass of Vulkan. The active adapter is shown in the performance panel ("GPU: …").
- The software renderer is detected by adapter info; heavy glow is disabled with it.
- **Video FX and composite — on the GPU by default (R.29)**: the kt-gpu engine is created on the first frame of `ui()` from the eframe device (and again when a project is opened — KtApp is recreated, sw_render is reset) and is injected via `player.set_video_gpu`. If it was not created → a warn-notice + CPU; DeviceLost → "poisoning" until the end of the session, a notice once (t185). With R.44 the CPU fallback is FULL (Pan3d also computes — item 308): the notice about the CPU is about the backend's speed, not about losing 3D.
  The performance panel line "FX: GPU (…)" / "FX: CPU" must be honest.
  Export (render_dialog) builds ITS OWN headless engine via `app::export_video_gpu()` — the UI queue does not share time with the 4K export.
  llvmpipe = "a GPU is present". The live scenario `gpufx` runs this path.

## File DnD (items 47, 52)

- **winit 0.30/X11 does NOT deliver the mouse position during file dragging** (the XdndPosition coordinates are discarded, the movement is not sent) — the position is read directly from the X server (x11rb `QueryPointer`, physical px → points via pixels_per_point). Wayland file-DnD is absent in winit → an honest fallback to the playhead.
- Per-frame states (`dnd_hover`) must not be erased by an unconditional `take()`: the position lives while the hover goes on, and is taken ONLY on drop (`dnd_keep_hover`, a regression in t47).

## Multi-viewport (item 51)

- The detached preview — `show_viewport_immediate` (deferred requires `Send+Sync`, which is incompatible with `&mut self`). One `egui::Context` for all viewports — `TextureHandle` works in another window.
- `ctx.input()` inside a viewport is the input of THAT viewport: the shortcuts are duplicated by calling handle_keys from the window; Esc of the preview window is scoped locally (it does not trigger the global Esc cascade).
- Do not draw foreground overlays with rects of another viewport in the main window.

## Transitions and range markers (R.24, items 139–142)

- A Move drag onto a neighbor is classified by the pure `plan_move` (timeline.rs): one partial neighbor → a crossfade batch on release; everything else — a clamp STILL IN THE PREVIEW + a hover hint. New obstacles must get into plan_move (foreign is already there).
- A fade at the edge (R.25) — the same transition with an empty side: it is drawn as ONE diagonal (a crossfade — X), the window edge is pulled as usual (the empty side is not clamped by the body), and the items "Fade in"/"Fade out" appear in the clip menu only at a FREE edge (there is no neighbor and no fade yet).
- BREAKING gestures (moving a clip, deletion, punch-out) go through `KtApp::apply_batch_restoring` — it appends the return of the handles of the dead crossfades in the same undo step (R.26). A new delete/move path added bypassing it will again start losing material.
- The handles are drawn ALWAYS: in the transition window, to the left of the joint — the continuation of the incoming clip, to the right — the outgoing one (`transition_handle_rects`).
- The edges of a transition — apply_coalesced(SetTransition); a roll of the joint — a batch of trims (the window moves with the joint by itself, do not touch the offsets). The reconcile of transitions in the model is executed ONCE per batch — do not call it from individual mutations.
- The M gesture: region + cursor inside → marked_range by the region / toggle; outside — a point (the user's wording, item 142). The range strip lives on the time caption row; the end is grabbed on the flag strip; snap/stop — both edges.

## Writing the project to disk (R.31–R.32)

- Periodic autosave runs IN THE BACKGROUND (serialization + writing in a separate thread): a synchronous write from `eframe::App::save` every 30 s dropped the FPS to zero (item 197). `App::save` is left cheap, the final write is in `on_exit`.
- **All edits of the model from the UI go through the wrappers of `KtApp`** — `apply`, `apply_batch`, `apply_batch_restoring`, `apply_coalesced` (item 200). A direct call `self.model.apply*` bypassing them does not move `model_rev`, and on it hang both the gate of the background autosave ("the revision is the same — the asterisk is cleared") and the frame caches (coverage, names, paths). The symptom of the defect: FX edits after taking a snapshot looked saved and were lost on exit. Live paths (drag, the window session) send a command ONLY when the value has really changed — otherwise the revision grows every frame of a gesture and needlessly resets the caches.
- **`on_region_changed` does NOT call `Player::set_project`** (R.43 item 296). The region and `loop_enabled` are a "convenience", the engine does not read them from the project at all: the loop arrives separately via `set_loop_region`. `set_project` is a clone of the whole project plus a rebuild of both renderers, that is, a stop of the audio render thread: on an edit with 1961 events every shift of the region boundary emptied the ring and gave callback underruns. The edits of the CONTENT are conveyed to the player (`apply`/`apply_batch`). The guard — t286.
- **All writes of the project go through `write_project_guarded`** (item 199): the background write compares the model revision with the last written one and cancels itself if a newer one is already on disk. Without this the asynchronous autosave overwrote a fresh Ctrl+S with its stale snapshot — edits were lost. A new write path must go through the same function.

## Synchronization with the player

- **Everything that is synchronized with the player "on change" must also be synchronized when a project is OPENED.** Lesson (item 54): the loop region was passed only from `on_region_changed` — a loaded file with a loop played linearly past the region.

## Timeline geometry

- Mapping of positions outside draw_timeline — only through `TlGeom` (the frame cache, display rows from `Project::rows()`); do not create formulas "offset(kind)+lane" outside TlGeom (t47/t50).
- **Rows of DIFFERENT height (R.35 item 1)**: the service rows (morph/camera) are three times lower than the clip lanes, the floor — `THIN_ROW_MIN` (a row of 5 px cannot be hit with the mouse). The rule is one — `row_height`; the tops of rows are prefix sums `row_offsets`, and they lie INSIDE `TlGeom` (a private field, filled by `new`) — they cannot diverge from `rows`.
- **Not a single division by the row height.** "How many rows were passed" — `row_at_y_unbounded`, "where is the row" — `row_top`/`row_center_y`, "the fraction within the clip lanes" — `lane_y(kind, lane, frac)`, the zoom anchor — `anchor_at`. The formula `(rel / lane_h).floor()` is wrong for any mixed heights, and this is exactly how the clip drag computed the row.
- `vertical_layout` takes TWO counters (clip rows and service rows): the weight of a service row is 1/3, therefore three morph tracks cost as much as one clip lane. The limits 30/84/16/120 are not to be touched — they were set at the user's request (R.6).
- `KT_GEOM_DUMP` gives each row its own `"h"`; `lane_h` at the root of the dump is the BASE height. Live scenarios read `row.get("h", g["lane_h"])`.

## Debug env

- `KT_AUTOPLAY=1` — play right after opening (performance measurements without input).
- `KT_VIDEO_GPU=0` — a forced CPU path of video FX (comparison/debugging);
  `=1` — require the GPU (a failure to create it is visible as a notice).
- `KT_SIMD=0` — a forced scalar in the CPU kernels (R.32): the AVX2 path is byte-for-byte equal to the scalar one, so the switch is visible only by speed.
- `KT_DETACH_PREVIEW=1` — start with a detached preview.
- `KT_GEOM_DUMP=<file>` — write the timeline geometry for live scenarios (R.27): the zone of tracks, zoom, row centers in window coordinates.
- `KT_FILE_PICK=<path>` — the answer of the system file-picker dialog (R.67): on Linux rfd goes through xdg-portal, which is absent in Xvfb, and silently returns "nothing"; the menu item in a live scenario is pressed for real, only the choice itself is substituted.
- `KT_METER_DEMO=1` — feed fictitious levels into the meters: the look of the meter (glow, zones, peak strip) cannot be checked without audio hardware, and "by eye from the code" it has already broken twice.
- `KT_DECORATIONS=1` — restore the window's WM decorations (a fallback for capricious WMs; by default the window is borderless, the toolbar = the titlebar, resizing — the `resize_zone` zones along the perimeter).
- **Shift+wheel in egui arrives as scroll.x** (smooth_scroll_delta) — the vertical gestures of the timeline read the raw `Event::MouseWheel` with modifiers (item 12 of R.6); the pan branch is gated by "without Shift".

## egui traps that cost rounds (R.16)

- **`ui.max_rect()` inside a `ScrollArea` is the VIEWPORT in the content's coordinates**, not the panel: when scrolling it moves together with the content. Decisions like "the cursor left the widget" must be made by `ui.clip_rect()`. Because of this the media bin counted a move to the bottom edge of the screen as "dragging out onto the timeline" and rolled back the positions — and the content physically could not become longer than the screen, so it "does not scroll even with the wheel" (item 118).
- **The wheel over a `ScrollArea` is muted during ANY drag** (`dragged_id().is_none()` in `scroll_area.rs`). Make a programmatic scroll via `ui.scroll_with_delta_animation(..., ScrollAnimation::none())` — it goes through `pass_state` and is not suppressed by this gate; with a motionless cursor in the auto-scroll zone a `request_repaint` is mandatory, otherwise the frames stop coming.
- **`key_pressed` turns ON the keyboard AUTOREPEAT.** For "holding" read the events: the first press — `Event::Key{repeat:false}`, the end — `key_released`. A held space previously sent a series of play/pause (item 120).
- **`lane_order_insert` puts the row before the next track of ITS kind**, and if there is none — at the end of the list. "Insert below the clicked row" after it requires an explicit permutation of the display row (item 122); with R.56 the number of a new track is the RANK of the row, and the order and the insertion go in ONE batch (one undo step).
- **Everything that shows a state must also show a TEMPORARY state.** A speed-up by space changed the real speed, while the speed widget showed "1×" — this was found only by a live check (item 120).
- **`#[cfg(test)]` in the middle of a UI file TRUNCATES the catalog scanners** (R.49). `font_tests::sources()` returns the file UP TO the first line `\n#[cfg(test)]` — so the test literals are thrown out of the scan. A side effect: an attribute placed in the middle of `app.rs` makes the whole second half of the file "invisible", and t125 declares two dozen live keys unused. Keep the test wrappers at the very end of the file or do not create them at all.

## Localization (R.19) — read BEFORE ANY edit of text

- **The source language of the codebase is English.** Any new string literal (UI, log, `#[error]`, `eprintln!`, panic) is written in English. The Russian interface text lives ONLY in `i18n/ru.toml`; comments and `docs/*.md` are in English too. The literal rule is fixed by test 131.
- **Not a single label outside the catalog.** `tr(k::section::CONSTANT)` for strings without arguments, `tf!(key, a, b)` — with arguments (placeholders are positional, `{0}`/`{1}`: Chinese and Arabic regularly need a different order). The constant's path is written IN FULL: test 125 searches for usage by it, and an abbreviated `use` would hide the key.
- **There is no plural form.** We write "noun: N", not "copied N clips": this is the user's rule from the wording "the Russian sample is shown as data: "clips copied: {}"", and it also removes the tables of plural forms for five languages.
- **The order of fonts in `install_fonts` must not change.** The Noto subsets go AFTER Ubuntu-Light and Phosphor: Latin and Cyrillic must stay behind Ubuntu-Light (Noto CJK contains them too), the PUA icons — behind Phosphor. The precision of the predicate "an atom of an Arabic letterform" in `i18n::bidi` holds on this order, and on it the whole order of Arabic text rests.
- **Arabic: egui 0.36 shapes the cursive script, but does NOT reorder.** The runs are cut by the font letterform (a space breaks the phrase), harfrust reverses the glyphs inside a run itself, and the runs are laid out in logical order. Therefore the WORDS are rearranged, and the characters inside a word remain logical; a per-character `reorder_line` makes it worse. The analysis — `src/i18n/bidi.rs` and item 126.
- **The contents of a `TextEdit` are never reordered** — only the catalog strings. Otherwise the cursor and the selection would diverge from what is shown.
- **The window title ≠ its id.** `Window::new(title)` derives the `Id` from the title, and the window position lives under it: every window must have an explicit `.id()`, otherwise changing the language will reset the position of the dialog (test 129).
- **Strings assembled in a background thread must not be translated** — the language lives in the TLS of the UI thread. DATA travels over the channel, and the string is concatenated at display time (this is how the render result summary is done).
- New text → `i18n/en.toml` + `ru.toml` + three translations; the rules for the translator are in `i18n/README.md`. New hieroglyphs → rebuild the subsets with `python3 scripts/subset-fonts.py` (otherwise t100 turns red, and correctly so).

## UI texts (item 17/21/25 of R.7 — the user's requirement)

- Do NOT make obvious labels. A value with a unit explains itself: "100 px/s", "1280×720", "30" in the FPS combo — without the prefixes "zoom:", "Resolution:", "FPS:". A state visible through the button highlight (Loop) or through the value itself must not be duplicated in text ("(loop)", "(stop)", "≠1×").
- All hints go through `KtApp::hint()` (a popup + the bottom right of the status bar), not a bare `on_hover_text`.
- The hint text itself is taken from the catalog (see the section "Localization").

## X11 StartDrag/BeginResize (item 28 of R.7)

- `_NET_WM_MOVERESIZE` takes the pointer away from the application: ButtonReleased does NOT arrive, egui holds `dragged` forever, and ALL the rest of the UI loses hovered (scroll/hovers are dead until the next click). After every `send_viewport_cmd(StartDrag|BeginResize)` `KtApp::release_pointer_after_wm_grab` is mandatory (ctx.stop_dragging + a reset of the pointer).
- The "mouse over the widget" gates for the wheel are `contains_pointer()` + `i.pointer.hover_pos()`, NOT `response.hover_pos()` (the latter is suppressed by someone else's active drag).

## The preview of an event computes with the SAME map as the render (R.37 item 3)

- The waveform and the filmstrip mapped a pixel to material LINEARLY and ignored even `reversed` — a slowed clip drew something other than what it plays. Now both go through `kt_engine::ClipWarp::material_off`: the formula lives in the engine, do not create your own in the UI (the same rule as for the timeline geometry).
- The filmstrip's step is chosen by the CONSUMED span of the source (`ClipWarp::used`), not by the length of the event: otherwise a slowed clip takes a step twice as coarse and degenerates into repeated buckets.
- The quantization of the thumbnail key to the ladder FROM ZERO OF THE FILE is untouched — the shared cache with the bin is intact (t62). Stretching does not generate a new decode (invariant No. 8): only the mapping "pixel → key" changes.
- **The warp map is in the cache by `model_rev`** (`warp_maps()`), next to `coverage_cache`/`names_cache`. Building the profile for every pixel column is an allocation in the draw path (invariant No. 12). The cache key is a PAIR (revision, number of clips): the model is replaced wholesale without a revision bump when a project is opened and in harness tests.

## The effects picker — the gallery of thumbnails (R.54, items 430-432)

- **`MenuConfig::close_behavior(CloseOnClickOutside)` on the picker is mandatory.** The egui menu default is `CloseOnClick`: the popup closes on ANY click inside, including a click on a thumbnail — with it the gallery is stillborn. A choice closes the popup with an explicit `ui.close()`. The guard — t547 (input inside the popup does not close it).
- The thumbnails are computed ONLY by `FxThumbService`, and the base frame is given to it by the caller as a ready image. Invariant No. 8 holds: drawing may only ENQUEUE, the dedicated thumbnail worker decodes. This service has no decoder of its own, no sidecar DB and no persistence (the expensive half of the preview is already stored as a JPEG in `thumbnails`; persistence exists in the neighboring service of reel previews — there nobody has frames in the DB, R.58 item 463).
- **A cell BELOW THE SCROLL FOLD is neither drawn nor computed** (`ui.is_rect_visible`). The list is laid out in full — the scrolling needs the full size — and without the guard a single opening of the gallery ordered 160 thumbnails with a dozen and a half visible. Worse: an invisible cell honestly published its rectangle below the popup, and the live scenario clicked on it PAST the popup, into the timeline.
- **The accessible name of a cell is compound** ("Blur Heavy", while the visible caption is short). The dictionary of variants is shared across the registry: "Soft" exists for six kinds — without the effect's name neither a screen reader nor the harness can tell them apart. The concatenation is INSIDE `widget_info` (the closure is called only when accessibility is enabled; in a normal launch this is zero allocations).
- **The base frame: the exact thumbnail → the media poster → the built-in sample.** There are no empty cells. A NESTED clip has no frame and cannot have one (the reel has no file of its own, item 395) — so the sample at once. For effects visible only through transparency (`DropShadow`), the sample is given with transparent margins — on an opaque frame the shadow is identical by construction; on a REAL frame of the event we do not substitute, there passthrough is honest.
- **The moment is frozen only during PLAYBACK.** On pause the strip follows the cursor (it is motionless — the picture is stable), during playback it stays at the last value: recomputing dozens of cells at every clock quantum is pointless, and a flickering strip is worse than a slightly stale one.
- **The mis-click guard of 800 ms watches the DECODE ORDERS, not the render** (the owner's request "try to do it a second later (in case it was a mis-click)"): the render from ready frames and the sample goes at once, it is cheap. The countdown starts from the opening of the FX window.
- **Busyness and redraw.** `set_busy(playing || export is running)` every frame; a busy worker sleeps 25 ms between renders (rule 12). The gate `request_repaint_after` hangs on "the work is ALIVE" (`pending()`), not on "the picker is open": an open menu has no right to keep 60 fps while idle (defect t38).
- **Editing the generator's parameters invalidates the cells too** — `FxThumbService::invalidate_media` stands next to `ThumbService::retune_media`: the cell key remembers `ThumbKey`, and that does not depend on the parameters (the twin defect of item 412).
- **The cache of egui textures lives IN THE SERVICE, not in `KtApp`** — and this is not a matter of taste: the FX and morpher windows hold `&self.model` inside the ui closures (the morpher also calls its methods via `&self`), and a `&mut` to an application field does not fit there. `RefCell`, not `Mutex`: only the UI thread touches the textures.
- **The strength axis (item 433): a drag over a cell leads a blend of the neighboring points of the row** `[template, variant₀, …]`, the release adds what is shown, and a click WITHOUT movement is still exactly the named variant (t254). The count is from the CENTER of the cell by the absolute pointer position, not by accumulating `drag_delta`; on the release frame `dragged()` is already false, so `drag_stopped()` is read too. The axis is QUANTIZED (8 steps per interval): each position is its own recipe and its own render. "The main parameter" for a kind is deliberately NOT declared — two curated points are blended, and the useful range need not be invented (the analysis — item 433).
- **Categories are grouped AT DRAW TIME** (a stable sort by first appearance): the registry is sorted by meaning, and the heading "Stylization" would otherwise be printed five times. The order of the registry inside a category stays intact.

## Reel preview — frames and peaks by offline render (R.56, item 452)

"A reel has no filmstrip and cannot have one" was an incorrect record of a correct observation: a THUMBNAIL BY FILE is impossible (the reel has no file, and it must not be given a path — the watchdog would paint it offline, the index service would go at it with ffmpeg). The content, however, is rendered offline by the same composition code as everything else.

* **The `reel_preview` service** — the shape of a SERVICE (a queue, dedup, a negative cache, one job at a time), the key — `(reel, content_hash)`. **With R.58 (item 463) it has a DISK TIER** in the same sidecar — the `reel_*` tables keyed by (reel, revision, media file fingerprint). The earlier note "it is not put into the sidecar: its schema is keyed by the identity of the FILE" is WITHDRAWN: the identity of the file is what `ensure_media` is keyed by, not the schema — there are no foreign keys in it, and the media fingerprint we carry ourselves, because `content_hash` sees the edit but not a substitution of the file under it. DB reads and writes happen in the thread of the TASK; drawing does not touch sqlite at all.
* **The request is placed BEFORE the draw loop and by key** — therefore scrolling and zooming do not generate orders (invariant 8), and a repeated request is removed by dedup.
* **Pausing is mandatory:** the offline render decodes files, and during playback it would take the CPU away from real time (invariant 6).
* **The frames go out BEFORE the peaks:** the audio of the reel is computed for longer, and until then the waveform is honestly empty. The mini-map remained a placeholder.
* **The moment of the frame — the same map "event tick → material"** as for a file (t549): a slowed or mirrored instance is paged through correctly, and a second formula for this was not created.
* **`VideoRenderer`/`AudioRenderer` in kt-app are created only by the render dialog and this service** — the guard t602.

## Thumbnails and the sidecar DB (item 1/14 of R.7)

- **The thumbnail — in the proportions of the KIND of the source (R.62, t719).** The order is `thumbs::thumb_quality(info.view_size())`, not the constant 320×180: the decoder returns a frame of exactly the requested quality, and the constant gave a squashed vertical clip. The filmstrip cell centers a narrow thumbnail (`thumb_cell_fit`), the margins are the body of the event. Sidecar: a row `thumb_meta` (media → size) — without it the thumbnails of the media are considered old and are rebuilt; do NOT bump `SCHEMA_VERSION` for this (it would wipe the index and the peaks of all media).

- The keys of `thumbnails` are an exact PK (media, t): a "nearest" frame only through `MediaIndexDb::thumbnail_near` (the bin scrub reuses the filmstrip thumbnails). Do NOT extend `ThumbKey` (the shared cache bin↔timeline, test 62) — the near tolerance is passed as a queue parameter.
- **The translation "event tick → source frame" is ONE function** (`thumbs::material_at` / `material_key_t`, R.54): it is asked by the filmstrip, the waveform and the picker's thumbnails, therefore the keys are shared and new decodes are not generated by display. R.37 item 3 moved the stretch into `ClipWarp::material_off`, but left a hole: a REVERSED event without a warp has no map at all (`clip_warp` returns `None` — a reverse is not a warp), and the earlier `map_or(tl, …)` took the material directly. The first column of a reversed event showed the first frame of the piece instead of the last (t549, red on the old formula).
- Everything that may "fail" in the worker (the duration, the decode of a key) must publish a FAILURE (durations=None / the failed set) — otherwise the UI re-orders on every frame, and the FFmpeg-open counter grows in proportion to the interface's FPS.

## Panels must not swell from content (regression of R.7.1 item 3)

- `Panel::left/right` in this fork gives the panels a width ACCORDING TO THE CONTENT if the content requires more: `ScrollArea::auto_shrink([false, ..])`, `ui.set_min_width(available)`, a `Grid` with `TextWrapMode::Extend` and fixed `set_width` inside — all of them press on the panel. Symptom: the left column spreads out and the central area (the preview) collapses to nothing — "the preview vanished somewhere".
- Rules: by width — `auto_shrink([true, _])`; multi-column metrics — `ui.columns` (divides the available width) + `Truncate`, NOT a `Grid` with Extend; the bin's cards — width `available.clamp(min, PREVIEW_W)`; the safety net — `Panel::max_size`.

## UI/UX is made FOR THE USER (the requirement of item 17 of R.8, tightened by item 11 of R.11 — read first)

The rule appeared after three rounds in which the interface "compiled and drew", but could not be used, and it was rewritten as a checklist after the user sent comments about the same approaches for the FIFTH time. If you write UI code and have not run through this list — the item is not closed, whatever a green `cargo test` says.

### Checklist before closing ANY UI item

**Part of this list now BREAKS THE BUILD instead of asking.** R.38 showed that prose does not work: three complaints of that round violated rules 1, 7 and 17 VERBATIM, and right next to them stood the note "a repeated comment = the rule was not written down". The rules were written down. Therefore `apps/kt-app/src/reach_tests.rs` appeared:

- **t252** — there are no orphaned commands. An implemented command that no line of the application creates is a feature unavailable to the user (this is how the named areas of R.35 were lost: four commands, unit tests green, nothing to delete the area with).
- **t253** — every entity of the timeline has a full set of verbs. The table is exhaustive over `TlEntity`, and `TlEntity::ALL` has a fixed length: **a new entity will not build** until it is stated how it is renamed and deleted. "There is no verb" is accepted only with a reason, "it is elsewhere" — only with a reference to a test.
- **t254** — a click reaches its target: pressing and releasing in the center WITHOUT movement must give an action. `dnd_drag_source` is forbidden throughout the application.
- **t255** — a long name is truncated, and does not blow up the strip.

An item of the checklist without the mark "closed by test N" is a request for a new test, not permission to check by eye.

1. **Did you hit with the mouse the first time?** *(partially closed by t254: the click reaches the target; the width of the grab zone and "is it convenient" — in prose.)* Perform the action as a person: hover, press, drag, release, press again. If one has to aim — a defect, even when "the function works". The grab zone can and should be wider than the drawing: the window edges 8 px, the corners 16 px, the separators `resize_grab_radius_side = 8`, the fader wider than the widget. The visuals do not change in the process.
2. **Does it work DURING PLAYBACK?** The gate "only when paused" is set only if without it an invariant of the engine breaks, and not "to avoid thinking". The player can seek on the fly; the input window, search, markers, navigation must work during playback.
3. **Does nothing jump?** *(partially closed by t255: a long name does not grow the strip.)* The width/height/layout does not change when switching a mode, opening an input field, changing the text or the number. Fix the width by the gallery of the longest variant, not by an "approximate" number.
4. **The default is the useful one.** The useful mode is on by default, and a modifier TURNS IT OFF (a magnet, not "hold to snap"). The user must not have to configure the program in order for it to behave normally.
5. **An action does not extinguish a neighboring state.** Removing a region does not turn off the loop, the autosave does not clear the asterisk, a change of mode does not lose the layout. Side effects "for company" are a defect.
6. **There is a limit.** Everything stretched by the mouse has a min AND a max (the meter panel, the column width, the zoom). "Stretched it to half the screen" is about a missing `max_size`.
7. **The text fits.** *(closed by t255; the truncation helper is one for all places — `widgets::elided_name`.)* A long name does not stick out of its zone and does not overlap someone else's: measure `layout_no_wrap` + `galley.size()` and decide (truncate with an ellipsis / a label with a backing / wrap), rather than hope for "usually short".
8. **The widget is fitted into the program, not "put in".** A standard `TextEdit`/`Slider`/`ComboBox` in the middle of a branded toolbar is sloppy work — the input is drawn in the same rectangle and with the same font as the value it replaces. The user's wording: "you must not just slap an input field in there".
9. **Nothing superfluous appeared.** No checkboxes "out of place" (modes — as icons, as in file managers), no obvious labels ("zoom:", "FPS:"), no textual duplication of what is visible through the highlight of a button.
10. **The hint is exactly one and in one place** (item 14 of R.8): the bottom right of the status bar through `KtApp::hint()`, the priority hover → mode → message → the standby line. There are no pop-up popups or instruction overlays in the program.
11. **Non-printable characters are checked against the font** — see the separate section below. A square placeholder for the user = you did not check.
12. **Background work does not get in the way.** Autosave / re-ordering of thumbnails / invalidation of caches are not done during a gesture or playback and do not jerk the interface more often than a human needs.
13. **DO NOT WASTE SPACE (the requirement of item 4 of R.12).** Before creating a new strip, row or column, you must check whether this can be placed in an ALREADY occupied spot: on the same row in a different color, in a gap of a line, over the background, to the right of existing text, in a field that is already empty. A strip across the whole width of the screen for the sake of a caption that appears rarely is a defect, even if "it is neater that way". The references: the name of a marker is written on the time caption row (not in its own strip — item 3 of R.12), the name of a bin group — in a gap of the top frame line, the long name of a track — as a label over the start of the track. The user's screen is more valuable than your convenience in layout.
14. **A feature must work AGAIN, not just once.** A classic: Ctrl+L opened the timecode mask, but after Enter the widget stayed focused in egui, `egui_wants_keyboard_input()` stayed true — and ALL the hotkeys died until the user clicked the mouse elsewhere (item 8 of R.12). Everything that takes focus must GIVE IT BACK when closed (`surrender_focus`), and the test presses twice in a row.
15. **The window edges do not steal clicks.** Everything that lives within 16 px of the window edge (the window buttons, the menu, the bell) must get into the list of "zones without resizing", otherwise the press goes to the window manager and the button is dead (item 9 of R.12).

### A test asserts the CONSEQUENCE and probes FROM WHERE THE PERSON ACTS (R.40)

The rules above were written down — and did not work: the R.38 mechanism turned out green on a program where three of its own items did not work. The analysis — item 280, the complaints verbatim — `docs/user-feedback.md`, R.40.

- **"The field got set" and "the item is in the menu" are not consequences.** t262 asserted "`lane_heights` got a value, `row_h` changed" — true, but the event was drawn at the base height just as before, and was drawn that way. What must be asserted is what the person sees and does: the rectangle grew, a click hit it, the block moved, the frame caught it.
- **The point from which the test acts is part of the assertion.** t253 pressed RMB at `rect.left() − 30`, that is, in the HEADER of a row 34 px wide, where I had put the menu item myself. A person presses on the track and got the menu of empty space. Therefore `scene()` in `reach_tests.rs` returns a LIST of places, and every entity is probed from EVERY place: a new place of action is added to this list, and not "checked separately later".
- **A check "green in the harness" means nothing without a live scenario.** Everything the user complained about in R.40 was green in `cargo test` and broken in the window. A live scenario per item is a condition for closing the item, not a wish.

### Rules won by the R.28 round (the user demanded that they be written down separately)

16. **An impossibility of an action is shown as an ERROR, not as a faded hint.** A gesture did not pass (a push against a neighbor, an occupied spot, a forbidden layout) — the user must see: WHAT exactly is in the way (the obstacle is highlighted), a hint in the error style (not grey hover text on a par with ordinary ones), and WHERE to look if the cause is outside the view. The wording: "I had not yet scrolled there myself — I did not understand what was there, that there is still an event at the end that gets in the way, and there is also a whitish, faded hint; everywhere in the hints such situations are made as errors". A grey hover means "this is what can be here", not "cannot be done".
17. **Paired ways of the same action behave the same.** *(partially closed by t253: the set of verbs of paired entities is checked by a machine.)* If an action is available both by a gesture and by a menu item (and a button) — all paths must take the context into account equally. The reference failure: the M gesture with a selected region makes a range marker, while the menu item "Marker here" makes a point one. Before closing the item, find ALL the ways to invoke this action and check each.
18. **The name of a menu item says WHAT and WHERE it will happen.** "Fade out" is not suitable: a fade can happen both at the start and at the end of an event. "Remove crossfade", which in fact removes the fade from the end, lies to the user.
19. **A dialog must say where the decision can be changed later.** Any prompt with a choice ends with a phrase saying in which menu/setting it is switched — otherwise the user answered once and got stuck.
20. **Nothing grows back or switches off by itself.** Editing changes exactly what the user was pulling: "as many as there were must remain, as many as were pulled out of the fade — that is how much the fade shrinks, where the paired event was — there it remains". Clever logic that "corrects" the neighbors on the user's behalf is a defect, not a convenience.
21. **A function with an INDUSTRY name keeps that name in the interface.** "Duck under a track" is a description of a place, not the name of a function: "what is under the track that needs to be attenuated?? is this Audio-Ducking? why didn't you name it normally?" (the owner, R.63). A person comes to us from Vegas/Premiere/Resolve and looks for a FAMILIAR word; the explanation goes in parentheses after it, not instead of it. The guard — t756c.
22. **A new window — only from the registry.** "I am putting a ban on any agents producing any additional windows without an obvious purpose (like a window on closing the program). I am tired of agents spewing these windows and not thinking" (the owner, R.63). Every window is recorded in `WINDOWS` (`reach_tests.rs`) together with the reason why it cannot be part of an existing place; the guard — t757. The place for a form is looked for in this order: the head of the chain strip → a section of an existing window → a panel → and only then a window.
23. **A tool remembers what was entered into it.** "Why on earth am I forced to remember each time which parameters I entered last time?" (the owner, R.63). The settings of a tool (not of the project) live in `user_store` and arrive at the next opening; the guard — t756/t756b. The same rule applies there: "and it gives curves": if a quantity has time, it must have automation through the same shared widget, not its own form.
24. **One widget — not a copy.** Before writing a new form, look for the same quantity in the program: ducking is two compressors (background and speech), not four new fields next to the existing compressor. "Meaningless copies of the same code pile up everywhere, when one could even visually show the user that ducking works in the program, for example, through two compressors" (the owner, R.63).

25. **An automation key is set ONLY by a human gesture.** A `DragValue`/`Slider` with limits CLAMPS the incoming value and marks the response `changed()` — a field with a live curve showing the value under the cursor wrote a key on every frame of the game when the zigzag went beyond the passport (R.67, key-recorders on the owner's project). The rule: show the value already clamped by the passport, write only on `dragged()` / focus+`changed()` / reset; the guard — t786.
26. **A new UI scenario has a `PATH` line, a live scenario has a screenshot per step.** What the person sees is part of the contract (`- **PATH**` in the spec, the guard t799), and part of the report (`kt_live.step(name)` → `target/live-out/<scenario>/NN-name.png`, which `finish.sh` lists). "HOW MANY TIMES MUST I REPEAT SO THAT ALL THE AGENTS THINK ABOUT THE USER" (the owner, R.67) — seven rounds of prose did not work. The run is on OUR fixtures, not on his projects.
27. **A useful control is on by default and is remembered.** The geometry frame in the preview started `Off` and was not remembered — for a person it did not exist (R.67). The default is in `user_store` with the useful value; switching it off is remembered.

### How to read user remarks

- **The history of his requirements — `docs/user-feedback.md`** (primary source: what he asked for, how he phrased it, what he already repeated). Read it before working, append to it right after receiving a new list.

- **"A class of problems" ≠ "shrink everything in a row" (item 4 of R.13).** The complaint "the meters panel stretches over half the screen" did NOT mean "shrink the neighbouring panels too". The limit of the left column was cut "by analogy" — and it broke the user's work with the media bin for the THIRD time in a row. A remark may be generalised to uniform DEFECTS, but not to settings nobody asked about.
- **Before changing a numeric limit — `git log -S '<place>'`.** If the limit was already moved AT THE USER'S REQUEST, there is a comment next to it saying "set per item N of round M" — such a limit is not touched without a new request. If you set a limit yourself, leave the same kind of comment with the reason.
- **When changing the appearance, do not reorder layers.** The drawing order (backing → glow → elements → highlight) is part of the look, not an implementation detail: moving the backing on top of the glow killed the "real" look of the meters for two rounds. If a layer gets in the way, the SOURCE must be fixed (for example the tail of the gradient), not an opaque slab placed on top.
- **Symptom ≠ cause.** "The dimmed part of the scale looks lit" turned out to be not a partial fill of the segments but a soft glow tail 22 px above the level. The diagnosis without reading the code cost two rounds and a spoiled look.
- **A remark is a CLASS of problems, not one case.** It gives one or two examples ("the diagnostics icon does not fit the theme", "the track name sticks out"), and the program usually has more such places. Having gone over one icon — check the WHOLE set (this is how two wrong glyphs were found at once); having fixed one hint — clean up all the popups; having fixed the width of one panel — check the others.
- **A repeated remark = the rule was not written down.** If the user writes about the same thing a second time — first record the rule here, then fix the code.
- **If unclear — ask RIGHT AWAY**, before implementation, and do not pass a guess off as a decision. One question is cheaper than a round of rework.

### Content edits ≠ convenience edits (item 5 of R.11)

- "Content" — clips, FX, morpher, marks, tracks, media: only they raise `dirty_content`, only they put an asterisk in the title, and only they trigger autosave.
- "Convenience" — region, loop, zoom, scroll, cursor, bin layout, track names: they raise `dirty_ui`, are saved incidentally with any project write and on exit, but they NEVER pretend that the user changed something.
- **THE ORDER OF ROWS moved from "convenience" to "content" (R.35).** A morpher layer acts on "everything BELOW it", so the arrangement of tracks changes the picture and MUST go through undo. The order lives in `Project.lane_order`, is read through `project().rows()`, and is edited by the `SetLaneOrder` command; dragging a row raises `dirty_content` and rebuilds the render. The order no longer has its own mirror in `KtApp`.
- Do not introduce a new "something changed" flag: any new state must land in exactly one of the two buckets.

## Layout testing rule (requirement of item 9 of R.8)

- Any change to layout or interactive behaviour is accompanied by a harness test in `apps/kt-app/src/layout_tests.rs` (egui_kittest runs real frames without a window or GPU): panel width, dragging the separator, the cursor on the edge, sending `BeginResize`, button clickability, "the preview did not collapse".
- **In the harness, the button release is sent as an EXPLICIT event.** `Harness::drop_at` puts into the queue BOTH ButtonRelease AND PointerGone, and `step()` runs one frame FOR EACH event — `output()` after it shows the frame already without the command. When checking a `ViewportCommand` from a click (Close/Minimized), send `Event::PointerButton{pressed:false}` by hand and collect the commands after THAT frame (t101).
- **The harness is NOT proof that the feature works for the user** (lesson of item 7 of R.11). The meters panel width limit was green in t97, and yet "the panel stretches over half the window" in the live window: the user was launching a DIFFERENT binary (`target/debug/kinetren` — since 2026-10-22 this is what the kt-app package's binary is called — it is not rebuilt by `cargo test`, which builds a separate test binary). Before saying "done", build what the user launches and check by hand or by script.
- **Live scenarios — `scripts/live-check.sh`** (R.27, item 161): Xvfb + openbox +
  the real `target/debug/kinetren` in software rendering; the scenarios live in
  `scripts/live/*.py`, they check themselves (they read the .otio file the program saved and the screen pixels), artifacts go to `target/live-out/`. Coordinates are taken
  from the PROGRAM ITSELF (`KT_GEOM_DUMP=<file>` — a dump of the timeline geometry), and are not guessed. `finish.sh` runs them itself if Xvfb is installed.
- **Modal windows are probed through the `fields` of the same dump** (R.49). A button
  inside `egui::Modal` is not found in the accessibility tree, and its offset inside
  the window must not be guessed: the program reports the rectangles itself
  (`widgets::field_probe::note("ask.*", rect)`), and the scenario presses
  `app.click_probe("ask.restore")`. The same dump carries the `clips` of the LIVE model —
  a scenario that checks the guard of unsaved work must not save: Ctrl+S would test the save, not the guard.
- **`KT_AUTOSAVE_SECS` speeds up background writing** for scenarios (default
  30 s). The timer must not be faked from inside — that would no longer be the same program
  the user launches.
- **The window's close cross — `app.close_window()`** (WM_DELETE_WINDOW): exactly this path is intercepted by the guard of unsaved work, and `CancelClose` in eframe 0.36
  works for the ROOT window.
- **Double click in a live scenario: the gap between clicks is SMALL** (item 232).
  egui takes the click time from the FRAME in which it landed; in software rendering a frame
  takes ~0.2 s, so a "generous" pause of 0.1 s becomes 0.3+ s for egui
  and the double click falls apart into two single ones. Measured: 0.04 s — 5 of 5,
  0.10 s — 0 of 5.
- **A failed scenario must kill kt-app** (item 232): an exception from
  `Live.__enter__` does not call `__exit__`, and a forgotten program keeps running at hundreds of percent CPU, turning software rendering into a slideshow. The symptom —
  "suddenly ALL scenarios turned red at once": before looking for a regression in the
  code, look at `ps` and `uptime`.
- **The playback tempo is measured on the RELEASE build, and one must sample more often than the length of the region** (R.43, item 299). Two traps, both caught on the user's project: a probe once per second on a region of 0.89 s gives progress MODULO the region length (1.0 − 0.891 = 0.109, that is "0.13×" at an honest
  1.0×); and the debug build cannot keep up resampling three simultaneous chains
  "reverse + slowed" and honestly runs at 0.34× where the release runs at 1.00×. Debug
  is good for answering "running/stalled", but not "at what tempo".
- **A scenario that measures PLAYBACK must first prove that the media has resolved** (R.43, item 299). An unavailable source gets a placeholder (item 209) — this is standard behaviour, so the program silently opens, plays and wraps the region, looking entirely healthy. A copy of the project in a temporary directory loses the relative paths `sources/…`, and a whole run of "all sixteen regions play exactly" turned out to be a measurement of decoding PLACEHOLDERS. Put the copy next to the media (or a symlink `sources` next to the
  copy) and check the thumbnails in the bin.
- **The screenshot of a live scenario is taken with `xwd`, not `import -window root`**
  (item 227): ImageMagick on the GL window of eframe returns a BLACK frame (a 32-bit visual against the root window of depth 24), and the scenario compares black with black, printing "ok". The harness also waits for the FIRST drawn frame (the brightness spread has stopped being zero), instead of sleeping a fixed 2 s.
- **A gesture in a live scenario is released WITH A PAUSE** (item 216): between the last pointer movement and the button release — 0.35 s, otherwise in software rendering the application does not manage to process the last position and the gesture runs PARTIALLY. This is exactly what for years looked like "live scenarios are flaky".
- **The pixel after a gesture is read with `Live.pixel_settled`, not `pixel`** (R.37).
  The cursor moves before the decoded frame arrives, and the preview normally HOLDS the last frame — a bare read returns the picture of the PREVIOUS
  position. Waiting "until two reads match" is not enough: it will settle on the stale value.
  One must wait for a CHANGE (`differs_from`), and black must not be counted as a frame at that moment —
  this means "the composite is not yet complete".
- **`x_at` handles only what is ON SCREEN** (R.37). The view follows the cursor, and a moment that was visible at the start of the scenario drifts past the edge by the middle: `x_at` returns a negative x, the click goes past the window, and the view check "cursor > X" passes by itself, which hides it. The target moment must first be brought into view (Home) and one must CONVINCE oneself via `view_start` from the geometry dump. This is exactly how the `loop` step "checked" a stale pixel for three rounds.
- **Manual live checking — `scripts/ui-probe.py`** (python3-xlib + XTest): finds the window, moves the real mouse, presses keys, takes a screenshot, prints panel boundaries by a row of pixels. This is how the meters limit (90 px), the left column limit (420 px) and the timecode mask were verified. The drag there is done in three steps — see the next item.
- A screenshot is a supplement, not proof: all three earlier breakages "looked" perfectly fine in the picture.
- Debugging "who ate the click" — `KT_DEBUG_HITS=1` (egui's built-in highlighting).
- **A drag in the harness is repeated IN THREE STEPS**: press → move by ~8 px → keep moving. egui recognises the drag as started only after a threshold, and the code reads the pointer position IN THAT frame: if the test jumps from the grab point straight to the target, the grab offset is computed already at the target and the object stays in place — the test is "green on broken code" or the reverse. A human behaves the same way, so this is not a crutch of the test.
- **The test MUST check the CLICK, not the hover.** `t76_top_area_gives_space_to_preview` hovered EXACTLY over the centre of the "Close" button and was green all this time: hovering is not disturbed by the resize zone — only the press dies (the zone sends `BeginResize` on `primary_pressed` and takes the pointer away from the WM). A check that "the hint appeared" says nothing about whether the button works; press with `drag_at` + `drop_at` and check the CONSEQUENCE (a window command, a change of state).
- **Check at the WORKING resolution, not the smallest one.** A batch of export frames fit in the cache at 320×180 and did not fit at 640×360+ — a "live check" at a small resolution missed a black render (item 1 of R.15). The same goes for duration, number of tracks and frame rate: take a typical working case, not the minimal one.
- **Video tests must run with the project's fps ≠ the source's fps and rate ≠ 1** (item 14 of R.14). Matching grids degenerate the frame selection (floor and ceil give the same frame), and unit speed degenerates the playback clock — both errors lived for years precisely because all fixtures were 30 fps and all simulations ran at 1×.
- **A test MUST fail on the old code.** Before counting a regression test as written, temporarily restore the old formula and make sure it turns red (for t95 the old code gave 93910 instead of 90000). A test that is green both before and after checks nothing.
- The panel in egui 0.36 grows to fit its content: its ACTUAL edge (`PanelState::outer_rect`) may not coincide with the requested width, and then the separator "drifts" away from the border. The test MUST take `outer_rect`, and the content must not push the panel (see the section "Panels must not be inflated by content").

## Panel order decides (item 1 of R.9 — cost three rounds)

- Anything that is FILLED in during the frame (`hover_hint`) cannot be read in a panel that was drawn before its sources. The status bar is the second panel of the frame, and the buttons are drawn later: the hints were not visible at all, except for the bell (which sits inside the status bar itself). Such lines are drawn at the END of the frame in a separate layer (`Area(Foreground).interactable(false)` — otherwise the layer steals the window drag).
- The test MUST check what was DRAWN (`status_line_shown`), not an internal field: `hover_hint` is already filled after the frame, and the test turns green with a dead feature.
- The root `Ui` from eframe is framed by a default 8 px — because of it the whole interface did not reach the edges of the window ("the gap to the right of the meters"). We draw on the full `ctx.content_rect()`.

## Numerical stability of UI animations (item 7 of R.9 — the "painful to the ears" defect)

- `stable_dt` is NOT equal to 1/60: after idling it arrives equal to the idle-redraw delay (up to 250 ms, clamped to 0.1 s), and the economy mode gives 33 ms. Explicit integration schemes diverge on such dt: the fader animation flew off to 400 dB in three frames.
- To approach a target, use an analytic solution (exponential) or exponential smoothing — they do not diverge at any dt; the test MUST run BAD dt values (0.1, 1/30, alternation), not only 1/60.
- Everything that goes into sound is clamped IN THE ENGINE (`set_master_gain` — ceiling +24 dB, suppression of NaN/inf): the engine must not depend on the correctness of the UI.

## Icon and taskbar (item 6 of R.9)

- `with_icon` is mandatory, otherwise the window has no icon at all. On X11 winit does NOT set app_id — WM_CLASS equals the binary name (`kt-app`), therefore `StartupWMClass` in `.desktop` must match exactly that, otherwise the taskbar does not link the window to the icon. `with_app_id` is needed for Wayland.

## Time units: the playhead lives in AUDIO FRAMES (item 7 of R.10)

- `Transport` keeps the position in 48 kHz frames, and the model in 90 kHz ticks. The round-trip "tick → frame → tick" **truncates downwards** and is exact only on ticks that are multiples of 15. Any navigation of the kind "strictly to the right of the current position" MUST work on aligned values (`Tick::align_audio_up`), otherwise the cursor after a seek ends up one tick to the left of the target and the button silently sticks at one point — exactly how "Forward" did not work while "Back" worked (the downward truncation helped it).
- "End" for navigation is not only the end of the material: a marker or a region boundary may lie further right, and then pushing against the end drags the cursor back.

## Non-printing characters must be in the fonts (item 6 of R.10, rule and test — item 12 of R.11)

- The program loads EXACTLY six fonts (`install_fonts`, list — `icons::VENDORED`): Ubuntu-Light, NotoEmoji, emoji-icon-font (the egui default), the vendored Phosphor, and two Noto subsets (Simplified Chinese, Arabic; built by `scripts/subset-fonts.py` from the characters of the catalogues, 75 and 61 KB instead of 19 MB). What is missing from them, egui draws as a square placeholder, and the user sees it. This is how `●` U+25CF (the change indicator), `≡` U+2261 (the menu button of the narrow toolbar) and `✓` U+2713 (the checkmark in the "Reverse" item) got into the interface.
- **Rule: any ICON outside ASCII/Cyrillic is taken from Phosphor** (`icons.rs`, PUA codepoints) or drawn as a vector (like the window buttons). Do not substitute a "similar Unicode symbol" — check first. There is only one exception and it is not about icons: the writing systems of the translation catalogues (ideographs, Arabic script) come from the Noto subsets, and `scripts/subset-fonts.py` adds them there, not a hand.
- **Test 100 (`font_tests.rs`) checks this automatically**: it scans the kt-app sources RECURSIVELY (since R.19 part of the code lives in `src/i18n/`) and the `i18n/*.toml` catalogues, extracts the contents of string and character literals (expanding the `\u{...}` in which the icons are written), and requires a glyph for every non-ASCII character. Test code is excluded (the text of assertions is not seen by the user) — a test module in a file MUST begin with a line `#[cfg(test)]` at ZERO indentation, otherwise the scanner will not cut it out.
- The codepoint of a new Phosphor glyph is looked up through the GSUB ligature table (there are no post names in the font):
  ```python
  from fontTools.ttLib import TTFont
  f = TTFont('apps/kt-app/assets/Phosphor.ttf'); cmap = f.getBestCmap()
  g2c = {}; [g2c.setdefault(g, cp) for cp, g in cmap.items()]
  for lk in f['GSUB'].table.LookupList.Lookup:
      for st in lk.SubTable:
          for first, ligs in getattr(st, 'ligatures', {}).items():
              for l in ligs: print(first + ''.join(l.Component), hex(g2c.get(l.LigGlyph, 0)))
  ```
  Names in ligatures use `hyphen` instead of a hyphen (`lockhyphensimple` = `lock-simple`).

## Gestures on the event's top strip (R.28, item 12)

- `top_strip_h(lane_h)` (35% of the lane height, at most 12 px) — the Vegas-style gesture zone:
  the fade handle at each edge and the level line (video). Below the strip everything
  works as before: the body, trims, transition windows (test 157 holds this).
- **Corner fade** — a `TransitionEvent` with a ZERO external offset (entirely
  inside the event). Its handle is the corner of the clip, not the edge of the transition window: in
  the top strip `transition_at` skips such a fade. The handle snaps to the
  corner within 3 px, so a fade pushed in disappears entirely.
- **Level line** — ONE for both kinds (item 176): for video it is
  `ClipEvent.opacity`, for audio `gain_db` in the range of the v1 rail (±6.02 dB,
  0 dB — the midpoint); conversion — `clip_level`/`gain_to_level`/`level_to_gain`.
  The vertical volume rail is REMOVED: it stood inside the fade zone and made
  the fade on audio look unlike the fade on video. Live preview goes through
  `live_gain`/`live_opacity`, the write to history happens on release.
- Handles are drawn at the SELECTED event: a gesture that is not visible
  does not exist.

## Effect windows and input fields (R.33, items 217–219)

- **A numeric effect parameter is drawn ONLY by `widgets::param_row` /
  `params_grid`**: the range, step and unit are taken from `ParamSpec`
  (kt-model) — one source of truth. A custom form with its own
  bounds is exactly the mismatch that made Gain −24…24 in one
  window and −48…24 in another. Parameter labels come from the `[fxp]` catalogue,
  not from literals in the code.
- **The chain strip is one for all places** (`widgets::chain_strip`): both the
  event and the morpher. It has the master checkbox, a checkbox and a cross on each
  slot, reordering with the MOUSE, and copy-paste. A new place with a chain MUST
  call it, not draw its own.
- **The morpher's strip is ONE and starts with the purple morph slot** (R.35
  item 3). Effect kinds in the patch are stored separately (`video_fx`/`audio_fx`),
  but this is not shown to the user: `merge_chains`/`split_chains`
  glue them into one strip and split them back by
  `FxInstance::class()` — the kind of a slot is a function of the effect itself, so the gluing is reversible without side tables.
- **The strip's head is `ChainEdit.head`, and the selection is `ChainSel`, not a number.**
  "Head = index 0" reliably gives an off-by-one on effects;
  the head cannot be dragged, it has no cross, and it is NOT in the effects registry
  (t48 turns red on an attempt to add it) — the picker shows it as a separate
  first line in its own colour, so that it can be found by search.
- The FX window drives the WHOLE link group: a pair has one window session, the chains are in
  folds, and "Cancel" rolls back all of them. Opening FX from the partner
  finds the existing window instead of creating a second one.
- Copy-paste of chains and envelopes goes through the model (`kt_model::ops`) and
  at the same time puts JSON on the system clipboard; keys are re-hung
  on the entry point of the target on insertion.
- **The paste-prefix window (R.35 item 4)** opens ONLY when the buffer
  holds something NAMED: otherwise the paste goes silently and all the earlier scenarios stay intact.
  The paste position in it is FROZEN (the cursor under the window does not move it), Enter
  pastes, Esc cancels (and is first in the Esc cascade), on close
  the focus is HANDED BACK — otherwise the hotkeys die until a click elsewhere (rule 14).
  Confirmation is given by ONE `paste_block` → ONE `apply_batch`.

## Generator — the FIRST CRUMB of the event window (R.52 item 404 → R.59 stage 2)

There are no separate generator and title windows any more. Their form lives in
`gen_ui.rs` and is drawn by `fx_editor` in the pinned head of the chain strip
(`widgets::HeadSlot`, `ChainSel::Head` — the same machinery as the source substitution of the morpher). The owner's requirement: to see the source next to
everything that is edited on the event, not in a separate window.

- **Values are ON the EVENT** (`Command::SetClipGen`), curves —
  `SetClipGenAuto`, "edit in all places at once" — a separate command
  `ApplyGenToAll` (its own history step, the label carries ×N). The generator's kind
  remained a property of the MEDIA: this is its address and its cache strip (the decision of R.53
  stands), therefore a change of kind is `SetMediaPath`, and the model itself re-dresses the events
  (`fit_gen_to_media`), and this is also its OWN history step (rule 20).
- **The generator's head has NO switch** (`HeadSlot.on == None`):
  a disabled source would leave the event with no picture at all, and a dead checkbox would violate rule 9. The morpher has a checkbox, and these are different things.
- **Title presets set VALUES AND CURVES.** The animation formulas live in
  `kt_model::text_anim_curves`/`text_anim_values` — these are also what read
  old files, and if the migration diverged from the preset, a project from the previous build
  would come out wrong. The active preset is determined by the INVERSE function over
  BOTH halves (`active_preset(kind, curves)`): by the values alone an empty title matches the "typewriter",
  and without the curves a new title would be declared as typed out.
- **Everything that shows the frame of a generator event asks
  `kt_engine::clip_source_addr`** — the filmstrip, the coverage strip, the base frame of the picker gallery. `c.media` knows only the KIND, and by it a title would be drawn empty.
  The thumbnail source of the event is registered at the address `gen_source_addr(clip)` by its full path with the values
  (`format_gen_path`): the thumbnail service needs a frame, not an address.
- **The sync of sources is TWO snapshots.** `media_paths` (the set of media) and
  `gen_states` (generator values per EVENT). The first is kept by
  `sync_media_sources`, the second by `sync_gen_sources`: structural edits of the plan touch
  and arrive through `set_project` → `sync_generators`, while the VALUES of the plan are not touched (law P) and arrive through `Player::update_gen`.
  Recreating a source remains for exactly one case — the kind's passport
  changed (`can_retune_gen` says "no").
- **Any edit of the model goes through `sync_media_sources()`** (called from
  all apply wrappers AND undo/redo): the diff of paths `document().media` against
  the snapshot `media_paths` → replace/add/remove on the player + thumbnails. A new path of
  adding media MUST EITHER pre-register the snapshot and raise the source itself (as `add_media_clip_at`/`create_generator` do), OR
  rely on the sync — but not both at once (a double worker).
- Generator card: the caption is the kind's name, zero duration is hidden,
  the thumbnail is real (a factory source, bypassing the DB). A kind with defaults may
  give an EMPTY frame (for a title the default is an empty string): then the card
  writes the kind's name right in the thumbnail (rule 16, `gen_ui::blank_default`).
  The kind is changed from the card's menu even without events on the timeline — the list of
  kinds is ONE for all entries (`gen_ui::generator_kind_menu`).
- **Frame fractions are shown as percentages** (`ParamUnit::Percent`), the conversion
  lives in one place — `widgets::param_value`; in the model, in the file and in
  the curves the value stays a fraction. **A double click on a label** resets the
  parameter to the spec's default — "restore as it was" must not require
  remembering a number.
- **Hints of the shared widgets** (`param_row_bound`, `color_row`) go
  through the sink `widgets::hint_sink`: inside `Window::show` there is no `&mut
  KtApp`, and the place of display must remain one (rule 10). The mechanism
  is the same as for `field_probe` — the widget puts it there, `KtApp` picks it up at the end of the
  frame.
- Presets live ABOVE the rest of the form: they are reached for first, and scrolling to them would be a mockery (rule 1).
- **Harness trap:** the title form is the tallest in the program, and its
  "+ curve" menu in the harness window opens already beyond the bottom edge —
  the popup does not get into the accessibility tree. Curves are a property of ANY
  generator, so they must be probed in the compact view (t666).

## Transition window (R.33, item 226)

- The transition kind is chosen in TWO equal ways (item 17 of R.28):
  the submenu "Transition with next/previous" in the event's context menu, and
  the properties window (a double click on the transition window on the lane, or the item
  "Transition properties…"). Both take the list from `TransitionKind::ALL`.
- The kind's parameters are drawn by THE SAME `widgets::params_grid` (through the trait
  `kt_model::HasParams`) — a transition has no form of its own and must not have one.
  Non-numeric things (direction, iris shape, colour) are handled by their own controls.
- The kind's name is written INSIDE the transition window on the lane; we do not create a separate lane for it — if it does not fit, a marker dot. The crossfade simply remains "X".

## Automation in the UI (R.33, item 221)

- The curves are drawn by ONE widget — `automation::draw` (graph ↔ table).
  No effect may have its own keyframe editor: this is exactly
  how slowed (graph) and pan3d (a table row per parameter) diverged.
- The graph's time axis is the event's MATERIAL; the ruler and label format are taken
  from the project timeline (`ruler_steps`/`format_ruler_time`), and keys snap
  to the frame grid.
- **The value scale is computed from the curve's keys, not from the spec's range**:
  over the full range (for speed 0.25…32) an ordinary curve degenerates into a straight line. During a drag the window is frozen (`AutoView.drag_win`),
  otherwise the scale "breathes" under the cursor.
- **Editing a parameter while a LIVE CURVE exists is `SetFxAuto`, not `SetFx` (R.43
  item 291).** The field edits exactly what it shows (the curve's key), so the command too must be a curve command. `param_row_bound`/`params_grid_bound`/
  `draw_video_fx_params` return `widgets::ParamsChanged { base, curve }`, and
  the carrier puts BOTH sources of the edit into the command — the field AND the graph. The reference on which this always worked is the morpher (`morpher_editor.rs`, one flag for both): there an edit of the curve drifted back into the patch by the same flag as the edit of the base. While the FX window had one flag for five people, the number typed into the field died together with the frame — "resets to the previous value".
- **A field test asserts the MODEL, not the field's readings.** t265 checked `bound_value`/`bound_apply` as pure functions and was green on a broken program. t281 types characters into the field of the live window and reads `fx_auto` from the model. `DragValue` has NO node in the accessibility tree — it cannot be found by its label: the coordinate is taken from the real layout (between the label of its own parameter and the label of the next one), and the parameter label occurs twice — in the grid of values and in the curve selection button below, so the UPPER one is needed.
- **Editing a time in the table sorts the keys only on release**
  (`AutoView.t_edit`): sorting on every character moves the row out from under the hand — a direct user complaint.
- Conversion "material ↔ timeline" goes only through `kt_model::clip_material_at` /
  `kt_model::clip_timeline_at`. They take REVERSE into account and nothing else: since R.37
  speed does NOT compress the automation axis (otherwise the axis of the `rate` curve would depend on the `rate` curve itself, and the right part of the graph would be unreachable — that is where the auto-key sat which the editor seeds itself). The pair
  `kt_engine::material_at`/`timeline_at_material` has been removed; still do not introduce your own formula in the UI.

## Event selection (R.33, items 202–205)

- `KtApp.selected` is a SET (`BTreeSet<ClipId>`), not a single event.
  Operations on the "selected" take `selected_with_groups()` (the selection plus
  link partners); operations that make no sense on a set (A/V magnet, breaking a link, moving to a track, trim to the cursor) require
  `require_one_selected()` and honestly complain instead of staying silent.
- The rubber band (`Drag::Rubber`) starts ONLY from an empty spot: Ctrl+drag
  always, a plain drag — in the "Selection tool" button mode. Ctrl pressed
  ON an EVENT still removes the move magnet (item 2 of R.11) — the two gestures share the press zone, not a modifier. The hit test is computed by the pure
  `rubber_hits` through `TlGeom`.
- Moving a batch: `DragState.extra` (the other selected items + their original
  starts). An empty `extra` is the former single drag with its full semantics
  (crossfade from an overlap, sticking to the lane). A batch moves only
  in time, the limit is the pure `plan_move_block`, the commit is a batch `MoveClip` from the
  ORIGINAL starts through `apply_batch_restoring`.
- A double click on an event = the FX window; Shift+double and the menu item = a region
  from the event (one body `region_from_event`). A double click on the lane of a range marker =
  a region from its boundaries + selection of its events.
- **The harness and double clicks**: a double click in egui is time-based (two clicks
  closer than 0.3 s), and the default egui_kittest step is 0.25 s — a "double click" test without `with_step_dt(0.02)` checks a SINGLE click. The click history is
  shared across the harness: a second such gesture in a row becomes a triple click,
  so each double click needs its own harness.

## Missing sources (R.33, items 206–209)

- An unavailable file gets `kt_media::placeholder::PlaceholderSource`
  (a frame with the file name, silence) and works like ordinary media: the project
  opens, the preview is complete, rendering goes on. The set of placeholders is
  `KtApp.placeholder_media`.
- `open_source_indexed` remains STRICT (an error is an error): a placeholder is harmful
  when media is added to the project and when source permissions are polled. A separate
  `open_source_or_placeholder` substitutes it.
- A file that has come back is reopened IN THE BACKGROUND and substituted through
  `Player::replace_media` (stop the worker → clear its cache → raise the
  new one → bump the project version, otherwise idle prefetch would not notice).

## Impossibility of an action (R.28, item 5)

`KtApp.blocked` — the obstacle of the current gesture: a red highlight on the timeline,
`HintKind::Error` in the status line (it overrides the hover), and if the obstacle lies outside the view — the side, the distance and a red arrow at the
edge of the row. It lives for exactly one frame: it is set by the drag handler and cleared at the
start of the next pass.

## User library and paired widgets (R.48)

- **The user's file ≠ the model.** The library of FX presets lives in
  `user_store` (the autosave directory, `KT_USER_STORE_DIR` for tests and live
  scenarios) and does NOT get into the project's undo: undo edits the editing, not the
  library. The delete dialog and hint say so (rule 19).
  Under `cfg!(test)` the storage works only in memory — otherwise a test that
  opened the picker would read the developer's file.
- **Chain insertion is one function for all** (`kt_model::ops::
  append_chain_block`): it is called by the strip's copy-paste and by preset insertion, for an event and for a morpher. The domains differ (material against block time),
  the rule is one — "input to input". `ChainEdit.auto` is NO LONGER an `Option`:
  a silent branch that drops curves is impossible by type (t343).
- **A paired widget writes to the same place as the field.** The frequency-response graph of the equaliser edits values through the same `CurveBind` (the base or the selected key)
  as the numeric field — rule 17 literally. Values are read once per
  frame, and a write goes only on a real change (otherwise the model's revision
  grows every frame of the gesture). The wheel is absorbed ONLY over its own point,
  otherwise the FX window would be scrolled by it.
- **A window that grows from input breaks the test's coordinates.** The live
  preview of a name adds a row — the buttons slide down: they must be probed
  AFTER the redraw (a relative of the "drag in three steps" rule).

## Coverage strips and the decode scale step (R.46)

`coverage_runs` asks `BurstCache::coverage` by the DECODE QUALITY
of the clip (`Player::video_decode_qualities`), not by the preview's Quality: since R.46
a clip with a zooming chain warms up at a larger size than Quality, and a query by Quality
would show a warmed clip as cold. Pieces of one clip (a morpher layer)
may have different steps — the union of ranges is taken
(`merge_runs`). In the performance panel the line "Decode ×N" is shown ONLY when
the step is > 1.

## Three kinds of project and one sender to the player (R.50 item 392, R.55 item 434)

The model has THREE kinds of project, and confusing them means either writing the wrong thing to disk or playing the wrong thing:

| | what it is | for whom |
|---|---|---|
| `model.project()` | **working view**: the contents of the current reel sit in place of the root | the whole interface — drawing, hit testing, gestures, snapping, selection, windows |
| `model.document()` | **document** in the true layout: the root as root, reels as reels | the file, export |
| `model.reel_document()` | **the current reel AS ROOT** | offline counting on what is at hand, and the PLAYER inside a reel |

**What the player plays is decided by `KtApp::player_project()`, and no one else.**
At the root it is the document, inside a reel it is the reel itself from its own zero.
The sender is also one: `push_project_to_player()`.

The rule is not cosmetic. Before R.55 `set_project` was called from eight places,
each with its own `model.document()`, and standing inside a composition a person saw
on the timeline its events, but HEARD the ROOT. Substituting `reel_document` at a single
entry is not enough: the first edit (`apply_batch`, undo, a live level drag)
would return the root to the player, and there is nothing to notice it by — the defect leaves no
visible trace, the sound is simply wrong. Two guards: **t456b** (the working view does not leave
the module) and **t554** (direct forms `set_project(model.document())` and
relatives are forbidden, and the decision point is exactly one).

A consequence to rely on: **the event that a person drags
always lies in the ROOT of what is playing.** A live level drag therefore does not look for
its target through the tree of reels — it did, while the player was playing the root.

What goes to the file and to export is still `document()`. The discrepancy is named
to the person (rule 16): "Render" pressed from inside a composition says that the whole project is being exported.

**Entering seats the cursor ON THE MATERIAL (item 447).** The translation `source_in + (t − start)`
is clamped to the content of the reel: if a person entered without moving the cursor, the
translated position would end up beyond the material, and the view would honestly drift into
emptiness. The end of the content is taken AFTER the swap (`project().clips`):
`sequence(c.media)` after `set_editing` returns the record that holds
the contents of the ROOT.

**A test trap that cost a defect (item 436).** In all scenes of R.50
an event standing at ZERO was collapsed: the instance had `start == 0` and
`source_in == 0`, and the cursor translation `source_in + (t − start)` degenerated into an identity. t456 was green even on code that did not translate the cursor at all.
A scene with an offset is mandatory, and the test itself MUST contain a guard of the scene
(`assert_ne!(want, outside)`) — otherwise the degeneration will return silently.

## Track rows: number = rank (R.56, item 449)

Reordering of rows is ONE command for any kind (`reorder_rows` →
`SetLaneOrder`), and the model renumbers the contents. There is no second branch any more
("moving clips" for video and "a purely visual shift" for everything
else): because of it a video row dragged next to a morph or camera
row changed its place on screen and did NOT change the composite layer.
Consequences to rely on:

* an empty row moves on a par with a full one (previously there was a refusal
  "there are no events on these tracks");
* row names and heights live OUTSIDE undo and travel with the same `LanePerm` as
  the contents — otherwise a label would stick to another track;
* edits of ROWS are incompatible with an expanded band: the application collapses
  it itself and says so (`collapse_bands_for_rows`), because
  the model rejects such commands while a band is open.

## Expanding a composition IN PLACE (R.55, item 445)

The DEFAULT mode. From the root one does NOT LEAVE: the whole program plays,
the cursor is in its axis, the neighbours and transitions stay in place. Full entry (`enter_nest`)
remains the second mode — there the reel itself plays from its own zero.

**The gesture rule (refined by R.56, item 451):** a quick gesture gives the DEFAULT
mode, a named menu item gives the named mode. A double click → expand
in place, a repeated double click → collapse. **Enter is a DEEPENING CASCADE**:
the first expands in place, the second on an already expanded one gives full entry (the owner's
decision). Esc leads outward, and it too is a cascade. The decision "what Enter does" lives in
`enter_pressed_on_nest` and nowhere else.

**The selection of an instance survives the expansion and collapse of ITS band**
(`after_bands_changed(keep)`): without this, the second gesture finds no target and silently
does nothing. The rule "the selection lives in exactly one reel" stands —
the instance is an event of the ROOT, the band's addresses were protected.

**The chord lives in the chord column** (`widgets::menu_item`), not in the catalogue
row, and is promised only where it really does that: "Enter again" appears on "Enter the reel" ONLY when the band is open.

**Esc cascade:** … → clear the region and the selection → **collapse the expansion** →
leave the reel (and leaving under R.58 restores the band of the abandoned context —
see the R.58 section below). Collapse deserves less attention than leaving, so it comes
earlier; but later than clearing the selection — one key does not do two things.

**Three things that are easy to break:**

1. **`model.clip()` searches the WORKING VIEW.** The event of an expanded band lies in
   the canon inside the reel; if the function searched the canon, every gesture on the band would silently
   do nothing (an `Option` without explanation). Guard t571.
2. **The veil of the tails is drawn AFTER the events**, on top of them: otherwise "inside the window" and
   "outside the window" would differ only by a frame, not by the filmstrip and the waveform.
3. **The selection lives in exactly one reel.** A mixed selection looks ordinary, and a
   batch over events of different reels will not pass. A click on another band replaces
   the selection and SAYS so (t572).

**The rows of a band are labelled with the REEL'S NAME** and its own track number:
the root's `lane_names` would lie about them. (The `lane_names` themselves live in
`Project.ui`, and `swap_reel` does NOT swap them — a separate known defect;
because of it the full entry shows the root names on the reel's tracks.)

**One band at a time** — the limit is stated explicitly and is lifted with one line
plus a test: four levels of expansion are unreadable.

**Warning ×N** — once per reel per session (`shared_warned`), at the
first edit within the expansion of a shared reel. On every edit it would be noise.

## A band inside a reel, bands across contexts, the zone down to zero (R.58, items 460–462)

**Expansion in place works also INSIDE the full entry (item 460).** The guards
"one may expand only at the root" (`expand_in_place`, `enter_pressed_on_nest`)
stood with a wrong comment "the band is counted from the root, and another reel stands as the root". The model never forbade this: after `swap_reel` the canon is
exactly the current reel, `Band::of` counts the bases from IT, and `simulate`
(unmerge → swap → validate → swap) assembles a band inside a reel with the same
code. The decision "what Enter does" depends only on `bands.any(inst == id)`,
the context is irrelevant; the cascade inside is the same as at the root (t634, t635).
Lesson: a guard without a test of the CONSEQUENCE (t450 built a project but did not expand
it) has lived for years.

**Bands survive a change of context (item 461; the owner's decision of R.58 —
REVISION of R.56 item 5).** The former "Esc/crumb returns to the root WITHOUT the band" is
cancelled: EVERYTHING is restored, including the band through which one entered by the second
Enter — the Enter/Esc cascade is symmetric. Two memories, both store a `ClipId`, not a
`Band`:

* `NestStep::band` — the band of the context being LEFT at the moment of entry
  (taken BEFORE `set_editing`: the latter clears the bands unconditionally — a model invariant,
  restoration is the application's job); it is returned in `leave_nest`;
* `KtApp::reel_bands` — the band expanded INSIDE a reel at the moment of leaving it;
  it is returned on the next entry into the same reel.

The band is RECOMPUTED by `expand_band(inst)`: the contents of the reel could
have changed inside (bases and length are different). If the instance has disappeared or ceased to be an attachment — silently without a band. Undo across a context boundary carries ITS OWN
`bands` in the snapshot, and the snapshot wins — `nest_path` does not dispute it (t636;
t457 was rewritten for the new decision).

**The zone down to zero inside the full entry (item 462; the owner's decision: show it with
a veil, the cursor and playback do not go there).** Zero was a wall
only in the interface: `.max(0)` over the gesture's tick. The format and the model allow `start < 0` in
a reel, the instance's window does not play it. Rule: **inside the full entry the axis of the screen = the axis of the reel, and its zero is not a wall.** Two floors by name:

* `KtApp::time_floor()` — gestures over EVENTS (drag, edges, the band window,
  morph/camera, insertion by click, a drop from the bin): 0 at the root,
  "minus infinity" inside a reel;
* `timeline::CURSOR_FLOOR` — the cursor, region, markers, areas: always zero.

Bare `.max(0)` over `x_to_tick`/`snap` in `timeline.rs` does not exist — guard
t637c. Scrolling: `view_floor(tpp)` — at the root the former −40 px, inside a reel
down to the earliest event minus 40 px. The ruler and timecode left of zero carry a
sign (`format_timecode`, `format_ruler_time_step`; t98), the track background —
`BAND_TAIL_VEIL` with the same mesh as the band's tails, AFTER the events. At the root
nothing changed (t637, t637b). In the file a negative `start` travels as
`kt.start` (kt-otio, v8 by content; t638).

## Transport speed: scale, Shift, numeric input (R.57, item 455)

- **A limit without a recorded reason is a suspect, not a fact.** Before R.57
  the speed slider stopped at 0.25×, because that number stood in
  the clamp of the warp map. The coincidence was taken for a rule, and the interval
  (0, 0.25) became unrepresentable BY CONSTRUCTION: a dead zone of 7 % of the travel
  gave exactly 0, and its edge immediately equalled 0.25×. The program's real limit
  is different — the engine accepts `|rate| >= 0.001`. Before repeating a number in a neighbouring
  place, ask what it follows from THERE.
- **The scale is piecewise and lives in one place** (`slider_to_rate` /
  `rate_to_slider`, the constants `RATE_STOP`/`RATE_FLOOR`/`RATE_KNEE`/
  `RATE_KNEE_T`/`RATE_MAX`): linear 0.01→0.25 over the first 30 % of the travel,
  then a logarithm 0.25→2. Exactly 0 is given only by `|t| < RATE_STOP`
  (1.5 px from the centre). A pure logarithm cannot be here — it has no zero, and a freeze frame is needed.
- **Fine adjustment accumulates INCREMENTS** (`speed_drag_vx` +
  `drag_advance`), rather than dividing the absolute pointer position: Shift
  pressed in the middle of a gesture would otherwise drag the value to the centre in a jump.
  The "second wind" (item 81) is counted from the same virtual `raw`.
- **Numeric input — the field IN PLACE of the lane**, in the same 170×20 rectangle
  (the height was fixed by the owner in R.7) and the same font; the model is
  the timecode mask in the toolbar (`toolbar.rs`, `tc_edit`). There must be no
  window here (rules 3, 8, 13). An empty field shows the current speed
  muted — the first digit replaces it entirely. An unreadable value does NOT close
  the field and colours the underline with the error colour (rule
  16). Opening — right click AND Ctrl+click, both in one body (rule 17).
  `surrender_focus` stands on every closing path, including the external one:
  the Esc cascade clears `speed_entry` as its FIRST line, and the guard in
  `draw_speed_control` returns focus on the next frame (rule 14).
- **An absolute delta written for one scale breaks when the range is widened.** The hysteresis of the space-bar boost was 0.02 — 2 % of
  the base 1× and 40 % of the base 0.05: the boosted speed did not reach the
  player, while the widget showed the boost. Now `boost_needs_update` counts
  2 % OF THE BASE with a floor. Check in the same way any other constant written
  "by eye from one".

## The reel in the bin and an abandoned project (R.55, item 446)

**The reel's card lives next to the files and generators**, but distinguishes them by
a SET OF ADDRESSES (`reels: HashSet<MediaId>`), not by a string prefix: pseudo-paths
are parsed only by `kt_media::factory` (footnote to item 211), and a sequence has
deliberately NO `MediaRef` (item 378) — give it a path, and the watchdog will paint it offline, and the index service will start building peaks with ffmpeg.

The reel's card has: a mini-map of the contents instead of a thumbnail, a ×N badge ONLY when there are several places
(the same rule as on the event, t458), and the verbs — place, expand,
enter, rename, remove by address. Each verb is THE SAME BODY as the
event's menu item (rule 17).

**Dropping a `.otio` ASKS.** "Open as project" MUST go through
`guard_unsaved` and `claim_project` — the same door as the menu item: before R.55
here stood `*self = app`, and unsaved work vanished silently, bypassing
all of R.49.

**Why the guards of R.38 did not catch this** (and what to look at next
time): t253 is exhaustive over the entities of the TIMELINE, and the bin is a panel; t395
asks whether the panel responds to the right button, not WHAT is shown in it.
"Reachable" and "the person will find it" are different assertions.

## Public parameters of a composition (R.59, stage 3, item 470)

Two sides and one door. The **author** declares the handles with a right click on the
LABEL of a field INSIDE the composition; the **consumer** fills in what is declared in the
"Composition parameters" window of the instance.

- **One door — the field's label.** The context travels as `widgets::PublishCtx`
  (the owning reel, the event, the carrier, what is already declared), the claim goes through the sink `widgets::publish_sink` (the `hint_sink` mechanism: inside
  `Window::show` the model cannot be reached). The carrier of the window decides whether to give
  the context: a morpher-PATCH slot has no address (`ParamOwner` does not know it), and it passes `None`.
- **There is NO item outside a reel** (rule 9), and the hint explains why (rule 16) — but ONLY when compositions exist in the document
  (`explain_outside`): in a project without reels it would be a phrase about a mechanism
  that the person never set up, on every label. Guard — t518.
- **There is no door where one cannot enter.** A title has no paint properties at all
  (F-TXT-001), and `color_row` asks the carrier ITSELF
  (`value_type(stem).is_some()`), not a list of exceptions.
- **The owning reel — `KtApp::owner_reel_of` by the WORKING view.** An event of an
  expanded band belongs to the band's reel, not to the one you stand in; publishing into the "wrong" reel would die in `reconcile_published` in the same
  batch. Therefore the confirmation, if needed, ENTERS the right reel
  with the same body as the verb "enter" (rule 17), and for the same reason the
  "Public parameters" window opens only from inside: the publishing commands do not name the reel.
- **The handle's name is the AUTHOR'S TEXT**: it is drawn as it is, bypassing `tr()`, like the name
  of an event or a reel. This must be checked by the GALLERIES (`painted_texts`), not by the accessibility tree: the accessible name of the reset button is compound
  ("Reset <name>", the cell-reuse of the R.54 picker) and contains the name too —
  the first version of t684 was green on code that drew the catalogue string.
- **Each window's session is ONE history step** (a common gesture token, as with
  the FX window); "Cancel" returns the snapshot with the SAME token.
- **There are two badges and they differ**: on the event a pin at the TAIL of the label
  ("name ×N 📌"), on the reel's card — at the HEAD of the number ("📌6"). Zero is not
  written (noise of the same kind as "×1"), and what the pin means is said by
  the event's hint "Overridden: N of M".
- **The reel's preview is keyed by the INSTANCE** (`KtApp::reel_keys`:
  `ClipId → ReelKey{seq, instance_content}`), and `invalidate_seq`
  takes a SET of live revisions: one reel now has as many of them
  as there are differing instances. An empty set gives the former
  `content_hash` — the disk tier and projects without publications do not move.
- **Field labels give up their own rectangle** (`lbl.<param>`): the label is the
  target of a gesture (double click, right click), and a `Label` in the accessibility tree
  cannot be found by a live scenario.

## Clipboard states do NOT exist in the program (R.61)

The owner identified the defect by an indirect sign: "judging by the fact that the button can be "disabled" — there is some clipboard state in the code — which is bad."

- There is no clipboard mirror; the paste button NEVER goes grey. The contents are read ON PRESS, and only then is it decided whether pasting is possible. Guard — t699b.
- **Reading only by a background task** (`envclip::spawn_read`). On X11 the clipboard
  is OWNED by a window; after a copy the owner is our own window, and a synchronous request
  would go into our own event loop, which at that moment is drawing. It would hang
  in exactly the scenario for which the feature is made.
- Unparsed content is a NAMED error in the status line, not a greyed-out control.
- Pasting into another effect's kind lays out the fields (`ops::fit_envelope`): those matching
  by name AND type silently, the remainder — in a window; the refusal names the number of lost items.
- **A derived curve is not edited with the mouse** and has no key markers: the probe
  walks over what is drawn. Next to it there MUST be an action "Unlink", otherwise the control
  stays greyed out for good (rule 9).
- **The formula field checks the text on the fly and does not apply a broken one**: half-erased
  text has no right to drop the curve on every keystroke.

## R.63 — four rules of the owner (items 8, 12)

All four come from one list of remarks, and all four are about one thing: the interface is assembled where the person ALREADY stands, and remembers what it was told.

1. **Name by industry** (checklist 21, guard t756c). "Mute under the track"
   → "Ducking (muting background under speech)". The industry word comes first, the explanation in brackets.
2. **Windows only from the registry** (checklist 22, guard t757). `WINDOWS` in
   `reach_tests.rs` — the list of ids and REASONS. The order of searching for a place for a form:
   the head of the chain strip → a section of an existing window → a panel → a window.
   In R.63, two windows were removed by this rule: the composition parameters (it became
   the head of the strip, t754) and ducking (it became a section of the track window, t756).
3. **The tool remembers its parameters and gives up curves** (checklist 23, guards t756/t756b). The tool's settings — in `user_store`, values with time — via
   the common automation widget.
4. **One widget — not a copy** (checklist 24). Ducking is a compressor with a
   sidechain; with the next change (`sidechain-ducking-compressor`) it will become the live sidechain instead of a curve snapshot. The snapshot is left exactly until then.

Where to look in the code: `track_window.rs` (the ducking section), `pubparam.rs`
(`draw_inst_head` — the head of the strip), `fx_editor.rs` (`head_slot` of the reel instance and of the generator), `user_store.rs` (`Workspace::duck`).
