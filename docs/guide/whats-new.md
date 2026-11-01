# What's new

The program shows the section of its own version in the welcome window. One section per
month, newest first.

## kt26.11

- **If Kinetren crashes, a window says so at once** — the error, the version and the last lines
  of the log, with **Restore the project** and **Open the log folder**. Before, you learnt it only
  at the next start, if at all.
- **Restoring after a crash brings back your own footage.** The restored project no longer plays
  a lesson's test patterns in place of your sources, and keeps its own name.
- **Rendering a region that does not start on a frame** no longer closes the program.
- **The log keeps one file per run** (the four previous runs next to it), each starting with the
  version and the start time in your time zone.
- **Pitch no longer tears between short events.** Cut an event with pitch into many short
  pieces and they sound exactly like the uncut event; any pitched event starts and ends
  without a dip.
- **Changing an audio effect takes effect at once.** Put a filter where an equaliser stood
  and you hear the filter right away — before, the old effect kept playing with the new
  settings until you jumped in time.
- **The strength slider in the effects gallery** no longer mixes the settings of two
  different effects.
- **Every effect, transition and generator is now a plugin inside** — blur, glow, VHS, noise,
  the chroma key and the rest moved too. Your projects open and look exactly as before.

## kt26.10

- **Windows.** A portable Windows build; FFmpeg is found on the computer or downloaded with
  one click.
- **FFmpeg 9** works, as do 6, 7 and 8.
- **Under wine** the interface is dark everywhere and our own title row is back.
- **A quiet reminder about updates.** Once a day the program checks whether a newer release is
  out and says so in the bell — no pop-ups.

- **A welcome window at start.** New projects from templates, the recent projects with a
  frame of each, the tour and this page. One tile is highlighted: **Enter** takes it. Switch
  the window off with **Show at start**; **Help → Welcome window…** brings it back.
- **The tour.** Short lessons on a demo project made of generators. A step closes by itself
  when the project shows it done — see [Getting started](getting-started.md).
- **Help → Guide…** finds where a thing is done: type what you want, or open a page of this
  guide.
- **One window shell.** Every window closes by its cross and by **Esc**, the top one first,
  and no window grows by itself.
- **The render window** names the encoder before the first frame, counts the bytes and asks
  before replacing a file.
- **The FFmpeg window** tells what it downloads, where it goes, how fast, and why.
- **Effects are plugins inside.** The built-in effects moved into their own crates behind one
  interface; projects keep their bytes.
- **Faster event grids.** A grid computes one cell and copies it; a morpher layer at strength
  zero costs nothing.
