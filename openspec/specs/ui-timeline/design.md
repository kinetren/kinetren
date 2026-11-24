# Timeline interface — why it is like this

## What hurt and for whom

The interface "compiled and drew", but it could not be used, and
the owner for five rounds in a row sent remarks about the same approaches
("UI/UX is made FOR THE USER, yet you make it for who-knows-whom"). The concrete
pains from which the rules grew: the system heated up at 60 fps while idle because of "burning"
meters; the video showed exactly 24 frames/s at any quality, because
the playhead moved in quanta of the audio callback; the preview "disappeared without trace" —
the panel swelled with content; the width of the media bin was squeezed "by analogy" for three
rounds in a row; the button hints vanished because the status bar read them before
they were written; the fader flew to 400 dB ("painful to the ears"); square stubs
instead of icons; the "Forward" button silently got stuck; a value typed into the
FX field "reverted to the previous one"; the background autosave overwrote a fresh Ctrl+S.
A separate category — "you did not check": the meters' limit was green in a test, while the
user ran `target/debug/kt-app`, which `cargo test` does not
rebuild.

## Decisions and why

- **Repaint = movement, the preview key on the fps grid, meshes instead of primitives,
  throttling by epoch.** This is invariant 12 of the root CLAUDE.md: the UI has no right
  to heat up the system. Any new animated state MUST converge; a raw tick
  as the preview key is forbidden.
- **The display clock is interpolated** between the steps of the master clock (capability
  `transport`): audio remains the master, the display is computed. The raw output
  latency is smoothed, the key is clamped monotonically.
- **The UI/UX checklist — part of which breaks the build.** Prose did not work: rules
  1, 7, 17 were written down and violated word for word, therefore the guards of the
  capability `ui-reach` appeared (no orphaned commands, a full set of verbs for each
  entity, a click reaches the target, a name is truncated). The remaining items — in prose, but
  "an item without the mark 'closed by a test' is a claim for a test".
- **Limits are not moved at the owner's request**: a 420 px left column, 90 px
  meters, a 640×640 window, vertical 30/84/16/120, a speed strip height of 20 px.
  Next to a limit — the comment "set per item N of round M"; before a change —
  `git log -S`.
- **"A remark is a class of problems, but do not squeeze everything in a row."** One may generalise to
  homogeneous DEFECTS (all icons, all hints), but not to settings nobody asked about — the bin's limit was trimmed "by analogy" with the meters, and broke the work for the third time.
- **Do not waste space.** The name of a label — on the time caption row, the name of a
  group — in the gap of the frame, the name of a track — as a tag under the clip's name, the name of the
  transition kind — in the transition window, the morpher's strength — as a polyline inside the block. A strip for the sake of a
  rare caption is a defect.
- **One hint in one place** (bottom right of the status bar), without pop-up
  instructions; everything that is filled in the frame is drawn at the end of the frame as a separate
  layer.
- **Content ≠ convenience.** Two dirty flags, not a third; the order of the
  rows was moved into the content, because a morpher layer acts "on everything below" and the
  arrangement of tracks changes the picture.
- **All model edits go through the application's wrappers.** On the model's revision hang the
  autosave gate and the frame caches; live paths send a command only on a real change.
  The region and loop are not read by the engine from the project — editing the region does not rebuild
  the renderers.
- **One widget for one meaning.** The parameter row per the model's spec, one chain strip, one
  automation widget, one parameter grid for the transition and the camera
  (the parameters trait in kt-model), one truncation helper, one hint sink.
  The disarray of Gain ranges between windows is a direct consequence of "each window draws
  for itself".
- **The event's preview computes the same map as the render** (capability `video-plan`):
  the formula "event tick → material" lives in the engine and in one function of kt-app; do not
  set up one's own in the UI — the same rule as for the timeline's geometry.
- **Thumbnails and peaks do not produce decode from scrolling/zooming** (invariant 8): sidecar DB,
  peak pyramid, negative caches, LRU of sources. Anything that may fail for a worker publishes the failure — otherwise the UI re-requests every frame.
- **Localisation — catalogs, English literals, glyphs are checked by a test.**
  The rule about non-printable characters did not work without a test (two squares for the
  user), so the literal scanner became a test; the Noto subsets are assembled by
  a script from the catalogs, not by hand.
- **The test asserts the consequence and probes from where the person acts.**
  The R.38 mechanism was green on a program where three of its items did not work: the tests
  checked the reachability of what was made, not the consequence, and pressed into the row's header,
  where the agent himself had put the menu item. A live scenario per item is the condition for closing it.
- **Speed is judged by the release build.** The dev profile got `opt-level = 1` (the glob
  `package."*"` covered only the dependencies), but the measurement of pace — only
  `kt-cli probe --time` on a release build.

## Rejected and why

- **A gate "only while paused" for the input window, the search, the labels** — rejected: the player
  can seek on the fly; a gate is set only when without it an engine invariant breaks,
  not "so as not to think".
- **Checkboxes and obvious captions ("zoom:", "FPS:", "(loop)")** — rejected by
  direct requirement: modes are shown by an icon, state by highlighting the button.
- **A standard `TextEdit`/`Slider` in the middle of the branded toolbar** — rejected
  ("you can't just bang an inputfield in there"): the input is drawn in the same
  rectangle and font as the value it replaces (the timecode mask, the speed field).
- **A second "video warp" toggle as a separate field** — rejected: the slot's checkbox in the
  chain strip already exists and is on by default; a second toggle of the same meaning breaks the rule "nothing superfluous".
