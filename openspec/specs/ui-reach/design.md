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

## View → FFmpeg… (windows-build, 2026-10-26)

The FFmpeg window (`kt_ffmpeg` in the WINDOWS registry: per machine, outside any project,
reachable when nothing plays) shows where FFmpeg was found and why each other candidate was not,
and offers Download (Windows; the source, size and GPLv3 are shown next to the button before the
click), Locate folder… and Rescan; at startup without FFmpeg it opens once with one notice — a
notice without buttons would be a dead end (rounds 33–40). It sits under View because the program
has no Help menu, next to the other per-machine items (Language, telemetry). A late load brings
placeholders back through `poll_returned_sources`, without a restart (t850, real loader, on Linux
and under wine). The UI language on Windows comes from `GetUserDefaultLocaleName` when no
`LC_*`/`LANG` is set (t861).

## The FFmpeg window's three states (ffmpeg-download-window-info, 2026-10-29)

**Before** the click the window says what Download will do: the archive, its size, the host and
the mirror, the start of the checksum (the full value in the hover — the body must not widen the
560 px window), the folder it goes into and that deleting the folder removes it. **While** it runs:
the host and mirror in use, the bytes with the speed and the time left — computed by the window from
its own samples over the last 5 s, a dash when the bytes stop for 2 s (an average from the job's
start would read low exactly after a mirror switch), and the mirrors already tried with their
reasons. **After**: where it was installed and that the SHA-256 matched, or every mirror's reason.
**Open folder** is one helper (explorer / xdg-open, detached, never waited on) with a test seam; it
shows the installed folder, else a loaded FFmpeg's folder, else the download root — and is hidden for
a system SONAME, which has no folder. t948–t953, live `ffmpeg_download` (its success state is t952's:
the scenario cannot build a 7z).

## The welcome window and the lessons (welcome-window-and-tour, archived 2026-10-30)

Moved here from the archived change's design (its task 7.2).

### D1. Revision: a start window is taken (F-HLP-027 verdict)
F-HLP-027 decided "a start screen as a separate window — no: at launch we do not have an empty window
but a drawn editor, and a modal over it violates rule 13". Revision: **the welcome window is taken,
switchable, and the former start stays one checkbox away** — reason: the owner's direct request of
2026-10-28; the note's own concern is answered by (a) opening over an EMPTY project only (a project
on the command line or a recovery skips it), (b) the primary tile making the common case one Enter,
(c) "Show at start" off restoring exactly today's silent open, named in the window and in View (UI
rule 19, "say where to get it back"). The note's smaller half (a visible recent list) is delivered
inside the window.

