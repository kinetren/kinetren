# Reachability of the interface — why it is this way

## What hurt and for whom

The complaint "you can create it, you cannot manage it" came from the owner for seven rounds in a row:
"I don't understand how to delete a saved area region… RMB gives nothing", "I cannot add a morpher event
to a morpher track in any way", "I don't understand how the camera track works", "in the fx-chain widgets
it is awkward to activate parameter input", "the whole chain falls apart along its length". And most importantly —
not about the code: "I am tired of reminding the agents to think about these details. FORCE THEM TO CHECK THIS".

The honest answer: the checklist in `src/apps/kt-app/CLAUDE.md` already forbade three of these complaints
word for word, right next to it stood "a repeated remark = the rule is not written down" — and it did not help.
Prose drops nothing. Proof of the failure mechanism: four commands of named areas were implemented, covered by
unit tests of the model, and were never called from the application; the tests checked the model, not reachability.

The first wave of tests (t252–t255) also did not work: the next round was titled "you didn't check shit at all", and all
eleven complaints were correct while `cargo test` was green. The tests checked the REACHABILITY of what was made (the item
is in the menu — in the 34-px header of the row, where it was put), and not the consequence from the point where a person
really presses (on the track).

The third pain — hints. The cry "WHERE DID THE BUTTON HINTS GO?!" and "I don't understand how to
delete" were not one problem solved halfway, but two: the status bar is a pull channel (hover — learn), while
"I don't understand how…" is a failure of the push channel: you cannot hover the mouse over what you did not find.
Seven of the nineteen complaints of the round of dead ends lived only there. Hence the wave "the right click never stays silent":
the reconnaissance found twelve objects with a hint and no menu, and two hint promises that were never kept.

## Decisions and why

- **Reachability rules break the build, they do not ask.** `t252` parses `enum Command` FROM
  THE SOURCE (a list in the test would be a second place of truth) and requires its construction in
  the product corpus. `TlEntity::ALL` — a literal of fixed length in the product code:
  add an entity — the whole application will not build until the row of verbs is filled in.
  `CtxTarget::entity()` — an exhaustive `match`: if you create a menu target, you must name the entity.
- **Verbs — four, and an absence is paid for by a reason.** Create/Rename/Delete/Open;
  `Reach::Absent(reason)` and `Reach::Elsewhere(link to a test)` with a length check —
  otherwise "no" and "somewhere over there" turn into the same prose checklist. `Open` was introduced for
  nested reels: without it there was nowhere to record "enter", and entering is the only thing by which a nesting differs from a clip;
  the verb turned out to be meaningful for the former entities as well.
- **The test asserts the consequence and probes from the point of a person's action.** `scene()` gives
  a LIST of places per entity (the row header AND the track), its own scene for each place. On the new table
  the test at once named exactly the two dead ends about which the owner had complained.
  "The field got set" is not a consequence; "the rectangle grew and a click below the old border hit it" is a consequence.
  A live scenario per item is the closing condition, because the harness proves nothing about the real window
  (the first 16 px of the gutter are the window resize).
- **One interaction instead of `dnd_drag_source`.** An idiom already accepted in five files of the repository: the label is drawn,
  over its rectangle (extended by the grab zone) — `Sense::click_and_drag()`; the prohibition is closed by a one-line scan of the whole application.
- **One truncation helper for all names.** Otherwise half of the places drift apart again: the same name drew the source field of
  the morpher, and it too stretched the window.
- **A second registry of targets for panels, rather than extending `CtxTarget`.** The timeline targets have a common scene (one strip,
  one hit test by position, one menu at the end of the frame) and a common kind of identifier (time and track); the panels have neither —
  the slot lives in the FX window, the card in the bin, the fader in the meters. In one enum half of the branches would never meet
  in their own place. The targets cancel each other out: the menu is one, and a stale timeline target would give a green result on a broken one.