- **Smart logic that "corrects" neighbours on the user's behalf** — rejected
  ("NOTHING ANYWHERE SHOULD GROW OR BE SWITCHED OFF"): an edit changes exactly what was pulled;
  a repeated gesture on a transition adjusts only the offsets.
- **An explicit integration scheme for animations** — rejected: `stable_dt` after
  an idle period of up to 250 ms, the scheme diverges; only analytic/EMA.
- **Own geometry formulas ("offset(kind)+lane", division by the row height) outside the geometry cache** — rejected: they lie on mixed row heights.
- **A per-character `reorder_line` for Arabic** — rejected: harfrust
  reverses the run a second time, the letter forms are computed on the reversed
  sequence; words are reordered instead.
- **A synchronous project write from `App::save`** — rejected: it dropped the FPS to zero
  once every 30 s; saving is done by a background thread with a revision guard.
- **Forging the autosave timer from inside for live scenarios** — rejected: it would not be the
  program the user runs; there is `KT_AUTOSAVE_SECS`.
- **Plural forms in the catalogs** — rejected: "noun: N"
  would require tables of plural forms for five languages.

## Non-obvious details

- **The activity of a meter is "the bars move", not "they burn".** The condition "above the minimum"
  stuck after the first sound; the measurement under the cursor holds only while the cursor
  is moved (`src/apps/kt-app/src/meters.rs`, `MeterState::is_active`, `tick`,
  `meter_holds_level`). The glow rows sit on fixed dB stops; the alpha is
  `glow_row_alpha` from its own dB row.
- **The preview's frame key is aligned to the grid**: `preview_key_tick`
  (`src/apps/kt-app/src/app.rs`); the display position is `Player::video_playhead`
  with `latency_smooth` (`src/libs/kt-engine/src/player.rs`); the composition buffer
  is reused by `compose_preview_into`.
- **The order of the meter layers is part of the look**: underlay → glow → segments → highlight.
  Moving the underlay over the glow killed the "real" look for two rounds;
  the symptom "the faded scale glows" was the tail of a gradient, not the fill
  (`src/apps/kt-app/src/meters.rs`, `t40_glow_layout`).
- **The separator lives at the ACTUAL edge of the panel** (`PanelState::outer_rect`), not at the
  requested width: the egui 0.36 panel grows to fit its content
  (`src/apps/kt-app/src/layout_tests.rs`, helper of the actual rectangle).
- **The resize zone is not a widget**: the pointer is read at the end of the frame, `BeginResize`
  is sent at the press itself (the widget lost the hit test, and `drag_started` came
  after 6 px — already outside the 4-px zone); two radii 8/16 px
  (`src/apps/kt-app/src/app.rs`, `resize_zone`). Everything within 16 px of the edge is in
  `no_resize_rects` (`src/apps/kt-app/src/toolbar.rs`).
- **X11 `_NET_WM_MOVERESIZE` swallows ButtonReleased**: after every
  `StartDrag`/`BeginResize` — `release_pointer_after_wm_grab`
  (`src/apps/kt-app/src/app.rs`); the wheel gates are `contains_pointer()` +
  `hover_pos()` from input, not `response.hover_pos()`.
- **The eframe root `Ui` is framed by 8 px** — we draw on the full `content_rect`
  (`src/apps/kt-app/src/app.rs`), otherwise "the meters' indent on the right".
- **The hint is drawn last in the frame** `Area(Foreground).interactable(false)`
  — otherwise the layer steals the window's drag; the test reads `status_line_shown`, not the field
  (`src/apps/kt-app/src/app.rs`, `status_line`). Inside `Window::show` there is no
  `&mut KtApp` — the widgets put the hint into `hint_sink`
  (`src/apps/kt-app/src/widgets.rs`), the same device for `field_probe`.
- **`ui.max_rect()` inside a `ScrollArea` is the viewport in the content's coordinates**:
  decisions "the cursor has left" are made by `clip_rect()`. The wheel of a `ScrollArea` is muted for the
  duration of any drag — programmatic scrolling through
  `scroll_with_delta_animation(…, none())` + `request_repaint`
  (`src/apps/kt-app/src/bin_panel.rs`, `bin_edge_scroll`).
- **`key_pressed` enables auto-repeat**: holding the space bar is read as the event
  `repeat:false`, the decision is made on release; the acceleration is the analytic exponential
  `space_boost_step`, the hysteresis of sending is 2 % OF THE BASE, not absolute
  (`src/apps/kt-app/src/app.rs`).
- **The playhead in audio frames**: navigation "to the right" — by `Tick::align_audio_up`,
  "the end" accounts for the markers and the region (`src/apps/kt-app/src/app.rs`, `nav_jump`,
  `t82_stop_points_and_jumps`).
- **Row geometry**: `TlGeom` with `row_height`, prefix sums,
  `row_at_y_unbounded`, `lane_y`, `anchor_at` (`src/apps/kt-app/src/timeline.rs`);
  `KT_GEOM_DUMP` gives each row its own `h`, `lane_h` at the root — the base.
- **Magnet**: a candidate "without snap" does not take part in the comparison, the link group
  is excluded from the targets, the line is returned together with the result (`snap_move`,
  `src/apps/kt-app/src/timeline.rs`).
