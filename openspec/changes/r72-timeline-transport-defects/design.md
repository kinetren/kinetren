# Design

## Context

Motivation — proposal.md. The facts below were read from the code on 2026-10-30. Paths are under
`src/apps/kt-app/src/` unless named.

- **Space** (`app.rs`):
  - constants `SPACE_HOLD_MS = 220`, `DOUBLE_PAUSE_MS = 400`, `SPACE_BOOST_MAX = 4`, `SPACE_BOOST_SECS = 5`;
  - `handle_keys` reads raw `Event::Key{Space}` and ignores modifiers;
  - `tick_space_boost` starts the boost after 220 ms, and starts playback when stopped;
  - `space_boost_step` is an exponential with no ease-in;
  - `on_space_released` snaps back to the base rate and never pauses. No reverse path exists.
- **Event zones** (`timeline.rs`, closure `clip_at`):
  - order FadeLeft/FadeRight (top strip) → **Level at any x within ±`LEVEL_GRAB_PX = 5`** → TrimStart → TrimEnd → Body;
  - trims shrink with width (`edge_grab_for_width`), Level never does;
  - `press_origin` and the hover/hints use the same zones.
- **Gain**:
  - the event line maps ±`RAIL_DB_SPAN/2` = ±6.02 dB (`clip_level`/`gain_to_level`/`level_to_gain`);
  - the menu field has the same limit;
  - the track fader goes to `TRACK_GAIN_MIN_DB = −60` (`kt-model/src/project.rs`);
  - no badge on events (`row_badges` is for track headers).
- **Range marker**:
  - `marker_end_at` uses `.find` (first, not nearest) and works only on the ruler;
  - below the ruler `region_edge_at` (±8 px, full height) wins;
  - hover checks only marker starts and region edges;
  - the end drag snaps through `snap_targets`, which include the marker's own `end()` and both region edges;
  - a marker made by `marker_here` equals the region and snaps back to `region.end`.
- **Vertical zoom** (Shift+Ctrl+wheel):
  - `tl_scroll = row*new_h − (pos.y − lanes_top)` assumes equal rows, though morph rows are `/1.5`, camera rows `/3`, and there are per-row overrides;
  - `vertical_layout` computes `scroll_max` from uniform heights.
- **Scrollbar** (`timeline.rs` ~4855):
  - 6 px, drag only, not in `no_resize_rects`;
  - flush with the window it sits in the East resize zone (`WIN_EDGE = 8`, corner 16);
  - it lies inside the right jump zone.
- **Jump zone**:
  - `edge_margin = max(width/8, 40)`, at both edges;
  - a click on EMPTY space seeks and jumps the view;
  - the clip branch is tested first, so over an event the click selects and seeks, and a quick
    second click opens the FX window;
  - the left zone starts after the gutter (`GUTTER_W = 4*BADGE_W + 6`). Badges sit at each row's
    bottom-left inside the gutter. Which part of the zone the owner lost is checked live (task 2.4)
    before the fix.
- **Captions**:
  - `tl.lane.add_morpher_here` / `tl.lane.add_camera_here` are drawn in empty service rows (`empty_svc`);
  - the same actions exist in `service_lane_menu`;
  - the hover hints are `tl.hint.morph_lane`/`camera_lane`.
- **Track reorder**:
  - `Drag::LaneRow` in the gutter (press outside the height edge) → `reorder_rows` → `SetLaneOrder`, one batch;
  - no test, no cursor, no hint.
- **Frame step**:
  - none; ArrowLeft/ArrowRight are unbound;
  - Back/Forward → `nav_jump(fwd, false, shift)`;
  - `kt_time` has `frame_step()`.
- **Stall** (`kt-engine/src/player.rs`):
  - `Ev::Stall` when the head stands still ≥ 1 s while playing;
  - `last_head_ms` is not reset after the push, so one freeze pushes a record every pass;
  - it also counts at rate 0;
  - the application shows the count only in the perf box (`perf.flight_stalls`).
- **Freeze**:
  - rate 0 of the SlowedReverb warp slot is a freeze (R.57);
  - there is no command; `Command::Split`, `SetFx`, `SetFxAuto` and the split wrappers in `app.rs`
    (`split_selected`) are the parts.

