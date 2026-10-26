# Inventory: every key and gesture kt-app handles (2026-10-26)

Paths are relative to `src/apps/kt-app/src/`. "Page" = the row in `docs/guide/shortcuts.md`
that states it (✓ once written). Global hotkeys are dead while TEXT is being typed
(`ctx.text_edit_focused()`, the timecode mask, the speed field — `app.rs:7085`); a focused
button or slider does not take them (R.63 item 4). The detached preview window calls the same
`handle_keys`, except that Esc there only returns the preview (`app.rs:1743`).

## Keyboard — global (`KtApp::handle_keys`, `app.rs:7071`)

| Keys | Handler | Condition | Does | Page |
|---|---|---|---|---|
| tap Space | `on_space_released` → `on_transport_toggle` (`app.rs:2593`) | press < 220 ms | play / pause | ✓ |
| tap Space again | `transport_toggle` (`app.rs:2504`) | within 400 ms of a pause | to the region start (no region — to 0), stays paused | ✓ |
| Space while a played-through region | `transport_toggle` | loop off, cursor ≥ region end; or loop on, cursor outside | play from the region start | ✓ |
| hold Space | `space_boost` (`app.rs:2533`) | ≥ 220 ms, playing | speed ramps to 4× over 5 s; release — back to the base speed at once | ✓ |
| A / Home | `nav_jump(false,…)` (`app.rs:2453`), `stop_points` (`app.rs:8179`) | A without Ctrl/Alt | previous stop: 0, region edges, markers (and range ends), edges of runs of events | ✓ |
| D / End | `nav_jump(true,…)` | D without Ctrl/Alt | next stop; past the last — the end of the project | ✓ |
| Shift+A / Shift+D | `nav_jump(…, drag_region)` | no Ctrl/Alt | the same jump, the left / right region edge follows; no region — the region from the old to the new position | ✓ |
| Shift+Home / Shift+End | `jump_stop(far)` | | to 0 / to the end of the project | ✓ |
| Ctrl+L | `tc_edit` (`app.rs:7186`) | also while playing | open the timecode mask | ✓ |
| Ctrl+F | `search_open` | | project search | ✓ |
| S | `cut_action` (`app.rs:5021`) | no Ctrl, no Alt (t868) | split at the playhead: the selected events and their link partners; nothing selected — every event under the playhead. Never cuts out the region | ✓ |
| Delete / Backspace | `delete_action` (`app.rs:5083`) | | selected morphers/cameras first; else with a region — every event crossing the region (with partners); else the selected events | ✓ |
| Ctrl+Z | `undo` | no Shift | undo | ✓ |
| Ctrl+Shift+Z / Ctrl+Y | `redo` | | redo | ✓ |
| Ctrl+S | `save` | | save | ✓ |
| Ctrl+C | `copy_selection` (`app.rs:4562`; `Event::Copy`) | | with a region — what lies in it; else the selected events | ✓ |
| Ctrl+V | `paste_at_cursor` (`app.rs:4877`; `Event::Paste`) | | paste at the playhead, row under the mouse; the playhead moves to the block's end (repeat = chain); named items first ask for a name prefix | ✓ |
| M | `marker_here` (`app.rs:4440`) | no Ctrl/Alt | marker at the playhead; a marker near it — removed; region with the playhead inside — a range marker over the region (again — removed) | ✓ |
| I / O | `set_region_edge_from_cursor` (`app.rs:4495`) | no Ctrl/Alt | region start / end at the playhead (I with no region: a 1 s region from the playhead) | ✓ |
| L | `toggle_loop` | no Ctrl/Alt | loop on / off | ✓ |
| Alt+M / Alt+S | `toggle_track_badge` via `key_target_track` (`app.rs:1406`) | | mute / solo: the selected event's track, else the row under the pointer | ✓ |
| Enter | `enter_pressed_on_nest` (`app.rs:3349`) | no modifiers, a reel instance selected | collapsed — expand in place; expanded — enter the reel | ✓ |
| Esc | `escape_action` (`app.rs:2698`) | | one step: speed field → search → preset/paste-name dialogs → top window (published parameters, render, track, FX, morpher, notices, FFmpeg, diagnostics) → cancel the drag → clear region and selection (loop stays on) → collapse an expanded reel → leave the reel | ✓ |

## Keyboard — inside a field or a window

