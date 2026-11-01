# What's new

The program shows the section of its own version in the welcome window. One section per
month, newest first.

## kt26.11

- **Rendering a region no longer closes the program at 100 %.** A render of a region that does not
  start exactly on a frame — any region drawn by hand, even 0.2 s — used to reach 100 % and take the
  whole program down, sometimes with the file half-written. It now finishes, shows its summary and
  closes like any other.

- **A camera you can film your frame with: Camera ISP.** The new effect undoes the camera that
  made your footage back into a scene, then films it again with a simulated camera: a lens
  (defocus, distortion, chromatic aberration, vignetting, colour shading), a colour mosaic (RGGB
  and its phases, Quad Bayer, X-Trans, RGBW, RGB-IR with infrared pixels), a sensor that counts
  electrons with their real noise, blooming and banding, and the camera's processor with a block
  for every one of those defects — black level, row/column correction, defect pixels, green
  balance, raw denoise, lens shading, white balance, demosaic, distortion and colour-fringe
  correction, colour matrix, tone curve, denoise, sharpening and YUV with 4:2:0. Every block can be
  switched off or set wrong, "View" shows the picture after any stage, and the looks follow from
  it: "Phone at night", "Cheap webcam", "Security cam, IR", "Cinema raw", "Wrong CFA phase", "No
  IR-cut filter". The noise is the same on every play and render.
- **A real start window.** The welcome window has three columns — Start (the tour, templates
  drawn in their frame's shape, the five lessons with your progress), Recent (your projects with
  their pictures, as many per row as fit) and What's new, now readable in its own column. Each
  column scrolls on its own, and the window can be made as large as the screen.
- **File → Preferences… holds the program's settings.** Language, Economy UI, whether the welcome
  window opens at start, the usage statistics, the daily update check and FFmpeg are in one
  window, each switch in one place; the View menu keeps only what changes the view.
- **The language you chose stays chosen.** Opening another project used to forget it, and the
  next start came up in the system language; Economy UI was forgotten the same way.

- **A real VHS: the tape is written and read back.** The new **VHS Tape** effect does not paint
  stripes on the picture — it records it as a VHS deck does (the brightness on an FM carrier, the
  colour on a low "colour-under" carrier, two heads on a spinning drum writing slanted tracks) and
  plays the tape back. Snow, colour smear, the noise bar, the tear at the bottom and wobbly lines
  come out of the causes: click a head on the drawn drum and make it dirty, clogged or chipped,
  turn the tracking, switch SP/LP, a TBC, four heads for a clean pause, or record on one deck and
  play on another. The old **VHS** look is still there.

- **Datamosh — a real one.** The new Datamosh effect runs its own little video codec over the
  clip — motion search, a prediction loop, quantised detail, keyframes — and lets you break it on
  purpose: drop the keyframes so the previous shot gets dragged by the new shot's motion, repeat one
  frame's motion until it blooms, freeze the reference, scale, turn or swap the motion vectors,
  borrow the motion of the clip on the track above or below, lose detail or whole blocks. The
  breakages take curves: switch the mosh on and off, or put a Reset key on the beat to start clean
  from that frame. Presets: Cut mosh, Bloom, Transplant, Melt. Any frame looks the same when you
  play up to it, jump to it or render it.

- **Masks.** Right-click the picture → Mask → Rectangle, Ellipse or Pen, and draw right over the
  preview: the pen makes a Bézier path (click — a corner, click and pull — a smooth point). Each mask
  has feather (smooth or linear), expansion, opacity, invert and a mode — add, subtract, intersect,
  difference — and several masks combine from the top down. A mask on the event's picture follows
  the event when you move, scale or rotate it; an effect can be limited to its own masks (under its
  parameters in the effects window) — a blur on a face, a colour only in a corner; a track can be
  masked from its window. The stopwatch on a mask animates its whole shape: drag a point at another
  moment and the shape is keyed there.
- **Composition previews no longer get rebuilt by chance when a project opens.** Opening a
  project sometimes found its own index file "busy" and drew the frames and waveform of every
  composition all over again; now the saved previews come back every time.
