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

## Deferred

- `lane_names` live in `Project.ui`, and `swap_reel` does not swap them — the full entry
  shows the root names on the reel's tracks (a known defect, capability
  `compositions`).
- Stabilisation of the XTest gestures of the live scenarios (flakiness in software rendering) — a separate
  piece of work, not done.
- The notes `docs/features/ui/*` (keys, layouts, the system file dialog,
  clip colours, the viewer's on-screen controls) — status "candidate", not part of the contract.