| Keys | Handler | Does | Page |
|---|---|---|---|
| digits, Backspace, Enter, Esc in the timecode mask | `toolbar.rs:640` | digits fill from the right; a pasted timecode is laid into the mask; Enter jumps, Esc cancels | ✓ |
| digits `. , - + × x`, Backspace, Enter, Esc in the speed field | `app.rs:6650` | minus = backwards; Enter applies, Esc cancels; a bad value keeps the field open, underlined red | ✓ |
| ↑ / ↓, Enter, Esc in search | `app.rs:2346` | choose a hit, jump to it, close | ✓ |
| Tab, Enter in a formula field | `automation.rs:1550` | Tab completes a name, Enter applies | ✓ |
| Enter, Esc renaming a track / marker / area | `timeline.rs:5206–5316` | save / cancel the name | ✓ |
| Enter, Esc renaming a bin card | `bin_panel.rs:703` | save / cancel | ✓ |
| Enter, Esc in a published-parameter name | `pubparam.rs:316,332` | save / cancel | ✓ |
| Enter, Esc in the paste-prefix window | `app.rs` (R.35 item 4) | paste / cancel | ✓ |
| Esc in the detached preview | `app.rs:1743` | return the preview to the main window | ✓ |

## Mouse — timeline (`timeline.rs`)

| Gesture | Where | Does | Page |
|---|---|---|---|
| click / drag the ruler | `timeline.rs:3072` | click — playhead (snaps to markers); drag on the playhead triangle — scrub; drag elsewhere — region | ✓ |
| double-click a range marker on the ruler | `region_from_marked_range` | region over it plus its events selected | ✓ |
| click empty space | | playhead | ✓ |
| drag empty space | `Drag::Region` | region (snaps) | ✓ |
| Ctrl+drag empty space; drag in the Selection-tool mode | `Drag::Rubber` (`timeline.rs:3321`) | rubber-band selection (with the tool on, Ctrl adds) | ✓ |
| click near the left / right edge of the tracks | `edge_heat` (`timeline.rs:3313`) | playhead there and the view jumps | ✓ |
| drag past the edge | `edge_heat` autoscroll | the view scrolls | ✓ |
| click an event / Ctrl+click | `timeline.rs:3260` | select (also places the playhead) / add to the selection without moving the playhead | ✓ |
| drag an event (Ctrl — no magnet) | `Drag::Clip` | move (a selection moves together); onto a neighbour — crossfade | ✓ |
| drag an event's edge | `TrimStart/TrimEnd` | trim | ✓ |
| drag a top corner | `Drag::Fade` | fade at that edge | ✓ |
| drag the level line; double-click it | `Drag::Level` | opacity (video) / gain (audio); reset to full / 0 dB | ✓ |
| double-click an event | | effects window | ✓ |
| Shift+double-click an event | `region_from_event` | region over it (with link partners) | ✓ |
| double-click a reel instance | | expand in place / collapse | ✓ |
| drag the edge of an expanded reel's window | `band_edge_at` | resize the instance | ✓ |
| drag a transition's edge / its cut; double-click it | `TransitionEdge`/`TransitionBody` | resize / roll; properties | ✓ |
| drag / double-click a morpher (edges resize) | `Drag::Morph`/`MorphEdge` | move; editor | ✓ |
| drag / double-click a camera (edges resize) | `Drag::Camera`/`CameraEdge` | move; properties | ✓ |
| drag a marker | | move it | ✓ |
| drag a region edge | `Drag::RegionEdge` | move it | ✓ |
| drag an area's edge | | resize the area | ✓ |
| click a header badge M / S / lock / FX | `timeline.rs:2433` | mute / solo / lock / track window | ✓ |
| double-click a track header | `timeline.rs:2440` | rename | ✓ |
| drag a header / its bottom edge; right-click the edge | `LaneRow`/`LaneHeight` | reorder / height; reset height | ✓ |
| right-click anything | `timeline.rs:2959` | its menu | ✓ |
| wheel | `timeline.rs:1972` | scroll along time | ✓ |
| Ctrl+wheel / pinch | `zoom_delta` | zoom around the mouse | ✓ |
| Shift+wheel | raw `MouseWheel` | scroll tracks | ✓ |
| Ctrl+Shift+wheel | `tl_v_zoom` (`timeline.rs:1937`) | all track heights, anchored at the row under the mouse | ✓ |

## Mouse — other places