- **Diagnostics holds all of Performance, with three graphs.** The Performance box no longer
  sits under the media bin — the bin gets the whole column. The gauge button (or View →
  Diagnostics…) opens one window with every number the box had and three graphs side by side in
  time: playback, FFmpeg activity and the cache. Make the window larger and the graphs grow.
- **Sound generators: tone, noise, sweep, beeps and silence.** Edit → Create a generator → Audio
  puts a line-up tone, white/pink/brown noise, a sine sweep, a train of sync beeps or a stretch of
  silence on an audio track. Levels are in dBFS (−20 dBFS of tone and of noise read the same on
  the meters), a tone or a beep can go to one channel, and the sound is the same on every play,
  seek and render.
- **"Create a generator" left the File menu.** It is in **Edit** now, and in a track's right-click
  menu, which puts the generator exactly where you clicked — a video track offers the picture
  kinds, an audio track the sound kinds. The media bin keeps its item.
- **Every window can be made bigger, and what is inside grows with it.** Drag a window's corner:
  the effects chain, the curve graph, the lists, the Guide and the render log take the new room
  instead of leaving an empty frame around a small box. Each window remembers the size you gave
  it — an effects window opens as large as the last one you enlarged.
- **The Guide reads like a manual.** Its tables are real tables, with a shaded header, lines
  between the cells and a key column as wide as its keys; every "I want to…" entry is its own
  card, the place to act written under it.
- **Adding an effect shows one big gallery.** The "+ effect" picker is as wide as its window and
  as tall as the screen allows — presets first, then the effects — with more thumbnails per row
  in a wider window.
- **No button is cut off at the end of the toolbar.** In a narrow window the buttons that do not
  fit (Render, Search, Diagnostics, Save…) move into a **⋯** menu at the end of the row.
- **Zigzag and the other segment shapes on one piece of a curve, or on a hundred.** Right-click
  a segment, or <kbd>Ctrl</kbd>+click several, or drag across empty space in the curve graph:
  under "Shape of all segments" a row appears with the same fields for just those segments.
  Values that differ show "—"; a typed number sets all of them, dragging the field shifts
  each by the same amount.
- **Windows: the welcome window names your project folders plainly** — `C:\Users\…`, not
  `\\?\C:\Users\…`.
- **A project sounds and looks the same on Windows and Linux.** Slowed+Reverb, the phaser and
  pitch shift gave slightly different bytes on Windows; every effect now uses the program's own
  math, so a render is identical on both — and on any processor.
- **Forcing an encoder this computer does not have** (VAAPI on Windows) now says so by name
  instead of an empty error.
- **If Kinetren crashes, a window says so at once** — the error, the version and the last lines
  of the log, with **Restore the project** and **Open the log folder**. Before, you learnt it only
  at the next start, if at all.
- **Restoring after a crash brings back your own footage.** The restored project no longer plays
  a lesson's test patterns in place of your sources, and keeps its own name.
- **Rendering a region that does not start on a frame** no longer closes the program.
- **The log keeps one file per run** (the four previous runs next to it), each starting with the
  version and the start time in your time zone.
- **The tour points where to act.** Every lesson step outlines its place — the menu item, the
  event, the marker, the button in a window — and **Show** scrolls to it or opens its menu
  instead of doing the step for you. Keys and buttons in the lesson text are drawn as keycaps.
- **The tour no longer lags or misleads.** "Play a part in a loop" counts the moment the loop is
  on, even with a window open; the transitions lesson teaches **Transition with previous** (an
  overlap stays a cut); after an effect or a transition the lesson says what changed and moves
  the playhead to where you can see it. "Next lesson" no longer asks to save the lesson's own
  copy, the lessons panel no longer hides the right end of the toolbar, and the render lesson
  tells silly jokes while it waits.
- **Click a marker on the ruler** to put the playhead on it.
- **Pitch no longer tears between short events.** Cut an event with pitch into many short
  pieces and they sound exactly like the uncut event; any pitched event starts and ends
  without a dip — also when the pitch comes after Slowed+Reverb or an echo, or from a morpher.
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