### D2. Revision: lessons in their own view are taken; the layer stays rejected (F-HLP-028, ui-reach)
`openspec/specs/ui-reach/design.md` "Rejected and why" names three stops for a tour. Revision:
**lessons in a docked side panel, steps closed by predicates over the project's state, are taken;
coach marks over the interface stay rejected** — reason: the owner's request of 2026-10-28, and each
stop is answered without weakening it: (1) repaint — nothing animates; a predicate is re-evaluated
only when the model revision or the open-window set changes, never per frame at rest; (2) "one hint"
— the panel is a panel (`SidePanel`), not a layer; the new guard `no_teaching_layer_over_the_ui`
turns the closed list of layers above `Order::Middle` into a test (t988), which also closes the hole
F-HLP-028 found in t83; (3) "nothing to aim at" — a step does not aim at pixels: it closes on the
project's state, and "Show" names a registry id (a window of `WINDOWS`, a panel, a menu path of
catalog keys). The addressee objection (the owner's complaints came mid-work) is answered by the
Guide window (D9), which serves the middle of work; the lessons serve the first start.

### D3. The welcome window is a registered window inside the main viewport
`kt_welcome` joins `WINDOWS` ("start: templates, recent projects with posters, the tour, what's new —
the first second has no project yet"). It is an `egui::Window` centred in the main viewport, not a
second OS viewport: F-HLP-027's ADDRESS line found that a second top-level window brings tiling-WM and
Alt+Tab defects (kde-invent #2124, kde-bugzilla #513581), and the detached preview is the only one we
have. It obeys the form contract (t779: the body scrolls, the actions stay outside the scroll).

### D8. The lesson project is a program-owned copy built from generators
`demo_project` moves from `src/apps/kt-cli/src/main.rs:1167` into a shared module the GUI can call
(kt-model, as a pure `Project` builder: it needs no media, only generator media `gen:<kind>`), in a
generator-only form, so every lesson project opens with real thumbnails and no missing sources (the
F-HLP-029 defect "eight nonexistent media" does not apply). Each lesson writes its copy to
`<data_dir>/lessons/lesson-<n>.kt` when started; "Start over" rewrites it. kt-cli's `demo` command
calls the same builder (one body, UI rule 17). F-HLP-029's "a copy in the folder the user named"
stays the rule for a future "Open demo project" — a lesson sandbox is not the person's project and
is not listed in Recent.

## Selecting segments is said where the person acts (segment-shape-fields, 2026-11-01)

The segment selection has no button of its own, so it is said in three places: the graph's hover
hint over a segment names Ctrl+click, Shift+click and the sweep; the row under "Shape of all
segments" names the segment's bounds or the count; the segment menu's header names how many
segments the choice touches. The row's probes are `shape_sel.*` — the prefix has no dot on purpose:
the live-coverage guard reads every dotted literal as a place, and a bare prefix is not one. The
graph takes the wheel for its own axis, so a live scenario scrolls the window body over "+ curve",
never over the graph (`src/scripts/live/segfields.py`).
## Lessons that point (tour-lessons-fixes, archived 2026-11-01)

The owner's lesson complaints (kkt-26.11.01 items 2–8, rows K2–K8): a step counted late, text
that ran together, targets nobody showed, a Show that did the step, a transitions lesson that
taught an overlap, effects with no visible result, a silent render wait.

### D1. The re-check key is the predicate's inputs, a `Copy` value
`Observed` was `{ open: Vec<&str>, render_done }`, rebuilt every frame (`open_window_ids` allocated a
`Vec` per frame — an invariant-12 defect of its own), and the key was `(model_rev, Observed)`. The
region and the loop switch live in the project but are set by `set_region`/`set_loop_enabled`, which
deliberately do not bump `model_rev` (R.43 item 296). The key becomes
`(model_rev, region, loop_enabled, open-window bits: u8, render_done)` — all `Copy`, compared every
frame, nothing allocated. Rejected: bumping `model_rev` on a region change — that is exactly what R.43
removed (a full renderer rebuild per region edge). Rejected: re-evaluating every frame without a key —
cheap on the sandbox, but the panel can stay open over the person's own project of thousands of events.

### D2. Text: lines of catalog keys, parsed by the Guide's inline parser, memoized per language
A step's text is `&'static [Key]`: each key is one line, a paragraph or (prefix `- `) a list item, ≤160
characters (the catalog ceiling, `MAX_VALUE_CHARS`, which exists because a reordered Arabic paragraph
cannot be re-wrapped). Inline markup is the Guide's subset (`guide::inlines`): `<kbd>`, `**bold**`. A
line may carry `{0}`…: the step's `args` substitute the catalog label of the control (`Arg::Label`) or
a menu path built from keys (`Arg::Path`, joined by ` › `, Arabic ` ‹ `). Arabic: the catalog table
holds visual order (reordered by `build.rs`), which would scramble the tags, so the lesson parses
`ar_logical` and reorders each run's text afterwards (`bidi::visual`) with the run order reversed — the
same atoms the build would produce for a one-line label. The parsed lines are memoized in `LessonsUi`
by `(language, lesson, step)`; the draw path reads the memo. Rejected: `\n` inside one catalog value —
it breaks the 160 ceiling and the Arabic reorder per line. Rejected: a new markup — the Guide's is
already tested (t989) and the owner pointed at it.

Convention: every key and every button or menu item a person presses is a `<kbd>`; names of things
(an effect, a marker, a track) are bold.

### D3. Targets are data; each draws itself through one thread-local mark
`Target` becomes `Menu(path) | ClipMenu{clip,item} | Clip(id) | Marker(name) | Ruler | Transition |
Morpher | Control(name)`, and a step has `targets: &'static [Target]` (the first one is where "Show"
goes). Each frame the panel publishes the current targets into `widgets::lesson_mark` (a `Cell` of a
`&'static` slice — no allocation). Menu items and controls are drawn deep inside closures without
`&mut KtApp` (`gen_ui::generator_menu`, `fx_registry`'s picker, the `+ curve` menu): they call
`lesson_mark::item(ui, key, rect)` / `lesson_mark::control(ui, name, rect)`, which draws the outline
only when that key/name is current — so the generator menu is outlined wherever it is drawn (File
today, Edit and the track menu after `generators-placement-and-audio`). The timeline targets are
computed in `lessons.rs` from `tl_geom` (the geometry the timeline already publishes for mapping) and
drawn on the background layer after the panels, clipped to the tracks. Every drawn outline records its
rect in the same thread-local (reused buffer), which "Show" and the tests read.

`Control` names are a closed list (`lessons::CONTROLS`); t1092 checks that each is marked by a
`lesson_mark::control("<name>"` call in the production sources — a renamed button cannot leave a dead
step. Rejected: outlines through `field_probe` — it is off outside `KT_GEOM_DUMP`.

### D4. "Show" scrolls, opens a menu root, never performs
Per target: `Menu` opens its root popup (each root's id is stored as
`("kt_menu_root", root_key)`, not only File's); `Clip`/`ClipMenu`/`Marker`/`Transition`/`Morpher`
centre the view on it (`center_view_on`) and scroll the rows so its row is visible; `Ruler` is always in
view; `Control` asks its widget to `scroll_to_me` on the next frame. A `Control` inside a closed window
was not drawn last frame — "Show" then goes to the step's next target (e.g. lesson 4 step 2:
`[Control("fxreg.add"), Morpher]`). The former `lesson_show` opened the fx editor, the morpher window
and the render settings — exactly the steps; all three are gone.

### D5. Lesson 3: the right-click item; the step stays on `p.transitions`
Text: right-click the second event → "Transition with previous" → a kind. Targets:
`[ClipMenu{clip: 3, item: TRANSITION_PREV}]` (clip 3 follows clip 1, butted at 0:12, in every fresh
lesson copy). The predicate already reads `p.transitions`; an overlap made by a drag adds none, so it
cannot close the step — the overlap = cut rule is untouched. Step 2 (another kind) targets the
transition on the timeline and the transition window's kind field.

### D6. The result line and the playhead: `Step::result` and `Step::look`
A step may carry `result: Option<Key>` (the line shown under it once checked, until the next step
closes) and `look: Option<Look>` (`ChainWith(tag) | Transition | Morpher`). When `lesson_tick` records
a step done during the tick (not on a reload of a finished lesson), it seeks the player to the look's
moment — the middle of the first event with that effect, the middle of the transition window, the
morpher's start + 4 s clamped into its span — and centres the view there. The seek is the program
showing the consequence; the person made the edit. Rejected: a third "watch it" step closing on
playback — it would close on the playhead, which moves every frame, and adds a gesture the owner did
not ask for.

### D7. Jokes: a catalog set, rotated on the UI clock, one repaint per switch
Eight keys `lessons.joke_1..8`. While step 5.2 is current and `render_job` runs without a result, the
panel shows `joke[(t / 6 s) % 8]` with `t` the egui input time (UI-only, never an offline path, so
invariant 11 is not touched) and calls `request_repaint_after` for the next switch (the render progress
already repaints; this only guarantees the switch). Rejected: random order — a test could not tell
"changed" from "the same by chance".

### D8. The panel is drawn after the timeline (found by the live walk)
The first form drew the lessons panel BEFORE the timeline so it spanned the window's height. It then
narrowed the timeline AND its bottom toolbar, a fixed left-to-right row clipped at the panel's
edge: in a 1440 px window the Loop button lesson 1 names (and Undo, Redo, Render, Search) was not on
screen. The lessons point at the timeline more than at anything else, so the timeline keeps the full
width and the panel sits beside the preview — it still narrows the editor and still adds no layer.
The harness missed it: a widget's recorded rect ignores the clip, so the guard (t1094) compares the
outline with the timeline panel's visible width. Rejected: making the toolbar row fit beside the
panel — a wider layout change of shared code, while the lessons need the timeline whole anyway.

### Risks / Trade-offs

- **The CJK/Arabic font subsets** must be regenerated for the new catalog text
  (`scripts/subset-fonts.py`); parallel branches touching catalogs will conflict on the binary subsets —
  resolved by re-running the script after the merge (noted in the hand-over).
- **Shared files**: `toolbar.rs` (menu roots, two marks), `timeline.rs` (one mark), `fx_registry.rs`
  (one line), `render_dialog.rs` (one line) — other executors edit those files; hunks are kept to
  one or two lines.
- **The bottom toolbar row is ~1460 px wide** (found by t1087 at 1440×860, with or without the
  lessons panel): below that width the Render button at its right end is cut. Not this change's
  layout: the lesson tests run at the owner's 1920×1080, t1094 holds the Loop button at 1440, and the
  row's own width is a follow-up for the window-layout work of this round (kkt-26.11.01 item 12).
- **Arabic run order** for wrapped lines stays as imperfect as every wrapped Arabic label today (the
  build-time limitation named at `MAX_VALUE_CHARS`).

## Windows that fill their area (windows-fill-their-area, archived 2026-11-01)

The owner, kkt-26.11.01 item 12 (row K12): "a needle's eye … and so everywhere … I am tired of
reminding". The body was capped at 0.62× the screen and shrunk to its content, half of the windows
were not resizable, and every earlier fix was of one window. One rule in the shell and one guard
over all windows (t1125–t1131); the full decision record is the archived change's `design.md`.

### Why the body must FILL, not only be allowed to
egui 0.36 sizes a `Window` by its content (`Resize::end`: the frame is `last_content_size`;
`desired_size` is only the room offered). A resizable window whose body shrinks to its content cannot
be made taller at all — the paste-prefix and preset-name dialogs grew in width only. So every window
draws its content through `widgets::window_body` (action row below) / `window_fill` / `window_log`,
and the body's height is the window's room minus the action row MEASURED last frame
(`fill_probe::measure_tail`), not a constant: a constant that differs from the real row by a point
moves the window by itself right after opening (t918).

### Fit then fill
A window opened with `.width(w, min)` has no stated height: for its first two FRAMES (counted with
`cumulative_frame_nr` — egui's sizing pass runs a window twice in one frame) the body fits its
content (`min_scrolled_height` = the room, `auto_shrink` on height), then fills. A fitted window's
minimum height must stay UNDER its content, or it opens fitted and jumps to the minimum. Rejected:
"fit until the size is stable" — it grew the welcome window frame by frame.

### Size memory per family
Per-object windows (effects, morpher, transition, camera, track) have per-object ids, so egui's
persisted resize state remembers only one object's window. `.remember(family)` keeps a persisted
`("kt_win_size", family)` (egui memory, eframe persistence — no new store), the window's OUTER rect
size from `AreaState` (it is what `default_size` takes). It is written only when THIS window's size
changed and never during its first two frames: two windows of one family open at once (creating a
generator opens its effects window) otherwise overwrote each other every frame. Rejected:
`user_store` (two truths about one size egui already persists).

### Fixed sizes need a reason (t1127)
A screen fraction, `max_height(<n>)`, `set_width(<n>)`, `set_min/max_width(<n>)`,
`desired_width(<n>)` in kt-app window code fails t1127 unless `ALLOWED_FIXED` names it with a reason
(a numeric field sized for its longest value, a thumbnail cell). The table may not hold an entry the
code no longer has.

### Guide tables and cards
Column widths come from the content: a column's minimum is its longest unbreakable word (a `<kbd>`
cap or `code` span glued to its neighbours), its natural width the unwrapped cell; the room left
over is shared by water-filling in proportion to `natural − minimum`, so the keys column keeps its
caps unwrapped and the action column takes the rest. Rejected: `egui_extras::TableBuilder` (a new
dependency, and its widths are declared, not measured). "I want to…" entries are framed cards, the
whole card clickable (rule 1); the index side is `clamp(0.3 × width, 240, 420)`.

### The picker gallery
One scroll; width = the host window's clip width minus the menu margins (floor `GALLERY_W`), so it
depends only on the host and typing cannot move it (the R.54 rule kept, its constant revised);
height = the screen below the button, stated with `min_scrolled_height` (a popup area's own room
stopped the gallery at 400 points). Effects are cards of `EFFECT_CARD_W` allocated with their width
up front (`allocate_ui_with_layout`): a wrapped layout wraps a child only by the size it asks for,
and a bare `vertical` ran past the edge and widened the popup to the screen; `Align::Min` keeps the
card tops aligned (`horizontal_wrapped` centres them).

### Found while applying
- A resizable window's edge catches the pointer a few points outside its frame: the message log,
  auto-placed in the top-right corner, covered the program's own window buttons (t129) — it opens
  below the title row.
- The render window's finished actions stood where Cancel was (t957, trick 3); the old window grew at
  the finish and hid it. They stand at the far right.
- A `ScrollArea`'s `inner_rect` can be taller than what it allocated (its default
  `min_scrolled_size` is 64): the body rect is the space it took (`ui.min_rect()` after it).

### Masks where a person looks for them (shape-masks, 2026-11-01)

The owner, kkt-26.11.01 line 55: "do not throw features anywhere". Three existing places, one widget
(`masks.rs`): the Geometry node's form (the picture — AE's layer masks, Premiere's Opacity), under a
slot's parameters (Premiere's per-effect masks), the track window (a Resolve window on a node), plus
the preview's right-click menu — on the picture itself. The group is collapsed while it is empty, so
the automation below it does not move out of view (t749/t750 caught it when it opened by default).

Found while building: with the R.64 frame on (the default since R.67) the frame covered a full-frame
picture and took the right click — the preview's menu never opened over an event. The menu body is
one function attached to the picture, the frame and the mask editor.

## The start window in panes and File › Preferences… (welcome-window-and-settings, archived 2026-11-02)

kkt-26.11.01 items 1 and 13 (registry rows K1, K13). The owner: the welcome window was "ugly", a
needle's eye that did not use its area, and the start switch stood three times — in the window, in
View, and as a different label for the same value; "it is time to form a settings section in File,
View is a mess of everything".

- **How others do it.** Resolve's Project Manager, Premiere's Home, Vegas's welcome dialog and
  Blender's splash all lay the start out in COLUMNS by kind of action (create · reopen · learn/news),
  each column scrolling alone; Premiere and Blender keep the "show at start" switch with the other
  preferences, not in the window. Vegas and VS Code put it in both places — the duplication the owner
  objected to.
- **Three panes, each its own `window_body`.** Start (the tour, templates drawn with their frame
  shape, the five lessons with progress), Recent (a poster grid whose column count follows the
  width), What's new (the page from the running version down, `guide::whats_new_from`, drawn by the
  Guide's renderer, headed by the newer-release line when the update state already knows one).
  Rejected: tabs (the owner wants to SEE What's new beside the start actions), one column (the
  needle's eye).
- **Sized, never fitted; placed centred ONCE.** A fitted window once grew to 2010 points from its
  own content (t986). `.size([1180,720],[820,460])`, bounded by the screen. An ANCHORED window is
  re-centred by egui every frame: once it became resizable its corner ran half as far as the
  pointer (the live step dragged 600, it grew 260) — `default_pos` instead; t1290 asserts the corner
  lands at the pointer.
- **No `ui.separator()` between the panes**: in a horizontal row it takes all the height left,
  pushed the action row down and grew the window by itself (t918) — `pane_gap` reserves the width,
  the rule is painted over the row's height.
- **Preferences are named by the industry** (rule 21): Resolve, Premiere, Vegas, Blender and AE say
  "Preferences"; "Settings…" next to "Project settings…" reads as the same window twice. Russian
  keeps the owner's word: «Настройки программы…» beside «Настройки проекта…».
- **One switch, one place (t1294).** Each preference has exactly one writer and it is in
  `prefs_window.rs`; View and the welcome window hold none. Rejected: comparing checkbox labels —
  the original defect had two DIFFERENT labels for one value. The welcome window keeps a
  "Preferences…" door, whose hint says the start is switched there (rule 19).
- **Revision of welcome-window-and-tour D1/D11** (the switch in the window AND in View, "said where to
  get it back") — the owner's new request names exactly that duplication.
- **Language and Economy UI moved into the user store** (`start.json`): as `KtApp` fields they died in
  `replace_with`, so File → New forgot the chosen language (t1295). The former eframe `lang` is read
  once when the store has none, so the first start of this build keeps the person's language.
- **Not added:** a GPU/CPU preference and an autosave interval — neither is exposed today
  (`KT_AUTOSAVE_SECS` is a test seam, the GPU fallback is automatic); adding them is a feature
  without a request.

## A window whose content comes later is sized (readme-shots-showcase, archived 2026-11-03)

The project search was a `.width()` window since windows-fill-their-area: the shell fits such a
window to its content for two frames, then fills that size. At opening its content was the empty
field (71 points), and the finds that arrived with the typing were laid out below its edge — Ctrl+F
showed nothing until the person dragged the window. Found by the README pipeline (its fixture runs
had been writing a 2 KB `search.webp` that nobody looked at). The window is sized now (460×320,
minimum 320×120), t1414. Rule for a new window: fit only a window whose content is complete at
opening; one that lists results, finds or anything that arrives later opens sized.
