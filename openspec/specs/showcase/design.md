# The public face of the repository — why it is built this way

## What hurt, and for whom

The repository was going public on GitHub with a 15-line Russian stub for a README. Over
four rounds of review (2026-10-21/22) the owner asked for:
- a complete English README with screenshots cut from real footage, and slowed-reverb
  automation curves visible in them;
- free footage instead of copyrighted music videos (Big Buck Bunny was rejected as old
  and overused), with no old samples left;
- a README about the product only — "how to run it, not technical junk";
- a wide free-layout bin, live meters, a nested reel shown expanded mid-edit with veiled
  tails, published reel parameters, Ctrl+F search, and the morpher as a before/after.

## Decisions and why

- **`kt-cli showcase` builds the project in Rust**, next to `demo_project`. It is
  compile-checked and validated, and it survives format changes because the library owns
  the writer. Rejected alternatives: `demo --sources` (`demo` is synthetic by contract,
  t388), and Python editing the OTIO `metadata["kt"]` envelope (that would duplicate
  format knowledge).
- **Deterministic layout**: a pure function of the sorted file names and the probed
  facts. Audio-only files are the music beds. Text files next to the footage are skipped,
  because FFmpeg's text demuxer would open `CREDITS.txt` as a "video". The reel's content
  is twice as long as an instance, so an expanded instance shows tails. Track heights are
  compact so an expanded reel fits a 1080p panel.
- **Screenshots through the live framework and probe names, never pixel coordinates.**
  `site/screenshots.py` showed the failure mode: its coordinates silently went stale.
  Every step checks its precondition and fails naming the step. Each shot runs in a fresh
  process, and recovery files are removed between shots, because a restore modal would
  cover the view. `still(quiet=…)`: 4K filmstrips arrive with pauses, so a zoomed
  timeline needs a longer quiet period. The morpher before/after is the same frame shot
  from a copy of the project with the morpher disabled, so no gesture can be
  half-applied.
- **Wheel gestures are not exact.** A synthetic Ctrl+Shift+wheel partly lands as
  Ctrl+wheel, and each tick is ×1.48 rather than ×1.25. Do not try to undo zoom with
  wheel ticks; set heights in the project and zoom only where precision does not matter.
- **WebP at quality 92** keeps UI text crisp and stays within the size budgets
  (900 KB per image, 8 MB in total).
- **The guard `readme-check.py`** (t826/t827) enforces: English text, valid links and
  anchors, images that exist, no orphans, size budgets, and the set of images referenced
  by the docs equal to `SHOTS` of the pipeline. The application icon is the only image
  allowed outside `docs/images/`.
- **Footage credits** live in `docs/images/CREDITS.md` and in `CREDITS.txt` next to the
  download. The README carries none: that is the owner's rule.
- **`shortcuts.md` is the one key reference, built from the code** (change
  `guide-shortcuts-reference`, 2026-10-26). The site dropped the landing's "Keyboard"
  section ("a reference does not belong there"), so the page has to be complete. It is
  written from an inventory of the handlers (key → `file:line` → condition), not from hints
  or the old landing. Several landing rows were out of date: "S cuts out the region",
  "release Space slows down", "the volume rail". Rejected: a guard that diffs the page
  against `handle_keys`. The key map is control flow in one function, so parsing it from
  Python would be a second, fragile reading of Rust. The kt-app `CLAUDE.md` instead asks
  for the row to be updated with the binding.
- **The `markup` class mirrors the site's renderer** (t869). The site fails its whole build
  on any `<`+letter outside code except balanced `<kbd>` pairs, so this repository checks
  the same subset where the text is written. A balanced `<kbd>…</kbd>` with no `<`,
  backtick or `*` inside is cut out first, which is the site's own rule. Known false
  positive: a bold one-letter button label ("**M**") reads as a key; write it as plain
  text. Rejected: bold keys on GitHub with caps only on the site, because the same text
  would look different in two places.

## Known limits

- The full `readme_shots` run takes several minutes, so `finish.sh` runs over its time
  budget. Running only the hero shot inside `live-check.sh` is an open option.
- Old images remain in git history until the owner squashes.

## Striking looks and the footage run (readme-shots-showcase, archived 2026-11-03)

The owner (2026-11-03): the README's effects were "dullish" and the simulations were not shown as
BEFORE/AFTER. Decisions and what the footage taught:

- **The showcase dresses its events from the gallery's own cells** (`showcase::look`: the plugin's
  template, then a named variant). The README shows what a click gives; a retuned preset retunes
  the showcase. Plain blur, noise and gain are banned from the montage (t1412); the VHS look-alike
  is not used where VHS Tape exists. The blurred backdrop of non-16:9 cuts keeps its blur: it is
  structure, not a presented look.
- **Frames were chosen by looking, in scratch sessions, before they were fixed in the pipeline.**
  Datamosh 6 frames after a cut is a clean frame (a drone barely moves); 20 frames after the Lake
  Bled → Charlotte cut the city is dragged over the hillside. A dirty or clogged VHS head loses the
  FM carrier and gives snow on every clip tried; a chipped head on the bright Coffee Run keeps a
  picture with the tape's dropouts and bleed. Camera ISP "Security cam, IR" washed the morpher's
  swapped picture to white, so the morpher plays it from a worn tape. Halftone comic was a pure dot
  field and Posterize with ink edges went black on this footage.
- **The hero waits for a playhead position, not a time.** A debug build's preview runs a second or
  two behind a 4K decode under a reel and two effects: "3 s after Space" showed "Loading…". The
  project's cursor sits a moment before the mid-edit reel instance, the looped region reaches past
  it, and the shot waits for the playhead inside it AND for a frame without the "Loading…" scrim:
  one run caught it mid-load, so the program publishes the scrim as `preview.loading` where it
  paints it, and the step takes the first dump with the playhead in place and no scrim.
- **The gallery opens on a plain cut**: its cells preview each effect on the event's output, so a
  styled event showed its own look in every cell. It is wheeled to its end, where the colour,
  distortion, camera and tape cells are; a cell's rectangle is published below the fold too, so
  "on screen" is its bottom inside the popup, and "settled" ignores the search field's caret.
- **Diagnostics plays the lightest stretch with the region cleared** (Esc): inside the loop the
  debug build filled the problem log with underruns that describe the build, not the program; with
  the loop merely off, playback stops at the region's end (the fixtures' short edit reached it).
- **A clip's own blurred backdrop does not disqualify it** for the effect shots: it is part of its
  picture.
- **The pipeline found two program defects on the way** — an untouched project with a reel asked
  "save your changes?" (durability, t1413) and the project search hid its finds (ui-reach, t1414);
  both were fixed in the same change, and the steps prove them live (the welcome step answers no
  question; the search step requires a window with results).