## Goals / Non-Goals

**Goals:** every R.72 row of the registry closed by a test that probes from where the person acts; each
gesture also in a live scenario with a screenshot per step (rule 26).

**Non-Goals:**
- Splitting "Marker over the region" into "always new" and "stretch the existing" is NOT made. The
  owner did not confirm it (R.71, question 9); the report shows him today's three behaviours, and a
  follow-up takes his answer.
- No JKL shuttle (F-TL-003 stays a candidate): Ctrl+Space and the arrows are what he asked for.
- No change of the boost limits (4× over 5 s — R.16/R.17).

## Decisions

### 1. Space: one press state machine, the threshold 0.3 s

The press records its start time, whether it started from pause, and the Ctrl modifier read at press
time. Before the threshold nothing happens. Past it, the hold mode runs:
- from pause — play (reverse with Ctrl), boost while held, and on release **pause where reached**;
- during playback — boost (reverse with Ctrl while held), and on release decay to the base.

A release before the threshold is a tap: today's toggle and double press, unchanged.

**Revision: `SPACE_HOLD_MS` 220 → 300 ms.** The owner chose "a threshold ~0.3 s" (R.71, question 7).
One threshold serves both "is it a hold" and "start the boost". Two thresholds would give a 220–300 ms
window in which the boost starts and then counts as a tap. Before the change, check `git log -S
SPACE_HOLD_MS`: the 220 is not the owner's number in R.16/R.17 (he set 4× and 5 s). If it were his, this
is a question to him, not a revision.