- **The right button is safe for probing.** A curve key is not deleted by a right click — only by a menu item; a destructive RMB would devalue the promise
  "the right button can be used to try" in all the other places (the owner's decision, fixed after the round).
- **A transition gets its menu by the WINDOW, not by its handles.** The handles are about the gesture (±6 px at the joint),
  the right click targets what is VISIBLE; the window is drawn over the neighbours — so the menu is given by it too (the rule "the transition wins the pixel over the clip",
  already so in hover and press).
- **The chord is an argument of the item, not part of the phrase.** The class is cured, not the instance: one label "Delete" served three contexts, and in one
  of them the promise was false. It is easier for the translator — the chord is not translated, and was edited in five catalogues.
- **The menu of a row lives in one place and is opened from two.** `service_lane_menu` /
  `clip_lane_menu` / `empty_place_menu` are called both from the header and from the track; a second copy was deliberately not created — two sources of truth are exactly the defect that was burned on.
  The same principle with "+M": the place of a block is computed by one `morph_span`, it is called by both the button and the menu item; the four paths into the lane — one body.
- **One door of publication — a label of the field inside the lane.** Reconnaissance: in none of the four editors with this mechanism is there a command "show outward everything interesting";
  the interface is assembled by hand where the author looks at it. Outside the lane there is no item at all, but not silence: the status bar explains where it lives.
- **One hint, in the status bar; the push goes to the second surfaces.** The word "tooltip" glued together three decisions: the overlap and the delay were accepted by the owner word for word,
  the capacity/direction were decided by no one. The verdicts of the reference wave look for a second surface
  (the context menu, the label itself, the place where people come on purpose), not for a return of popups.
- **The impossibility of an action is an error with an address.** The obstacle is highlighted, the error style is on top of the hover, beyond the view's edge — the side, the distance and an arrow; a third part
  is absent in any of the five core editors, therefore our Move gesture is the reference.
- **Scanning guards of "one home".** A promise in a comment ("the rule of pieces is ONE"), which there is nothing to check it with, stops being kept. The technique of t252 is reused for
  `source_in`, the search for media from a clip, the sender to the player, the offline render of reels and the floor of the time of a gesture.

## Rejected and why

- **A guided tour / coach marks over the interface — not taken.** Three stops: (1) the invariant "repaint only on
  movement" — a pulsing callout would become the sixth cause of 60 fps in the closed list `animating`; (2) the rule "one hint"
  is bypassed without breaking it: a callout on `Order::Foreground` does not trip t83, which catches only `Order::Tooltip` —
  the worst outcome, an overlap under a green guard; (3) there is nothing to aim at — neither a registry of
  commands nor stable ids, only rectangles recomputed every frame. And the addressee is wrong too: the complaints "I don't understand how…" were written by the owner in the middle of work, while the tour speaks to a newcomer on first launch.
  None of the five core editors has a live tour; Ableton, which chose the same architecture of hints, made a separate view rather than a layer.
- **Bringing back pop-up hints — not taken.** The overlap and the delay are the owner's direct words; the capacity of the row does not grow by a single character,
  because it cures none of the seven push complaints.
- **A destructive right click on a key (as in Resolve) — rejected** for the sake of the promise
  "a right click is safe to try" everywhere.
- **One enum for the targets of the timeline and of the panels — rejected:** half of the branches would never
  meet in their own place.
- **A list of commands inside the test — rejected:** a second place of truth; the source is parsed instead.
- **A submenu of transition kinds in its menu — rejected:** the kind is changed in the properties window, where the curve and the sides are visible next to it (the rule "nothing superfluous").
- **Checking "what is visible" by the accessibility tree — rejected:** it answers "what is declared". The label of a button is taken from the source text and does not contain the ellipsis; the accessible name of the reset button is compound and carries the name of the handle — the first version of t684 was green on code that drew the catalogue row. Assertions about what is visible are made by the galleys.
- **A second copy of the row menu in the header — rejected:** two sources of truth.

## Non-obvious details

- **t252 lies without four subtleties** (`src/apps/kt-app/src/reach_tests.rs`,
  `command_variants`/`production_corpus`): word boundaries in the search for `Command::X`
  (`SetFx` is a prefix of `SetFxOff`, and `egui::ViewportCommand::Close` contains `Command::Close`);
  the second level of the corpus — the bodies of the functions of `kt_model::ops` that the application calls
  (`LinkClips` constructs only `ops::magnetize_commands`); the corpus is taken from
  `font_tests::sources()`, which cuts the file at `\n#[cfg(test)]` — test code is not counted as reachability;
  `NOT_FROM_UI` starts empty and is held by three asserts.
  The limitation is recorded honestly: the test proves that the construction line EXISTS,
  a command in a dead branch will pass.
- **Two traps of the harness of t253** (`src/apps/kt-app/src/reach_tests.rs`, `scene`): Esc
  clears both the region and the selection — the shared harness silently broke the scene of the next entity,
  therefore the scene is its own for each place; a right click with a menu open CLOSES it rather than opening its own — the branch "no target" went to `continue` past the closing.
  Items are counted by the increase in occurrences of the label: for tracks the delete label carries a counter.
- **The first `WIN_EDGE` px of the gutter are the window resize zone** (`src/apps/kt-app/src/timeline.rs`
  and `src/scripts/live/deadend.py`): a press goes to the window manager. The gutter is
  34 px wide, a working strip exists, but both the harness and the scenario must aim at the middle —
  otherwise "green on the broken" turns into "red on the working". The zone is taken from the point of the PRESS
  (`press_origin`): `drag_started` arrives after the threshold, the pointer creeps off the edge, and a height gesture silently became a reorder.
- **The geometry dump — coordinates RELATIVE TO THE WINDOW, XTest moves the mouse across the SCREEN**
  (`src/scripts/live/kt_live.py`, `x_at`/`lane_y`): one's own formula over `row["top"]` clicks into emptiness and looks like "the feature does not work in the live window". The scenario first proves that
  the gesture HAPPENED (the first revision of `src/scripts/live/keys.py` was vacuous: the clip was at zero, the edge was not grabbed, and "the keys did not shift" was passing by itself).
- **`dnd_drag_source` kills the click on the device of egui 0.36**
  (`src/apps/kt-app/src/widgets.rs`, `SLOT_GRAB_PAD`): it registers a `Sense::drag()`
  rectangle over the content and after it, and `hit_test` returns `click: None`. The scan of t254b strips comments, otherwise it catches its own explanation.
- **`elided_name` returns a READY galley** (`src/apps/kt-app/src/widgets.rs`,
  `elided_name`, `SLOT_NAME_CHARS`): a `LayoutJob` with `max_rows: 1` and `overflow_character`,
  laid out in advance. A `WidgetText` made from a `LayoutJob` would have been fitted by the widget to the available width and would have returned a wrap.
  The reason for the wrap by letters: `horizontal_wrapped` sets `TextWrapMode::Wrap`, and a file name has no spaces.
- **The chord via `\t` is not a column** (`src/apps/kt-app/src/widgets.rs`, `menu_item`):
  in epaint a tab is four spaces, and the chords stand in a staircase; the real column
  (`Button::shortcut_text`) was not taken — deferred. For a clip the chord Del is promised only without a region: with a region, Del cuts the region.
- **The background of the bin is caught by the rectangle of the PREVIOUS frame and specifically of the bin**
  (`src/apps/kt-app/src/bin_panel.rs`, `bin_rect`): the bin learns its own height only after drawing the cards, while the interaction must be registered BEFORE them (what is registered later lies on top);
  `ui.clip_rect()` inside `ScrollArea` equals the viewport of the COLUMN, and the perf panel lives below the bin. The bin has a free strip at the bottom — on a dense grid
  the "background" physically did not remain.
- **The hit test of a curve key — over the marks that the DRAWING fills in**
  (`src/apps/kt-app/src/automation.rs`, `AutoView::key_at`/`key_marks`): previously the probe lived in the drawing loop and fired only on `interact_pointer_pos`, that is, with the button PRESSED —
  the key was silent both on hover and on the right button. The menu is attached BEFORE the early `return`s in `graph`; the buffer of marks is reused (we do not allocate in the draw paths).
- **The status line is drawn LAST as a separate layer** (`src/apps/kt-app/src/app.rs`,
  `draw_status_hint`, `status_line_shown`): the status bar is the second panel of the frame, the buttons
  are drawn later, and it read an empty `hover_hint`. The test checks what was drawn; the old tests read the field AFTER the frame and were falsely green. `tooltip_delay = 1.0e6` in
  `src/apps/kt-app/src/theme.rs` silences even other people's `on_hover_text`.
- **t83 catches only `Order::Tooltip`** (`src/apps/kt-app/src/layout_tests.rs`): above
  `Order::Middle` live eight named layers (four — input in place, four — the display of state), and none of them explains the interface; a layer on `Order::Foreground` would return the overlap
  under a green guard — the closed list of layers is not yet guarded by a test.
- **The sinks `hint_sink`/`publish_sink`** (`src/apps/kt-app/src/widgets.rs`): inside
  `Window::show` the model cannot be reached — the widget puts the request into the sink, and the application parses it after the frame. The labels of fields give their own rectangle (`lbl.<param>`): the label
  is the target of a gesture, and a `Label` in the accessibility tree cannot be found by the live scenario.
- **The explanation "publication lives inside a composition" — only when there are reels** (`src/apps/kt-app/src/widgets.rs`, `PublishCtx::explain_outside`; guard t518): in a
  project without a single reel the phrase would hang over EVERY label about a mechanism the person did not set up. There is no door where one cannot enter: for a paint title it is not a property — `color_row`
  asks the carrier itself (`value_type(stem).is_some()`), rather than a list of exceptions.
- **The owner reel — by the WORKING VIEW** (`src/apps/kt-app/src/pubparam.rs`,
  `owner_reel_of`): an event of an expanded lane lies next to the root ones but belongs to the reel of the lane; a publication into the "current" one would die in `reconcile_published` in the same batch.
  The confirmation, if needed, ENTERS the required reel by the same body as the verb "enter".
- **The obstacle lives exactly one frame** (`src/apps/kt-app/src/app.rs`, `blocked`,
  `blocked_text`): it is set by the drag handler and cleared at the start of the next pass. For a disabled egui widget `hovered()` is always `false` — the hint about greyness is dead by the device, it must be explained through `contains_pointer()` (not yet done everywhere, see
  "Deferred").
- **The gallery of the picker** (`src/apps/kt-app/src/fx_registry.rs`, `axis_at`, `CELL_DRAW_W`):
  the egui menu default `CloseOnClick` closes the popup on any click inside — `CloseOnClickOutside` is mandatory; the accessible name of a cell is compound (""Blur Dense"") — ""Soft"" exists for six kinds; a cell below the fold of the scroll is not drawn (`is_rect_visible`) — otherwise it publishes a rectangle below the popup and the live scenario clicks past it, into the timeline. The axis of strength
  is computed from the CENTRE of the cell by the absolute position of the pointer; on the release frame `dragged()` is already false — `drag_stopped()` is read.
- **`free_service_lane` searches for a row that is free ON THE INTERVAL** (`src/apps/kt-app/src/app.rs`):
  the first fix always reused the first row and hid a block under a block (the hit test goes from the end of the list). The catch of the frame was a `Vec<ClipId>` — the type itself declared "the frame is about clips".

## R.67: "user path" as part of the contract

- **A rule in prose does not work.** The checklist "UI is made for the user" held since R.8,
  but in R.67 the owner wrote: "HOW MANY TIMES DO I HAVE TO REPEAT THAT ALL ___ALL___ AGENTS SHOULD
  THINK ABOUT THE USER FIRST? … THIS IS COORDINATE-LEVEL, IT NEEDS CHANGING!". Therefore a UI-capability
  scenario with a kt-app test from t783 must have a `PATH` line: the mouse steps and what is visible
  after each one; the guard `specs-user-path.py` is called by test t799 and by `finish.sh`.
- **The exception MUST be MEASURABLE** (the same lesson as the window registry in R.66): a scenario
  without a gesture must state the reason in the line in words of no fewer than forty characters. The guard
  immediately caught two excuses from the author of the rule: "(not a UI scenario: the spec guard)".
- **Screenshots of the steps go into the report.** `kt_live.step()` takes a frame for each step of the PATH,
  and `finish.sh` prints the list of them. The run is on OUR fixtures: "item 1 but not on my
  projects" (the owner).

## R.68: the live check waits for the PROGRAM, not for the clock

- **96 % of a live run was sleep.** Measured by intercepting `time.sleep`:
  `nestparam` — 130.5 s out of 135.9 in 591 calls, `curvesrc` — 41.9 out of 43.2,
  `level` — 4.2 out of 4.7. The pauses were tuned to software rendering ("a frame
  takes ~0.2 s"), that is, the harness was waiting for something the program could tell it itself.
- **A frame is not enough — INPUT is needed.** The program reports both the number of the drawn
  frame and the number of input events it has read: a frame could have STARTED before the event
  reached us from the X server, and then "frame drawn" does not mean "gesture seen". This silently
  lost the level drag (`level`), and in the drag the unread mouse movements were summed into a jump — egui began
  the gesture already at the end point, and the clip did not move at all.
- **"Has reacted" is silence between frames, measured IN FRAME STEPS.** A fixed silence threshold
  declared the program calm in the middle of drawing as soon as the machine became slower (load from the workers).
  The threshold is computed from the OBSERVED frame step — the very property that a fixed pause lacked, and
  because of which things "turned red all at once" (§468).
- **The aim at a popup is read AFTER the silence.** The dump is written every frame, and the first
  frame with an open menu catches it in the middle of an animation: the click landed NEXT TO the item
  (`conform`, `curvesrc`). The probe rectangle is re-read once the picture has settled.
- **egui's memory lives OUTSIDE `KtApp`.** `*self = KtApp::open(..)` rebuilds the
  program's state from scratch, but scrolling, collapsing, window positions, focus and an
  unfinished drag live in `egui::Context`, which belongs to eframe. Without a reset, the third scenario in a row
  lost the drag events. This is the "reset workspace" from the contract of the bench.
- **"Clean" is counted by windows (`Order::Middle`) plus the program's own modals.** `Order::Foreground` is not
  suitable for this: the status line is drawn there too (the rule "one hint in one place"), and "clean"
  turned into "one window open" for no reason.
- **The scenario declares its OWN PROCESS** (`OWN_PROCESS = True`): the start check is recovery after SIGKILL, the question about unsaved changes, the lock,
  importing another version's file. The worker reads the declaration BEFORE launch and shuts the bench down,
  otherwise there are two kinotrain windows on the screen and `probe.window()` aims at the wrong one.
  An environment that is read at process start (`KT_AUTOSAVE_SECS`,
  `KT_FILE_PICK`) cannot be substituted on the bench — a mismatch is red and asks to declare an own process,
  it is not ignored.
- **What is live and what is headless is decided by the report, not by taste.**
  `scripts/live-coverage.py` matches the places the program publishes (named rectangles) against what the scenarios click: at the time of the round — 174 places, 66 touched, 108 NEVER touched. The report
  turns red only for things that are FALSE: an aim at a non-existent place and a spec link to a vanished scenario (a move of a check must CHANGE the address, not drop the scenario). Test t804.
- **The coverage report can lie IN ITS OWN FAVOR, and that is more dangerous than lying against it.** There are three kinds of aim, but only one was counted: a call of the probe by name; parsing of the dump and a click by coordinate (`f["param"] == "bin.reel"` in
  `nestparam`, the speed bar in `rate`); an aim by PREFIX, when the scenario does not know the name in advance (`startswith("auto.add.")` in `formula` — the parameter depends on the project). While only one was counted, the report called
  what was clicked by hand a hole, and ordered a second identical scenario to be written. All three must be counted: the 66 touched places were in fact 86.
- **The probe name in a live scenario is written as a LITERAL at the call site.** The report
  reads the sources and cannot expand a variable: move the name of the item into an argument of a helper function — and the place counts as unclicked, although it is clicked. Caught by the same scenario that was closing it
  (`trackverbs.py`, task 5.3).
- **The move to headless is a CHANGE OF ADDRESS, recorded by the machine.** The `MIGRATED` registry names, for every scenario that moved, the tests that hold its assertions, and the reason; the guard falls over both on a file still left on disk and on a non-existent address. Without the registry, "move to non-gui" and "delete" are indistinguishable by the tree — and that is exactly the substitution against which the rule "tests from the specs are not weakened" protects. In R.68 six scenarios moved this way
  (`nest`, `nestenter`, `nestopen`, `nestzero`, `fxfield`, `curvesrc` —
  175 s out of 1677 s of the sequential live run).
- **The number of holes has a DECLARED RATCHET LIMIT** (`UNTOUCHED_LIMIT`).
  It is neither a target nor a knife: a new place of the program either gets a live step, or the number is moved deliberately — in one line and with a reason. Without the ratchet
  the closed holes would silently open again, as already happened with rules written in prose.
- **A negative check after collapsing waits for SILENCE.** Collapsing a group in egui is animated: while it is moving, the fields are still drawn and honestly publish
  their rectangles, and `has_probe` sees what is already "gone". Before "this is no longer shown" a `settle()` is needed — a relative of the rule "the aim at a popup is read AFTER the silence".
- **A double click on the first second lands on the TRACK LABEL.** The label
  lies over the start of the row, and when the view has moved (the cursor drifted during play), `x_at(1 s)` falls under it: instead of the effects window, the rename of V1 opens. The aim must be at the middle of the edit, not at its start.

## R.69: the wait reached the GESTURES, and what this revealed

- **A rule that was written down but did not arrive is not a rule.** The wait on an observed event was built in R.68 and applied to the PROBES; the gestures themselves
  stayed on the clock, and the measurement showed **539 s of sleep out of a 1587 s run**
  (34 %). The static estimate gave 387 s and was understated by 39 %: it counted the CALL SITES, while scenarios call gestures in loops and repeats.
  The machine must count the sleep, not the eye.
- **Sleep hidden in the driver is no different from sleep in the harness.**
  123 s out of 387 lived in `ui-probe.py`, where the guard did not look, and most dangerous of all they were in the DEFAULT VALUES of arguments: at the call site of a gesture such a
  pause is not visible at all. The scope of the rule is the entire live check.
- **"A reason in words" is a weak measure; sleep has a BUDGET.** All 265 s in
  the harness were legitimate: each pause had an explanation longer than forty
  characters. The limit is the sum of the DECLARED seconds, checked without a run and independent of either the machine or the number of calls. A verdict on the ACTUAL sleep
  would catch the load instead of the regression, so the fact is printed, not judged.
  A hole was found at once: a pause with a computed duration did not count toward it —
  now in the harness the duration must be a number at the call site or a parameter default that is itself computed.
- **A MODIFIER NEEDS ITS OWN FRAME — and so does the keyboard.** The rule had been
  written for mouse gestures (`_mods`) and not applied to `key`: with a zero
  pause, Ctrl and S went to X in one packet, egui read "s" without Ctrl, and
  **Ctrl+S broke silently**. The gesture passed on screen, the scenario read the
  file from disk WITHOUT the edit and blamed the drag for it — an hour and a half of analysis. For the same
  reason, a character typed with Shift (`*`, `+`) drops out of a batch of typed text.
- **`settle()` answers "the picture has settled", and NOT "the gesture has been applied".**
  Silence after a gesture never comes at all: the program legitimately redraws
  under the pointer. Placed after releasing a drag, it burned its own timeout — **10 s on every drag** (+300 s per run) and the same amount on
  every hover (`analysis` 54 s against 17 s). After a gesture one waits for a FRAME;
  silence is requested where it is really expected, and with a short deadline.
- **A double click is a TIMING gesture, and the window is narrow on both sides.** Four
  events arriving in one frame are counted by egui as one click; spaced more than
  0.3 s apart — as two single clicks. When the round removed the driver's pauses, it also removed
  the spacing, and the click began to fail under load (`nestparam` red twice).
  The old 0.09 s came back, but now as named pauses within the budget.
- **The label "how much input has been read" can be STALE.** The dump runs at 10 Hz,
  so what was read in the last 100 ms is not reflected in it, and an increase of the
  counter after a gesture may belong to someone else — to the tail of the previous one. The label is a pair
  (frame, input), and the wait requires a dump that was certainly written LATER than the label.
  Taken singly, both conditions mislead: "input has grown" — by someone else's growth; "frame
  drawn" — by a frame that started before the event.
- **Speeding up exposed what the cost of checking had hidden.** Reading a screen point
  cost 84 ms (a 1920×1080 snapshot plus two ImageMagick processes) and
  gave the program a head start; a direct `XGetImage` costs 0.04 ms — and `gpufx` began
  to read the PREVIOUS frame. The rule "the pixel after a gesture is read by
  `pixel_settled`" had been written long ago; only the slow read was masking it. The direct path is
  accepted only after a cross-check against
  `xwd` on a single frame: the eframe window has a 32-bit visual, and reading the root
  window was already returning BLACK where the picture was (R.33 item 228).
- **The negative check is subtler than the positive one.** Probes accumulate between
  dump records, and collapses and popups are animated: "have the fields disappeared?",
  asked once at random, flickers. For this there is `wait_gone(name)` —
  to wait until a FRESH dump no longer carries the name.
- **The guard also catches the loss of its own author.** A drag body that by chance stayed
  the same after bisection survived three green runs and was found only by the
  rule "every gesture waits for the read input" (t807).
- **A test that breaks the guard breaks it on ITS OWN copy.** t803, t807 and
  t808 check the same script with violations, and corrupting the working tree
  made a neighbour's verdict random — this passed by the luck of ordering and
  surfaced in the first joint run. The guard has a replaceable root,
  and each test has its own copy.

## Deferred

- Verdicts of the help wave not implemented in code: the guard "chord from one source"
  (a chord as data, a real column `shortcut_text`; today `Ctrl+Y` and `Backspace` are not
  named in any catalog); the guard "refusal is an error everywhere" (extend the rule from the Move gesture
  to trim, grey `add_enabled` widgets and rejected model commands —
  `Severity` has no `Error` variant, `TimelineModel::check_on` is private); the guard of the closed
  list of layers above `Order::Middle`.
- Docs-as-code: the pipeline `site/build.py --check` and automatic screenshot capture by the real
  window are built, but `finish.sh` does not call them — documentation production without documentation.
- A learning layer in the form "the lesson checks the executed command" — only after a command
  registry (there is no "command" object, so there is nobody to say "step closed" to).

## The import door has a fourth input: projects of the former name (2026-10-21)

After the rename to Kinetren, the menu item is "Import older Kinetren project…". A `.kt`
file with the former `kinotrain` key is refused by Open, with the former name and the
door named, and it opens through the import like the other old forms (t832). The one
catalog string that names Kinotrain is `former_name_import_only`. The guard t833 keeps
the former name out of everything that does not handle the rename.