- **Overlap is classified by `plan_move`**; the gestures of a break go through
  `apply_batch_restoring` — the return of the handles of the dead crossfades by the same undo step;
  the reconcile of transitions — once per batch (`src/apps/kt-app/src/app.rs`,
  `src/apps/kt-app/src/timeline.rs`).
- **The top strip**: `top_strip_h`, the grab of the handles `fade_grab_for_width` (a third
  of the width, the middle is free), the level `clip_level`/`gain_to_level`
  (`src/apps/kt-app/src/timeline.rs`); the corner takes priority over the transition window in the
  strip — otherwise a pushed-in fade cannot be removed.
- **The preview of the material during gestures**: `preview_src_shift` — the trim of the start → the delta
  of the edge, everything else → 0 (`src/apps/kt-app/src/timeline.rs`).
- **The obstacle lives one frame**: `KtApp.blocked`, `block_view` decides
  visibility (`src/apps/kt-app/src/timeline.rs`, `src/apps/kt-app/src/app.rs`).
- **The frame only from an empty place**: Ctrl on an event removes the magnet — two gestures
  share the press zone, not a modifier; the hit — `rubber_hits`, the stop of the batch —
  `plan_move_block`, `DragState.extra` empty = the former single drag
  (`src/apps/kt-app/src/timeline.rs`).
- **Double clicks**: one body `region_from_event`, `marker_here`
  (`src/apps/kt-app/src/app.rs`); the choice of a marker — `pick_range_marker`
  (`src/apps/kt-app/src/timeline.rs`).
- **The edit wrappers** `apply`/`apply_batch`/`apply_coalesced` bump `model_rev`
  and call `sync_media_sources`; `on_region_changed` does NOT call `set_project`;
  writing — only `write_project_guarded` (`src/apps/kt-app/src/app.rs`).
- **Fields and chains**: `param_row`/`params_grid`, `chain_strip`, `elided_name`,
  `param_value` (percentages — only there) — `src/apps/kt-app/src/widgets.rs`;
  an edit with a live curve returns `ParamsChanged { base, curve }` — the base →
  `SetFx`, the curve → `SetFxAuto` (`src/apps/kt-app/src/fx_editor.rs`); the reference — the
  single flag of the morpher (`src/apps/kt-app/src/morpher_editor.rs`, `merge_chains`).
- **The automation widget**: the scale by keys, the window is frozen on a drag
  (`AutoView.drag_win`), the time sort on release (`AutoView.t_edit`),
  the conversion material ↔ timeline only via `kt_model::clip_material_at`/
  `clip_timeline_at` (`src/apps/kt-app/src/automation.rs`).
- **`DragValue` has no accessibility node** — the field's coordinate is taken from the real
  layout, the parameter's caption occurs twice, the top one is needed
  (`src/apps/kt-app/src/layout_tests.rs`, `t281`).
- **A window that grows from input breaks the test's coordinates** — the buttons are to be probed after
  the redraw (`src/apps/kt-app/src/layout_tests.rs`, `t345`).
- **Thumbnails**: the key `thumbnails` is an exact PK, "nearest" — through
  `thumbnail_near`, `ThumbKey` is not to be extended; one map `thumbs::material_at`/
  `material_key_t` (`src/apps/kt-app/src/thumbs.rs`); the warp map is cached by the pair
  (revision, number of clips) — the model is swapped without a bump when a project is opened
  (`warp_maps`, `src/apps/kt-app/src/app.rs`); the poster — `frame_score`.
- **The placeholder**: `open_source_indexed` is strict, it is substituted by
  `open_source_or_placeholder` (`src/apps/kt-app/src/util.rs`); `replace_media`
  must bump the project version, otherwise the prefetch at idle will not notice
  (`src/libs/kt-engine/src/player.rs`).
- **Fonts**: the order in `install_fonts` (`src/apps/kt-app/src/icons.rs`) —
  Latin behind Ubuntu-Light, PUA behind Phosphor, Noto last: on this rests the predicate
  "an Arabic atom" in `src/apps/kt-app/src/i18n/bidi.rs` (`visual`).
  The codepoint of a Phosphor glyph is looked up through the GSUB ligatures.
- **`#[cfg(test)]` in the middle of a UI file truncates the scanners** `font_tests::sources()`
  (`src/apps/kt-app/src/font_tests.rs`): the test module goes at the very end of the file,
  with zero indentation.
- **The full path of a catalog key**: t125 looks for `k::section::CONSTANT`; an abbreviated
  `use` hides the key (`src/apps/kt-app/src/i18n_tests.rs`); `Window::new(title)`
  derives the id from the title — an explicit `.id()` for each window.
- **The render result travels through the channel as NUMBERS**: the language lives in the TLS of the UI thread; in the background it would always be English
  (`src/apps/kt-app/src/render_dialog.rs`).
- **The harness**: `harness_lang` installs the real fonts; `drop_at` sends
  ButtonRelease and PointerGone, `step()` — a frame per event; window commands are collected after an
  explicit release; a double click — `with_step_dt(0.02)` and its own harness
  (`src/apps/kt-app/src/layout_tests.rs`).
- **Live scenarios** (`src/scripts/live/*.py`): coordinates from `KT_GEOM_DUMP`,
  modal windows through the dump's `fields` (`widgets::field_probe`), the snapshot `xwd`
  (ImageMagick returns black on a GL window), release with a pause of 0.35 s, a double
  click with a gap of 0.04 s, the pixel after a gesture — `pixel_settled`; first prove
  that the gesture happened and the media was resolved.