The **ease-in** is a smoothstep factor on the first second of the rise, multiplying the existing
exponential target. It is still analytic, because explicit schemes are forbidden (ui-timeline, "stable_dt
after idle"). The **decay** is the same exponential toward the base with a time constant of a quarter
of the rise (≈1.25 s to 95 %). The owner: "the reset should be faster than the build-up".

**Reverse release during forward playback returns straight to +base.** A smooth way through zero would
pass a freeze and a direction mini-seek in the middle of the gesture, which is worse than one flip. The
sign change is already a mini-seek in the render thread (transport).

Rejected:
- "always release = stop": the owner chose the threshold;
- a JKL shuttle instead: not asked, and F-TL-003 is a separate verdict.

### 2. Event zones: trims first, level by size

Order: fades (top strip) → trims (outer columns, `edge_grab_for_width`) → level → body. The level gesture
exists only when `row_h ≥ LEVEL_MIN_ROW_PX = 40` and `event_w ≥ LEVEL_MIN_W_PX = 3 * EDGE_GRAB_PX + 2 * LEVEL_MIN_SPAN`
(≈ 60 px). Its grab is `clamp(row_h * 0.08, 3, 6)` px. The constants carry the comment "set per
ktprompts_v26.10 U5 (R.72)". The vertical limits 30/84/16/120 (owner, R.6) are untouched.
`press_origin` and the hover read one function, `event_zone(geom, event_rect, pos)`. A second copy of
the order is how hover and press would drift apart (rule 17).

### 3. Gain to silence

**Revision: the event level line covers +6.02 dB … silence.** Before, it was ±6.02 dB, the v1 rail
(kt-app CLAUDE.md, "Level line"). The owner asked for mute (USABILITY 6), and v1 parity is closed. The
upper half keeps today's mapping exactly: 0 dB at the middle, linear dB to +6.02. The lower half maps
0 dB … −60 dB on a fader law, and the last 2 % of the travel is the floor. The floor `EVENT_GAIN_FLOOR_DB
= −60` is rendered as ×0 (`audio-render`). A value between −60 and −6 written by an older build reads
as itself, so no migration is needed. The menu field accepts down to the floor and writes `mute` there.

**The chip** is drawn in the label row after the source name. It reuses the name's truncation helper
(`widgets::elided_name` budget) and is drawn only when gain ≠ 0 dB or opacity ≠ 100 % (the rule "zero is
not written", as with ×N).

### 4. Range-marker end

`marker_end_at` picks the nearest end within the radius. Hover checks marker ends too and names them
(`tl.hint.marker_end`). The end drag excludes the dragged marker's own start and end from the snap
targets. Region edges stay targets, but the "back to where I started" snap is gone. Below the ruler the
region edge keeps priority — a person drags the region far more often than a marker's end — and the
hover says which.

### 5. Vertical zoom

The anchor stays `(row index, fraction)` from `anchor_at`. After the zoom the row tops are rebuilt by
`row_offsets(rows, new_h, overrides)`, and `scroll = offs[i] + frac*h_i − (pos.y − lanes_top)`.
`vertical_layout` takes the real heights for `scroll_max`. The rule "not a single division by the row
height" (kt-app CLAUDE.md) is exactly the bug here.

### 6. Scrollbar and jump zones

- The scrollbar's interact rect is 12 px wide (drawing 6 px). It is pushed into `no_resize_rects`, like
  the badges. A click on its track pages by one visible height.
- The right jump zone ends at the scrollbar's grab rect.
- The left jump zone is measured from the clip area's left edge. Task 2.4 first checks live where it was
  lost (a 4-badge gutter added in R.62 vs the zone's pre-R.62 left edge), and the test asserts a jump
  on a row whose badges are drawn.
- In a jump zone the jump wins over an event. A click (no movement) jumps; a press + drag on the event
  still moves it (the drag threshold decides); the double-click there is consumed as a second jump.

### 7. Captions out, guard in

The two catalog keys go away from en/ru/es/zh-Hans/ar. The guard test paints a project with empty
morph, camera and video rows and fails on any painted text whose rect lies inside a row rect.
`painted_texts` is the gallery check, as used in t684.

### 8. Track reorder: cue, not a new gesture

The gesture stays. Hovering the gutter's free area sets `CursorIcon::Grab` and the hint
`tl.hint.lane_reorder`. The test drags from the name and from above the badges.

### 9. Frame step: one body for keys and buttons

`frame_step(dir)`: pause if playing, then move to the aligned neighbour on the project fps grid
(`Tick::align_*` as in `nav_jump` — the "Forward" stick of R.10). ← / → use `key_pressed`, so autorepeat
gives the hold. Ctrl+click on Back/Forward calls the same body. The arrows are not taken while
`egui_wants_keyboard_input()`. `docs/guide/shortcuts.md` gets the rows in the same commit.

### 10. Stall: once per episode, then a notice

The engine sets `stalled = true` with the push and clears it when the head moves. No stall is recorded
while `rate == 0`. The application keeps `seen_stalls` and, per frame, posts `warn(tf!(stall_at, time))`
for each new episode. The perf label stays as a counter.

### 11. Freeze frame: three pieces, one batch

`insert_frame_hold(event, t)`:
- `Split` at `t` and at `t + D`, where `D = min(2 s, end − t)`;
- the middle piece gets the warp slot's `rate = 0` with its rate curve cleared (`SetFx` + `SetFxAuto`);
- the last piece's entry point is set to the material at `t` (`ClipWarp::material_off`, the same map
  as the render). The material that no longer fits past the end is simply not reached.

All of it goes through `apply_batch_restoring` (one undo step, transition handles restored). Linked
partners follow through the split's own group handling. The audio of the hold is silent at rate 0
(R.57). The name follows rule 21: "Insert frame hold (freeze frame)".

Rejected:
- a ripple that pushes the following events (Premiere's behaviour): rule 20, "nothing moves by itself";
- a single piece with a rate curve plateau: its axis is the material, and a plateau would need the
  curve to be rewritten under the person.

## Risks / Trade-offs

- [Space hold from pause pauses on release — a person used to "hold = boost and keep playing" loses
  that from pause] → the owner asked for exactly this (U4); during playback nothing changes.
- [Level gesture disappears on low rows] → the menu field and a zoom-in still reach it; the hover on a
  low row says "zoom the row to change the level".
- [The left jump zone's loss is not yet localised] → task 2.4 reproduces it live before the fix; the
  test asserts the consequence on a badge row.
- [Freeze on a warped event] → t1007 has a curve case; the last piece is checked by
  `ClipWarp::material_off` on both sides of the hold.
- [Test numbers collide with parallel branches] → t1000+ (the highest in the tree and in open changes is
  t991); renumber at apply if another branch lands first.

## Migration Plan

No format change. The event gain range grows only downward; older files read the same values.
Rollback is a revert of the change's commits.
