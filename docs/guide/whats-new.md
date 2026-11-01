# What's new

The program shows the section of its own version in the welcome window. One section per
month, newest first.

## kt26.11

- **Zigzag and the other segment shapes on one piece of a curve, or on a hundred.** Right-click
  a segment, or <kbd>Ctrl</kbd>+click several, or drag across empty space in the curve graph:
  under "Shape of all segments" a row appears with the same fields for just those segments.
  Values that differ show "—"; a typed number sets all of them, dragging the field shifts
  each by the same amount.
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
