# Design: readme-shots-showcase

## D1. The looks come from the gallery's cells

`showcase::look(id, variant)` applies the plugin's `ui.template` and then the named variant's
`sets`, the same two steps the picker does (`fx_registry`). The README then shows exactly what a
person gets by clicking that cell, and a plugin that retunes a preset retunes the showcase too.
Rejected: literal values in the builder (`plugin_with(id, &[…])`), which drift from the presets
and show looks nobody can pick.

## D2. What "striking" means here

The montage cycles through eight looks that change the picture at a glance (colour, shape,
pattern, lens), and the timeline carries one simulation in the montage and one in the insert.
Plain blur, noise and gain are banned from the montage (t1412). The blurred BACKDROP behind
non-16:9 cuts keeps its blur: it is structure, not a look the README presents. The VHS
look-alike (`vhs`) is not used where VHS Tape exists (owner, 2026-11-01: an imitation is never
offered as the simulation).

## D3. The morpher's look is a simulation

The morpher before/after shows a source swap played back from a worn rental tape (VHS Tape): the
same frame becomes another film on a tape. Camera ISP "Security cam, IR" was the first choice; on
the footage it washed the swapped picture out to white (D8). The shot is taken on pause, so the
simulation's cost does not touch playback shots.

## D4. The gallery shot is the gallery's end

The gallery shows its rows top-down (presets, Geometry, Mirror…): the old shot showed the
least visual cells. The step wheels the popup until the probe of the last strip
(`fxreg.var.vhs_tape.0`) appears. A cell below the fold is not drawn and publishes no
rectangle, so the probe is the visibility test.

## D5. The welcome window carries a real recent card

A recent card has a poster only after its project CLOSES (welcome D5). The step opens the
welcome window over the showcase, picks a template (which closes the showcase and queues its
poster), waits for the poster file, and opens the window again from Help.

## D6. The footage

The free Wikimedia set of `fetch-showcase-media.py` (in `kt.9/sources/showcase-media`, outside every worktree) (owner, 2026-10-22: "old samples must not be
there"), downloaded by the owner. `../sources/README_SOURCES` is not used.

## D7. A background preview is not an edit (found on the way)

The welcome step picked a template over the untouched showcase and got "save your changes?".
`dirty_content` was false; the question came from `dirty_ui`, which the reel preview service set
each time a render arrived (`app.rs`, since R.56). Three readers trust that flag: the unsaved-work
guard, the 30 s recovery write and the write on exit, so an untouched project with a reel asked a
question about nothing and left a recovery file that the next start could offer. A finished
preview now asks for one repaint and touches no dirty flag (t1413, red on the old line). The other
writers of `dirty_ui` were checked: each follows a person's gesture (bin layout and groups, region
and loop, band expand/collapse, the meters' width, the geometry frame toggle, detach, the
automation axis) or the lesson sandbox, which never reaches Recent or recovery. The step no longer
answers the question: it proves the fix live.

## D8. What the footage taught (the frames were looked at, then re-chosen)

- **Datamosh** 6 frames after the cut showed a clean frame of the previous shot: a drone moves
  too little in a quarter second. 20 frames after the Lake Bled → Charlotte cut, the night city's
  lights and towers are dragged across the hillside and both shots are recognisable.
- **VHS Tape**: a dirty or clogged head loses the FM carrier, and the simulation honestly gives
  snow, on every clip tried (night city, dusk drone, cartoon). A chipped head keeps the picture
  with the dropouts and noise of a damaged head; on the bright Coffee Run cut the tape's softness
  and colour bleed are plain. The gesture is the same one (head A of the drum, its condition row).
- **The hero** at "3 s after Space" showed "Loading…" over the previous cut: a debug build's
  preview runs a second or two behind a 4K decode under a reel and two effects. The step waits
  for the playhead to be inside the reel instance instead of a fixed time, and the looped region
  reaches past it (0.25–0.55 of the edit).
- **The gallery** previews each cell on the event's own output: on a styled event every cell
  showed its look on top (all kaleidoscopes). It opens on a plain cut.
- **Looks that read as nothing on this footage**: Halftone comic (a pure dot field), Posterize
  with ink edges (black). They became Halftone print and Posterize with a vignette.
- **Diagnostics** inside the loop filled the problem log with the debug build's underruns; the
  shot plays the edit's lightest stretch with the loop off. The numbers describe the program,
  not the build.
- **A clip's own blurred backdrop** (a frame a little wider than 16:9) no longer disqualifies it
  for the effect shots: it is part of its picture.

## D9. The search window showed no finds (found on the way)

The `search` step on the footage cropped a 71-point window: the field and nothing under it, with
"char" typed. Since windows-fill-their-area the search window was a `.width()` window: the shell
FITS such a window to its content for two frames, then fills that size. At opening the content was
the empty field, so the window measured 71 points and kept them; the finds that came with the typing
were laid out below its edge (t1414 found them at y 383–422 under a bottom edge at 390). The fixture
runs since docs-refresh had written a 2 KB `search.webp` and nobody looked. The window is now SIZED
(460×320, minimum 320×120): room for the finds from the start, and the person can still drag it.