- **What the player plays — `KtApp::player_project`**, the sender
  `push_project_to_player` (`src/apps/kt-app/src/app.rs`); two time floors —
  `time_floor` for gestures and `CURSOR_FLOOR` for the cursor
  (`src/apps/kt-app/src/timeline.rs`); there are no bare `.max(0)`.
- **The icon**: `with_icon` is mandatory; on X11 WM_CLASS = the binary name `kt-app`,
  `StartupWMClass` in `.desktop` matches it (`src/apps/kt-app/src/app.rs`).
- **Detaching the preview** — `show_viewport_immediate`, one `egui::Context`;
  `ctx.input()` inside a viewport is that viewport's input, the shortcuts are duplicated
  (`src/apps/kt-app/src/app.rs`, `t51_preview_detach_state`).
- **A bare letter yields to its Alt and Ctrl chords** (t868, change
  `guide-shortcuts-reference`). `cut` read `S && !ctrl` while every other letter
  also excluded Alt, so `Alt+S` soloed AND split the event under the playhead in one
  frame. t742 pressed `Alt+S` and asserted only the solo: a test of a chord must also
  assert that the bare letter's consequence did NOT happen, with the playhead placed
  where that consequence would be visible (inside the event). Shift stays allowed:
  `Shift+A/D` is the same jump with the region edge dragged along.

## R.67: the graph draws what sounds; the curve block is one

- **The line is built by sampling `curve_value` per pixel**, not by segments between keys.
  t698 asserted the model and was green on a graph that drew straight lines: "visually, the user
  cannot see what kind of curve is there at all". An assertion about the drawing is made on the
  drawn polylines (`painted_lines`), not on the model.
- **The handle of a shape parameter — at the middle of the span.** Parametric shapes (zigzag, third
  order) have no geometric Bezier handles, and turning a number in a field is exactly "instead of entering
  digits". The grab is taken from the POINT OF PRESS (`press_origin`): egui declares a drag when
  the pointer has already moved past the threshold, and the current position can be further than the grab zone.
- **The block "curve as a parameter" is one per slot, generator and reel instance**, and it refers to the
  curve that the person SEES as selected. Before, "the shape of all segments" silently applied the rule
  to the active curve — hence "only one field changes, although I set zigzags in two fields".
- **The reference is a group in the block, the completion is Tab in place.** Pop-up layers are forbidden
  (`ui-reach`), so the list of names lives in a collapsible group, and the candidates go to the status line.
  The content is built from the evaluator's registry (`formula::FUNCTIONS`) — not a second copy of the language.
- **One scroll per window.** The window's outer `vscroll` of FX scrolled the action buttons too:
  "Done" slid past the bottom edge as soon as a block appeared under the graph. Now only the body
  scrolls (`window_body`), and it itself leaves room for the buttons
  (`ACTIONS_H`). It was found by a live scenario; the harness was silent.
- **The geometry frame is on by default and remembered.** A useful control that is off by
  default and hidden in a context menu does not exist for a person (rule 4).
  The button on the frame is registered AFTER the controls: the frame's body covers the whole frame and otherwise
  takes the click from it.

## kkt-26.11.01 item 14: Performance lives in Diagnostics, with three graphs (diagnostics-three-graphs)

- **The owner counted the graphs, he did not name them.** The perf box of the left column had two
  (Quality, FFmpeg activity), the Diagnostics window one (Cache churn); moving the box makes three,
  and they are kept because each answers a different cause of "it does not play smoothly": the
  display (FPS, V-fps, hits), the decoder and disk (FFmpeg activity), the cache (churn, no-read
  drops). A CPU/GPU load graph was rejected: it is what sends people to the wrong fix (F-UI-006).
- **The always-visible layer is the status line** (vfps, cache, underruns in ms) — the industry's
  light in the viewer; the detail is one window opened on purpose. The column belongs to the bin,
  and the bin's frame reaches the column's bottom: the freed space is its background (drop, right
  click), not a dead strip under a short frame.
- **The window's body is one scroll; the graphs share its viewport.** A larger window gives larger
  graphs; a lower screen scrolls the body. A fixed default of 720 points put the title and its
  cross above a 420-point screen (t779): the default height follows the screen.
- **A rolling graph is built once per sample** (`graphs.rs`: one mesh for the lines, galleys for
  the labels, keyed by rectangle, generation and language). The auto-scale had stepped per DRAW —
  its speed was a function of the UI frame rate and the picture of the frame count; it steps on
  `push`, as v1's 500 ms timer did (t1149). The graph names its accessibility node with its title
  in a closure (only with accessibility on): the render window's speed chart is found by it (t956).
- **"Economy UI" is one switch, in View.** The box repeated it; the owner's item 13 is this class
  ("the checkbox in the window AND duplicated in View"). Its hint moved onto the View item.