| Gesture | Where | Does | Page |
|---|---|---|---|
| click / double-click the preview; right-click | `app.rs:6305` (300 ms window) | play / pause; detach / return; menu (frame controls) | ✓ |
| double-click the timecode | `toolbar.rs:616` | type a position (a single click does nothing) | ✓ |
| Back / Forward buttons with Shift | `toolbar.rs:587` | the jump drags the region edge | ✓ |
| speed strip: drag top / bottom; Shift; double-click; right-click or Ctrl+click | `app.rs:6525–6618` | constant speed / shuttle (release returns); fine; 1×; type the speed | ✓ |
| fader: drag, wheel (Shift fine), double-click, mute button | `meters.rs:772,844` | preview volume 1 dB / 0.2 dB steps; 0 dB; mute | ✓ |
| bin card: click / Ctrl or Shift+click / double-click a generator / drag | `bin_panel.rs:938` | place at the playhead / toggle in the selection / its parameters / onto the timeline or around the bin | ✓ |
| drag the bin background (Ctrl/Shift adds) | `bin_panel.rs:256` | select cards with a frame | ✓ |
| chain strip slot: click / drag / cross | `widgets.rs:1464` | select / reorder / remove | ✓ |
| double-click a parameter label; right-click it inside a reel | `widgets.rs:1113` | reset to the default; publish the parameter | ✓ |
| automation graph: click empty / drag a key / double-click a key / right-click a key | `automation.rs:969,1160` | add a key to the active curve / move / delete / key menu | ✓ |
| automation graph: wheel / Ctrl+wheel / drag the ruler | `automation.rs:688,1150` | scroll / zoom the graph; move the main playhead | ✓ |
| automation segment handle: drag, Shift+horizontal | `automation.rs:1203` | shape tension or amplitude / periods | ✓ |
| key-before / key-after buttons with Ctrl | `automation.rs:572` | lock every curve of the slot | ✓ |
| EQ graph: drag a point; wheel over the middle one | `eq_graph.rs:250` | frequency and gain; Q | ✓ |
| transform frame: drag body / corner / rotate handle (Shift — 15°) / anchor / crop side; Alt | `geomctl.rs:395–456` | move / scale / rotate / move the anchor / crop; Alt widens the anchor's grab zone | ✓ |
| title bar: drag / double-click | `toolbar.rs:298,565` | move the window / maximize or restore | ✓ |

## Not bindings (grep hits that are not a key or gesture)
- `kt_model::AutoKey::…` in `transition_editor.rs`, `user_store.rs`, `automation.rs:1269`,
  `pubparam.rs:869`, `camera_editor.rs:178` — automation keys, not keyboard keys.
- `widgets.rs:734/740` — the menu-item helper; its chords are listed with their callers.

## Menu chords (`toolbar.rs`, `timeline.rs`)
`Ctrl+S` Save, `Ctrl+Z` Undo, `Ctrl+Shift+Z` Redo, `Ctrl+C` Copy, `Ctrl+V` Paste, `Del`
Delete (and the event menu's Delete only when there is no region), `S` "Split / cut region",
reel items "Enter, 2×click" / "Esc, 2×click" / "Enter again". All match a handler except the
one below.

## Hint / code mismatches (proposals for the report, not edited here)
1. **Edit → "Split / cut region" (`S`)** — `S` calls `cut_action`, which only splits;
   cutting out the region is the event menu's punch-out. The label promises what neither the
   item nor the key does.
2. `tl.hint.volume` "Clip volume: drag, double click — 0 dB" is the audio half of the level
   line — correct, but the landing's "volume rail" is that line, not a rail.

## Landing rows (brief from kt.9.site `remove-landing-keys`) — verdicts

| Landing row | Verdict |
|---|---|
| Space play/pause | matches |
| second tap — stop, to the region start | differs: the second tap within 400 ms of a pause goes to the region start, or to the project start when there is no region, and stays paused |
| hold Space — up to 4×, release — slow down | differs: ramps to 4× over 5 s; on release the base speed returns at once, no slow-down |
| A/D and Home/End — previous/next marker, region edge or event | matches; "event" = the edge of a run of overlapping events, plus the project start/end |
| Shift+Home/End — very start/end | matches |
| Ctrl+L — jump to a typed timecode | matches (digits fill from the right) |
| S — split at the cursor / cut out the region | differs: `S` only splits (selected events, else all under the playhead); cutting out a region is the event menu's punch-out |
| Del — delete the clip or everything in the region | matches: with a region, every event crossing it; selected morphers/cameras go first |
| Ctrl+C/V — copy clip or region contents / paste at the cursor as a chain | matches |
| Ctrl+Z, Ctrl+Shift+Z, Ctrl+S | matches; Ctrl+Y also redoes |
| hold Ctrl while dragging — no magnet | matches |
| I/O | matches |
| Shift+A/D — jump dragging the region edge | matches |
| L — loop | matches |
| M — set/remove a marker | matches, and with the playhead inside a region it makes a range marker |
| Esc — step back (close the top window, clear region and selection, leave the reel) | matches, but the cascade has more steps: the speed field, search, dialogs, cancel a drag, collapse an expanded reel before leaving |
| Ctrl+F — search | matches |
| wheel — scroll the timeline | matches |
| Ctrl+wheel — zoom in time | matches (around the mouse; pinch too) |
| Shift+wheel — scroll tracks | matches |
| Ctrl+Shift+wheel — track height | matches (all tracks) |
| double-click the preview — detach | matches; a single click is play/pause |
| double-click the timecode — type a position | matches |
| double-click the speed strip, volume rail, fader — reset | speed strip (1×) and fader (0 dB) match; differs: there is no volume rail, the level line in the event's top strip resets to 0 dB / full opacity |
