# Tasks: readme-shots-showcase

Test numbers t1412–t1419 are reserved by this change.

## 1. Striking looks and the new shots

- [x] 1.1 `kt-cli showcase`: `look(id, variant)` from the gallery's cells; the montage cycle, the
  PiP, the insert and the morpher dressed in striking looks and simulations; t1412
- [x] 1.2 `readme_shots.py`: the gallery scrolled to its end, the `welcome` shot, effect shots on
  an event with no look of its own; the pipeline green on the fixtures (`live-check.sh readme_shots`)
  - Done. 18 of 18 images on the fixtures. The gallery's settle ignores the search field's blinking
    caret (a 2×17 px change every few seconds kept `still` from ever settling)
- [x] 1.3 A finished reel preview no longer marks the project edited (`app.rs`); t1413; the durability scenario; a What's new line; the welcome step answers no question
  - Done. Probe on the live program (the showcase opened, untouched 25 s, then a template): the
    old build asked "save your changes?" with `dirty_content` false and left
    `probe.kt.recovery.kt`, so the next start offered a restore; the fixed build asks nothing and
    leaves no file. t1413 red on the old line, green after; kt-app 513 tests green
- [x] 1.4 The project search shows its finds: the window is sized, not fitted (`app.rs`
  `draw_search`); t1414; the ui-reach scenario; a What's new line
  - Done. Found by the search step on the footage (a 71-point crop with "char" typed). t1414 red
    before (the finds at y 383–422 under the window's bottom edge at 390), green after, with
    t918/t921/t1125/t1127; kt-app 514 tests green

## 2. The footage run

- [x] 2.1 Run `readme-shots.py --sources /media/e/developer/kt.9/sources/showcase-media`; look at every image at its
  native size; re-take weak frames
  - Done in five runs, each image looked at; what each run taught is design D8. Scratch drivers
    surveyed the styled cuts, the Datamosh offsets (0.24–3.3 s) and the VHS head conditions on
    three clips before the choices were fixed in the pipeline. The fourth run found the search
    defect (task 1.4), the fifth is the committed set: 18 images, 10–252 KB, 1.6 MB in all
- [x] 2.2 README and guide: the four effect shots and the welcome window referenced with alt
  text, alt text of the changed shots updated; `AWAITING_FOOTAGE = []`; `readme-check.py --check`
  - Done. `docs/images/CREDITS.md` already names all nine clips of the footage

## 3. Close

- [ ] 3.1 The showcase spec's scenarios (archive); `cargo test --workspace`, clippy
- [ ] 3.2 `./scripts/finish.sh`, the version in the report