- **The window opens at its floors where the screen has the room** (orch stage 9, t2304). The
  fixed 720-point default fell under the floors as rows were added (telemetry-journal's six journal
  lines outside the scroll, memory-ledger's line): on the owner's working size the third graph
  opened under the fold, and a corner drag first paid off the shortfall, so the graphs took 9
  points of a 140-point drag instead of their quarter (the live `diagnostics` went red). In egui's
  sizing pass of a window it never laid out, the body claims the instruments + the graphs at
  their floor + six journal lines (as measured in that pass), down to 48 points above the screen's
  bottom as a fitting body does; egui's `Resize` then opens the window at that height. A size the
  person gave, or egui remembers, is never touched; the 720 stays the least default.

## The toolbar overflow (windows-fill-their-area, archived 2026-11-01)

Below ~1460 points the bottom toolbar row cut off Render at its right end (kkt-26.11.01 item 12; noted
as deferred by tour-lessons-fixes). The row is 8 groups (transport, timecode+speed, tracks, edit,
zoom, region, undo/redo, output); each group's width is measured when it is drawn in the row and
kept in `KtApp::tb_group_w` (a fixed array — no per-frame allocation). Next frame the groups are
laid from the left while they fit with the `⋯` button; the rest are drawn inside the `⋯` menu by
the SAME `tb_group` function (one body, rule 17). The first two groups never move. A group in the
menu keeps its last measured row width, so the decision does not flicker. The first frame has no
widths and draws every group (clipped), which measures them. Guard t1132 (a ⋯ at 1100 points that
reaches Render; none at 1700). Rejected: shrinking buttons or a second row (the owner set the row's
look; a second row costs the timeline height, rule 13).

## Economy UI moved to File › Preferences… (welcome-window-and-settings, archived 2026-11-02)

The note above ("one switch, in View") is revised by the owner's kkt-26.11.01 item 13: the program's
preferences leave View for one window, File › Preferences…; View keeps only what changes what is
viewed (Detach/Dock, Diagnostics…, Notices). The switch is still ONE (t1294 counts its writers), and
its value lives in the user store, so opening another project no longer resets it (t1295).

## What Copy takes, and the triple click (selection-copy-target, archived 2026-11-04)

Copy takes, in this order: ONE selected event (counted by link group) whole, with the region left
in place; else the region; else the selection. Written in `copy_selection` only, and the status
line names the rule that fired (`copied_event` / `copied_region` / `copied_events`), because the
three branches look alike on the screen.

- **Link groups are counted, not ids.** A click on a video event selects only it, but its audio
  partner travels anyway; the person who clicked one A/V pair has selected "one event". The
  consequence is written down: a range marker that crosses exactly ONE event, double-clicked and
  copied, gives that event whole, not the cropped piece (t926). Remembering whether the selection
  came from a click or from a region gesture was rejected: an invisible state that decides what
  Copy takes is a mode, and modes are what the owner keeps complaining about.
- **Why not "any selection wins"** (Premiere, Resolve): it would change the range-marker
  double click (R.33 item 8 selects the crossing events AND sets the region), whose Ctrl+C would
  stop cropping to the marker. **Why not Vegas's "selection limited by the region"**: it needs a
  third block builder (selected ∩ region) and it reverses the owner's request, where the one event
  must not be cropped. Several selected events keep the region rule until the owner decides.
- **A triple click undoes its second click.** egui reports `triple_clicked()` on the third press,
  AFTER `double_clicked()` fired on the second. The double-click branch leaves a one-frame record
  (`pending_double`: FX window opened / reel expanded / level reset); the triple-click branch sets
  the region from the event and closes the window or collapses the reel the gesture itself
  opened. The level line keeps its reset and ignores the third click. **No delayed double
  click**: waiting 0.3 s to learn whether a third click comes would open every FX window late,
  and the double click is used a hundred times more often than the triple (the FX window flashes
  for ~0.3 s during a triple click; t927 asserts it is closed after the third).
- The chord column of "Region over this event" and the event's hint name the triple click;
  Shift+double-click still works and is the same body (`region_from_event`, t929).
- Open and unchanged: Delete still follows the region-first order (the owner asked about
  copy-paste only; "what Ctrl+C takes is what Delete removes" is the recommended next answer).

## A window that grew by itself (shared-window-shell, archived 2026-11-04)

The owner's morpher window widened by one physical pixel per frame after Ctrl+= pressed once or
twice (Ubuntu, 1920×1080, native scale 1.0, zoom ≈ 1.1–1.2), "slowly when the mouse does not move" —
the idle repaint rate. The cause was found by bisecting the owner's scene (t923): the **window-level
scroll** `.vscroll(true)` (the morpher and camera windows; notices and diagnostics had it with narrow
bodies). egui's `Resize` only grows (`desired_size.max(last_content_size)`); with a window scroll a body
a fraction of a pixel wider than the window is reported as content and the window keeps it, every frame.
Without `.vscroll`, no zoom of the sweep 1.05…2.0 grew (0 of 20 against 4 of 20 with it). The minimal
reproduction outside the tree had missed it by trying 1.25/1.5/1.75/2.0 instead of 1.1/1.3 and by
different margins; it is the program's own zoom, not the OS scale, that triggers it. The rule is the
one of "One scroll per window" (R.67), now applied to all windows: **the shell never sets a window-level
scroll**, only the body scroll (`window_body`). The width-rounding variant (b) was kept in reserve and
not needed. `track_window`'s own copy of the body scroll also grew its height 30 points a frame up to a
ceiling; it now uses the shared body.

## A program-chosen reel name follows the language (i18n-gaps, archived 2026-11-04)

- **D3. Recognised by its text, shown in the current language.** `reel_display_name(stored)` returns the
  current language's default plus the same number if `stored` equals the default reel name of ANY of the
  five catalogs, optionally followed by `" " + digits`; any other text is returned untouched. Every place
  that shows a reel name goes through it (strip head, bin card, crumbs, settings title, published-parameter
  windows, rename prefill), and the search and the rename uniqueness check compare both forms. Rejected: a
  flag `Sequence.name_auto` (a project-format field for a display concern, and it would not fix the owner's
  EXISTING file; teaching the reader to set it for old names is the reader learning an old form, which
  R.67 forbids); an empty stored name (names are unique addresses in the model and OTIO needs one); leaving
  it as data (the person never typed the word, the program wrote it). Cost, written down: a person who
  deliberately typed exactly "Reel" or "Reel 3" sees it in the other language too (t932b keeps their
  other texts untouched).
- **D4. A Cyrillic placement guard.** t933 extends t131 beyond literals: catalogs other than `ru.toml` value
  by value (the `[lang]` endonym excepted), and committed fixtures as text with a declared list of Cyrillic
  test data and a reason per entry. The `.desktop`/MIME files are checked against `ru.toml` by t930.

## The timeline stops fighting the hand (r72-timeline-transport-defects, archived 2026-11-06)

- **One order of gestures, `event_zone`:** fades in the top strip, trims, level, body. The level used to
  be tested before the trims at a fixed ±5 px, which on a low row covered a third of it, corners
  included. Its grab now follows the row, and below 40 px of row or 61 px of width there is no level
  gesture, only the menu's number field. That takes back what R.40 item 12 asked of a 20 px event; the
  owner's later word (U5.3) decides, and t278 and the live `level` say so.
- **Gain to silence:** the upper half of the line stays the v1 rail, the lower half is a square law to
  −60 dB, and the last 2 % of the travel is mute. The square, not linear dB: linear gave the audible first
  12 dB a fifth of the travel. The floor is ×0, not −60 dB, and a nested piece adds up with
  `add_event_gain`, or a muted instance would come back by 6 dB.
- **The chip takes its room from the name**, which is elided only when a chip is shown: a chip that
  falls off a long name would say nothing.
- **A marker's end leaves its own old place out of the magnet BY VALUE**: a marker made over the region
  lies on the region's end, so leaving out "the marker's edges" still snapped it back.
- **Vertical zoom rebuilds the row tops at the new height.** The product of row index and height is
  the "single division" this file forbids, and `scroll_max` now comes from the real heights too.
- **A click in a jump zone jumps over anything** without touching the selection (rule 5: a jump
  must not destroy a multi-selection), unless Ctrl or Shift is held. With the project's start on screen
  the left zone stands down: otherwise the first eighth of a fresh project could not be clicked.
- **The left zone was not lost by rect:** a click beside the header always jumped. Its gradient has run
  under the header since item 20 of round 6 while clicks there did nothing, and R.62 grew the header
  from 34 to 62 px. So the header's free part became the hot end. A double click there stays the rename
  and gives the view back: the same undo as the triple click on an event, not a delay on every jump.
- **`if … && let Some(X) = self.drag.take()` consumes the drag whatever X is.** The height drag's release
  did that since R.38, so a track reorder died with no symptom but "nothing happens". The proposal
  called the gesture working: nothing had ever tested it. One release, one `match` (t1637).

## V-fps counts new pictures; a held frame says it is catching up (stream-history-on-device, archived 2026-11-10)

- **What hurt:** V-fps counted every tick change; his trace: 114 shows in 6 s, 111 a held stream frame,
  45 distinct pictures, the counter at 20 where "at most 5 are new" (row 393). A held frame looked like
  playback and nothing said otherwise.
- **Decision:** a composite that shows a held stream frame is no new picture (`PerfState::take_composite`);
  held frames a second show next to V-shown in Diagnostics while non-zero; the preview draws a small
  "Catching up…" note in its corner over the moving picture. The Loading scrim keeps the owner's rule
  (an incomplete frame after 180 ms): the note is not a scrim and asks for no frame of its own.

## Deferred

- `lane_names` live in `Project.ui`, and `swap_reel` does not swap them — the full entry
  shows the root names on the reel's tracks (a known defect, capability
  `compositions`).
