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
