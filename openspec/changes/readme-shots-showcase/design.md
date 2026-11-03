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

The morpher before/after shows source swap + Camera ISP "Security cam, IR" + a soft glow: the
same frame becomes another clip filmed by a night camera. The shot is taken on pause, so the
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