- Stabilisation of the XTest gestures of the live scenarios (flakiness in software rendering) — a separate
  piece of work, not done.
- The notes `docs/features/ui/*` (keys, layouts, the system file dialog,
  clip colours, the viewer's on-screen controls) — status "candidate", not part of the contract.

## The memory numbers say what they count (frames-cache-growth, 2026-11-10)

"cache: 39/535 MB" was audio/video, read by the owner as used/limit (found-defects 397). The status bar
now has "frames: held/bound MB" and "memory: P MB" with one hint; Diagnostics' first row, which
duplicated the occupancy line, says "Program P MB · rest R" with a hint naming the rest (decoders,
graphics driver, interface — no counter of their own). A separate "Program" line was tried and dropped:
it put the instruments past the window's fixed 720-point opening height, and the window opened short and
grew by itself a frame later (t918, t1147; the shell's sizing pass is row 442). The caches' line is one
line whatever its numbers. Rejected: one long "video V / cap · audio A · program P" item — t128 holds
the status items to 190 points in every language.

## Loading names the catch-up; V-fps counts complete pictures (catch-up-shows-loading, archived 2026-11-11)

- **What changed and why:** the owner decided against the held stream frame (kkt-26.11.10b K1, K5:
  "либо есть кадр либо нет") and put the catch-up where Loading is written (K2: "в том же месте где
  loading писать"). The corner note and the held/s count of stream-history-on-device go: a frame short of
  its history is incomplete, the app keeps the last full picture under the scrim, and the scrim's text is
  "Loading — catching up: N" (`perf.loading_catching_up`, five catalogs; the "noun: N" form, no plurals)
  while `Composed.catching_up` is above zero, plain "Loading" otherwise. The probe `preview.catching_up`
  lets the live `datamosh` scenario see it (t2416).
- **V-fps:** `take_composite(moment, complete)` — only a complete composite of a new moment is a new
  picture (t2358). The `loading` trace record carries the positions short in `b`; `kt-cli trace
  loading` says so per stretch (t2417).
- **t2415 under load** (stage 11's finish.sh, 2026-11-14): the test set the frame on screen's state and
  read one frame; under a full workspace load the harness's last ask outlived its 5 s grace and its answer
  landed in that frame (`catching_up` 0, "Loading"). The answer is the program's own news: the test now
  sets the state and draws until a frame lands no composite, and its scene asks again without waiting
  so an answer is in flight (reproduced red on the old body: composites 3 → 4, the scrim read
  "Loading"). The first fix also asserted that the ask was still pending after its frame and that its
  answer landed later — a timing assumption of its own, red in the third finish.sh; the test now
  asserts nothing about when an answer lands.

## "Marker over the region" split (marker-region-split, 2026-11-18, F-TL-016)

The owner (`v26.10` line 33; his answer of 2026-11-07, в.4): the region-edge item did three things under
one name. Now "Marker over the region" (`marker_over_region_new`) ALWAYS adds a range; an equal range is
removed (M's toggle); a different marker at the region's start is not changed — the model keeps one
marker per tick, so the item refuses with a notice naming "Stretch the marker over the region"
(`stretch_marker_over_region`), offered only when `marker_to_stretch` finds one. M keeps its R.24
behaviour, stretch included (t142 is frozen; his words were about the menu item). The live dump carries
`markers` as [tick, length] for scenarios. Class walk (one name, results by hidden state): "Marker here",
"Clear the region", the marker menu's items and the empty-space region items each have one result; only
this item changed.

## A marker's note at its flag (marker-notes, 2026-11-18)

The inline-field pattern of the marker's name, multi-line: Enter is a new line, so the note is kept
by Ctrl+Enter or a click elsewhere (a stray click does not throw a typed note away) and Esc cancels;
one `SetMarkerNote` when it closes. The note glyph (Phosphor `note`) stands after the name on the
time-label row (rule 13), the text is read in the status line on hover (rule 10), its lines joined by
" · ". Class walk (a remark with only a name to hold it): areas, events, tracks, the bin's media and
groups have names only and no request; reels have their public parameters' author text. "с пометками"
is read as free text with lines; a checklist or styled text would be a question to him.

## bin-drag-moment (2026-11-19, v26.10 MW1, kkt-26.11.18b U9)

### D1. The moment is the press point

The scrub strip already maps the pointer's x to the file's time (F-MED-008). The press that starts a
drag on that strip is the moment the person was looking at, so the card records it at
`drag_started` (`KtApp::bin_press_moment`) and the carry-out payload `BinDrag(media, moment, length)`
takes it. A press off the strip (the name, an audio or generator card) carries moment 0: the start
lands under the mouse, as before.

### D2. Which point lands under the mouse

`timeline::BinLanding` from the modifiers of THIS frame: none — the moment; Alt — the start; Alt+Shift
— the end (his "еще какую-то комбинацию": Shift is added to Alt, the end being the start's mirror; the
right button he also named is the context menu and stays so). The tick under the mouse is snapped
first (the magnet catches what is under the mouse), then the start is that tick minus the offset,
held at the time floor; a start held at the floor shows in the ghost, which is drawn from the same
number as the drop. Alt pressed with the button is the window manager's move on many Linux desktops;
pressed after the drag began it reaches the program — the live scenario presses it so.

### D3. The ghost is the event

The silhouette was a fixed ~3 s mark at the mouse for every drop. For a bin card the length is known
(the thumb service's duration; a reel's content end), so the ghost is that length from the start the
drop will use, with a white tick at the mouse. A file dropped from the system is opened only on the
drop: its ghost stays the mark.

### D4. The card's hover text

egui keeps a dragged widget "hovered"; the card's file-and-loudness text, drawn after the timeline,
replaced the drop's line in the status bar for the whole carry. A dragged card writes no hover text.

### D5. The carried card holds its moment

The card's strip follows the pointer's x (F-MED-008); while carried, the pointer is far right of the
card, so the strip showed the file's last frame and its timecode — not what lands under the mouse.
A carried card scrubs at its press point: the picture and the timecode are the moment that lands.
Found in the first live run (screenshot `01-step-moment`).

### Class walk

The mechanism: a drop places by the start alone, whatever the person aimed at. Checked:
- bin card → timeline track: done here (files, generators, reels by their length);
- file from the system (X11 DnD): no moment exists before the open, the length is not known until it;
  the start at the mouse and the ~3 s mark stay;
- bin card → morpher source (`morpher_editor.rs`): a source substitution, not a placement; the moment
  is carried but not used — a source-in from it is not asked for;
- "Place at the cursor" in the card's menu: places at the playhead, no pointer and no moment; unchanged;
- dragging an event on the timeline: the grab point already stays under the mouse (`DragState.grab`).

## A triple click is one gesture (found-defects 817)

egui counts a click as a triple when the click two before it fell inside twice the double-click
window, and checks the distance only to the last click: the first of the three may be anywhere. The
program widens egui's windows by its busy frame (`KeyClock::widen_clicks`: a double up to 1.3 s, a
triple up to 2.6 s), so on a loaded machine a click on the effects window's "Done" 2.5 s before a
double click on an event made that double click a triple (live `formula`, stage 21's finish run):
egui's `double_clicked()` is false on a count of three, no FX window opened, a region was set over
the event and the cursor jumped to its start. `clicks.rs` reads the frame's primary click once
(`draw_all`) and keeps a count per gesture: a triple only when the click before was itself a double,
otherwise a double when it follows the click before within the window, as egui judges a double.

Class walk — every place that read egui's double or triple click (25): the timeline (event body,
level line, transitions, the markers' ribbon, the morph and camera bands, the track header's
rename), the node graph (canvas, node, chip), the automation graph's key, the fader's reset, the
mask handles, the bin card (a generator's form, the click that is not a double), the toolbar's
fields and background, the preview's click/double-click step, the effects window's row reset. All
now read `clicks::double`/`clicks::triple`; the one triple (the event body's region) keeps t927/t928.

The second way a double click was lost (the same `formula` step, red again on a loaded machine with
the count fixed): egui judges a click with the windows set a frame ago, and the frame that read the
first click was slower than the ones the window was measured on (155 ms of the UI thread, then its
present); the second click read a frame later was past it, two single clicks. `raw_input_hook`
widens the windows by the gap since the frame before began, when that frame asked for this one at
once, BEFORE egui reads the input (t3131). A rest's gap never widens them. Each click the UI reads
is a `click` trace record (egui's count, the kept one, the gap, the window): the next such red is
read from the trace, not guessed.

## window-chrome (2026-11-24, v26.10 T5, T9, U10, W1)

### D1. The layer order: nothing of the main window over a tool window

- One layer, `under_windows()`: an egui `Area` of `Order::Background`, shown empty at the start of each frame. The panels
  are the root area of the same order, registered first, so the layer is painted and hit over every panel and under every
  window. An area registered before the panels' root stays under them, so the layer is moved to the top each frame
  (found by live `nestdead`: the rig forgets egui's areas inside a frame, and the title was gone and its crumb dead). Not `Order::Middle` (the plan): egui 0.36 paints a layer it does not know as an area LAST in its order — over
  every window — and has no `move_to_bottom`; an area there would also stand in every list of windows.
- Painted in it: the glow of the preview and of the meters, the project's title with its clickable crumbs, the resting
  status line.
- A LIVE status line (hover, a refusal, a message) stays `Foreground`: it answers the pointer or an action, which may be
  inside the very window that covers the strip; under it, the one place for hints (rule 10) would be silent there.
- The glow is NOT one mesh per thing (the plan): the meters' inner glow lies under their bars and the halo over the
  neighbouring panels; one mesh cannot be on both sides of the bars. His sentence asks why the halo shows and the body
  does not; now neither does.
- The guard is the closed list of `no_teaching_layer_over_the_ui` (t988): each row with its reason, and a row no source
  uses fails too. No separate t3935.

### D2. A maximized start (T5)

The mechanism (reproduced live under openbox): eframe saves at exit the state AND the size a maximized window has, the
screen; at the next start winit sends the maximized state before the window is mapped, and the window manager drops it
(X11). The window comes up the size of the screen and NOT maximized: a drag moves it at full size (his "в том же
размере"); maximized for real and dragged, it goes back to its "normal" size — the screen — and only the frame shrinks.
- `window_frame::note_normal` keeps the last size that was not maximized in egui's saved memory (written when it changes);
- `window_frame::adopt_start`, once, on the third frame: a window over the whole monitor that is not maximized gets its
  normal size (or 80 % of the monitor), centred, and then the maximized state, so the window manager knows both;
- `restore_window` sends the normal size when the window has not been normal in this run, and a drag places the window
  under the pointer;
- the bottom toolbar's strip had a drag of its own that sent only `StartDrag`: a maximized window moved at full size.
  It takes `window_drag_from` now. The restore button and the double click take the same `restore_window`.

### D3. The toolbar's wheel (U10)

Over the toolbar strip, a wheel delta (y, or x with Shift) moves a horizontal offset of the row by the delta,
clamped to [0, need − avail]; the transport and the timecode (the groups that never leave the row) stand, the groups
after them slide under, the clipped edges fade over 12 px; the "more" menu holds what is not wholly in view. The offset resets when the window grows past `need`. No scroll bar (rule 13).

### D7. Class walk — "the main window's drawing over a tool window" and "a title that names an id"

| Place | Order | Over windows today? | Here |
|---|---|---|---|
| preview glow | Foreground overlay | yes | D1 |
| meters glow | Foreground overlay (halo) + panel (body) | halo yes | D1, one mesh |
| status hint | Foreground | yes (the bottom strip) | D1 |
| toolbar title text | toolbar row | yes (T9) | D1 |
| timeline drag ghosts, insert marks | Foreground areas (`timeline.rs`) | during a drag only — correct | listed in the guard |
| lesson outline | Foreground | intended | listed |
| hint popup / tooltips | Tooltip | intended | listed |
| event window titles | `TITLE` with `#N` | — | D6 |
| transition window title | `TITLE` with an id | — | D6: the events it joins |
| track window title | the track's label | fine | — |
| morpher window title | `TITLE` with an id | — | D6: the row and the start |
| track window, camera window | the track's label, the camera's name | fine | — |
| drag of a maximized window | header strip, top strip, window buttons, bottom toolbar | the toolbar's own drag did not restore | D2 |
