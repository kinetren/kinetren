# History of the user's requirements

The primary source on WHAT the project owner asked for and HOW he phrased it.
`openspec/specs/<cap>/` answers the question "which defect and which test closed it",
the crates' `CLAUDE.md` — "how to write code now"; this file answers the question
**"what the user wanted and why the decision is exactly this"**.

## Rules of keeping it (mandatory for agents)

1. **Read BEFORE working.** Before changing UI/behaviour, check whether this place was discussed earlier. If a limit, mode or view was set by a direct request — it must not be changed without a new request.
2. **Write IMMEDIATELY.** A new round of remarks → first record the list here (in the user's own words, not a retelling), then fix. Answers to clarifying questions are recorded here too — they are the decisions.
3. **Mark repeats.** If an item repeats — put ⚠ and the number of repeats: a repeat means the rule was not written down or was written weakly.
4. **Reference the closure**: the test number and/or the spec scenario.

## Cross-cutting requirements (must not be violated)

| Requirement | The user's formulation | Where it is enshrined |
|---|---|---|
| System FFmpeg | "no own build of FFmpeg and no plugins" — only runtime dlopen of the system one | invariant No. 10 of the root CLAUDE.md |
| In-process export | without an external `ffmpeg` binary, without temporary files | invariant No. 9 |
| Tests are mandatory | every engine change — with a regression autotest; do not weaken tests 1–111 | root CLAUDE.md, openspec specs |
| UI no worse than C++ v1 | "the UI must be professional" | apps/kt-app/CLAUDE.md |
| UI is made FOR THE USER | "UI/UX is made FOR THE USER, and you are making it for who-knows-whom, and I have to label everything" (R.8 item 17) | checklist in apps/kt-app/CLAUDE.md |
| A remark = a CLASS of problems | "I often write 1–2 examples, and in the program there are many similar places" (R.8 item 17) | checklist, section "How to read remarks" |
| Do not waste space | "DO NOT WASTE SPACE… instead of spawning an empty line across the whole screen — you can neatly write text in the space already allotted on the screen" (R.12 item 4) | checklist item 13 |
| Check what the user runs | "so you are not checking a damn thing you do. DOUBLE-CHECK" (R.12, after "I took the build from debug") | rule of the finish: `scripts/finish.sh` + version in the report |
| Check in a maximized window | "you have to check in a maximized window, not the way you do — in a small one" (R.13) | `scripts/ui-probe.py`, test t85 with `maximized` |
| Do not squeeze what was not asked about | "the media widget IS SQUEEZED BY WIDTH FOR THE THIRD TIME ALREADY" (R.13 item 4) | the rule "a class of problems ≠ squeeze everything", a comment at the limit in the code |
| Ask right away | "if something is unclear — better ask me right away" (R.8 item 17) | checklist |

## Rounds

### R.4 — performance and timeline UX (10 items)
System lags, a file dropped not under the cursor, the FX picker, reverse speed, the double slider, track order, the detachable preview, defects of the meters' glow.
→ closed J1–J9, tests 38–51.

### R.5 — "the system lags" (6 items) + diagnostics
AMD machine, drop on Xorg, the compact speed control, the preview window.
Key dialogue: "with playback fps=60, vfps=24" → the cause was found: the playhead
moves in quanta of the audio callback. The outcome "yes, it became super fast. we need to write
down the studied problems for other agents" → lessons in apps/kt-app/CLAUDE.md, tests
55–56.

### R.6 — theme and timeline (items 11–23)
The user's decisions: the theme — "fewer rounded corners and gradients, **BUT NOT FLAT/BORING INTERFACES**"; the titlebar — "merge it with the toolbar"; item 15 — "Space twice quickly"; rounding — "minimal (1px)"; about the hint — "I want it to be there always, this is it, so it is not there on hover, is that how it works?".
→ tests 58–64, borderless window, zoom-out, the groove, filmstrip.

### R.7 — 30 items
A storm of FFmpeg opens, the post-FX video cache, transport and follow, the menu ("File + Edit + View"), clicks on the previews, the two-band speed control ("two parallel bands, keep the final height of the widget the same"), a cleanup of the status bar.
Separately: **do not make obvious labels** (items 17, 21, 25) — "zoom:", "FPS:", "(loop)" were removed. → tests 65–74.

### R.7.1 — 7 items
⚠ "where did the video and sound preview go from the ui" — a regression after R.7 (the panel was swelling with content). Decisions: the project name "move to the header of the left column, and do not forget that if the text turns out long, nothing prevents writing it over the preview"; "Everything at once in one list"; store the viewport in "In the project .ktproj".
→ item 75, the rule "do not swell panels with content".

### R.8 — 17 items
⚠ "I CAN'T CHANGE THE WIDTH OF THE PREVIEW WIDGET AGAIN… IT'S FUCKING ANNOYING, **introduce a testing rule already**" → a harness appeared (egui_kittest) and the rule: any layout change — with a test (t76+).
Decisions: the search palette "centred"; the marker name — "inline on the timeline"; the indicator of unsaved changes — "an asterisk *"; bin modes — as an icon, not a checkbox.
Rule of item 17 (see cross-cutting requirements). → tests 76–82.

### R.9 — 11 items
⚠ "**WHERE DID THE BUTTON HINTS GO?!?!?!** … you did this item not to the end! Double-check all the hints you edited" → the order of the panels; the hint is drawn last in the frame (t83).
⚠ "it hurts my ears" — the fader flew off to 400 dB (t84).
Decisions: the window drag — "an invisible strip ~12px"; the bin poster — "lazily, up to 5 attempts". → tests 83–88.

### R.10 — 8 items
Named markers, renaming of tracks, Ctrl+F, Ctrl+L, the diagnostics icon, `*` instead of a tofu square, the "Forward" button (it was getting stuck), the A/D hotkeys.
→ tests 89–94.

### R.11 — 13 items
Answers: the magnet is switched off with "ctrl, I see no problem using it, since during a drag they will hardly be scrolling"; the timecode mask — "digits right to left"; the bin groups — "frame + name + lock".
Rule-requirements: item 11 "fix/finish the rule for agents about the CONVENIENCE of the interface, **I'm already tired of writing messages for the 5th time about the same approaches**"; item 12 "introduce a rule for agents — any non-printable characters of the text in the UI must be checked in the fonts" (→ test 100 immediately found three squares).
Other: the magnet did not work, Esc switched off the loop, the autosave was too frequent, markers on the cache strips, the meters without a width limit, "you can't just shove an inputfield in there" (the timecode mask), the track name was sticking out. → tests 95–100.

### R.12 — 9 items
⚠ item 1 "there are too few segments in the LED, **I asked to remove the partial filling, not to reduce their number**" — I solved the wrong task in R.11.
⚠ item 2 track labels "make them properly" (second attempt).
item 3 the version: "isn't it obvious what is meant, ktYY.MM.DD-commit_hash?? USER CONVENIENCE AND ERGONOMICS".
⚠⚠ item 4 "THE MEDIA WIDGET IS SQUEEZED BY WIDTH FOR THE THIRD TIME ALREADY".
item 5 the cursor gradient spills over the cache strips and should glow from the side of the play.
⚠ item 6 "for the second time I'm already writing about the problem with resizes" — the collapse button is not pressable halfway.
Answers to clarifications: the width of the meters — "90 px"; the version date — "the commit date".
Separately, in the middle of the round: "I took the build from debug" → the `finish.sh` rule.
→ tests 101–105, items 101–105.

### R.13 — 6 items
item 1 "the audio monitor… you can see it's fake… the LEDs look too bold or what" → analysis showed that in R.11 I rearranged the layers and killed the glow; I restored the v1 order.
Then: "the glow of the audio monitor is still very weak" → the alphas were raised, measured by pixels (13/15/14 → 24/32/20).
item 2 clarification: "I didn't say that you can't go over clips — I said don't make a mess out of letters… you could at least guess to write the track names NEATLY under the names of the clips themselves".
item 6 clarification: "I don't understand what the problem is — make the separation zone more active to the left and that's it" → a dedicated drag strip to the left of the meters panel.
item 11 window drag: "at the very top above the preview there is a one-pixel strip on which it's useless to try to drag, and if you go a little lower — it works".
Media groups: the name was not saved on Enter, the frame was drawn over the cards, the cards became too big.
→ tests 106–111, t85/t99/t101/t102.

### R.14 — frame stutter (2 items)
"if you set the playback speed to something small, well 0.25x, then you can see that the frames during playback kind of stutter — they jump back a couple of frames. such stutter (without slowing down) is also always present in the rendered videos. Both problems must be covered by autotests, without fail." → tests 112–115.

### R.15 — regressions of my own round (2 items)
"1. in the render there is now a black picture… 2. if you select a region and play it in a loop, the video freezes the second time but the audio does not. Both cases must be put into a test. And run all the tests, all of them." → tests 116–117.

### R.16 — 5 items
1. "in the media widget it does not scroll by itself when moving a source/group to the edges, but I'd like it to — it does not even let you scroll with the wheel" → item 118, test 118.
2. "regarding "Back"/"Forward" I want to give the possibility to select a region while holding an extra button — which button do professional programs use? shift? or another? a+<key> sets the left edge of the region, d+<key> — the right one."
   The user's answer to the clarifying question: **"Shift+A / Shift+D (as in professional programs)"** → item 119, test 119.
3. "it should be like on YouTube — holding the space bar speeds up playback temporarily, and the speed gradually grows. don't break the functions of the double and single press."
   Answer: **"Up to 4× in ~2 s"** → item 120, test 120.
4. "tone down the glow of the audio monitor, and also — its glow flickers over the whole height, which is strange, for example for the lowest segments — after all they are "lit" almost always. And also — on pause, for some reason the monitor doesn't go out but freezes."
   Answer to the clarification: **"It goes out, but while scrubbing/seeking it shows the level under the cursor"** → item 121, test 121.
5. "the timeline context menu has an item to add a morph track, but there are no items for audio/video tracks, and I'd like the addition to happen not at the very bottom, but after the track that was clicked" → item 122, test 122.

### R.17 — 3 items (check of round 16)

⚠ 1. "the mouse scroll there still doesn't work; if I grab an object and poke it fairly quickly at the edge — we scroll, but then the source flies back by itself and I'm left in the scrolled-away position" — a repeat of item 1 of R.16: the autoscroll works, but the wheel is still dead during a drag (egui itself mutes the wheel on a ScrollArea while something is being dragged), and a poke past the edge turns on a sticky "eject" and rolls the card back → item 123, test 118.
2. Shift+A/D — "ok".
3. "it should be not in 2 s but in 5, otherwise it's too fast" — the speed-up with the space bar is stretched to 5 s (the 4× ceiling is kept) → item 124, test 120.
4. Meters — "ok".
5. Tracks from the menu — "ok".

### R.18 — selling landing page (not about the editor, about the site)

"We need a "selling" landing site for the editor, which will tell the consumer what our product is, what can be done with it and how, plans, etc. I do NOT mean telling about the ffmpeg dlopen and other such things, I need a landing page for the user, cool, dark."

The user's decisions on the clarifying questions:
1. "plans" = **roadmap** (what is ready / what is next), NOT pricing. There is no pricing block on the site and it is not to be added without a new request.
2. The language of the landing page is **English** (the program's interface, however, is Russian).
3. Place it: a file in the repository (`site/index.html`) **and** publish it as a page by link.
4. Show the product with **live screenshots** of the application, not a drawn mock-up.
5. Screenshots — "do it as it is, a bit later we'll translate the interface, don't make callouts": the Russian UI in the shots is left as it is, there are NO English callouts over the pictures, and there is no disclaimer about the interface language on the page either.
6. The roadmap includes exactly what he chose: titles and transitions, colour correction and LUTs, fades + automation of any FX parameters, more audio FX, more export formats, packaging as deb/rpm/AppImage/Flatpak. **Do not put Wayland and UI localization on the roadmap** — he excluded them explicitly (the language "we'll fix it ourselves").

Incidental observation (a defect, not fixed in this round): the clip hint in the status bar promises "double click — FX", while `timeline.rs` on a double click sets a region on the span of the link group; the FX editor opens only from the right-click menu ("Effects…").

### R.19 — multilingual interface

"We need to introduce a multilingual system en/Spanish/Chinese/Arabic/ru for the program."

**This cancels the decision of R.18 item 6** ("do not put UI localization on the roadmap, we'll fix the language ourselves"). The new request is stronger than the previous exclusion.

The user's decisions on the clarifying questions:
1. Arabic — **RTL text, LTR layout**: the strings are reordered by UAX#9, the panels/toolbar/timeline are not mirrored (this is how Premiere and Resolve behave in the Arabic locale). He added: "if there is a lot of code — it must be split into files".
2. The glyphs of Chinese and Arabic — **a subset by translation catalogues, made by a script, the result is committed** (`scripts/subset-fonts.py`). Do not vendor the fonts whole and do not rely on the system ones.
3. The language on first launch — **auto-detection by the OS locale**; the user's choice overrides the auto-detection and is saved.
4. Scope — the whole GUI **and** the error messages that the user actually sees. And separately: "and if something remains in Russian — then it must be translated into English, and from then on all agents by default write all strings in English".

**A cross-cutting rule derived from item 4: the source language of the codebase is English.** Any new string literal (UI, log, `#[error]`, kt-cli output) is written in English; Russian UI text lives only in `i18n/ru.toml`. Comments and `docs/*.md` remain in Russian — this is the repository's convention; the point is about string literals. Enshrined by test 131.

### R.20 — landing page: generator, separate files, multilingual (3 items)

1. "make the landing page be generated from different parts (i.e. so that it is generated into html from something formalized)" → `site/build.py` + sources: `content/structure.toml` (page form, pictures, key chords, effect names, numbers) and `content/<lang>.toml` (prose only). The boundary is hard: the translator has nothing to break in the markup, and editing the markup does not touch the catalogues.
2. "I don't like that the resulting landing page is one fat html blob" → `site/dist/` with separate CSS, fonts and pictures. It was 446 KB in one file (89% base64, the timeline screenshot embedded TWICE); it became 29–40 KB per page with shared assets. The old `site/index.html` was removed.
3. "introduce multilingual… the language can be understood automatically from the browser. The language needs to be in the path for SEO" → `/en/ /ru/ /es/ /zh-Hans/ /ar/`, the codes are the same as in the application's catalogues. The root `/` is a dispatcher that repeats the logic of `i18n/detect.rs` (localStorage → navigator.languages → `zh-*` to simplified → English); it was checked on the same cases as test t130.

The user's decisions on the clarifying questions:
4. Translations: **en + ru first**; es/zh-Hans/ar are placeholders with English for now.
5. Untranslated languages are **generated, but with noindex**, and kept out of the sitemap and hreflang. Done without manual marks: the build counts the share of values that differ from English, and the `translated_min` threshold in structure.toml decides by itself. Once the catalogue is translated, the language is indexed from the next build.
6. The Artifact preview is NOT done any more — "only the site", there is no single-page build in the generator.
7. There is no domain yet: the base is set by `--base-url`, a placeholder `https://kinotrain.app`; before publication rebuild with the real domain.

Found and fixed along the way (defects inherited from R.18):
- the pictures did not have `height:auto` set when `width`/`height` were given in the markup — all scaled screenshots were stretched vertically (this was the complaint "the pictures are squeezed-stretched");
- the Latin font subset did not contain Cyrillic: the Russian page would have been solid "tofu";
- the old file had no `<!doctype html>` — the browser rendered it in quirks mode;
- fontTools stamps the build time into the font → the build was not reproducible, and `--check` caught a mismatch on every run (cured with `SOURCE_DATE_EPOCH`).

### R.21 — check of round 20 (2 items)

1. "the pictures fell off the site" — the files themselves have no defect: the paths `../assets/shots/*.webp` resolve both over http and over `file://`, checked in a real browser. The pictures disappear when the page is opened in a viewer that takes ONE html and does not serve the neighbouring files (that is how the built-in preview pane behaves — the fonts are empty there too). This is the reverse side of the decision of item 2 of R.20 "I don't want one fat blob": on an ordinary site the assets lie next to the page. So that this does not hurt the eyes, `python3 site/build.py --serve` was added — it builds and raises an http server over `dist/`; this is how it should be viewed.
2. "since the first versions of the site, when it is open the CPU is loaded heavily, although there is nothing going on there" — confirmed by measurement: **108% CPU in complete idle**. The culprit is the background "auras": three elements of 52vw with `filter:blur(90px)`, `mix-blend-mode:screen` and an infinite `transform` animation — the compositor was endlessly re-blurring and re-blending three huge layers. It was replaced with one static layer of radial gradients; at the same time the `backdrop-filter` on the sticky header and the last `filter:blur` under the hero panel were removed, and the 3D recalculation of the hero now does not run when the hero is off screen, and does not write identical values. It became **3.5%**, the look did not change.

   Rule for the future: on the landing page do not set up infinite animations over `filter`/`mix-blend-mode` — this is not "a bit of beauty" but a constantly running compositor. The softness is given by the gradient itself; blur is not needed for it.

### R.22 — screenshots in the page's language (1 item)

"and why are the pictures everywhere in Russian? make it so that everywhere it is in the needed language" — fair: with R.19 the application has five languages, and all pages hung one Russian set, taken still in R.18.

Done: `site/screenshots.py` — a re-shoot of all eight frames in each of the five languages. The window language is set by `KT_LANG` (that same debug override from app.rs); no clicks on the menu are needed. The coordinates are the same for all languages: the layout of the panels does not depend on the language, which is enshrined by test t127. The demo project (six tracks, markers, a region with loop, FX chains, a morpher) is generated by the same script, so the screenshots are reproducible, not "taken once by hand".

The result lies in the directories `assets/shots/<lang>/`; `build.py` substitutes the set by the language of the page and silently falls back to English if there are no frames for the language — a new language does not break the build. The Open Graph cards also became per-language: in the link preview there must be the same window as on the page.

Pitfalls along the way: at first I shot all frames in one run, closing the windows by clicking the cross. The morpher window did not close (its cross is not in the same place as in the FX windows), it covered the "File" menu, and both render frames captured the morpher. Now each frame gets its own run of the application — this removes a whole class of such errors.

The dist weight grew from 0.67 to 1.9 MB, but that is five sets: the visitor still downloads only his own language, and the page weighs the same.

### R.23 — seek to the right of the loop region plays the region, not the material under the cursor

"if I select a loop region and press play — everything works; then I click with the mouse to the left of the start of the region — the cursor jumps there as it should and plays what is there, then I click to the right of the end of the region — and the cursor also jumps, BUT — what is played is not what is under the cursor, but what is in the region on repeat. i.e. there is a bug on the face of it — on the left everything is ok, on the right it is not. it is very strange how the desync is built between what is played in the preview and the cursor displayed on the timeline". The second question: "isn't there a fundamental problem here?"

The cause is not a race of the clocks, but mismatched conditions of belonging to the loop: the transport (the cursor) wraps only on "crossed the end from inside", while the audio render, the prefetch and the display clock folded into the region any position to the right of the START of the region (a guard without an upper bound). The user's expectation, from his own wording: what is under the cursor must play, and the region and the Loop button are not reset in the process. → items 132–134, tests 132–134 (on the way, latent defects of the same class were uncovered and closed: reverse-loop silently broke around the circle, switching Loop off on the fly jumped the sound far ahead; the timeline cursor was moved to the same display clock as the preview and the timecode — invariant 12).

### R.24 — move to OpenTimelineIO (architectural round)

"we need to move to otio files, is it really possible to write custom information into otio files? do it so that we can be as compatible as possible with the otio files of other programs, read and write them, not break what we don't know how to read there when saving, and add our own so that nobody breaks ours either?" + "does otio have something like a json-schema/openapi?" + "I want to switch to otio right away … throw out the whole old ktproj code as much as possible".

The owner's direct decisions (after research and clarifying questions):

1. "shorten kinotrain to kt in the keys" — the namespace of custom data in OTIO: `metadata["kt"]`.
2. "ktindex must not be deleted — it is our cache and it is not tied to the project's files at all, or am I wrong?" — right: according to the code, the DB is keyed by the media path + size/mtime/xxh3, and it is tied to the project only by the file name (`with_extension("ktindex.db")` works for .otio without changes). We do not touch it.
3. "could we take some dead library, NOT a stub, and finish it (kt-otio)? …take a look at what is in those libraries, what crutches they have" — checked: there are no live ones (stubs from 2020, FFI over C++, the only pure Rust one is AGPL, which does not suit our profile). kt-otio is written from scratch; the techniques were looked at: typed metadata envelope, UnknownSchema passthrough, `OTIO_SCHEMA` versions.
4. "I always made two markers myself in the editor, but used them in essence as a marked_range — I'd like to move right away to the marked_range logic, but keep single markers if really needed." The gesture (his wording): "with a region selected and the cursor inside it — m gives a marked_range, with the cursor outside the region — a single marker at the cursor".
5. "similar conceptual things may still come up — ask about them too" — the cross-cutting rule of the round: conceptual divergences from OTIO are referred to the user's decision, not resolved silently.
6. Clip overlaps: **switch to a Transition inside the model** was chosen (not conversion at the format boundary): clips butt-joined, explicit transitions, video gets a real dissolve for the first time.
7. Media paths: "relative; fix the double counting; fallback/absolute — only if other editors have it" (they do: Resolve/Hiero write absolute file:// — the fallback is allowed).
8. Foreign incomprehensible constructs: passthrough + a grey placeholder.
9. Removal of the pinned t2_three_clip_overlap_is_one_over_n — sanctioned (the 1/N rule goes away together with the legitimacy of 3+ overlaps).
10. Import of old 3+ overlaps: split across tracks WITHOUT volume compensation, with an import report.
11. Multiple overlap/absorption on drag: a clamp up to the joint already in the preview + hint.
12. The fate of .ktproj: "for converting an old ktproj there must be a SEPARATE ITEM, as a PROJECT it is NOT OPENED ANY MORE, and there is no need to drag crutches along" — this CANCELS his own earlier wording "leave in the menu the ability to open a ktproj read-only". The open dialog is only .otio; a separate item "Import .ktproj…" converts and opens the result.

### R.25 — cross-render conformance with a foreign editor

The task in his words: "now we need to write a test that takes several otio files from the internet, generates synthetic sources, opens the otio through a foreign editor (very desirably headless) — renders it, and also renders in our program — in the end the rendered files need to be compared for gaps/discrepancies/etc."

Decisions of the round:

1. The foreign renderer — **melt** (MLT 7.22, the engine of Kdenlive/Shotcut), fully headless: "item 1. I installed melt". The path: the python adapter `mlt_xml` (otio-mlt-adapter) converts .otio → .mlt, then `melt … -consumer avformat:…`.
2. The .otio samples from the internet — **committed copies** (fixtures/otio-samples + an update script with a pinned SHA): "from the internet" is about the origin, not about the delivery method; the test must work offline and deterministically.
3. The comparison — **video + duration**; audio is deferred (the mlt adapter's audio is limited, and equal-power against a linear mix gives a guaranteed discrepancy).
4. The comparison tiers — **two**: a strict pixel-by-pixel comparison for files made of constructs that both renderers render; a smoke check (neither crashed + durations matched) for files with foreign constructs (nested Stacks, Premiere effects).
5. Reconnaissance of the real samples, before writing the test, found TWO defects in reading foreign .otio (a clip without `source_range` = the whole available_range; a clip with `media_reference: null` = unlinked) — decision: **"Fix in this round"**, not to bypass them with a preparation script.
6. A transition at the edge of a track (fade-in/out) was a v1 limitation "dropped with a notice" — decision: **"Implement fade-in/out"** for real. The reference library treats such a transition as a dissolve with a Gap, melt renders it, and a foreign fade must not be silently lost.

### R.26 — material under a crossfade must not be lost

A complaint in his words: "right now if I make a clip overlap and re-enter the editor — then when I drag things apart, for some reason the events come out cut; in Sony Vegas, if you drag overlapping events apart, they come apart normally, what overlapped stays without trimming. It turns out inconvenient if I changed my mind." And the question: "Is it really possible to do this normally or does it contradict otio?" — it does not contradict: the transition offsets are numerically equal to the trimmed material, so it can be returned exactly.

Decisions of the round:

1. The material comes back **when dragging things apart AND when deleting a neighbouring clip** — an "orphaned" clip gets its piece back.
2. "yes, I want the handles to stay, so that in the future the length of the fade can be adjusted (or let it be adjustable right now)" — the handles are visible on the timeline as a semi-transparent continuation of the clip under the transition window, permanently.
3. The fade-length adjustment itself is not changed: the edge of the window is pulled as before and is limited by the clip bodies; after the material comes back the bodies are longer — and the fade stretches wider by itself.

### R.27 — live checks are automated

His request in his words: "listen — these tests annoy me, maybe we can automate it so that it runs and screenshots everything itself in a virtual screen?" — after the live check of R.26 could not be carried out: the session went to the lock screen, and the kt-app window became unavailable.

Decisions: we install `xvfb + openbox` (a window manager is needed so that the window expands — the rule of R.13 "check in a maximized window"); the live scenarios are run both by a separate command `scripts/live-check.sh` and automatically from `finish.sh`. The scenarios check themselves (they read the .otio saved by the program and the screen pixels), and they take the coordinates from the program itself — otherwise they would break from any layout change.

### R.28 — 12 items after working in the assembled version

The key points, in his words:

- item 5 (⚠ a repeat of the requirement about UI/UX): "the attempt to intersect several events is also done dumbly — I haven't scrolled there myself yet — didn't understand that there is apparently still an event at the end that gets in the way, and it turns out there is a pale faded hint, everywhere in hints such situations are made as errors — this is a remark on the fact that you are once again ignoring my requirements on construction and convenience of UI/UX, WRITE IT DOWN FOR THE AGENTS".
- item 6: "I separate the events by moving the left one, and for some reason the left event "grows" a video track, without audio, how is that even allowed?!?! …everything must be precise — as much as there was, that is how much must remain; as much as I pulled out of the fade, that is how much the fade shrinks; as much as there was in the right event, that is how much remains; where the "paired" event was, there it remains and does not go anywhere / does not get trimmed. NOTHING ANYWHERE SHOULD GROW/BE CUT OFF… there should be no complicated logic of any kind. And right now you have a mess."
- item 9: "I am interested in a project option that will allow overlapping several events of one track at the same time… Make such an option in the context menu and turn it on by default… ALSO IT IS MANDATORY THAT THE BANNER SAYS WHERE IT CAN BE REVERTED. And oblige the agents too to THINK about the user in ui/ux."
- item 11: "marker here with a region selected must produce a region marker, not a single marker, you are again not thinking about the user, it is obvious that there are paired ways to do something and it is obvious that agents can always check such things during ui/ux development."
- item 8.x: a regression of playback (growing underruns in loop, the picture sticking, computer noise on small regions, worst of all on a clip with slowed_reverb): "It feels like the last edits broke something. Before, I jumped around both small regions and large ones, and everything worked CLEANLY."

Decisions of the round:

1. Overlapping of clips is a project option, ON by default, switchable from the context menu; at the first overlap a banner explains the consequences and says where to revert it; "no" switches the option off for the project.
2. Format: the layers are written as adjacent Tracks in the .otio + one field in our envelope metadata.kt. To the direct question "IS IT REALLY IMPOSSIBLE TO WRITE OVERLAPS, SIMPLY IGNORING THE STANDARD?" — it is really impossible: in OTIO a clip has no field "start on the timeline", the position is determined by the order of the children of the track, so two simultaneously sounding clips in one track cannot be expressed physically.
3. Import of .ktproj — "as it was, with overlaps": tracks are not multiplied, nothing is cut, micro-overlaps are preserved.
4. Order: first the critical (items 9, 1, 8.x, 6, 7, 3), the rest (2, 4, 5, 10, 11, 12) — in the next round. The second half was closed by the same round later: items 169–175.
5. The UI/UX rules from items 5, 9, 11 were recorded in apps/kt-app/CLAUDE.md (items 16–20 of the checklist).

### After R.28 — uniformity of the level

- "item 3. the fades must look the same for audio and video." Analysis: the fade window was drawn identically; what told them apart was the vertical volume rail of v1 inside the fade zone. The user's decision from the proposed options — **a line along the top with the PRIOR range of ±6 dB** (not "0 dB at the top"): the boost is kept as it was, at the cost that for audio the line sits by default in the middle of the clip. The rail was removed. Port of item 176.

### After R.28 — who is on top when overlapping

- "let's do it like in Vegas — on top is the one that came later." The v1 rule ("on top is the one whose start is smaller") was replaced with the Vegas one; the timeline strip and the hit test were brought into the order of the composition. Port of item 177. **Visible consequence:** in the 76 overlap zones of his project a DIFFERENT clip is now shown than before.

### After R.28 — a real film in the cross-render

- "on the internet I came across some open-source otio files of public films — I propose taking some film whose sources weigh less and also rendering it with both programs." Taken: Netflix Open Content "Sol Levante" (CC BY 4.0). The idea worked before the first render: the file immediately hit a gap with NTSC frequencies (item 178). The test — item 179; the media is downloaded by the script and is not committed.

### R.29 — GPU video rendering and the course toward AE compositing (architectural round)

The original wording (4 items):

1. "effects are computed on the cpu??? we have egui, and that presupposes a gl-like interface, doesn't it?"
2. "we need to think about the effects system as a whole, so that by default there is always gl/vk, and only if there isn't — the user gets a notice about it and the work goes on the cpu (and even then only for 2d)."
3. "In general I'd like to move in the direction of After Effects, i.e. the effects must have a compositing engine. And effects essentially start to be categorized into a 2d version and a 3d version (for example pan 2d, and pan 3d)."
4. "I don't know how such a system is properly structured, so that both 2d and 3d are in the project."

Decisions of the round (chosen by the user from the proposed options):

1. **Determinism: "Float GPU + tolerances".** The shaders are f32; byte-for-byte identity is kept within one machine/driver; the video golden tests move to tolerances GPU-vs-CPU (the CPU path remains the byte reference and the fallback); the public promise of the landing page "identical files, down to the byte" is qualified ("on the same machine"). Rejected: a fixed-point GPU with full byte-for-byte; "GPU only for preview, CPU for export".
2. **Depth of 3D: "Full AE", in stages.** Verbatim: "item 3. and don't forget that there may be several cameras. maybe it's worth taking something ready-made." The evaluation of ready-made engines was carried out: only glam is taken (maths); bevy_render/three-d/rend3/vello/skia were rejected (a different major version of wgpu, a GL stack, maintenance mode, irrelevant, a C++ bundle). To the question "use the camera as a texture?" — render-to-texture is built into the scheme by construction (all passes write into offscreen textures), "source = the view of camera N" is a stage of precomp; a real 3D engine, if needed, is connected as a SOURCE through the MediaSource trait, the compositor does not change for the engine.
3. **The first attempt: "infrastructure + port of 10 effects + the first 3D effect"** (Pan 3D: a layer in perspective + a camera, an end-to-end proof model→format→render→export).
4. Consequences sanctioned by this round: Noise is moved to a counter-based RNG (parallelizable; the noise picture changes — regeneration ONLY of the noise part of the golden of t26); the timeline semantics stay Vegas (the order "on top is the one that came later", fades, handles); the CPU fallback does not draw 3D effects (the clip without the effect + a notice), export of a 3D project without a GPU is a refusal with a clear error; the roadmap of the site is not touched without a separate request. The full plan — the discussion of R.29.

### R.30 — continuation of the R.29 course ("let's go further")

Upon completion of the first attempt (GPU + Pan 3D + cameras) the user replied "let's go further" — a continuation of the approved staged plan of the AE course. Attempt 2: **automation of video-effect parameters by keyframes** (matches the item of his public roadmap `n_fades` — "keyframe automation for any effect parameter"). Decisions within the already agreed plan: the domain of the curves is the clip progress 0..1 (as with CamMotion and the cameras); audio parameters — the next attempt (SlowedReverb already has automation, the volume has its own fade mechanics); the file with automation is written as v2 (the rule of "silent loss" from R.29).

### R.31 — check of R.29/R.30 on his machine (AMD, RADV)

1. "Right now my computer says there is no gpu acceleration" + the text of the error:
   `In Device::create_render_pipeline, label = 'pan3d' … Shader global
   ResourceBinding { group: 0, binding: 0 } is not available in the pipeline
   layout — Visibility flags don't include the shader stage`. At the same time
   "GPU: AMD Radeon Graphics (RADV RAPHAEL_MENDOCINO) / Vulkan, FX: CPU".
2. "about once every ~30 sec the program freezes, and on the graph the FPS drops to 0, as if some timeout were happening. I don't even have to press playback. and the cpu is audibly straining briefly at that moment."
3. "sometimes when I open the render window the fps also drops and does not recover until I close it."
4. "the render is broken — try to look at the pre[view] yourself."
5. Question: "does the cpu code use avx and similar simd instructions? I have an amd."
6. Cross-cutting (above in this same message): "don't forget to commit" — commit completed rounds on your own.

Decisions of the round: (1) the visibility of the uniform is VERTEX_FRAGMENT + a test gate "the adapter is there, the engine did not build = failure" (item 194); (2) a background autosave instead of a synchronous save from eframe::save every 30 s + a detector of long UI frames in the performance journal (item 197); (3) a cache of max_source_resolution by model_rev (item 196); (4) export: without post-FX memoization + a chunk budget per number of layers (item 195); (5) SIMD: none explicitly and deliberately — the CPU path is the byte reference, auto-vectorization for AVX would change the f64 sums and the determinism between machines; the acceleration is the GPU.

### R.32 — SIMD for CPU rendering

Confirmation of the fix of R.31 item 1: "Yes FX: GPU (AMD Radeon Graphics (RADV RAPHAEL_MENDOCINO))". A new requirement (a continuation of the question of item 5 of R.31):

- "we need simd for the cpu render, and it has to be done carefully somehow, what if there are no super-modern instructions."

Decisions: a runtime detection (`is_x86_feature_detected!`) with a SCALAR fallback, the build stays baseline x86-64 (no `-march=native`); two levels Scalar/AVX2 (a CPU without AVX2 gets exactly today's code); the SIMD kernels MUST be BIT-IDENTICAL to the scalar one — the goldens of t26/t11/t67 are not regenerated and serve as the criterion (invariant No. 11 is not weakened); `KT_SIMD=0` — a forced scalar. Port of item 198.

### R.33 — 10 claims: transitions, FX UI, automation, timeline UX

The round was brought as one list with the request "they need to be divided into stages, discussed and decided", "on any items I may be wrong — ask questions where I am wrong and about subtle points", "make notes for the agents". Separately it was said: **"Backward compatibility is not needed"**.

His claims in his words (only repeats are shortened, the wording is preserved):

1. "when joining events now the transition=fade is made by default (both for video and audio), I'd like to make different transitions, throw in 10–20 different modern ones, their management needs to be unified like the fx-chain."
2. "the ui/ux of the fx window is very meagre and not uniform. A revision needs to be carried out across all input fields (not limited to the fx windows), text, numeric, etc., and even, for example, the field "New source" — I'd like it to be possible to drag and drop a file into it, and to drag and drop from the project sources or to choose via opening the media widget. **The ui of the fields limps everywhere.** Also the morphers' fx-chain has crosses, you could also add checkboxes to switch the effects on and off and the ability to move the order with the mouse. And replace the fx-chain of event effects, **because in two places the same meaning is done, and neither is finished**. And also — for the same event with magnetized tracks, you could give all the chains at once in one window, it should be simpler that way. Well, one could give the ability to hide-switch. Also a master checkbox to disable the current fx-chain is lacking, in morphers and fx batch likewise, everything should be unified."
3. "all effects have very few control parameters."
4. "automation of all parameters of all effects is needed, not only of slowed&reverb."
5. "the ui/ux of automating all parameters needs to be thought through very well, since in v1 it was done absolutely disgustingly and inconveniently, especially when you have 10 effects with complex transitions and they need correcting across different events. Now in v2 everything is done differently everywhere: in slowed a graph, in pan3d a list-table where each row is one parameter… I'd like a unified automation control widget, and so that one can switch graph<->table-list, and don't forget about copying. The graph should be made convenient, like the project timeline (reuse the widget's code). And in the current table-list there should be several parameters per row, not one, and they should be movable with the mouse (and right now there is a stupid thing: the program seems to reset/sort neighbouring values if I type after t=1.0 the next row t=0.5, in short, it is inconvenient). And the preview is also lacking there, you guess somehow from the normalized time, whereas it could be done so that the cursor inside the mini-timeline moves the cursor on the main timeline, so that you immediately see what it refers to. Of course one must take reversing and speed into account too. By the way — **it is not clear why we work in normalized time?**, why not write normal time by default right away, you can even work with both right away, on the timeline/table of fx automation write either one or the other, without cluttering."
6. "There's not enough copy-paste — both of individual effects and of fx-chains, and of automation (with the future timeline it turns out you can select and do ctrl+c ctrl+v and so on?)."
7. "on the main timeline area-selection is lacking, right now with the cursors you can select only everything that is in them; it could be done by holding ctrl to select with an area, or as in Vegas — a toggle button. And of course this should be taken into account everywhere, including in the creation of morphers."
8. "on a double click on a marked_region it must be selected entirely; if there are several overlapping ones — then by the region's name."
9. "when rendering with a marked_region selected, the name of this region must be substituted into the output file; in general the template for all renders should always be "kt-<projectname>-<region_name>-<its length in seconds>…"."
10. "in rendering and preview, the absence of sources must not be a blocker, the user needs to be notified and rendering must go on; for missing video a placeholder must be substituted."

**The user's answers to the clarifying questions (these are the decisions of the round):**

1. **The domain of the automation keys — "to the material"** (the option "like Premiere/AE and as already done in Slowed"). This is also the answer to his question "why do we work in normalized time": the domain "clip progress 0..1" was chosen in R.30 as the simplest for the engine (the same as for cameras), but nobody chose its semantics consciously at trimming — it is "rubbery", and `Split` duplicates the whole curve onto both halves. Now the key sits on its own frame of the SOURCE: trimming hides/reveals keys without distorting the animation, Split divides the curve by itself, reverse mirrors by itself, the speed is taken into account by itself. The two domains (the video ParamCurve and SlowedKeyframe) are merged into ONE. In the UI seconds are shown + the normalized value in the second plane.
2. **Transitions: all three families, ~16 types.** Wipes (linear ×4 directions, diagonal, iris, clock, shutters, luma), shift (push, slide, cross-zoom, spin, whip-pan with motion blur), stylized (blur-dissolve, pixelate, glitch/VHS, film burn/flash, dip-to-colour black/white/custom). The default of the join stays Crossfade — the new types are chosen explicitly.
3. **Audio at a transition — a crossfade with a choice of curve** (linear / equal-power (default) / S-curve / exponential). We are not making stylized audio transitions.
4. **A double click on a clip opens the FX window** (the status-bar hint has promised this since R.18 and has been lying until now); a region by event — Shift+double click and a menu item.
5. **Frame: Ctrl+drag always + a toggle button "Selection tool"** (in button mode an ordinary drag on empty space draws a frame). Ctrl+click — add/remove from the selection. Multiple selection of events is introduced (right now exactly one is selected), operations on the set: move, delete, copy-paste, morphers, FX.
6. **A double click on the strip of a range marker** sets a region by its boundaries AND selects the events inside; when several overlap — the one whose flag/name was clicked.
7. **Order of stages: C6 → C1 → C2 → C3 → C4 → C5** (first the quick visible wins, then the foundation).
8. **The placeholder for missing video is a dark banner with the file name and an icon** ("you see at once WHAT is missing"); audio — silence. Notifications: a notice list when the project is opened, a warning in the render dialog BEFORE the start, counters of what was lost in the summary AFTER.

Decided by me, not by him (he may correct it): the render name template `kt-<project>-<region>-<len>s.mp4`, where len is the whole seconds of the range; a region without a matching named marker (or with an empty name) — the segment is omitted; the names are sanitized (spaces→`-`, forbidden file-system characters→`_`).

**Division into stages** (each — its own attempt with its own tests):

- **C6** timeline and render UX: multiple selection + frame, double clicks, the file name template, placeholders of missing sources (his items 7–10).
- **C1** foundation: a unified domain of keys, parameter descriptors (min/max/step/units/label — one source of truth), automation of ANY parameters including audio, the master switch of the chain, the copy-paste model, format v3 (his items 4, 6 and half of 2).
- **C2** a unified fx-chain widget (events/morphers/batches), a revision of ALL input fields, the window of all chains of an event, the source field with DnD and a media picker (his item 2).
- **C3** a unified automation widget: graph ↔ table, the mini-timeline moves the cursor of the main one, copy-paste of keys (his item 5).
- **C4** transitions (his item 1). **C5** more parameters for all effects (his item 3).

**Known defects found by the reconnaissance of this round** (they are fixed in their own stages, recorded so that they are not lost):

- `Split` copies the automation curve entirely into both halves — the animation is duplicated (C1, disappears with the change of domain).
- The "+ keyframe at playhead" button of Slowed puts the timeline progress into the source domain: the point lands not under the cursor when rate ≠ 1 (C1).
- The graph of Slowed is cut off on the rate axis at 2.0, while the numeric field lets you go up to 4.0 — a point above it cannot be placed with the mouse (C3).
- The same parameters have DIFFERENT ranges in the FX window, in the morpher window and in `param_specs()` (a third source of truth, used by automation) — C1/C2.
- Unsalted ids (`Grid::new("vfx")`, `ComboBox "panmode"/"pan3dcam"`) collide when the chain section is called twice in the morpher window (C2).
- The labels of FX parameters — 37 raw English literals outside the i18n catalogue (C2).
- `set_fx_coalesced`/`set_fx_auto_coalesced`/SetFxBypass/SetTransition do not bump `model_rev` — the background autosave may clear the asterisk with unrecorded FX edits (C6, item K1: this is a class, not a single function).

Not done without a new request: kt-cli still ignores the region on export (a divergence from the GUI); the roadmap of the site is not touched.

## Round R.34 (2026-09-06): one animation mechanism for everything

After the closing of R.33 the user asked "what's next?", heard out the three remaining tails and decided:

1. **kt-cli and the region — "we don't do it"** (a direct answer to the question). The divergence from the GUI remains deliberate.
2. **The rest — "do it"**: a general refactoring of the automation widget (it stops knowing about the clip) → cameras and CamMotion onto it → automation of the transition parameters next.

The order and the justification are mine, accepted by his "do it":

- **K1** `AutoCtx` stops being clip-dependent: the domain, the range and the translation "position of the curve ↔ timeline tick" come from outside. Without this neither a camera nor a transition can be fed to the widget.
- **K2** CamMotion **merges with Pan**: Pan already has pan_x/pan_y/zoom in the specs, so "CamMotion with keyframes" is exactly "Pan with curves". The conversion on reading removes a whole duplicating mechanism instead of porting it.
- **K3** cameras get a **properties window and curves**. In the course of the reconnaissance it came out that there is no camera editor at all: an event can be created from the menu of the camera track, it is drawn as a turquoise block — and that is all; it cannot be selected, moved, deleted, or its parameters corrected. Meanwhile the comment in `add_camera_at_playhead` promises "parameters are edited in the camera window (double click)" — that is, the code lies exactly the way the FX hint lied before item 204.
- **K4** transitions: curves on the parameters of the kind. Keys are ticks from the start of the transition window (not a fraction): "nothing grows by itself" (item 20 of R.28), editing the edge of the window opens/hides the keys, like trimming a clip, instead of stretching them.

## Round R.35 (2026-09-06): morphers — layer, time, areas, copy-paste

Five claims, verbatim:

1. "I'd like the morphers to be active also in time, i.e. if we "play" them then they also morph/apply effects. So as not to lose space, the morph tracks can be reduced in height three times. The time-activity checkbox is enabled by default for each morpher in the widget."
2. "I'd like the morphers not to be tied only to specific events, but to be able to also work with events by a reference to a marked_region, and also — the area-selection can be made saveable as a marked_region in the form of a marked_area, and also referenced from there; both options are convenient and important. And if the user changes his mind, he can change/delete/add a region/area in the morpher widget to other ones."
3. "The item "Add morpher" (both for clips and for a region) can be cut from two to one, and in the morpher ui they can be merged (maybe make the fx-chain right away, in which, besides the other fx, there are fx for morphing — mark their items purple and raise them in the list)."
4. "When selecting everything in a row on the timeline and doing ctrl+c ctrl+v, I want the links to be set also for morphers (i.e. I copied with a morpher — then it will be aimed at the new objects), and marked_areas are also copied when selecting and copying, and marked_region also together with those inside it. The names are unique, and you can ask the user to enter a prefix, or it can be generated in the field himself."
5. "One more mode of morphers — by default, the morphers apply to everything that is BELOW this morpher, and there is no need to torment yourself with the selection. The morpher's tracks by default (when the toolbar button is pressed) we create on top of all."

Plus the earlier conditions of the round: backward compatibility is not needed, ask questions where he may be wrong, make notes for the agents.

**The user's answers to the clarifying questions (these are the decisions of the round):**

1. **"Below" is counted BY ROWS, and the order of tracks moves into the model.** The question stood thus: today `lane_order` is a purely visual property in `UiState`, outside undo, and the engine does not read it at all; if "below" is counted by rows, the picture will start depending on the arrangement of tracks. The option "by rows, the order → into the model" was chosen: dragging a track = changing the edit, and this goes through undo. "The morpher acts on all tracks" and "count by rows, but keep the order in UiState" were rejected (the latter would make the render depend on UI state that undo does not return).
2. **A partial overlap — the target is CUT along the block boundaries** (like an adjustment layer), not "they intersected — we patch the whole thing". The conscious cost: for audio the chain on the piece starts anew, the reverb tail is not carried across the boundary.
3. **Morph tracks AND camera ones — three times lower** than video/audio (both are "thin": only events are on them).
4. **A reference to a range/area — BY NAME**, the names are forcibly unique ("main", "main 2"), renaming itself corrects the references of the morphers. The option "introduce real ids" was rejected: markers today are addressed by time, and translating all five marker commands to ids costs more than the uniqueness of names, which he already asks for in item 4.
5. **The single item "Add morpher" ALWAYS creates a morpher in the "below" mode** — predictably, not a "smart default from context".
6. **On insertion — a dialog with a pre-filled prefix** (not a silent generation).
7. **Bake honestly cuts the clips**: the invariant "bake == what was drawn" remains, but new joints appear on the timeline. "Forbidding bake for the layer" was rejected.
8. **The border of the layer STICKS to the edge of the crossfade and of the overlap zone, and this is visible.** The question arose from a finding of the reconnaissance: the cut breaks not the target itself but its NEIGHBOURS (for video the Vegas rule "on top is the one that started later" — a piece stuck out over the neighbour; for audio the crossfade ramp is computed from the geometry of the clips). The exact but expensive option "the engine learns the geometry of the original" was rejected: it creates a second source of truth about geometry and deprives bake of precision. It is recorded as a possible future stage, if the sticking gets in the way on live editing.
9. **Order of stages: layer → chain → height → area → copy-paste.**

**Found by reconnaissance; the user did not complain about this:**

- An audio clip with its OWN `slowed&reverb` cannot be cut at all in principle: the speed warps the consumption of the source, and the entry point of the piece computed linearly would point to the wrong place. Such a clip is patched whole, and this is shown.
- The default order of rows (`V..A..M..C`) puts the morph track at the VERY BOTTOM, so "everything below it" = empty. That is why the single menu item must also raise the morph row to the top — which matches his own request in item 5.
- The master switch of an event's chain (`fx_off`) also switches off the morpher's effects, because the morpher appends them to the same chain. For a layer this is wrong.

## Round R.36 (2026-09-07): reconnaissance of other editors, the base for future features

The occasion is not a defect but a finding. The request verbatim:

1. "We need to carry out a research of the features of commercial editors. Here I came up with Morphers, and then by chance I read on reddit that a person switched to the fresh version of Vegas because of some "Adjustment Events", and it turns out these are almost our Morpher tracks, but there they are only for fx."
2. "Now I'm interested in all the tricks of the main editors."
3. "I'm interested not just in a list of tricks, but in the "tricks behind the tricks", the simplest example — **the morpher in Vegas lets you control the transparency of the effects overlay with a slider, and we do not have that.**"
4. "I'm interested in the pros and cons, the feedback of users."
5. "Create a database of our future features, put each note into it (it is better to make a file tree with categories and sub-categories)."

**Answers to the clarifying questions (they are the decisions):**

1. **Scope — a broad slice of ~50 notes** across all 13 categories (3–5 in each), not a full tree of 118 and not a targeted 24. The tree is set up in full, it is filled halfway; what is not covered is visible in a table in the README.
2. **Editors — a core five**: Vegas Pro, Premiere Pro, DaVinci Resolve, Final Cut Pro, After Effects. Avid/Fusion/Nuke, open-source and AI editors (CapCut, Descript, Runway) are NOT included in the first wave.
3. **Format — YAML frontmatter + an index script** (not a passport table). Accepted in spite of the convention of docs/ "no machine-readable": the user asked precisely for a DATABASE, and the summary tables must be assembled, not maintained by hand.
4. **Method — parallel research agents** (in batches), not a sequential collection in the main dialogue.

**What the check found before the work began** (the user did not complain about this, but it changes the statement): the Vegas Adjustment Event diverges from our morpher not in one place but in two, and the second is more important than the first.

- **The transparency slider** — their event has an opacity envelope AND fades at the edges, that is, the effect is blended PARTIALLY. Ours is a binary overlay: `MorpherEvent.enabled` on/off, and `MorphPatch` has not a single strength field.
- **What exactly is processed.** Vegas and AE apply the layer's effects to the ALREADY MIXED picture of the tracks below. We, in `Project::effective_clips()`, patch EACH clip separately, and the FX are computed per layer BEFORE the composite. Because of this, the "layer transparency" has nowhere to be implemented in our code without touching the composite (invariant No. 7 "the composition code is one"), — and for the same reason our layer can do what theirs cannot: substitute the source and the entry point for each target.
- **Restriction by tracks** — here the reconnaissance corrected me myself: Vegas has NO choice of tracks. The only way to narrow "everything below" is to make the needed tracks compositing children of the carrier track, that is, to rebuild the structure of the project's composition (and conversely: rebuilding the layout silently changes the zone of action of the grade). For us this is `MorphTargets::Areas` with the `lanes` field — declarative and without rebuilding the project. Here we are not catching up, we are ahead.

The answer to the premise "they are only for fx there": not only — the Adjustment Event has trim, split, copy-paste, mute/lock, like an ordinary event. But we have what they do not have: the substitution of media and the shift of `source_in` (the morpher as a "substituting layer").

The catalogue of the reconnaissance is `docs/features/`, the rules of keeping it are in its `README.md`, the decision "why it exists and why the axis is by features" is item 245.

## Round R.37 (2026-09-07): slowed&reverb — time domain, video, preview

**Claims verbatim:**

1. "slowed&reverb counts time in the automation God knows how, take as a rule for it — that t=0..1 must be taken neither as the time of the source, nor as the time of the source taking into account the slowing-speeding, but as the time interval cut out by the current event and WITHOUT accounting for the slowing-speeding."
2. "slowed&reverb for video (a separate checkbox by default — with some simple generation of frames, first ordinary and later fruc_vulkan in the future). The effect for audio/video can be combined right away, rather than added separately."
3. "slowed&reverb must be taken into account in the preview of audio/video in the events."

"Backward compatibility is not needed. On any items I may be wrong — ask questions where I am wrong and about subtle points. Make notes for the agents."

**What exactly he caught (the reconnaissance confirmed three different defects in item 1):**

- The axis of the `rate` curve is computed through the warp, that is, it DEPENDS ON THE CURVE ITSELF: move a value — and the point drifts across the screen.
- An early speed key shifts WHEN the late keys of OTHER parameters take effect (`mix`, `gain`, `pan` are also indexed by the consumed material).
- The right part of the graph is unreachable: the axis is marked by the `duration` of the material, but one can only get as far as `duration · avg_rate`. The auto-key which the editor sows itself sits exactly in this dead zone — that is, the curve by default has a key that will never fire.

**The user's answers to the clarifying questions (they are the decisions of the round):**

1. **The keys remain in ticks of the MATERIAL, the axis is linear.** Everything he voted for in R.33 (trimming does not distort the animation, Split divides the curve by itself, reverse mirrors by itself) is preserved; only the warp is removed from the axis. This cancels exactly one phrase of the decision of R.33 — "the speed is taken into account by itself".
2. **An effect instance on EVERY event of the A/V pair.** One addition puts both; `rate` and its curve are ALWAYS in sync; the reverb and the frame mode are separate; deleting one removes both; breaking the link group stops the synchronization.
3. **The parameter "Frames", the default is "nearest"** (we hold the frame); "blending" (a linear blend of the two neighbouring ones) is done in this same round as a separate stage; fruc_vulkan — the third mode later.
4. **`GOLDEN_AUDIO` is regenerated EXACTLY ONCE.** The golden project of test 11 contains a slowed with the rate automation, and the change of domain changes the sound deliberately. Plus new direct tests of the new semantics, so that the reference is not the only guard. `GOLDEN_VIDEO` does not move.
5. **The uncommitted work of R.36** (the `docs/features/` catalogue) — commit it as a separate first commit, not mixed with this round.

**Decided by me, not by him (he may correct it):**

- "A separate checkbox" for video is the SLOT CHECKBOX in the chain strip, not a new field: it already exists, is on by default and is drawn as a strikethrough. The checklist item 9 forbids introducing a second switch with the same meaning.
- The rule of item 1 applies to ALL parameters of the chain, not only to `rate`: right now `mix`/`gain`/`pan` are also indexed by the consumed material.

**Division into stages** (each — its own attempt with its own tests):

- **C1** the automation domain: the warp map is turned into output time (`S = ∫rate` instead of ln/expm1), the processor samples by `tau`, "material ↔ timeline" stops depending on the chain of effects.
- **C2** video slows down, "nearest" mode: one point of editing (`clip_material_pos`), from it prefetch, preview and export follow by themselves.
- **C3** one effect per A/V pair, a synchronous `rate` (the rule in the model).
- **C4** the preview of events: the waveform and the filmstrip on the same map.
- **C5** the "blending" frame mode.

**Known defects found by the reconnaissance of this round** (they are fixed in their own stages, recorded so that they are not lost):

- `RATE_CLAMP` is applied INCONSISTENTLY: `segments()` clamps the ends and then interpolates, `eff_rate()` interpolates and then clamps. They diverge if a key lies outside 0.25..32 — the map and the DSP drift apart, and this is misses of the prefetch and silence. Such a key cannot be set by hand from the UI, but `migrate_legacy_slowed` copies the value without a clamp (C1).
- The `ease` checkbox is drawn also for the `rate` curve, where it does NOTHING — it is forcibly switched off in `from_curves` (C1).
- `warp_prefetch.rs` (t17) fills the LEGACY `automation` field when `fx_auto` is empty, and the migration is not called in the tests: it has no curve at all, and it has been under-tested since R.33.
- `integral_map.rs` asserts nothing — it only prints (C1).
- The domain of audio automation is NOT mirrored under `reversed`, while the UI mirrors it: for a reversed audio clip with curves they diverge (a defect predating this round).
- The anchor of the reverse mirror for audio includes `WARP_SOURCE_MARGIN_FRAMES` (8192 frames ≈ 5 video frames) — a reversed A/V pair will drift apart by these frames until the anchor becomes margin-free (C2).
- `windows_on` goes to the reference modes BEFORE the veto on the warp: warped clips are cut by the morpher layer in the `Marks`/`Areas` modes (C2).
- `Split`/`TrimStart`/`punch_out_region` count `source_in` linearly and lie on a warped clip — already today, for audio. Outside the scope of the round, recorded.

## Round R.38 (2026-09-07): dead ends — you can create, but you cannot manage

**Claims verbatim:**

1. "I don't understand how to delete a saved area region, I saved it and now it hangs there, I don't understand how to rename it — RMB gives nothing"
2. "I can't in any way add a morpher event to a morpher track"
3. "add morpher layer also works strangely"
4. "if I press the same add morpher layer in another area — it creates one more track for me"
5. "I don't understand how the camera track works"
6. "in the fx-chain widgets it is problematic to activate the input of parameters"
7. "if in the Source crumb I specify a source with a long name — the whole chain flies apart by its length"
8. "also if you throw in many crumbs — the last label runs into the edge"
9. "I don't understand how to change the length of a morpher event"
10. "increase the height of the morpher track by 2 by default"
11. "allow changing the height of each track individually"
12. "the morpher has no possibility to apply fade and opacity — the plan is not kept"
13. "in the morpher I cannot set the automation of effects"
14. "in the automation timeline I don't understand how to zoom"
15. "…when pressing a key — so that the values written in the fields have a link to the key"
16. "it feels like the effects have some separate "default" state of values"
17. "I added a second automation parameter — but only the first one is edited"
18. "the control buttons for the elements of the automation timeline are still lacking"
19. "when copying elements — by default you should put not a prefix but a suffix"

And the main thing — not about the code:

> "For all items, examples of how you did not think about the user and duplicated everything everywhere, half of it is a dead end. Double-check your other things too. I'm tired of reminding the agents to think about these trifles. OBLIGE THEM TO CHECK THIS, WHAT IS THE PROBLEM??"

"Backward compatibility is not needed. On any items I may be wrong — ask questions where I am wrong and about subtle points. Make notes for the agents."

**The answer to "what is the problem" (he is right, and here is the mechanism of the failure):**

The checklist in `apps/kt-app/CLAUDE.md` ALREADY forbids three of these claims verbatim — rule 1 (hit with the mouse the first time) for item 6, rule 7 (a long name does not stick out) for items 7–8, rule 17 (paired ways are equal) for item 4. The same place says "a repeated remark = the rule was not written down". The rules are written down. It did not help. **Prose drops nothing.**

Proof that settles the dispute: in `libs/kt-model/src/model.rs` lie four written and unit-tested commands — `RemoveArea`, `RenameArea`, `SetAreaRange`, `SetAreaLanes` — and NOT A SINGLE call in the whole of `apps/`. The tests are green, the feature "saved areas" has shipped, and an area cannot be deleted or renamed at all (his item 1). The tests checked the model, not the REACHABILITY.

Therefore the round begins not with a UI edit but with tests that fail `cargo test`: "no orphaned commands", "each entity has a full set of verbs", "a click is a click", "the text fits".

**The user's answers to the clarifying questions (the decisions of the round):**

1. **Scope: first the dead ends and the widgets** — the mechanism + items 1–11 and 19. The automation as a whole (items 12–18) goes in the next round, it is coherent.
2. **Item 13 is postponed** together with the strength envelope: the domain of the morpher's curve must be chosen together with item 12.
3. **Item 3:** if there is a region — a block by the region; if not — from the cursor. The same rule as for the M gesture.
4. **Item 14:** a lock button "link zoom with the timeline", enabled by default.
5. **Item 19:** the default suffix is the word "copy" ("final copy", on repetition "final copy 2").
6. **Item 12:** video and audio at once.
7. **Item 11:** a drag of the bottom edge of a track in the gutter.

**Decided by me, not by him (he may correct it):**

- **Item 10 "increase by 2" = TWICE**: the morph row 28 → 56 px from the base 84. The camera one stays three times lower, the rest is covered by item 11.
- **Items 15/16 — one cause**: the parameter field with a live curve edits the BASE, but shows the curve's value. That is why there is a "separate default state". It is fixed by making the field edit the SELECTED KEY. Both — in the next round.

**Decisions of the second half of the round (items 12–18), taken along the way:**

- **Item 12: the strength of the layer is an operation of the CHAIN, not a field of the event.** This way it gets automation for free (the fades are its curve), enters the key of the post-FX cache and does not touch the event format. The blend goes with the state BEFORE the layer's contribution: the strength rolls back the layer, not the event's own effects.
- **Item 13: the domain of the layer's curves — the time of the BLOCK.** The morpher has no material: it is a layer of edits. The same domain as the camera and the transition.
- **Items 15–16 — one cause, and it was confirmed:** the field edited the BASE, while the engine took the value of the curve. Now the field edits exactly what it shows.
- **Item 17:** a new automation parameter becomes the ACTIVE curve (before, a click on the graph kept putting keys into the first one).
- **Item 18:** the buttons live in the automation widget itself, so all carriers of curves get them at once — the event slot, the camera, the transition, the morpher.

**Dead ends of the same class that he did not name** (found by reconnaissance, fixed here too):

- `Drag::MarkerEdge.left` is described in a comment as working, there is even a test matching this variant — but only `left: false` is constructed, and the release handlers also match only it. A drag of the left edge of a marker silently goes into `_ => {}`. A second case where the PROSE asserted a working feature.
- `last_band` is set in one place and never cleared — the documented branch "there was no frame, take the region" after the first frame of the session is unreachable.
- `MarkedArea.color` is a live field of the format, always `None`, there is no UI for it.
- The hint says "empty space" above a real morph/camera block.
- `region_edge_at` is checked BEFORE the morpher and the camera: a block whose edge coincides with the boundary of a region cannot be grabbed.
- The duration of the camera is a read-only label in the window; there is nothing to change it with.
- Inserting an FX chain into a morpher silently loses the curves (part of item 13, postponed).
  → closed in R.48 (item 342): the type `ChainEdit.auto` stopped being an `Option`, and the silent branch no longer exists; guarded by t343.

## Round R.40 (2026-09-07): the second wave of reconnaissance — hints, help, guides

> **Why 40, not 39.** The section was written as R.39 and was committed under this number
> inside someone else's commit (`7cdcc88`). Meanwhile a parallel session was running its own round under the same
> number — warp, curves, reverse (commits `d7396c9`, `db6fd9e`, `8591348`,
> `b86e2c2`, items 270–272). Their number is already in the immutable commit messages, mine was
> only text, so mine was renamed. R.39 stays with the warp round; its entry
> in this file belongs to that session.

The request verbatim:

> "Now I'm interested in the same template — the system of hints/documentation/guides, so that later I can compare everything and make it for myself."

**The answers to the clarifying questions (they are the decisions):**

1. **Coverage — the five core NLEs PLUS strong non-NLE software**: Ableton Live (Info View), Blender, VS Code (the command palette), JetBrains, Photoshop. The rationale for the question, with which the owner agreed: in this domain the strongest solutions were born OUTSIDE video editing, and limiting it to five NLEs would miss the main thing.
2. **Scope — about 35 notes** (in fact 33: rule 2 of the catalogue — if there is nothing to write in "The trick behind it", the note is not created — costlier than a round number).
3. **The decision "there are no pop-up hints" — UNFROZEN.** See the separate paragraph below.

### Unfreezing item 14 of R.8 (important, this is a change of a standing requirement)

The requirement "there are NO pop-up hints at all, the only place for a hint is the bottom right of the status bar" stands since item 14 of R.8, enshrined in `theme.rs` (`tooltip_delay = 1.0e6`, a barrier delay that mutes even foreign `on_hover_text` from egui) and guarded by the regression guard of test t83 (not a single `egui::Order::Tooltip` layer). By the catalogue's rule such a thing does not change without a new request — **and this is that request**: to the direct question the owner answered "you may propose a revision, if the reconnaissance shows that otherwise it cannot be taught".

What this means in practice and what it does NOT mean:

- The reconnaissance may propose a revision — but it is obliged to do so at a high cost: a note touching the prohibition must quote the owner's words it contradicts, name t83 and write exactly what in it would have to be weakened.
- In itself this does NOT cancel either `theme.rs`, or t83, or rule 10 of the checklist. Not a single line of code changes in this round: the wave ends with verdicts.

**What the reconnaissance found before the work began** (the owner did not complain about this, but it changes the statement): the word "tooltip" glues together THREE independent decisions, of which the owner accepted two, and the third nobody accepted — it is a consequence of the chosen slot.

1. **Overlap** (the popup covers what you are looking at) — accepted by the owner, item 14 of R.8.
2. **Delay** (the hint does not appear immediately) — accepted by the owner verbatim: "I want it to be there always, this is it, so it is not there on hover, is that how it works?".
3. **Capacity and direction** — one line of ≤160 characters, and to find out what it says you must ALREADY hover over the object. Nobody decided this.

Our line is an ideal **pull** channel and is not a **push** channel at all. The cry "WHERE DID THE BUTTON HINTS GO?!?!?!" (R.9 item 1) and "I don't understand how to delete a saved area region" (R.38 item 1) are not one problem solved halfway, but two different ones, of which the first is solved. **You cannot hover the mouse over what you have not found.** Seven of the nineteen claims of R.38 live in the third line and only in it.

Therefore the subject of the reconnaissance is declared to be item 3, and items 1 and 2 are not touched in the verdicts: the notes look for a SECOND SURFACE that does not cover and does not delay (the context menu, the label itself, a place where the user comes deliberately), and not for a return of popups.

### State of the domain at the start of the round

There is no help at all: no Help menu, no F1 (the key is free), no "About the program", no key cheat sheet in the program, no welcome screen, no demo project from the GUI, no user manual, no FAQ, no CHANGELOG, no link to the site. `Ctrl+Y` and `Backspace` are not described anywhere. Item 19 of the checklist ("the dialog must say where the decision can later be changed") is not fulfilled by a SINGLE line of the catalogue. At the same time the descriptions of the effects are already translated into five languages — but they lie in `site/content/*.toml` (`[fxdesc]`), not in the program, and we already have a documentation pipeline (`site/build.py --check`, `site/screenshots.py` drives a real window through `src/scripts/ui-probe.py`).

The catalogue of the second wave is `docs/features/help/`; the rules of keeping it are common with the first wave.

## Round R.41 (2026-09-08): bug trackers of open editors

The request verbatim:

> 1. We need to carry out a research of the features of open editors. Collect their list, find their bug trackers.
> 2. Dump the bug trackers.
> 3. Carry out an analysis of what they complain about, what they want, where the pros and cons are.
> 4. Supplement the DB.

**The answers to the clarifying questions (they are the decisions):**

1. **Where the findings go** — "Add a new category if you consider it necessary, **we are collecting features and requests**". The category `resilience/` (code RES) was set up: the preservation of work, recovery, behaviour under load and on foreign hardware. The caveat about "requests" was accepted as an equal half of the work: the `wishlist` part of the trackers is literally "what the users of foreign editors want", that is, the direct subject of the base.
2. **Trackers — all 12**, including three architecturally close to us: MLT (the engine under Kdenlive and Shotcut — the only public example of our construction "engine + applications"), OpenTimelineIO (OUR project format) and the FFmpeg trac (we depend on the system FFmpeg, invariant 10).
3. **Token** — "I will put the token into .env in the root of the project".
4. **Scope — broad, ~45.**

### About the token and `.env`

`.gitignore` did **not** cover `.env`, and this was closed by the FIRST action of the round, before the file appeared: `/.env` and `.env.*` were added, verified with a trial file (`git check-ignore` + `git status`). `tracker-fetch.py` reads from there exactly the key `GITHUB_TOKEN`, never prints it, logs it or saves it; the output shows only "token: present/absent". Without a token everything works, only more slowly (60 requests per hour instead of 5000).

### The axis of the base is untouched

A category `kdenlive/` or `foss/` cannot be set up — item 245 rejected the axis by editors, and rightly so: for a note "what they complain about in Kdenlive" the verdict "take/do not take" has no subject, and "The trick behind it" cannot be written. Therefore **the tracker enters as a source, not as a column**: the registry is in `docs/features/_trackers.md`, the findings are spread across the existing notes, across new notes in the old categories and across `resilience/`. Not a single line of the ranking is left without an assignment — otherwise the result would be "we found 340 bugs about proxies and did nothing with it".

### Protection against statistics instead of a finding (rule 12)

A note that declares `trackers:` must carry a line `SIGNAL:` with two numbers and a share (the count of one tracker against the share of another) and a line `ADDRESS:` with a path `src/…`. The `SIGNAL:` filter is not repeated across two notes. This is the third in turn protection against an exit: the first wave forbade retelling Wikipedia, the second — the conclusion "we need a reference", the third — a lone number.

### What was found before the work began

The method was run on the full dump of Kdenlive (6346 issues, a single query), lexicon coverage 77.8%. Two findings, for the sake of which the wave is undertaken:

- **The class "foreign hardware and drivers" is fixed worse than all the other topics**: the share of real fixes is 0.26 against 0.38–0.61 for the others. A foreign driver cannot be fixed from the application. For us this is a measurable cost of invariant 10 (the system FFmpeg via dlopen) — and against it we already have a mechanism they do not have: a chain of encoders with a real check of opening (invariant 9).
- **"Loss of a project" is the topic with the maximum density of duplicates** (0.23 duplicates per issue against 0.075 for effects): rare and rediscovered anew, people stumble on it independently and do not find a predecessor. Meanwhile at us the autosave **overwrites the user's file**, there are no project versions, and the write is atomic via tmp+rename, but without `fsync`.

Also: at Kdenlive `WORKSFORME` — 1549 issues out of 6346 (24.4%), therefore the wording "closed = fixed" is forbidden in the base; the share of real fixes is counted.

## Round R.40 (2026-09-08): "you didn't check a damn thing" — and that is true

**Claims verbatim:**

1. "item 2 about adding a morpher on a track via RMB is not done"
2. "item 2 I can't move the morphers between different tracks"
3. "item 4 again works strangely — you again didn't check a damn thing, you can add a morpher layer a hundred times, and it will create it every time on the same track — as a result there may be a heap of morphers hidden behind the same place"
4. "the morphers for some reason are not selected via area-select, you didn't check a damn thing"
5. "item 5 is not done — I don't understand how to add anything to the camera track — what is it for?????"
6. "item 11 is not done — the track changes its height but the events are still tiny — you didn't check a damn thing"
7. "item 12 feels not done — I'm trying to catch the opacity on a small event there but it doesn't work"
8. "item 14 in the automation timeline: if the event/morpher is small, then the time on the ruler is set only to 0:00, the very first one, which is stupid. And if you zoom in, that time flies off to the side… the ruler should show the next second as well and ticks too, preferably (and there are no ticks on the main timeline either)"
9. "item 15 the lock doesn't fully work, it needs to be made so that it has to be UNLOCKED for the timeline synchronization to switch off, and the lock needs to remember its state (at least for the whole project)"
10. "item 18 is lacking, besides the set-key button — two buttons: key BEFORE and key AFTER the current position (to fix the previous and the next parameters), and also "+keyframe at playhead" turns out to be a superfluous button"
11. "when I drag the left edge of an event, my automation keys also shift towards the start, and this behaviour cannot be changed in any way"

**All eleven were checked against the code. All eleven are correct.**

**Why the mechanism of R.38 missed them — this is the main lesson of the round.**

In R.38 the "there are no dead ends" tests were set up, and I calmed down on them. They check the REACHABILITY OF WHAT WAS DONE, not the RESULT the person expects:

- t253 asserted "the morph track menu has an item 'add morpher'" — and that is true. But the harness pressed RMB at the point `gutter_x = rect.left() − 30`, that is, in the HEADER of the row 34 px wide. The user presses on the track itself, where the menu of empty space lives. **The test checked the place where I put it, not the place where he pokes.**
- t262 asserted "`lane_heights` got a value, `row_h` changed" — also true. But the event was drawn at the base height and is still drawn that way: throughout `draw_timeline` there is one variable `lane_h = vl.base_h` for 36 uses. **The wiring was checked, not the consequence.**
- t258 asserted "the second +M does not create a second track" — true, and it silently puts the second morpher over the first, from where it can no longer be grabbed (the hit test goes from the end of the list).
- For selecting morphers with a frame there was no test at all.

**The rule of the round: a test MUST assert the OBSERVABLE CONSEQUENCE and do it FROM THE POINT FROM WHICH THE PERSON ACTS.** "The field was set" and "the item is in the menu" are not consequences. Plus the condition of the round: a live scenario for EACH item — everything listed was green in the harness and broken in the window.

**The user's answers to the clarifying questions:**

- **Item 4:** look for a free morph track among the existing ones, create a new one only if there is no free one ("but you need to check — maybe another track is already free").
- **The lock:** one for the whole program, remembered in the project.
- **Keys and trim:** "I don't understand why it has to move to another frame???? how to make it NOT move anywhere, whether I drag the left edge left or right — the key was always on the same frame." It was figured out: the key does NOT move — `clip_slice` does not touch it, it stays on the same frame and at the same moment of the timeline. What LIES is the GRAPH: its axis is normalized to the event, the event got shorter — the key moved from 50% to 33%. It is fixed by the axis (the linked lock shows the timeline window), not by the data. The opposite behaviour — "stretch the animation together with the event" — is done by an option that is off by default.
- **Found beyond what was named** (the camera block is twice as tall as its row; the frame cancels the selection of the morpher; the level line is visible without its own handles; the geometry dump lies to the live scenarios; scrolling the FX window removes the lock): "Yes, all at once".

## Round R.42 (2026-09-08): cloud capabilities of commercial solutions

Verbatim: "In the same way we need to research and save information in the DB about the cloud features of commercial solutions: what is basic there, what is enough and not enough, what people complain about". The fourth wave of reconnaissance of the feature base, by the same method as R.36 (features), R.40 (hints) and R.41 (bug trackers).

### Four decisions of the owner

1. **The axis — `collab/`, by ACTION, not by place.** I proposed this instead of the literal `cloud/` and explained why: "cloud" is a property of delivery, not of a feature; bin locking works the same way on a shared disk and in the cloud, and a category by place would repeat the mistake that item 245 had already rejected for the axis by editors. The owner chose `collab/`.
2. **Dump all five** new sources (Syncthing, OpenCue, Kitsu, Ayon, Flamenco) — where the five core editors have no public tracker at all.
3. **Scope — ~45 notes**, as in the previous waves.
4. **Keep Avid excluded.** I showed that it invented bin locking and noticeably changed it in August 2026, but the owner confirmed the decision of R.36. Avid does not get a key; it is quoted in the prose with the mark `[NOT VERIFIED]`, all the more so because `avid.com` and all its subdomains return 403 — there is no way to check it live.

### What shaped the form of the wave

**There is no network in the code in any form** — 0 matches for `std::net|TcpStream|reqwest|hyper|tokio` across all of `src/`, not a single HTTP/TLS crate among the 505 packages of `Cargo.lock`. Cloud features are not "unfinished" with us: they have no foundation. Therefore the conclusion "there is no network, so we do not take it" was known BEFORE the reconnaissance and would have written itself forty-five times — hence rule 13.

**In the open world the cloud is not requested.** On the corpus of the twelve NLE trackers there is ONE real request for joint editing (`kde-invent #1243`); eight thematic regexps give a total of fractions of a percent. This is not a failure of the lexicon, but a finding: nobody there works together. Therefore a second corpus is needed.

### Found during preparation: three defects in the data of R.41

The wave began by checking its own foundation, and the foundation turned out to be defective. `blender-vse` lay in the cache duplicated in full (3992 lines for 1996 issues: the resumption appended to the file without deduplication) and contained 100 pull requests (the PR filter had been written only for github). `ffmpeg-trac` was 3095 records, **every single one empty** — `csv.DictReader` parsed a response where the columns come with a capital letter and the file is preceded by a BOM, and silently produced garbage. The real ranking corpus is 25 442, not 25 442.

**No countable claim of the base suffered**: not a single note has these two keys in the `trackers:` field, not a single `SIGNAL:` line relies on them, and the shares are computed WITHIN their own tracker. Only the final corpus totals were broken, and they have been corrected with a note of the previous value.

Fixes closing the class: deduplication by id on write; the PR filter for gitea; parsing trac via `utf-8-sig` with normalization of the column names; in `tracker-rank.py --check` — the share of empty records and duplicates with a non-zero return code; in `tracker-fetch.py` — a warning about an export that ended with zero. The last one was added after my own PR filter, written as "the key is present", cut off all of gitea down to the last record: GitHub does not return the `pull_request` field for issues at all, while Gitea returns it equal to `null`.

### Rule 13 — the fourth generation of protection of the output

A note of `collab/` must carry `OFFLINE:` (what remains on ONE machine) and `NETWORK COST:` (what is paid for the second half). The main innovation: **the anchor `OFFLINE:` is checked AGAINST THE TREE** — the file must exist today. This is the first check of an address for existence in the base, and it mechanically enforces rule 8: a path to a crate not yet written is already an implementation schema, and it will not build. The word "network" is deliberately absent from the dictionary of mechanisms.

## Round R.43 (2026-09-09): keys, reverse, clocks under starvation

Three claims after working on the assembled version on a real project `untitled2_v26.09.08.otio` (1961 events, 27 media, 124.9 min). The owner at once demanded that the work be divided into stages, discussed and decided, and separately said: "On any items I may be wrong — ask questions where I am wrong and about subtle points". Backward compatibility of the format is not needed — stated directly.

### Complaints verbatim

1. "in the automation timeline for the slowed-reverb keys, for example, I enter a value in the rate input field and it resets to the previous value, but if I move the key on the graph, the value changes as it should. By the way, in the morpher it works fine, it's a floating bug."
2. "it feels like automation is not taken into account when slowed-reverb plays in reverse"
3. "the region "looping_5" is somehow broken on playback, underruns keep growing; it is not the only one: there is also "area 3333" where the left underruns grows, and the right one also starts a bit later, and then there is no sound or video at all for the main event, only the next one appears in M3TEOR. By the way, M3TEOR is slowed down for some reason, I don't know why. In problems1 there is a trio of events with reverses, and there the sound from some other event somehow appears, and there is also an underruns problem (specifically at this spot the underruns broke a long time ago, we won't do a bisect), and also there the picture is broken because of slowd. In rendereg the automation curves don't work on the very top event, is this a problem with the project format?"

### Owner's clarifications to my questions

* Point 2 is about the **event with the "Reverse" flag** (a mirrored clip), with transport running forward. The reverse-transport branch (`render_frames_raw`, FX deliberately off "like Vegas") has nothing to do with this complaint.
* The event in `rendereg` is **id 2114**, and it DOES have curves in several effects. Verbatim: "clip 2114 there HAS CURVES IN SEVERAL EFFECTS, and it is the very top event, are you looking at it or what??"
* 1774 events of exactly 90 ticks over ~494 s is a **deliberate freeze-frame/stutter**, not a gesture defect. So the engine must be able to handle this.

### My mistake, which made the clarification necessary (rule for agents)

I determined the "very top event" from the track index in the .otio and named the wrong clip.
**Video is written to OTIO bottom-up** — `for lane in (0..p.video_lanes).rev()`,
`libs/kt-otio/src/convert.rs:376`, "later track on top". The layer map
(`metadata.kt.layers.video`), however, goes straight through the tracks, starting from zero. The order of rows on screen is set by `lane_order` → `Project::rows()`. A lane must not be derived from the track number — only from this pairing.

### What turned up during analysis (diagnosis, not guesses)

Complaint 3 turned out not to be a set of separate problems but one cause with four faces:

* **The clock stops when the ring starves.** `varispeed_fill` with an empty ring pulls the last frame and does NOT increase `consumed`, while the playhead moves by exactly `consumed`. Every callback-underrun frame is a frame the timeline did not advance over, so the program physically plays slower than real time. This is the "M3TEOR is slowed down", and the "no sound or video for the main event".
* **"underruns broke a long time ago" is true.** The misses were counted in DIFFERENT units: frames for an event without FX and bursts for an event with FX. The discrepancy is 1024x; the numbers are incomparable. A bisect is indeed not needed — this is not a regression but different units from the very beginning.
* **"Left/right underruns" are not channels**, but "render/callback". The interface did not label this, and the owner read them as L/R. Now labeled.
* **"Sound from another event" in reverse** — the audio mirror axis included the resampler margin (8192 frames = 170.7 ms), while video was anchored to the material actually consumed. For the event in `problems1` the offset was 63 % of its length.

### Rule recorded from this

**The instrument is fixed before the patient.** The first stage of the round was not the complaints but the counters: while underruns are measured in two different units and unlabeled, and the clock lag is not visible at all, any measurement of "better/worse" is self-deception. The owner himself pointed out the broken counter, and that is the only reason he did not insist on a bisect.

### Second wave of R.43: the first pass did not help

Complaints verbatim: "in problems1 there is still a problem; before the problems there was sound from the source "far" and with problems it's "salute", and the marked regions still have the problem, it feels like my project itself is breaking something in the program, because there is a slightly earlier version of the project untitled2_v26.09.04.otio where at least rendereg works correctly and looping5 also works without problems."

The user brought media files, BOTH versions of the project and two `ktflight.log` files. The earlier version was the answer: it has **zero morphers**, while the later one has seven. Everything else (sound from a foreign source, dead curves, a frozen region) turned out to be one defect — item 298.

**What this teaches for the future.**

* **"It feels like my project breaks the program" is a diagnosis, not an emotion.** The user named the exact boundary ("here it works, here it doesn't") and attached both sides of the boundary. Comparing two files gave the cause faster than reading the code.
* **The first wave treated symptoms, because there was nothing to look at.** Four real defects (items 289–297) were found and closed, but the root was in the model, and there was NOTHING to ask "which media sounds at this place". Hence `kt-cli probe` — and the rule: for a complaint "here sound/picture comes from another event", run it first, rather than reading the engine.
* **Ask the user for a file, not a description.** `ktflight.log` with lines `PLAY head=211.572` … `PAUSE head=211.572` proved the freeze within a second; the earlier version of the project pointed to the morphers. The user provided both artifacts himself when asked.

## Round R.44 (2026-09-13): geo-effect chains blur — transform concatenation

The complaint about compositing quality on grid/pan3d chains is a direct continuation of the tail of R.43 ("What this round did NOT close"): the top event of rendereg with the chain event_grid → pan3d → event_grid → pan3d → event_grid (id 2114). The owner again asked to "split into stages, discuss and decide", to ask questions "where I am wrong and on the subtle points", said "Make notes for agents", and stated directly: "Backward compatibility is not needed."

### Complaints verbatim

1. "Besides the audio-video fx items - there is a very strong lack of flexibility in compositing effects and completeness of the compositing pipeline, and it shows up as follows (this is in rendereg): take a source, make a grid 2x2 on it, overlay a 3dpan with parallax effects that by the end of the automation go back to normal, overlay another grid 2x2, you get 4 nice parallaxes, overlay 3dpan again, and at the moment when the parallaxes return to normal - we zoom the picture to eliminate our grids. This way there should be a parallax smoothly flowing into a normal picture, everything works, BUT - at the moment of zoom pixel smearing is visible, because in fact there is no compositing and the program is not designed for such a thing. I have marked AT LEAST one problem."
2. "Probably this requires fundamental changes to the renderer. I don't know - but it turns out in multi-polygon mode it should form a tessellation, and already do uv-mapping based on the scale and other parameters - that's the first thing that came to mind and it already sounds complicated, suggest your own ideas too, maybe I'm wrong and my proposal covers only one case of one effect."
3. "Another question - how not to lose performance, i.e. when to turn on composite rendering and when not? Because if in a single event over an interval there is just one pan3d scale=0.5 - then compositing is not needed at all, and if it's something a bit more complex - then it is needed, and here too there is a problem."

### Diagnosis (from the code, not a guess)

Each video FX is a separate full rasterization into a buffer of output size, with quantization to 8 bits between steps; the chain from the complaint is six rasterizations, and after the second grid only a quarter of the detail remains, and the zoom magnifies its remnants. The source is also downscaled to output resolution by the decoder (`SWS_FAST_BILINEAR`).
Tessellation is not needed and is ruled out by the environment (llvmpipe/downlevel: fragment passes only): all five geometric FX are analytically invertible (affine / tile-modulo / homography — pan3d places a flat quad with z_clip = 0), so an inverse per-pixel UV mapping in a fragment shader covers the same cases more simply and more precisely. The owner's intuition of "uv-mapping based on the scale" is correct — only per pixel, not per vertex, and without tessellation.

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Concatenation always; the question "when to turn it on" does not exist.** Consecutive geometric FX (Pan, CamMotion, Mirror, EventGrid, Pan3d) are executed in ONE pass of inverse mappings — as in Nuke/Fusion; in AE this is called collapse transformations. A series of length 1 without minification must remain byte-for-byte today's single pass; a series of length k is one pass instead of k, that is, CHEAPER than stepwise. Non-local FX (Blur/Glow/Edges/VHS/Noise/Greenscreen) and LayerMix are barriers: the raster is materialized there as before.
* **Pan3d is also computed on CPU** (inverse homography — "just per-pixel math"): the R.29 decision "3D GPU only" is CANCELLED. The CPU path is again the full byte reference, and export without GPU no longer refuses (`GpuRequired` is removed).
* **Supersampling by the Jacobian under minification EVERYWHERE** — including single grid and pan with zoom<1; recomputing the affected goldens was sanctioned (the eventgrid line in t26 was expected). FACT of the implementation: recomputing was NOT needed — under minification of exactly ×2, box supersampling arithmetically matches the old bilinear in the half-texel phase, all goldens are intact (item 309). NOT touched: the internal resamples of Blur/Glow (their blurring is the essence of the effect), transitions (`transitions.rs`/`composite.wgsl`), `fit_into`, the decoder downscale in Quality.
* **There are NO switches at all.** To the question "does it make sense to make a switch? on/off? enabled by default." — after analysis the option was chosen with neither a UI toggle NOR a diagnostic env flag: the old behavior is an artifact of the implementation, not a style; the second path would mean permanent support.
* **Mitigating barriers — later.** To the question "can the non-local barriers be bypassed? well, the same edges and vhs?" — exactly nowhere (in Nuke a per-pixel node likewise breaks a series), but the stages are fixed in F-CMP-001: (1) per-point Greenscreen/Noise "per tap" inside the merged pass — they will stop breaking geo series; (2) barriers that account for resolution (edges/vhs/blur/glow at ×2–×4 with the net zoom of the tail; kernel parameters are resolution-invariant). Also there — "decode larger than Quality at net magnification >100 %" (the only case where sharpness is lost BEFORE the chain; it touches cache keys and prefetch).
* To the question "if we have a GPU, we always compute on it, right?" — yes, with a live GPU all video runs on GPU (llvmpipe counts as one); CPU is the reference and fallback (no adapter / TooLarge / DeviceLost / `KT_VIDEO_GPU=0`), and after the round it becomes FULL. Per frame the CPU only builds the program — a few 3×3 multiplications in f64, O(chain length).

→ implementation: items 306–310, tests t296–t301 and t66d, feature note F-CMP-001 (`docs/features/compositing/transforms/transform-concatenation.md`).

## Round R.45 (2026-09-13): "what to do about the blockers" — softening the barriers

Right after checking R.44: "I checked, it seems to work. Now let's think about how to deal with the blockers." "Blockers" are the concatenation barriers: non-local FX (Blur/Glow/Edges/VHS/Noise/Greenscreen) that break geo series. The same caveats as in R.43/R.44: "Backward compatibility is not needed", "On any points I may be wrong, ask questions where I am wrong and on the subtle points", "Make notes for agents".

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Both softening stages in this round; "decode larger than Quality" — later.**
  (a) Per-point Greenscreen/Noise stop breaking geo series: they are applied to the value of each subtap of the merged pass, in chain order. A single Greenscreen/Noise (outside a series) keeps the old kernel CODE; bytes are guaranteed at K=1 of its segment (with a zooming tail the bytes change per stage (b) — this is deliberate). (b) Non-local Blur/Glow/Edges/VHS execute at K× resolution, K = net zoom of the chain tail, quantum 1/32 and ceiling 4 — as for supersampling taps.
* **The Noise grain is pinned to the grid of ITS OWN step**: zoom/rotation after it carries and scales the grain together with the picture (the film grain of the material), rather than holding it on screen (overlay grain). Closer to the spirit of the old behavior.
* **MAX_INTER_DIM = 4096** — the ceiling for the side of K× buffers, a PROJECT constant (not a device limit — otherwise export would depend on the machine): preview — full K=4, export 1080p — up to K=2, 4K — without K×. It can be raised with one constant.

### Subtle points fixed before work

* K is additionally limited by "compression" further up the chain:
  K = quantum(min(tail zoom, sharpness squeezed before the barrier)). Otherwise `[vhs, zoom]` without a grid would pay for a K× buffer in which the detail is still 1× — its source ("decode larger than Quality") is deferred to a separate pass.
* **"Flapping of K"**: K depends on the zoom automation — on the frame where the threshold 1 + 1/64 is crossed it jumps 1→2. Hysteresis is forbidden by determinism; the size of the jump is limited by the tolerance of the invariance of the kind (test 303). Precedent — the quantum of supersampling taps (item 309, t300).
* The mechanism "build the geo program in K·out" was REJECTED: it breaks the integer geometry of the grid (10 px / 4 columns at K=2 give a different cell layout). Instead — conjugation: the program always works in 1× semantics, and the scales are supplied by two affines at the edges (at K=1 — identity, R.44 bytes untouched).
* Goldens are NOT regenerated at all in this round: K=1 paths are byte-for-byte.

→ implementation: items 311–315, tests t302–t306, stages (1)(2) of the F-CMP-001 verdict.

## Round R.46 (2026-09-14): "let's do it" — decode larger than Quality

The owner himself came back to stage (3) of the F-CMP-001 verdict, which had been deferred twice: after an explanation of its meaning ("So, what about the decode larger than Quality? What's the point?") — a short "let's do it". The standing caveats of the earlier rounds still apply: backward compatibility is not needed, there are no switches at all, notes for agents are kept.

Essence: the decoder downscales the frame to Quality right away (`sws_scale`, item 310a) — with a net zoom of the chain >1, sharpness is lost BEFORE concatenation. The fix: request the pre-FX frame at S×Quality (S is an integer "step" 1..4), when the chain magnifies and the file has headroom of native resolution.

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Step S — PER CLIP**: the maximum net zoom over the whole duration of the clip (from the automation curves and cameras). One cache key per clip: the warmth bars are exact, decoder spans are not broken, memory is predictable. The cost was accepted: a clip that zooms only at the end is decoded large in full (LRU eviction limits it); editing a curve across the step threshold = overheating of the clip ("flapping", precedent of the tap quantum, item 309).
* **Fractional headroom of native is rounded UP**: native 1920×1080 with Quality 1280×720 and zoom ×2 → decode 2560×1440 — sws stretches 1920→2560 right during decode, and all 1.5× honest details of the file reach the chain. Overspending is limited to one step; the rule is simple — "any headroom of the file works".
* **Enabled EVERYWHERE — preview and export**, one code path (invariant "preview == export"). It switches itself off when the file has no headroom over Quality, and on neutral chains (net zoom ≤ 1 + 1/64).

### Subtle points fixed before work

* The decode size is part of the SINGLE source of truth `layers_at`/`wanted_frames` (lesson of t247d): prefetch, preview and export take the quality from the order, nobody patches it on themselves. The step is deterministic: S = f(project, native, Quality) — NO dependence on the device or budgets (otherwise export would depend on the machine). Decode is never smaller than Quality (native smaller than Quality → S=1).
* Leading barriers (before the first geo run) are executed at S× FORCIBLY — the physical frame is already S×, and the benefit formula of R.45 does not vote here.
* A side fix of a latent defect of R.45: a single Mirror was routed to Legacy unconditionally, while the plan assembly asserts 1×-edges — the chain `[grid, vhs, Mirror, blur, zoom]` was tripping debug_assert even before this feature. Rule: Mirror-Legacy only when k_in == k_out.
* The export chunk and prefetch lookahead budgets counted the frame size by Quality — with S>1 an underestimate by a factor of S² (the return of "black render" item 15 and cache-black) — are fixed in this same round.
* Interleaving of sws/seek steps on one file (two clips of the same media with different S) — accepted: spans amortize, the cost is measured in release.
* Goldens are NOT regenerated: the byte references have no net zoom, and the native of the synthetic material is smaller than Quality — S≡1 on all existing paths.

→ implementation: items 317–323, tests t308–t318, stage (3) of the F-CMP-001 verdict.

## Round R.47 (2026-09-14): "not enough effects" — +14 audio and +16 video

The request verbatim: "1. not enough effects for audio, need 10-20 more
2. not enough effects for video, need 10-20 more, both 2d and 3d."
A direct continuation of R.29 ("effects are categorized into 2d and 3d versions", course toward After Effects) and R.33 ("throw in 10-20 different modern ones there" — then about transitions). The standing caveats still apply: backward compatibility is not needed, there are no switches, notes for agents are kept.

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Audio — the whole list, 14 effects**: Filter, EQ, Delay, Chorus,
  Flanger, Phaser, Tremolo, AutoPan, Vibrato, Distortion, Bitcrusher,
  StereoWidth, Compressor, Gate. **Pitch shift and look-ahead limiter — deferred to a separate pass**: both require an end-to-end delay mechanism (analysis window/lookahead), which is absent from the `read(local, out)` contract, and pitch shift additionally conflicts with the warp's monopoly on consuming the source.
* **Video — the whole list, 16 effects**: point-wise ColorGrade, Duotone, Posterize, Vignette; barriers Sharpen, Pixelate, ChromaticAberration, RadialBlur, DropShadow, Halftone; geometric distortions LensDistort, Twirl, Ripple, Kaleidoscope (one new nonlinear primitive of concatenation); 3D — CornerPin, Cylinder3d.
* **An honest order of audio slots.** Before R.47 the processing order was fixed (warp → reverb → volume), and a Gain placed BEFORE the reverb still sounded after it. With 14 effects the order must work as in video and in a DAW: processing goes in chain order. The cost was accepted: projects with an automated Gain before Slowed+Reverb will sound different (the input fade no longer chokes the reverb tail — the tail rings out); a constant Gain is not audible (reverb is linear). GOLDEN_AUDIO is intact — the reference has one slot.
* **One round, in waves: audio → video**; each wave is green commits, can be checked along the way.

### Subtle points fixed before work

* The audio chain moves from a monolith to a node graph (`AudioNode`); single-slot chains are byte-for-byte (proof by branches), multi-slot ones change per the decision on the honest order.
* A side fix of a latent defect: `Freeverb::reset()` did not zero the predelay ring — with predelay > 0 up to 200 ms of old material leaked through seek (nondeterminism; red-proof test).
* The file format is NOT bumped (precedent "one Pan3d did not bump the version"): an old build will play/show the clip without the new effect, the file stays intact, and the effect travels as foreign-passthrough and survives re-saving.
* t48 ("audio effects exactly 2") and t190 (manual list of 12 variants) are strengthened: the registry `FxInstance::all()` + `tag()` is introduced, and the tests are moved to properties with completeness locks — per the owner's decision.
* A video effect must declare its class to the concatenation scheduler (point-wise/barrier/geo) — one forgotten silently becomes a barrier and breaks geo series; in the first video wave compiler locks are put in place.
* Nonlinear distortions (twirl/ripple/kaleidoscope/cylinder) compute trigonometry: CPU f64 vs GPU f32 — a new tolerance class `FrameTol::WARP` by measurement ×2; lens (polynomial) stays in GEOM.

→ implementation: items 324–336, tests t319–t333, feature notes F-AUD-009…012, F-FX-001…004, F-CLR-007 (+ statuses of F-AUD-001/002).

## Round R.48 (2026-09-14): "let's plan these candidates"

After R.47 the owner asked "what next?", received three candidates and replied: "let's plan these candidates". The round closes the debts that R.47 explicitly deferred, and two UX bottlenecks that grew out of R.47 itself (43 effects — it became expensive to assemble a chain by hand, and an EQ without a graph remains a grid of seven numbers).

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Pitch shift — BOTH engines**, "both item 1 and item 2 with a global switch in the project": WSOLA (time domain, cheap, good on voice) and the phase vocoder (FFT, smoother on polyphony, smears transients). The engine is a PROJECT setting, not an effect parameter: one choice for the whole edit, and a switch is heard immediately everywhere.
* **Auto-ducking — in this round, as a separate wave.** It closes F-AUD-002 entirely together with the limiter ("music drowns out the voice" — the most frequent live complaint about any editor, per the reconnaissance).
* **FX presets — global for the user** (+ several factory ones): visible from any project. The project remains self-contained — an inserted chain lives in the .otio in expanded form; the preset is only an insertion template.
* **EQ — an interactive frequency response graph**: a response curve and three band points; dragging moves frequency and gain, the wheel adjusts Q; the numeric fields remain and stay in sync with the graph.

### Subtle points fixed before work

* **Lookahead, not delay.** An effect with look-ahead reads the source FORWARD of the output position, while the output stays aligned with the input: for the rest of the engine the delay is zero, the playhead and crossfades do not move. The cost — the source window extends forward (one function serves both render and prefetch, otherwise a permanent cache miss).
* **"Warming up" and "sounding" are separated.** Today this is one number (`chain_tail`): it both warms up the seek and extends the end of the event. Lookahead enters the warm-up but does NOT extend the event — otherwise the clip would start sounding after its own end.
* **Changing the lookahead window rebuilds the processor** (a structural edit, like changing the reverb geometry): the ring length is part of the node's device, not a parameter value. It is heard as a mini-seek, documented.
* **A neutral pitch shift must be byte-for-byte the same as before**: zero semitones and no curve — the node does not touch the buffer (the rule "default of a new parameter = previous behavior"). GOLDEN_AUDIO is not touched.
* **Ducking is a snapshot, not a live sidechain**: on an explicit request the command computes the envelope of the source track and puts a curve on Gain (one undo step, no more than four keyframes per pause). It does not "grow back" by itself; if the music changes — call it again. A live sidechain (the sound depends on another track in real time) is a separate large piece of work, outside the round.
* **Presets are the user's file, not the model**: saving and deleting a preset do not go into the project's undo (undo edits the edit, not the library); deletion warns about this with a hint. The file survives unknown effects from future versions (they are kept as is) and is not overwritten by a newer format version.
* In passing, the tail of R.38 is closed: inserting an FX chain into a morpher silently lost curves. The reconnaissance verdict explicitly forbade starting a preset library before this.

→ implementation: items 337–348, tests t322g, t334–t350, feature notes F-AUD-001/002/013 and F-AUT-004 (all closed).

## Round R.49 (2026-09-15): "let's do item 1, item 2, item 3, item 4" — durability, push channel, offline analysis

To the question "what next?" the owner received four candidates and replied verbatim: "let's do item 1, item 2, item 3, item 4, fruc_vulkan not yet". The occasion for the discussion was a finding of the feature reconnaissance: 47 addresses of OWN defects, recorded in waves of notes and never once fixed. Checking against the code before the round showed that not a single class had closed — only the line numbers had gone stale.

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Breakdown**: R.49 takes three directions — durability of work, offline analysis of material, push channel; **nested sequences go to a separate round R.50** (per the reconnaissance this is XL: the frame plan becomes a tree on which all video goldens stand, and the import behavior of foreign files changes incompatibly).
* **Autosave NO LONGER writes to the user's file.** Background writing goes to its own recovery file; the project is changed ONLY by an explicit save. At startup, if the recovered one is newer than the project, the program asks. The previous design left the user without a second state on disk: there was nowhere to roll back to, yesterday's file had already been overwritten.
* **Pilots of offline analysis — two**: integrated loudness (LUFS/dBTP) and synchronization by waveform. **Noise profile — not now**: it drags in a spectral kernel, the first opaque parameter of the model and an edit of the prefetch window, that is, a round of its own.
* **Nesting (decision for the future, for R.50)**: a nested sequence is a **reference to an object INSIDE the project** (the Premiere/FCP path), and it **inherits the parent's settings**. It has no fps or size of its own — so there is also no recalculation of grids, "smearing", or judder.

### Subtle points fixed before work

* **Autosave stops clearing the asterisk.** Today it clears it, and this is locked by a test; while the project rules say the opposite. After the round the code will match the rule, and t194/t200 are rewritten — this is a direct sanction from the owner, not a weakening of tests of a frozen range.
* **The program starts asking before exit and before switching projects.** The right not to ask was legitimate precisely because autosave wrote over the user's file — there was nothing to lose. The owner removed that, so the right to stay silent is removed too.
* **The lock is placed on the PROJECT FILE, not on the program.** A ban on a second copy would break live scenarios (they run the program in parallel) and the scenario "opened a second time to look at another piece". A second copy works, but honestly warns and writes its own recovery.
* **Strictness toward broken files.** A clip with a corrupted section of our metadata no longer silently loses the A/V link, volume and opacity: loading refuses and names the address of each broken clip. The cost: a file corrupted by the PREVIOUS defect will now not open — but it already opens incorrectly today, just silently.
* **The width of the meters panel moves out of the project file** into the user storage: today, opening someone else's project, you lose your own setting, and by saving you impose yours on theirs.
* **The demo project starts working without media files** (synthetic sources by default): the eight paths it references do not exist in the distribution at all.

### What came out of R.49 in the end (added after the work)

* **The program no longer loses work.** Background writing goes to its own recovery file, the project is changed only by Ctrl+S; at startup unsaved work is offered for restoration — with the path and time; exit and project switching ask; a killed program leaves the work on disk, and this was verified by a real SIGKILL in a live scenario.
* **The right mouse button stopped staying silent** in twelve places, and two hints that for years promised non-existent behavior became true: a double click on the level line resets it, and the "Del" chord is no longer promised where it does nothing.
* **The program learned to listen to material**: loudness by broadcast standard (LUFS/dBTP/LRA) for each source and for the finished export, synchronization of two recordings by sound with one menu item. Both counts run on a pass that was already needed — no new waiting appeared for the editor.
* **Auto-ducking moved to the background**: before, the window froze for tens of seconds.

What was deliberately NOT done and why: there is no "normalize to −23" button — the normalization target is chosen by a person (broadcast −23, streaming −14, cinema −27), and applying it to an already recorded file means re-encoding it once more; synchronization does not bind events into a pair — that is a separate decision for the editor; the right button on a curve keyframe does not delete it immediately — otherwise the promise "you can safely try things with the right button" stops being true everywhere.

## Round R.50 (2026-09-15): "let's go on" — nested sequences

The owner said "let's go on"; next in his own breakdown came nested sequences, split out of R.49 into a separate round. Two decisions that determined half of the code were made back then and are repeated here without change; two more were needed before work began.

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Nesting — a reference to an object INSIDE the project** (the Premiere and FCP path), not to a file on disk: one sub-timeline sits in several places, and an edit inside changes them all at once. *(accepted in R.49, for the future)*
* **Nesting INHERITS the parent's settings.** It has no fps, raster or rate of its own — so there is no recalculation of grids, no "smearing", no judder, for which the nest is famous among all five competitors. We deliberately differ from all of them. *(accepted in R.49, for the future)*
* **Foreign nested `Stack`s are read as real sequences ALREADY in this round.** The compatibility gap recorded in item 158 is closed: we honestly read foreign nested compositions and honestly draw emptiness in their place, so the `nested_example` sample had permanently sat in the smoke tier of the cross-render. The cost was explicitly accepted by the owner: importing foreign files begins to give a DIFFERENT model than today.
* **Anything may be put inside a nest, including morpher layers and cameras.** The alternative ("only events and transitions, collapse under a morpher — an honest refusal") was rejected: it is about half as long, but it leaves the user with a class of "this cannot be collapsed".

### Subtle points fixed before work

* **A nest with an empty chain is IDENTICAL to its content** — a direct consequence of inheriting the settings. Therefore expanding it inside `effective_clips` (where the morpher is also expanded) gives picture and sound before the real plan subtree appears, and serves as the REFERENCE-ORACLE for it: the finished subtree is checked against the expansion byte for byte.
* **A defect that does not exist yet was found.** The composite accumulator stores premultiplied color with straight alpha, while the layer overlay expects straight. Feeding the assembled frame of a nest back into the composite makes the picture twice as dark EXACTLY in the semi-transparent places, which is easy to dismiss as "that's how it's meant to be". It is cured by a single conversion at the nest boundary, and a red-proof is mandatory for it.
* **The event number becomes unique across the WHOLE document**, not within a reel: envelopes of material and transitions are stored in global maps keyed by this number, and coinciding numbers from different reels would silently exchange references to material and start timecode.
* **The nesting depth limit is 4**, and this is a calculated number, not a round one: each level keeps two accumulators and a result in video memory, about 25 MiB at 1080p, against a budget of 128 MiB.

### What came out in wave A (items 378–383)

Collapse, save, open, render, expand — works from end to end. Editing inside for now goes through "expand, fix, collapse": entry inside is wave B. A chain, reverse and warp on the nest itself are REJECTED until wave B with an explanation, rather than computed incorrectly: from the outside they must apply to the assembled picture, while expansion would apply them to each piece — that is a different result, and it cannot be silently emitted (rule 16).

The compatibility gap of item 158 is closed: foreign nested `Stack`s and `Track`s inside a `Track` are read as real reels. The cost that was stated in advance has been paid — importing foreign files now gives a different model than it gave before.

### How R.50 ended

**Nesting works from end to end.** Collapse a piece of the edit, enter it with a double click, fix it, exit, save, open — and all of this with ONE reel placed in several places: an edit inside changes all of them, and the "×N" badge on the event says how many places will change together and cycles through them.

The main thing is not the convenience of grouping, but the BOUNDARY OF PROCESSING ORDER: a chain hung on a nest is applied to the ASSEMBLED picture and to the common mix, not to each piece. Exactly this could not be expressed any other way.

The compatibility gap of item 158 is closed: foreign nested `Stack`s are read as real reels. With the strict cross-render tier the sample still did not become one — but for the opposite reason: a strict run showed that the REFERENCE discards nested `Stack`s, and it is not us who diverge.

A live scenario found a defect that no test had seen: inserting a copy next to a nest SILENTLY did not happen (the counter of fresh addresses did not see events inside reels). This is exactly the class of defect for which live scenarios were created.

## Round R.51 (2026-09-16): transitions — "symbiosis of two events"

The complaint verbatim (the first on the list the owner asked to "split into stages, discuss and decide"):

> there are not enough transition effects - for some reason you base all effects only
> as an overlay of A on B, where B has an "effect", and you don't have transitions with a symbiosis of two events, there are no transition inversions, etc.

And separately: "Backward compatibility is not needed. On any points I may be wrong, ask questions where I am wrong and on the subtle points."

### What the reconnaissance showed before the questions

The architectural complaint was not confirmed: a transition pair collapses into a single layer (`PlanLayer::Dissolve`), the kernel sees both frames, and 7 of 16 kinds move both. But it was confirmed by impression: **Push and Whip are broken** — the entering frame is sampled with an offset of −dir instead of +dir, the coordinate is clamped to the edge, and instead of frame B its smeared first row/column enters. That is exactly how "one of the frames has some effect" looks. The tests are blind: t225 ran solid colors (any coordinate gives the same color), t224 checks CPU against GPU — and the mirrors are broken identically. A second defect of the same kind found: a repeated drag of the fade corner and a repeated Move-drag silently reset the chosen transition kind to a crossfade and erase its curves (against rule 20 "nothing switches itself off").

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **"Symbiosis of two events"** = (a) transitions where BOTH frames move, and (b) **the transition must work on an overlay** (an overlap of clips on one track), not only on a butt joint. The gallery with live mini-previews and linking a video transition to sound were NOT chosen (the decision of R.33 "sound always crossfades" remains).
* **Transition inversion — a checkbox on the kinds** ("swap the roles of the sides"), NOT a progress curve as in Vegas and NOT ease presets — both options were offered and neither was chosen.
* **New kinds — all four families**: 3D (page peel/roll, cube, flip/doors), pattern masks (blinds, checkerboard, random blocks, spiral), split/squeeze/stretch/swap, and parameters for the existing ones (an arbitrary center for iris/zoom/spin, etc.).
* **Composition — "modern mix"**: Vegas classics plus fashionable ones from Premiere/Resolve/CapCut (shake, film burn, light leaks, zoom blur); strict copying of the Vegas set was offered and not chosen.
* **Backward compatibility is not needed**: the anchor "exactly 16 kinds" (t223) was removed by its new decision, the behavior of existing kinds may be changed (the push/whip fix changes the picture of old projects), the format is bumped, old builds get an honest refusal on the new file.


### How R.51 ended

**Both words of the complaint became code.** "Symbiosis": Push/Whip are fixed (the real frame enters, not a smeared edge — the defect had lived for four rounds, because both guards were blind each in its own way), shifts where both frames move were added (split/squeeze/stretch/swap), and transitions began to work ON AN OVERLAP: the window is the overlap zone itself, it has no handles (the window follows the trim of the clips — the lesson of Kdenlive from F-TL-010 is removed by construction), the audio curve on an overlap started to work, and the zone of a bare overlap is marked with hatching. "Inversion": a checkbox on the transition event, executed by the plan as a single substitution `kernel(b, a, 1−k)` — the kernels, mirrors and cache keys are untouched; for symmetric kinds the checkbox is honestly hidden.

The kinds now number 33 (patterns, shifts, stylized, volumetric), and iris/clock/zoom/spin gained an arbitrary center (animatable). The volumetric ones (flip/cube/door/peel/roll) went in WITHOUT a single new pass — into the free 16 float uniforms of the composite; the homographies are computed by f64 code shared by both backends. In passing, the defect of R.50 was closed: a transition inside a nested reel silently did not crossfade the sound.

Deliberately NOT done: the progress curve and ease presets (the owner chose the checkbox — they were explicitly offered), the gallery with mini-previews and linking a kind to sound (not chosen), the decode step for zoom-in transitions (a known limitation, item 400 — a separate round, if sharpness is needed).

## Round R.52 (2026-09-16): video generators — "at least text, of various kinds"

The complaint verbatim (the first on the list the owner asked to "split into stages, discuss and decide"; the questions were asked on 2026-09-14, implementation started after R.47–R.51):

> there are not enough video generators, at least different text, etc. and so on
> as in serious programs

And separately: "Backward compatibility is not needed. On any points I may be wrong, ask questions where I am wrong and on the subtle points. Make notes for agents."

### What the reconnaissance showed before the questions

There were no direct requests about generators in the history — but the owner had chosen titles for the roadmap back in R.18 ("titles and transitions, color correction and LUT…"), and the transitions from the same line were already done (R.33, R.51). Feature notes already existed with verdicts "take it, do together": F-TXT-001 (text in the frame, priority A) and F-TXT-003 (generators) — both independently arrived at "a generator/title = a third kind of source without a file (`MediaSource`), with the scheme in the media path and a single factory". By that point the parsing of the pseudo-path `synth:` had been duplicated in three files and had once already diverged in case (footnote of item 211) — the factory was needed even without generators.

### Owner's decisions (answers to direct questions, AskUserQuestion)

* **Order**: foundation (the factory) + solid color → static text → the remaining generators (gradient/checkerboard/noise/SMPTE/timecode) → animations. The variants "text first" and "all the simple ones first" were offered and not chosen.
* **Text engine — the shaper right away**: harfrust (positioning: GPOS kerning, ligatures, cursive scripts) + ab_glyph pixel rasterizer. The variant "just ab_glyph, shaping later" was offered and not chosen (Noto Sans has no legacy `kern` table at all — without a shaper the titles would have had no kerning).
* **Fonts — bundled + system right away**: vendored faces as the determinism reference (goldens only on them) + a drop-down list of system fonts; a project on a machine without the font — substitution with the vendored one + a visible notification. The variant "only bundled ones first" was not chosen.
* **"Backward compatibility is not needed"** was clarified (multi-select): (a) old builds are not required to handle new projects — the generator degrades in them into a visible placeholder, and we do not provide converters; (b) legacy import does not convert titles/generators of old formats. The variant "reading old projects may also be broken" was NOT chosen — byte stability of untouched old files remains.

### How R.52 ended

**"At least text, of various kinds" — done, and not only text.** Eight kinds of generators: solid color, TITLE (text with harfrust shaping — GPOS kerning and ligatures are honest, bundled Noto Sans + SYSTEM fonts with substitution and a notification, outline/shadow/plate with a preserved layer order, safe zones, drag directly in the preview), linear and quadrilateral gradients, checkerboard, noise field, SMPTE bars with a 1 kHz tone, timecode. A generator is an ordinary event: trim, transitions, FX chain, copy-paste, undo — all for free (a third kind of source without a file). The engine, GPU and kt-fx were not touched by a single line; GOLDEN_AUDIO/GOLDEN_VIDEO did not move, the title has its own reference.

In passing, a systemic debt was closed: the parsing of pseudo-paths, which lived in TWO diverging copies (footnote of item 211), moved into the single factory in kt-media; "Remove from project" stopped leaving a ghost worker; the generator editor windows joined the Esc cascade. The format is a first-class `GeneratorReference.1` (+ import alias SMPTEBars), kt.v was NOT bumped: an old build gets a visible placeholder, not silent corruption.

Deliberately NOT in this pass (doors left open): Arabic cursive/bidi for titles (the shaper is already in place), word wrap by width, italics, a layer-order editor, per-character animations (F-TXT-006), automation of generator parameters, timecode burn-in at the delivery stage.

Items 402–408, tests t469–t495, live scenarios `generator.py` and `title.py`. Implementation: see the commits of R.52.

## Round R.53 (2026-09-16): "let's keep polishing" — generators become a tool

After R.52 was handed over, the owner said "let's keep polishing". The reconnaissance found three clusters: defects (in places the feature is unavailable, in places it lies), the cost of the fix, and deferred capabilities. The owner accepted the list presented, and answered verbatim on the cost of the fix:

> why on earth do the project cache and decoder restarts etc. get reset on clicks and all sorts of actions with TITLES??? this sounds terrible and there is no connection at all

The connection was ours: the generator's parameters live in the media PATH (the decision of R.52), and changing the path for the engine means swapping the file — hence the decode stream is shut down, its frames are cleared, and both renderers are rebuilt twice on EVERY keystroke in a title field.

### Owner's decisions (AskUserQuestion)

* **Order**: first the defects, then new capabilities; **one round in a row**, the report at the end (the variant "two rounds with a pause" was offered and not chosen).
* **Decoupling of editing**: the generator **REMAINS a media source**, but editing its parameters stops being a file swap. The variant "take generators out of the media pipeline and draw directly in the composite" was offered and REJECTED — with a justification that had to be acknowledged as correct:

  > You say the cache is meaningless, it is meaningless if the generator is simple, but what if there are complex algorithms there? And it turns out the cache strip makes sense — it does the work, it generates, well, it kind of "decodes", so to speak.

  A title computes a distance transform over the whole frame and blurs the alpha three times, noise computes millions of hashes per frame: their "warm-up" is real, and the strip does not lie. The statement "coverage bars for a generator are meaningless" was withdrawn — it holds only for trivial kinds.
* **New kinds** (all four groups chosen): shapes (they stood in the name of our own note F-TXT-003 and had not been made), a transparent backdrop, a countdown, engineering tables.
* **Title animation** (F-TXT-006) — in this pass, after the defects.
* **Animation presets set EVERYTHING**, including position and anchor: a "crawling line" not shifted off the edge of the frame is useless. Rule 20 is not violated under two conditions — the whole set goes out in ONE undo step, and manual tweaking honestly shows "matches no preset".

→ implementation: items 409–426, tests t496–t542, live scenarios `shapes.py`, `title.py`, `generator.py` (items 425–426 — defects that these scenarios found after the stages were handed over).

## Round R.54 (2026-09-17): effect picker — "an icon would not hurt, and a prerendered thumbnail"

The owner's complaint in full, verbatim:

> I don't like the effect selection widget - a list is of course good, but some kind of icon would not hurt, and a prerendered thumbnail of the effect overlay would not hurt either (i.e. opposite each effect there should be a strip of thumbnails for each preset for the specific position of the timeline cursor, and it's worth thinking about not drawing all the thumbnails at once, but only trying after a second (what if it's a misclick), and if the computer can't handle it, then slow down the rate). There can be many presets - you need to manage to fit both the names and the previews. Also there are default thumbnails of effects (we overlay the effect on the same previews that we show in the media widget) - they need to be rendered after the source is added to the project (or if it hasn't been done yet), but only when there is no heavy activity. And save these default previews into the index DB. Here it's necessary to think carefully about where to put the render code, so as not to break the current pipeline, or is that already taken into account there?

Plus two permissions that removed half of the ceremony: "Backward compatibility is not needed" and "On any points I may be wrong, ask questions where I am wrong and on the subtle points."

### What the reconnaissance objected to the owner (and he agreed)

* **Persisting thumbnails in the index DB is not needed.** The expensive half of the preview — the DECODE — is already persisted: the source thumbnails at 320×180 are stored as JPEG in the `thumbnails` sidecar DB since R.7. Applying the chain to a finished frame costs a fraction of a millisecond (video FX are pure stateless functions, measurements in item 198), that is, CHEAPER than reading the result from sqlite. Together with persistence, the background job "render after the source is added" also fell away: there is nothing to warm up except the ordinary poster thumbnail, which is already warmed.
* **The one-second guard is needed not for the render, but for the REQUESTS to the decoder.** A render from already finished thumbnails can be shown immediately; waiting makes sense only before asking the worker to decode a missing frame (FFmpeg open/seek) — otherwise a misclick costs a real decode.
* **"A strip under each preset" required refining the formulation**: since R.48 a preset is a CHAIN BLOCK in a library outside the project, it does not belong to an effect, and a "strip of presets opposite the effect" cannot be expressed on this model.

### Owner's decisions (AskUserQuestion)

* **Layout — a strip of thumbnails in EACH effect row** (a gallery with scrolling), the name under the thumbnail in a small size, truncation plus the full name on hover. The variants "two panels as in Vegas" and "a hybrid with a flip" were offered and not chosen.
* **Icon — a category glyph PLUS a generated thumbnail**: the effect is overlaid on a built-in test pattern. The variant "Phosphor glyphs only" was rejected (for 27 kinds no suitable glyphs can be found, and there have already been cases of tofu), and the variant "picture only" too.
* **Nothing is written to the index DB** (see the objection above), a RAM-LRU per session.
* **The frame of the strip is WYSIWYG**: the chain already on the clip PLUS the candidate at the end, on the nearest thumbnail to the cursor position. The variant "a clean frame plus one effect" was rejected: after a click the result would not be what was shown.
* **The effect strip = a template + built-in VARIANTS + one's own single-effect presets of this kind.** Variants are our factory sets of parameters with names in the catalog (a click adds the effect already configured); the R.48 library is not touched by this.
* **The section of library presets also gets thumbnails** — including multi-effect blocks; audio presets and the audio picker remain textual (a thumbnail of sound is meaningless).

### What turned up along the way (and what was not in the complaint)

* The egui menu has the default `CloseOnClick`: the popup closes on ANY click inside. A gallery with clickable thumbnails would have been stillborn — `CloseOnClickOutside` is needed.
* Cells BELOW THE FOLD of the scroll were counted and published their rectangles: 160 thumbnails per opening with a dozen and a half visible, and a live scenario clicked an invisible cell past the popup — into the timeline.
* The captions of variants are shared across the whole registry ("Soft" exists for six kinds), so the accessible name of a cell had to be made composite.
* The filmstrip and waveform of a REVERSED event without warp showed not what plays: the first frame of the piece instead of the last. The hole remained from R.37 item 3 and was closed by reducing the formula to one function.
* For an f32 parameter, the spec's even ceiling after `set_param` turns out to be OUTSIDE the spec: the clamp is computed in f64, and the write to f32 rounds up.

### Addendum of the same round: the strength axis

To the question "what is the main parameter of each kind?" (after the gallery was handed over) the owner answered "let's do it". We did NOT declare a "main parameter" for each kind as FCP does — `VHS` and `Color Grade` have four equal ones, `Mirror` has no numeric ones at all, `Corner Pin` has eight equal coordinates. Instead of a declaration the axis runs between two CURATED points — variants — and the "useful range" does not need to be invented: it is already set by the extreme variants.

→ implementation: items 427–433, tests t543–t551, feature note F-AUT-006, live scenario `fxpicker.py`.

## Round R.55 (2026-09-17): compositions — "and by default a hybrid convenient mode is needed"

Nine complaints of the owner, verbatim:

> 1. And slowedandreverb does not trigger on precompositions
> 2. Also on events with a transition (like fade) the morpher breaks and overlays the edits on the whole event instead of "by time"
> 3. When creating a composition, if there was a morpher in the region, for some reason it does not get there, probably the cameras too and the generators, check it
> 4. reverses do not work on a composition
> 5. and also when editing inside a composition, the project timeline plays, but on the timeline itself there are the events of the composition,
> 6. and also if you go into a composition, we end up at the same t where we were, but the composition runs from t=0
> 7. I move the project file onto the timeline and a new timeline opens, but I want it to give me a choice - open it as a project or insert it as a composition
> 8. I don't know how to create a media generator, we have the code but it's unclear how to create it in the UI.. probably a lot of other things are unavailable from the UI
> 9. I would like to try making the transition into compositions a bit more clever, we don't clear the timeline and open a new one, but "push aside/expand" the composition in the same timeline, i.e. we can edit it a little without a harsh transition, and if the composition is inserted with a trim, then on the sides of the expanded composition we show somehow cleverly (darkened?) what else there is inside the composition, maybe it's possible to edit that too, think well about how to make this convenient for the user so that he can edit the composition and immediately play the result, and if he wants he can work with the whole composition directly (as now), but by default a hybrid convenient mode is needed, which gives playback both on the main timeline and on the composition at the same time

Plus the previous two permissions: "Backward compatibility is not needed" and "On any points I may be wrong, ask questions where I am wrong and on the subtle points", plus "Make notes for agents".

### Where the owner was wrong (and this matters more than where he was right)

* **Items 1 and 4 — not "does not trigger", but a DIRECT PROHIBITION.** `validate_reel` rejects a reel instance with a reverse or a time-stretching chain (`NestInstanceNotChainable`, R.50 item 382). The picker button template is `slowed_reverb(0.82, …)`, that is, `rate != 1` right away, and the command fails with an English technical text in the status bar. The prohibition was stated honestly: there was no mechanism (a sub-mix read out of order).
* **Item 3 — generators are NOT lost.** They live in the shared `project.media`, and the clip moves inside together with the reference. What is lost is `lane_order` and the cameras, and the morpher moves in and dies there: without `lane_order` the row order is the default one, in which the morph rows come last, and the "everything below" mode requires a target BELOW the layer — and there is nothing beneath it.
* **Item 6 — the cursor is in fact translated**, the VIEW is not translated: the scroll remained in the root axis. No test saw this defect, because in all scenes the instance sits at zero and the translation formula degenerates into the identity — the same hole as t456.

### Owner's decisions (AskUserQuestion)

* **Tails beyond the trim when expanded — darkened, EDITABLE, the edge of the instance can be dragged right there.** The variants "read-only" and "do not show" were offered and not chosen.
* **A reel in N places: an edit edits ALL places**, reference semantics are preserved; on the first edit — a warning with the verb "Unlink this copy". The variants "ask every time" and "always unlink" were rejected: editing all places is exactly what the reel is created for.
* **In full entry mode the REEL ITSELF plays from t=0.**
* **Morpher: do not cut for the sake of the chain — kill the strength outside the window.** The variant "cut, but only narrowing" and the variant "leave as is, but name it" were not chosen. The wording of the question required a second pass: to the first the owner answered "I don't understand the problem, why doesn't the morpher overlay cleanly along the boundaries but splashes over the whole event if the event has an innocent transition" — and he was right, because cutting for the sake of the chain is not needed at all.
* **A reel becomes an object of the bin** (a card, ×N, the full set of verbs).
* **Of what was found along the way in the round, only the loss of work on the `.otio` drop is taken.** The rest (undo locks itself inside the reel; two instances share the reverb; the morph layer bypasses the warp prohibition) is documented and deferred.

### What turned up along the way (and what was not in the complaint)

* **Dropping an `.otio` does `*self = app` without `guard_unsaved` and without `claim_project`** — bypassing everything built in R.49. Unsaved work disappears without a single question, and the file lock is not taken.
* **The bin does not show reels at all.** A composition can be placed a second time only by copy-pasting an already placed instance; creating an empty reel or deleting a specific one — there is no way.
* **Reverse transport over an nest — silence.** `render_frames_raw` does not wrap the fetcher, the prefetch order goes inside, and the read does not reach it: the invariant "order == read" is violated right now.
* **The reel address in the video plan goes without `NEST_TAG`** and numerically coincides with the address of a FILE with the same id — exactly what `validate` forbids.
* **The nesting discipline did not make it down into the crate `CLAUDE.md` files**, and `docs/architecture.md` does not know about nesting at all (the last edit was before R.50).

## Round R.56 (2026-09-19): compositions — "compositing messes up again"

Five complaints of the owner, verbatim:

> 1. compositing messes up again - so if you slow down the time with slowed-reverb, the same moving text in the composition will start to jerk frame by frame, while compositing assumes the animation will work more smoothly, and so on; probably the same problem with automation in nested slowed compositions too
> 2. somehow it's strange: in the project in rendereg3, if you place a video track under the ceiling (in front of the highest morpher) and drop a video there - it doesn't display at all..
> 3. compositions have no preview for some reason
> 4. I copy the last ZEF in a long composition from rendereg3 via RMB and then go to paste it, also via RMB, and it takes and pastes one part on top and carries the other part (audio) down, and at the very bottom the morpher, because there really was no room for it
> 5. if I press enter on the selected expanded composition I don't get inside, although the RMB item is named exactly that and labeled enter and works. If I just press enter the composition hides back, I want double enter to go into it
    
Plus the earlier permissions: "Backward compatibility is not needed", "On any points I may be wrong, ask questions", "Make important notes for agents, tests".
Project reproduction: `untitled_v26.09.19/untitled_v26.09.19.otio`.

### Where the owner was right and where he was not

* **Item 1 — right, and deeper than he formulated it.** The internal time of the reel was snapped to the frame grid (`clip_source_pts`) — a reel has no decoded frames to cling to, and the sub-composite was additionally memoized by the rounded tick. But even with continuous reel time the title itself would have remained stepped: generators, like files, were requested only on the ticks of the grid.
* **Item 2 — right, but the cause is not the morpher.** The engine stacks layers by the NUMBER of the track, the screen shows them by `lane_order`. A track added from the morph-row menu received the LARGEST number (the bottom layer) and was drawn third from the top; dragging a video row onto a morph/camera row was "purely visual". Item 50 promised "z-order == screen" and it was violated in two places.
* **Item 3 — right; he clarified what he meant:** "there is no preview on the timeline like normal events have — frames and audio peaks." The preview window of a reel renders; on the event the video half is a mini-map, the audio half is empty. This was a decision of R.50 ("a reel never has thumbnails") — it is removed.
* **Item 4 — right.** Paste knew nothing about tracks at all: the clipboard from an expanded strip carried numbers with DIFFERENT bases for video/audio/morph, and "Paste" from the RMB menu put things at the cursor rather than at the click point.
* **Item 5 — right.** Enter was a toggle "expand/collapse", and after expanding the selection was reset — the second Enter hit nothing; the label "(Enter)" stood on an item that the key did not invoke.

### Owner's decisions (AskUserQuestion)

* **Item 1: procedural — continuously, footage — by frames, PLUS blending of frames on a nest** in this same round. The variant "only procedural, defer blending" was offered and not chosen.
* **Item 3: filmstrip and waveform on an event that is a nest**, as for ordinary events.
* **Item 4: RMB "Paste" — the time AND the row of the click.** The variants "click time, tracks as in the original" and "at the cursor, as now" were rejected.
* **Item 5: the first Enter expands (the instance stays selected), the second Enter on an expanded one — collapses and enters fully; Esc/breadcrumb return to the root WITHOUT the strip.** The variant "on exit, expand again" was rejected.

Engineering rules of the round (not owner decisions, but following from R.35 "dragging a track = changing the edit"): the track number within a view = its rank among the rows of that view; blending on a nest is done on the LEAVES (a flag in the sub-plan context), not with two sub-composites.

## Round R.57 (2026-09-19): signed speed — reverse inside slowed and precise transport speed

Two complaints of the owner, verbatim:

> 1. reverse playback in the slowed effect (negative rate)
> 2. in slowed, and in the timeline too, the rate controller should let me set values
>    without turning the range -0.25...0...0.25 into 0, i.e. I should be able to set
>    0.05 or -0.03 for example.

Plus the earlier permissions: "Backward compatibility is not needed", "On any points I may be wrong, ask questions", "Make important notes for agents, tests".

### Where the owner was right (everywhere) and what stood behind it

* **Transport.** The limit of 0.25 lives ONLY in the slider's mapping: a dead zone of 7 % of the travel gives exactly 0, and its edge is immediately 0.25× — the interval (0, 0.25) is unrepresentable by construction. There is no owner's decision behind this: the number coincided with the warp clamp, and the coincidence was taken for a rule. The input field was removed in R.7 at his own request ("two parallel bars, do not change the height") — the form is preserved, the field returns IN PLACE of the bar, not as a window.
* **Slowed.** A small or negative value was silently turned into 0.25× by three independent clamps (the warp map, the processor, the field). A negative speed did not exist as a concept: the map is invertible only with strictly growing consumption, the audio warp reads the source strictly forward, reverse is a static mirror over the whole clip, and the prefetch and the reel sub-mix take the window by its two ends. Note F-SPD-002 called this "a separate round with an honest price" — and it has come.

### Owner's decisions (AskUserQuestion)

* **Minus in slowed — like the Vegas velocity envelope:** the position in the material = entry point + ∫speed; minus goes BACKWARD through the material inside the event, the direction may change in the middle of the event, 0 — freeze frame, at the edge of the material — the frame is held. The "Reverse" flag remains a mirror on top. The variant "minus = a second input to the Reverse button" was rejected.
* **Sound extends to |rate| = 0.01, silence only at zero** (the reverb tail rings out). The variant "mute below 0.25" was rejected.
* **Transport: a smooth scale ±0.01…±2 + Shift + numeric input** (right click or Ctrl+click on the bar); stop — a narrow zone of 1–2 px in the center; double click — 1×; the height of the widget does not change.
* **Slowed range −32…+32, zero allowed**; the automation graph — a zero line and a logarithmic scale by absolute value.

### A consequence that needs to be known

The constant −1 from the start of the event goes into the material BEFORE the entry point (this is exactly the semantics of Vegas). It equals the "Reverse" flag at +1 only if the entry point is shifted by the length of the event. This is a definition, not a defect.

## Round R.58 (2026-09-20): compositions — a strip inside the reel, preview cache, the zero of the reel, the reel step

Five complaints of the owner, verbatim:

> 1. for some reason, when I open a composition I can't expand a nested
>    composition inside it (I open the composition on the whole timeline and can't
>    expand the nested one inside it without getting into it already)
> 2. Compositions' previews on the timeline, it seems, are not cached and every opening
>    of the project is rebuilt from scratch.
> 3. when switching between compositions/root timeline they reset the expanded state, and it would be nice if they kept it so as not to get confused
> 4. When editing a fully opened composition I can't drag an event beyond its limits, but with the composition expanded in the root I can. I don't need playback in the fully opened composition to go to 0.
> 5. The program crashes in the project untitled_v26.09.20.otio.recovery.otio when I stand at ~23:32 (`video.rs:1293: a clip without video fx never gets a
>    decode stage`, `plan.rs:262: leading stop forbids a decode stage`).
>    Before that I added fx to a composition inside a composition (I think it was
>    clip 105) and at that moment it also crashed during playback.

Plus the earlier permissions: "Backward compatibility is not needed", "On any points I may be wrong, ask questions", "Make important notes for agents, tests".

### Where the owner was right (everywhere) and what stood behind it

* **Item 1.** The refusal is two explicit guards in the application (`expand_in_place`, `enter_pressed_on_nest`) with the comment "the strip is counted from the root, and a different reel stands as the root". The comment is wrong: after `swap_reel` the current reel IS the root, the model does not forbid a strip inside a reel, and the undo snapshot already carries `scope + bands`. The cause was in people's heads, not in the model.
* **Item 2.** The preview cache of reels is RAM only, created empty on each opening. The R.56 note "it can't go in the sidecar — it is keyed by the file's identity" was a property of the `ensure_media` API, not of the schema: there are no foreign keys in the DB, and additive tables have already been created three times.
* **Item 3.** A direct consequence of the R.56 decision item 5 ("Esc returns to the root without the strip"), extended to ALL transitions: `set_editing` clears the strips unconditionally, while `NestStep` does not store them.
* **Item 4.** Zero is a wall only in the UI (`snapped.max(0)` in screen coordinates). The format and the model allow a negative `start` in a reel, and the engine does not play it (the instance window is `[source_in, +duration)`), while the strip converts time via `dt = inst.start − source_in` — hence the asymmetry.
* **Item 5.** The decode step of a reel is ONE (the maximum over the instances — that is how it should be, otherwise an inner clip would have two cache keys), but the same number was going as the INPUT OF THE CHAIN of each instance. An instance with no effects, with a leading layer strength (morpher), or with effects without resampling received a sub-composite at S×Quality, which nobody returned to Quality: in debug — a panic, in release — the layer silently dropped out of the frame with `complete == true`.

### Owner's decisions (AskUserQuestion)

* **Restore ALL strips, including the one through which the second Enter entered** — a revision of R.56 item 5 ("exit does not restore the strip"). The Enter/Esc cascade becomes symmetric. The variants "all except the entry one" and "only the root" were rejected.
* **Show the zone down to zero inside an open reel with a veil**, like the tails of the strip; scrolling there is possible; the cursor and playback do not go below zero. The variant "do not show" was rejected.
* **The disk cache of reel previews — in the finished index file `<project>.ktindex.db`.** The variant "a shared data folder" was rejected.
* **Deferring the OTIO growth.** An incidental finding: each reel instance is written as a full expanded `Stack` to a depth of 4 — the recovery file is 115 MB, 15,220 clips with three reels; autosave writes this every time. Recorded as a defect, not fixed in this round.

## Round R.59 (2026-09-20): parameterization of compositions, property model, physics, cost

Four complaints of the owner, verbatim:

> 1. we need parameterization of compositions - suppose someone created a project template for a news broadcast, obviously there can be a crawl line, logos, ads, the news headline, they move back and forth, get shown, etc., and obviously people won't climb into the composition every time and change the text, logos, etc.? We need to think very well about what can be done for this. Or like on streams in OBS, where scenes are made into which sources are substituted - and they show the screen and the chat, etc. This is, for example, the base one.
>    And the user may even need to parameterize an effect parameter somehow.
> 2. the previous item about parameterization points to the idea of refactoring the system of parameters/automation keys, because at some point a plugin system will appear, which must be super flexible like AE, able to control everything and always, so already here one should think about something systemic instead of crutches.
> 3. How deep is the problem - maybe a user will want to add physics, to the same letters in the text generator, so that when slowed down and sharply accelerated they, for example, fall - for this a really very good system must be thought out, so that one can control everything, including from plugins, when the plugin system appears. For the depth of the problem you can study ktprompts and the future features of the program from the db.
> 4. And with all this it is obvious that if the project is simple without tricks - then it must not lag at all, because plugins/curves are not applied / nobody wants physics, and conversely, if physics was added to only one source - that is no reason to break and weigh down the work of the whole project.

Plus the earlier permissions: "Backward compatibility is not needed", "On any points I may be wrong, ask questions where I am wrong and on the subtle points, do not limit yourself in questions", "Make important notes for agents, tests, etc.".

### Where the owner is right (everywhere) and what stood behind it

* **Item 1.** Today there is nothing to parameterize a composition with: a reel has not a single own parameter field, and an instance differs from its neighbor only by its effect chain, level and window. Worse, the title text does not belong to the event at all: it is part of the address-string of the MEDIA, shared across all places of use — so "change the headline in the second issue" has physically nowhere to be written. The note F-TXT-002 three rounds ago named this precisely ("a template is a nested composition with a public set of parameters"), but gave the verdict "do not take" on the grounds that "we have no nested sequences AT ALL". Nesting was done in R.50–R.58 — the premise of the verdict is removed.
* **Item 2.** "Crutches instead of a system" is an exact diagnosis, and here is its form: a parameter for us is a FIELD OF AN ENUM VARIANT, the descriptor is static (`&'static`), addressing is by string, and exactly three carriers out of a dozen are animatable. Everything that is not a number is not a property: flags, lists, colors (stored as triples of numbers), text, a media reference are drawn by hand in their own windows. A plugin cannot be embedded in such a system in any way: its descriptor is known only at runtime.
* **Item 3.** Physics is not "one more effect", and this is the main finding of the round. Our video path is STRICTLY stateless: a frame is a pure function of time, and post-FX memoization, the GPU cache and determinism (invariant 11) all rest on exactly this. A simulation depends on history and cancels this property. The answer is not "introduce state", but "make the simulation a function": compute it from the start of the event with a fixed step, then the state at a frame is again uniquely defined. Incidentally: today the generator sees only its own time and DOES NOT KNOW the speed of the event — and it is exactly the speed that the owner wants the letters to fall with. The speed lives in the engine and must reach the generator as numbers.
* **Item 4.** The requirement of zero cost is not new, but an already binding rule ("nothing in the hot path may be O(whole edit)"), and for audio it is fulfilled perfectly: without curves — not a single extra operation. For video it is half-fulfilled: an event without curves pays nothing, but an event WITH curves clones the whole effect chain on each frame and looks up parameters by string comparison.

### Owner's decisions (AskUserQuestion)

* **Scope — the FOUNDATION**: notes into the feature base, the property model, generator parameters on an event with automation, public parameters of a composition with overrides. Expressions, physics and plugins — only notes and interfaces. The variants "together with expressions", "together with physics" and "paper only" were rejected.
* **Generator parameters — PER EVENT**, like those of effects. Media remains a kind of generator and a cache strip (the decision of R.53 stands), "edit everywhere at once" becomes a separate command.
* **Plugins — all three models at once in the interfaces**, the order of introduction: built-in, then dynamic libraries, OpenFX brought back later. The verdict of F-EXT-001 ("rejected") is EXPLICITLY reconsidered by this.
* **Expressions — their own mini formula, computed live** on each frame; there are no external interpreters.
* **Types of public parameters at once**: number, text, color, flag, choice, media; curves — for number and color, the rest changes in a step.
* **Names of public parameters — free text of the author.** This is the content of the project, like the name of an event, not the text of the program.
* **An instance overrides only what is PUBLISHED**, by value or by curve. The variant "any property inside by address" was rejected: the template would lose its interface and would break on editing inside.
* **Physics — a simulation from the start of the event with a fixed step**, the state at a frame is a pure function; physics gives the generator NUMBERS, not pixels.

## Round R.60 (2026-09-22): parameterization of compositions — "I don't see automation curves anywhere"

The owner's complaint, verbatim:

> About the phase of parameterization of compositions:
> Parameterization of compositions is done strangely: I put an effect, exposed a parameter outside, and suddenly discovered that I can put different values in different places and there will be something like automation curves. At the same time in the parameters window there are only the fields themselves - but I don't see automation curves anywhere.
> This is very strange. Yes, automation curves are needed, but how did such a mistake even get through? There is not a single button in the window about automation. Also the min max values are always set to 0 by default, which is strange, other defaults could be set.

### What the reconnaissance showed (the owner is right on both items)

* **A hidden automation recording mode.** In the "Composition parameters" window the numeric knob had an unnamed `+` glyph. It created a curve with ONE key under the cursor, and from then on every edit of the field put a new key at the cursor position (`bound_apply` with `key = None`). There was no graph, no key table, and no hint in the window — "put different values in different places" is exactly this recording. This is precisely the defect that the R.34 decision "one animation mechanism for everything" was meant to close: for an effect slot and for a generator the curves are drawn by a shared widget, while the instance window bypassed it and created a second, invisible one.
* **"How did such a mistake get through at all".** Test t683 asserted that "a number with a curve puts a key in the instance's material" and was green — it checked the MODEL, not what the person sees on the screen. The lesson of R.40 ("a test asserts the observable consequence and probes from the point where the person acts") was not applied to this window. Hence the guard t693: a list of windows where a value is edited over time, and the requirement that each goes through the shared widget.
* **Zero within the limits.** `PubRange` stores the narrowing as `Option<f64>`, and the field drew an empty value as `0.0`. Meanwhile the target already has its own descriptor (`ParamSpec.min/max/default`), and it is precisely that which is the legitimate default. There was no descriptor "by address" in the model at all: there were `addr_type` and `addr_value`, but not `addr_spec`.

### Decisions of the round

* **Override curves are shown by THE SAME widget** as for the slot and the generator (`automation::draw`): graph ↔ table, the active curve, a key at the cursor, a shared axis lock. The instance window will not have its own key editor.
* **A curve is created by a named action** "+ curve" in the "Automation" section, the menu item — the author's knob name verbatim. The unnamed glyph is removed.
* **A key at the cursor is written only to the VISIBLE curve**; for a knob without a curve, editing the field is a constant. No hidden recording remains by construction.
* **The default limits of a knob are the target's descriptor** (`Project::addr_spec`). Only the NARROWING is written to the file: a value equal to the target's limit is stored as `None`. Resetting to the target's limits is one action. The value at the consumer is clamped by the target's limits, and on top of them by the author's narrowing (it was `±1e9`).
* **There will be no copy of the limits in the publication** — for the same reason that the template value is not stored (R.59): the copy would diverge from the target at the first edit.

## Round R.61 (2026-09-22): segment shapes, a curve from a curve, formula, envelope buffer

Two complaints of the owner, verbatim:

> 1. in fx/gen etc. I would like to use something like a view-curve from another automation curve
>    (of the same event, but say of another effect, another value), the ability to have a custom recalculation formula (the resulting graph is also drawn but not edited with the mouse), and as a continuation - the ability to set an automation curve
>    via formula input (without clicking through the mouse).
>    And also when I copy-paste a curve from one effect, I want to paste it into another, let it at least suggest remapping of fields. And also - judging by the button possibly being "disabled" - there is some state of the clipboard in the code - which is bad, when the button is pressed it should take the value from the real clipboard and only then check whether it can be pasted, and offer actions.
> 2. in automations I would like to set the shapes of segments, one could for example turn on third-order curves for all, one could have zigzags, one could have straight lines, one could change the parameters of curves or set a generation formula from time, or one could configure a separate curve in one's own way, with another type, etc.

Plus the earlier permissions: "Backward compatibility is not needed", "On any points I may be wrong, ask questions where I am wrong and on the subtle points, do not limit yourself in questions", "Make important notes for agents, tests, etc. so as not to return to the same kinds of problems".

### What the reconnaissance showed

* **The clipboard — the owner is right by an indirect sign.** The "Paste envelope" button can grey out, and this truly meant a state inside the program: copying writes BOTH to the system clipboard AND to a mirror `env_clipboard`, and the availability of the button is decided by the mirror. Something copied in another window of the program cannot be pasted. In the second half of the same place, the button greys out for a different kind of effect — that is, remapping is not merely absent but forbidden.
* **Today the shape is one for the whole curve** — a boolean `ease` with the smoothing `smoothstep01`. The request for "segment shapes" breaks this field entirely, which is legitimate given "backward compatibility is not needed".
* **A place for sources has been laid in advance.** `CurveSource` with the variants `Keys`/`Formula`/`Link` has existed since R.59 and is today loudly rejected by validation: it was created precisely so that the evaluator would arrive in the next round without breaking the format.
* **Three feature notes have already been written for this** (F-AUT-007 expressions, F-AUT-008 linking, F-AUT-009 segment shapes), and the owner disputes one of them — see below.

### Where the owner diverged from the reconnaissance (and decided for himself)

The note F-AUT-009 claimed that **a zigzag is not a segment shape but a different SOURCE of values**: "a zigzag specified by a number of periods is not expressible by keys at all, and trying to squeeze it in as a sixth kind into the closed list is the direct path to Smooth 2". The owner decided otherwise: a zigzag is a shape on the segment between two neighboring keys. The reason: a zigzag between two keys has a start and an end set by the keys, while a source describes the curve WHOLE and has no keys at all; mixing them would mean demanding a formula where the person wants "wobble a bit on this piece".

A consequence that the note predicted correctly and that was accepted together with the decision: **a shape must have its own set of parameters, not one stiffness number.** Therefore a shape is an enumeration with data, and a sixth kind breaks neither the file nor the screen.

### Owner's decisions (AskUserQuestion)

* **Scope — three stages in one round**: segment shapes and the buffer, then linking, then the formula. The order from the notes: linking before the formula, because a formula without a recipient degenerates into an expensive way to draw a curve with the mouse.
* **Zigzag — a SHAPE of a segment** (a revision of the verdict of F-AUT-009 item 3, see above).
* **The paste button is always active**; on press the real system clipboard is read; if it does not parse — an error with a reason. The mirror inside the program is removed.
* **Remapping**: fields matching by name and type are placed silently; for the remainder — a matching window "curve → target parameter".
* **Addressing of a link — any event of the project** (chosen against the recommendation "only one's own event"). The derived restriction: a link does not cross the boundary of a reel — outward because the reel would stop being self-contained and law E would break, inward because public parameters of a composition already exist for that.
* **The default shape for new keys lives in the PROJECT**, not in the machine settings: a machine setting would give "one file — two behaviors" (a lesson of Vegas).
* **Non-numeric values are also covered** (chosen against the recommendation "only number and color"): a flag — by threshold, a choice, text, media and color — by snapshots at the marks of the VALUE axis of the leading property.

### What was decided along the way of the analysis

* **The amplitude of a zigzag — in units of the parameter, not as a fraction of the difference between keys.** The most frequent zigzag is a wobble around a CONSTANT value, where the difference equals zero: a fraction would collapse the feature exactly where it is needed. It is clamped by the target's descriptor.
* **The default shape must reproduce today's smoothing bit for bit**, otherwise `GOLDEN_AUDIO`/`GOLDEN_VIDEO` would move, and that is a regression of determinism, not a new capability.
* **Speed (`rate`) accepts only the direct form and refuses LOUDLY**: the closed form of the inverse warp map rests on a piecewise-linear profile. The rule already exists for `ease` and simply becomes wider.

## Round R.62 (2026-09-23): aspect ratio, event geometry, project window, object track

The owner's complaints, verbatim:

> Fix the aspect ratio, for example if the project is 16:9 and we drop a 4:3 source in - it is stretched in an idiotic way:
> 1. I would like to mainly control the pan of the event and, when the source is first added to the timeline, the crop/adjust/as-is aspect ratios of the event; right now this moment is not worked out at all, at least there is no project settings window as in serious programs
> 2. I am not at all sure how this is done properly in serious programs, because for someone one place needs stretching, and for someone not, and what if someone also needs compositions that are square and others rectangular inside a rectangular project? Or different fps, different color encodings, etc.? A very complex point.
> 3. I would like to make tracks, overlay effects, etc., i.e. so that tracks have their own overlay capabilities.
> Backward compatibility is not needed.

And in the course of the questions:

> in Vegas, events always start with Pan as the first item in the chain, which has masks and panning, etc. - maybe this is an outdated approach, but I would not want it to be worse than in Vegas, it needs to be cooler than there.

> it seems to me fx pan 2d can be simplified, but if you turn on pan 3d then it needs to be taken into account somehow. And nothing prevents adding pan several times, right?

Plus the earlier permissions: "On any points I may be wrong, ask questions where I am wrong and on the subtle points, do not limit yourself in questions", "Make important notes for agents, tests, etc. so as not to return to the same kinds of problems, nuances, approaches".

### What the reconnaissance showed (the owner is right on all three items)

* **The stretching is not in the composite but in the decoder.** `sws_scale` pulls the frame to exactly the project size; the aspect ratio of the source is not part of the call; the compositor does not fit a frame "of foreign size" but rejects it. Neither the project, nor the event, nor the reel has a field "conform rule"; not one regression test feeds material with mismatched proportions, so the stretching passes green. The note F-MED-004 measured this as early as 08.09 and gave the verdict "take it, priority A, as the layer's geometry, not as a checkbox".
* **The combo of Pan modes from the three items is dead.** The Fit and Fill branches are byte-for-byte identical, and by the time of Pan the aspect is already lost; Pan is a crop frame in the source's coordinates and cannot produce margins by construction.
* **As in serious programs.** The set of rules is the same for everyone (fit / fill / stretch / as is), but the LEVEL differs: Resolve — the project (retroactively for everything), FCP — the clip (two hundred clips — two hundred edits), Premiere — a program setting (does not affect what is already placed), Kdenlive — a framework profile (the option "do not touch the size" in fact adds a Transform effect). Two lessons not found in the manuals: "fit" puts BLACK into the frame, and on an upper layer this erases everything beneath it (Pitivi #1378) — the margins must be transparent; "as is" is not the absence of a rule, but the only option that does not invent pixels.
* **There is no track as an entity.** Counters plus `lane: u32` on the event; name and height are outside undo. Audio is summed clip by clip directly into the output, video is composited clip by clip with alpha-over, and in the morpher the input is per-clip. The key "(kind, number)" for FX state is unsuitable: inserting a track renumbers its neighbors and would kill the state of the chains (invariant 3) — a track needs its own stable id (F-AUD-004, F-TL-006 predicted this).
* **There is no project settings window** — only a combo of fps/resolution from five presets in the preview panel; there is no arbitrary input.

### Owner's decisions (AskUserQuestion)

* **The level of the rule — project + exception on the event.** The project has the default (applies retroactively to all that follow it); the event has "follow the project" or its own.
* **The default — "fit" with TRANSPARENT margins.** It scales both up and down.
* **Geometry — a built-in node of the event, first in the chain, like Pan/Crop in Vegas.** The node can be dragged along the chain (effects before it work in source space, after it — in project frame), the leading one cannot be deleted, and the SAME node can be added from the catalog again later in the chain. The FX "Pan" (2D) is abolished; Pan3d and CamMotion remain effects and work on the conformed frame. Masks — a separate change afterwards, but the node is designed with a place for the mask.
* **On placement: the first clip into an empty project sets the project settings** (raster, fps, if fps divides 90 000); afterwards a mismatch does NOT ask (UI rule 4), but is reported in the status line with the address "where to change it" (rules 16, 19).
* **Reels get THEIR OWN raster and THEIR OWN fps** (chosen against the recommendation "raster only"). This is a revision of the R.50 decision "inherit everything" — confirmed by a separate question with the named consequences: law E and the oracle "reel == expanded" hold only with settings equal to the project's; its own fps — quantization of the reel's continuous time to its grid (the frame is held, no blending), the audio grid is common; a reel instance is conformed by the same rule as a file.
* **"Different fps, different color encodings" of sources — not in this round**: create notes in docs/features (frame-rate conforming, color management) and record "Deferred".
* **Track mute and solo — project content, and they apply to export** (chosen against the recommendation "solo — listening only"). Consequence: the render window must list the muted tracks and the active solo BEFORE start (rules 16, 19), and kt-cli must print them.
* **Track properties in this round — all four**: level (dB / opacity), FX chain (audio and video), lock, blend mode of the video track.
* **The input of a video track's chain — the composite of its own events ("track = an implicit reel")**: transitions, overlaps and fades inside, then the chain, level, downward blending; a track without FX, at level 1 and with normal blending, goes the old way byte for byte.
* **Track envelopes (over timeline time) — a separate change afterwards**; for now the values are static, through the property descriptor.
* **Two changes**: `frame-conform-and-event-geometry` (A) and `track-object-and-track-fx` (B); B3 depends on A1, so A goes first.

### What was decided along the way of the analysis

* **The contract "the decoder returns exactly the requested quality" does not change — what changes is WHO computes the requested one**: a single geometry function of (native, raster, rule, step) gives both the decode size and the placement rectangle; the order still equals the read (invariant 4).
* **An audio track without chain, level and mute is summed into the output in the old order** — otherwise `GOLDEN_AUDIO` would move from a single reordering of f32 additions; the track stage appears only for a track that needs it (precedent — "input equals step → bytes unchanged").
* **Mute is a multiplier of 0, not a removal from the graph** (invariant 3): lifting mute does not start from a cold reverb.
* **The file version — v11 for the whole round R.62** (both changes), predicate by content.

## Round R.63 (2026-09-24): version 26.09.23-d5da7cf — file does not open, silence in the loop, formulas, focus, ducking, publication, windows

The owner's complaints, verbatim:

> This concerns version 26.09.23-d5da7cf7
> 1. my project untitled stopped opening in the new version
> 2. I created a new simple project paintitblack and there the region-loop clearly at the end breaks the sound, there silence is taken. This is a real regression. It happens with sources/The Rolling Stones -
>    Paint It, Black (Official Lyric Video) [O4irXQhgMqg].webm but with MCR-T ft. Joost -
>    Buurman Uit Berlijn - this happens less often.
> 3. I don't understand how to enter formulas into automation curves - I write it in and it either erases the formula or nothing happens at all
> 4. if you start typing something in the formula field, the input focus goes there, and no matter how I click on the rest of the interface - pressing space does not lead to pause, this is an example, probably the same situation with all other fields and buttons, until I close the effect window - the same space will not work.
> 5. it is impossible to specify the shapes of curves in automation curves - the context menu just disappears, I don't have time to reach it, and also it is not clear between which points (and are they points?) I will set the shape.
> 6. the fx geometry is still very poor, just dull boring input fields, even in Vegas it is done better, and we need to be better than Vegas, in Pr/Ae it is more convenient than in Vegas, we need to make it more convenient than anyone. AT MINIMUM we must have convenient editing right in the preview, that is the minimum.
> 7. I don't understand how to manage tracks, I didn't find it, maybe it hasn't been done yet in this version.
> 8.1. I don't understand what "duck under a track" "duck under track" is??? what??? what is under the track that needs to be ducked?? is this Audio-Ducking? why didn't you just name it normally? A remark to future agents.
> 8.2. also a very fucked-up way of controlling it. It could be done more conveniently simply via fx-audio on the whole track. A remark to future agents.
> 8.3. even if we assume that I really wanted such a window (no, I didn't want it) - why the hell am I forced to remember every time which parameters I entered last time? Why does the agent ignore the user's convenience requirements and force him to enter all the parameters again if he changed his mind? Why does it ignore the requirements of automation curves and their very presence? These are remarks to future agents.
> 8.4. a conceptual remark - even your next obvious step - you will just shove ducking in as an fx, and not think about the user, about how as ducking one can show the same duplicated audio-compressor widget with dimmed fields, i.e. a compressor on the background object and a compressor on the foreground object. This is also a remark - meaningless copy-pastes of the same code keep piling up everywhere, when one could even show visually to the user that, for example, ducking works in the program through two compressors.
> 9. why does the first geometry have no ability to publish parameters while the second has? This first geometry goes as an fx, right? Or do we have duplicated code?
> 10. I took MCR, put it in a composition, put Radial blur on it, published the amount parameter, changed it on the composition - and it doesn't change.
> 11. If you publish a parameter without entering the composition fully, then for some reason we automatically enter the composition fully.
> 12. Overall it is done damn idiotically that the composition parameters open in a separate window, when they could be put into the same "fx-chain" for convenience. I am imposing a ban on any agents spawning any additional windows without an obvious purpose (like a window on closing the program). I'm sick of agents shitting out these windows and not thinking.
> 13. paintitblack ktflight - that is what I was doing before the underruns flew. They stopped later, but such behavior is scary, I just jumped to a section with one long event (yes there are others above it, and yes this one has effects) but that is no reason to load video with such underruns for so long.

And along the way: "there are different sources in developer/kt.9/sources too" — the webm of Rolling Stones and the review-mp4 were placed in `sources/`, the MCR-mp4 is not on this machine.

### Rules the owner set in this round (mandatory for all agents)

* **Call things by their common names.** "Duck under the track" instead of "Ducking (audio ducking)" is a naming error, not a matter of taste: the person did not understand what it meant. A menu item says WHAT will happen (rule 18), and in the industry's own words.
* **BAN on new windows without an obvious purpose.** A window is allowed only where it cannot be avoided (a question when closing the program with unsaved work). Parameters that can be shown in a place that is already open (the chain strip, a panel, a row) are shown THERE.
* **Parameters of any tool are REMEMBERED** — the person does not re-enter them if they change their mind; a tool with a curve must have an automation curve that can be edited and recalculated.
* **Do not multiply copies of one widget.** If the program already has an instrument (the compressor), a new function (ducking) is shown through it, not through its own set of fields: two compressors, one on the background (with the key from the foreground) and one on the foreground.
* **A UI rule must be verified in a LIVE window** — in this round five of the thirteen items were green in the harness (the formula, focus, the form menu, the file, the loop).

### What the reconnaissance showed

1. **The file does not open — reproduced on this machine**: `untitled_v26.09.19.otio` (and its recovery from 09.20) is rejected by validation with `RateShapeMustBeLinear(ClipId(17))`. The file has 14 `rate` curves written with `"ease": true`; the R.61 reader expands `ease` into a "smooth" shape, while the speed validator from R.61 accepts only a straight line. Before R.61 the warp did NOT read the `ease` shape at all (only `cam_at` read it) — the speed curve always sounded piecewise linear. So the correct migration for `rate` is "linear", which is how it sounded, and the refusal on reading is a format regression (the promise "the bytes and sound of old files do not move" was broken; backward-compatible reading is always required — "do not" applied to WRITING by an old reader).
2. **"Silence at the end" — an event consumes more material than there is.** In paintitblack, events 3/4 (Rolling Stones) last 3.3…320.3 s at rate 0.75 and consume 237.7 s of material, while the file lasts 226.27 s. The last ≈11 s of the event read past the end of the file; the engine computes the window by the EVENT (law P — the plan does not know the media length), and a miss inside the window = "silence with an underrun". **The reconnaissance of task 1.1 is closed by measurement:** our kt-media returns exactly 226.268 s for this webm (like ffprobe) — the duration is correct; and `trim_max_end` for a non-mirrored event returns `i64::MAX / 4`, that is, the right trim has NO limit at all. The edge was dragged to the right already at rate 1 (then the overrun was 90.7 s), and slowing down reduced it to 11.5 s. Neighbouring event 5 (277.9 s → 208.4 s) lies inside the material and does not glitch; for MCR the event of 17.6 s is why it is "rarer".
3. **Formulas.** Each frame the formula field re-reads the text FROM THE MODEL and applies only legal text: an unfinished input (`sin(`) is erased on the next frame, and the first legal prefix (`t`) is applied immediately and zeroes the keys. In addition, the graph draws `c.keys`, while a formula curve has empty keys until resolved — so nothing appears on screen.
4. **Focus.** egui 0.36 gives focus to a `TextEdit` only on Esc/Tab, on another focusable widget, or when the field disappears; a click on the timeline (`Sense::click_and_drag`, not focusable) does not touch focus. `handle_keys` returns to `egui_wants_keyboard_input()` — all hotkeys are dead until the window is closed. Rule 14 is in the checklist, but there is no general mechanism "a click outside the field releases focus".
5. **Form menus.** Both `menu_hit` and `seg_hit` are computed from `hover_pos()` EVERY FRAME, and the content of `context_menu` is redrawn every frame: the pointer moves to the popup — the hit disappears — `ui.close()`. The keyframe menu suffers the same way. The area is not highlighted and not named.
6. **Geometry.** `EventGeometry` — the rule, the crop of 4 sides, position, zoom, rotation, anchor; the form — numeric fields of `params_grid`. There are no on-screen controls (F-UI-001 — "candidate", verdict "take it").
7. **Tracks** — version d5da7cf is the archive of change A; change B (the track as an object) did not yet exist in it. In kt26.09.24 the M/S/lock/FX badges sit under the row name.
8. **Ducking** — the `draw_duck_dialog` window with four fields, a snapshot of the curve onto the events, parameters are not saved (`DuckParams::default()` every time), the curve is not editable as an instrument curve. The note `compressor-and-limiter.md` already contrasts "snapshot" with "live sidechain".
9. **The lead geometry is not published deliberately**: `pubc = (!lead_sel).then(...)` — its address is not `ParamOwner::FxSlot` but the event field `geometry`, and there is no `ParamOwner` variant for it. The second node is an ordinary slot. There is no code duplication; there is a hole in addressing.
10. **Radial blur in a composition** — the model path (`apply_overrides` → `set_value` through the passport) and the sub-renderer key `(reel, instance_content)` are in place; the t-tests cover `FxSlot` only for `zoom`. The cause was not found by reading — to be reproduced.
11. **Entering on publish** — `Command::PublishParam` edits the CURRENT reel, so the confirmation enters inside (`enter_reel_for_authoring`). The command needs an explicit reel address.
12. **The instance parameters window** — a separate `InstParamsWin`; the generator event already has a precedent: the source parameters are the pinned head of the chain strip (R.59).
13. **Underruns** — per the flight log from 3501.8 s of the session every jump (817, 528, 539, 500, 172 s) gives ≥1 s of complete silence (u≈48000 frames/s, w=4800 — a miss of the pre-roll), while the offline render cost of the same places is 2 % of real time. The long event — review-mp4 870 s (clip 8 without FX, audio clip 9 with one FX). To be reproduced in a live window with these files.

### The owner's decisions during R.63 (asked directly)

- **Ducking → a LIVE SIDECHAIN COMPRESSOR** (not a curve snapshot, not an "effect from four fields"). R.63 did only what does not touch the engine: the name by industry usage, a section in the audio track window instead of its own window, parameter memory. The sidechain itself — as a separate change `sidechain-ducking-compressor` next.
- **Geometry on the preview — IN ADDITION to the fields, not instead of them** ("and not instead of the fields but in addition to them, and in a normal form"). As a separate change `geometry-on-screen-controls` next; the verdict on F-UI-001 was raised from "candidate".
- **The underruns of item 13 — on the LONG review file, not on Rolling Stones** ("the underrun is not on rolling stones but on the long review"); the files are in `developer/kt.9/sources`.

## Round R.64 (2026-09-25): on-screen geometry controls — "make it the most convenient"

The round's claim is item 6 of the R.63 list, postponed there as a separate change by the owner's decision. His words, verbatim:

> 6. The geometry FX is still very poor: plain boring input fields, even Vegas does it better, and we need to be better than Vegas; in Pr/AE it is more convenient than in Vegas, we need to make it more convenient than everyone. AT MINIMUM there must be editing right in the preview that is convenient, that is the minimum.

Clarification given as a direct answer to a question (R.63):

> item 1. and not instead of the fields but in addition to them, and in a normal form

Decisions given as direct answers to questions before the start of R.64:

- **Take ALL the geometry at once**, not half of it: move, eight scale handles, rotation, the ANCHOR with compensation (the picture does not drift — including on a rotated layer, where After Effects has this broken) and cropping with its own handles.
- **The field form is "as in Pr/AE"**: a "Transform" section with collapsible groups, sliders for zoom and rotation, and a small frame diagram next to the numbers.
- And verbatim: **"and don't forget for the future that all the other widgets and windows also need to be gone through"** — that is, to bring not only geometry to this look. Recorded as a separate future job so that it is not lost.

## Round R.65 (2026-09-26): ducking becomes a live sidechain compressor

The round's claim is the decision from R.63 postponed there, recorded there verbatim:

> **Ducking → a LIVE SIDECHAIN COMPRESSOR** (not a curve snapshot, not an "effect from four fields").
> In R.63 only what does not touch the engine was done: the name by industry usage, a section in the audio
> track window instead of its own window, parameter memory. The sidechain itself — as a separate change
> `sidechain-ducking-compressor` next.

His R.63 complaints from which this grew (verbatim):

> 8.2. also a fucking awkward way of controlling it. This could simply be done more conveniently via audio
>      FX on the whole track. Note for future agents.
> 8.4. a conceptual remark — even your next banal step: you'll take and cram ducking in as an FX, and won't
>      think about the user, about the fact that as ducking one could show the same duplicated audio-compressor
>      widget with dimmed fields, i.e. a compressor on the background object and a compressor on the foreground
>      object. This too is a remark — everywhere meaningless copy-pastes of the same code accumulate, when one
>      could even visually show the user that, for example, ducking works in the program through two compressors

Decisions given as direct answers to questions before the start of R.65:

- **The instrument lives ONLY in the track's chain** (like the Track FX ducker in Resolve), not on the event. The key is another audio track as a whole.
- **Remove the snapshot entirely**: the "compute the volume curve" button, the `ducking.rs` module, curve generation and their tests will no longer exist. The path of "handing the envelope outward" is not kept — the owner chose this knowing it would disappear.
- **The dimmed second compressor is a REAL speech compressor**: the second widget edits the compressor in the chain of the KEY track (and creates it there if it does not exist yet), rather than merely showing the level. Speech is compressed by its own instrument, music by the sidechain.

## Round R.66 (2026-09-26): window forms — "go through all the widgets"

The round's claim is a direct request from the owner, given in R.64 and recorded then:

> and don't forget for the future to go over all the other widgets and windows too, this will need to be done

That is, to bring the forms of ALL windows to the look he accepted in R.64 for the "Transform" section: collapsible groups, X/Y pairs on one line, units from the passport, a slider for a bounded value, a diagram next to the numbers, reset with one action.

Decisions given as direct answers to questions before the start of R.66:

- **Scope: a guard over all windows at once, re-layout of the six densest.** The machine check covers ALL 21 windows of the `WINDOWS` registry (the body scrolls, action buttons are reachable, units and limits come from the passport, not a single number outside the common widget), while the FX, morpher, transition, camera, project settings and render windows are re-laid out by hand.
- **The deferred limit of R.64 is taken in this same round**: when the transform frame is smaller than about 3 grab zones of a scale handle, the handles are moved OUTSIDE its edges, and the body stays inside. Visible in his own project `paintitblack.otio`: the rule "as is" plus the stub give a frame of 45×25 px, where the eight handles cover it entirely and there is nothing to grab the body with.
- **The criterion for a "normal look" is the "Transform" sample** from R.64, not the agent's taste.

## Round R.67 (2026-09-26): old files, whole curves, keys that record themselves, the frame, reel previews, stills

The owner's complaints, verbatim (his numbering; "6" occurs twice, and "5" comes after "6" — as in the original):

> 1. The project 26.09.26/untitled2 does not open — what is the reason for this?
> 2. And in untitled, for example, rendereg3 is broken — as if half the effects dropped out — answer what this is due to
> 3. The automation key curves are done terribly, they simply do not work: I set a curve between two points and nothing happens, I cannot set curves between several points, and if I set curves globally on all automation keys (the one that says Shape of all segments) — then it works (for example a zigzag), but sometimes it does not work either (only one field changes, although I set zigzags in two fields — the second field does not change at all, and if I delete and re-add it for this field and specify a zigzag, then it works too) — VERY badly done. And also — visually the user cannot see at all what kind of curve is there, and there is not even a possibility to adjust something with the mouse instead of typing numbers into the fields. VERY BAD. NOBODY THOUGHT ABOUT THE USER AT ALL. Also the fields again grow in width beyond the window, which is also VERY BAD. AGAIN NOBODY THOUGHT.
> 4. You specify a formula in the curves — and on the graph again absolutely nothing is displayed, and you cannot even understand whether it works at all, and I said there were two points in the task: 1. about the shapes of segments — in essence only the basic item of all shapes is 50% done. 2. about view curves — I simply do not understand how to assign/insert them.
> 6. It is completely unclear what can be specified in curve formulas, there is no reference, no hints at the moment of input, etc.
> 5. Suppose I typed a zigzag — and then, during playback, for some reason keys appear on the graph, right during playback — by themselves; this was also not tested and not checked by the agents
> 6. If after this you enter a formula — the points disappear, the zigzag disappears — and that is correct on one hand, but on the other hand — maybe I changed my mind or made a mistake and would like to return to the zigzag? And in general this block of automation keys is formally made very crookedly. Once again — I can EITHER draw the points myself and place them, or place them in a table, or set a curve over the whole range, or a formula, or set something on separate areas, AND ALSO A COMBINATION OF ALL OF THIS. And the interface must also make it clear that you can do it this way and that way and both at once. And right now there is an incomprehensible mess there.
> 7. HOW MANY TIMES DO I HAVE TO REPEAT SO THAT ALL ___ALL___ AGENTS THINK ABOUT THE USER FIRST? WE DISCUSS THE SAME THING IN EVERY TASK, AGAIN AND AGAIN! THIS NEEDS TO BE CHANGED RADICALLY!
> 8. Again about the same thing: why on earth does the user need the fx-chain covered by a long label "Composition parameters"? It is also blue… if you select it, you get a long blue rectangle. Again two childish mistakes in one place, which the agents do not check at all.
> 9. I said that all the controls should be moved as much as possible also to the preview — they made "Geometry", and in the end, instead of control in the preview, we only have meagre control inside the fx widget. Again the agent did not work, does not understand what things are implied in such programs as "preview". Even if you look at this preview in the widget — it is absolutely not informative, if you zoom it even 2x — there is simply nothing in the preview. AGENTS DID NOT WORK AND DID NOT THINK!!!!!!!
> 10. I open paintitblack and the preview of the composition AGAIN is not saved, every time it computes the preview again; this bug already existed before with another project.
> 11. Again the agents did not work and did not think — in the automation curve of the composition parameters you cannot set the shapes of the whole segment (as in fx) — it turns out we have some kind of duplicate code there instead of a generalization! And if you enter there, for example, t*2 — it just erases the field after enter and nothing happens at all.
> 12. The automation curves in the composition parameters do not work, I set points (a hill from 0 to 1 and back to 0) — the value is just 1.0 and does not change. The agents DID NOT WORK. DID NOT TEST. Region: NOT_WORKING_KEYS, project: paintitblack_v26.09.26
> 13. Static pictures are not displayed at all.. there is an eternal Loading... THIS IS FUCKING BULLSHIT. Not only does it not work — you will also only find out about it at the moment of viewing.
>
> These points need to be finished thoroughly. Work through each point separately, one by one. Backward compatibility is not needed. On any items I may be wrong; ask questions where I am wrong and on subtle points, do not limit yourself in questions. Make important notes for the agents, tests, etc., so that we do not return to the same kinds of problems, nuances, approaches.

Round projects: `sources/26.09.26/{untitled2,untitled,paintitblack}_v26.09.26.otio`.

### What the reconnaissance showed (before the questions)

1. **untitled2** — the envelope file is **v5** (18.09): in the morphers `"targets": null` (then — "everything below"); the reader accepts a list or an object `{on: …}`, `null` — no. The project-format spec promises that a file opened by the previous build will also open in the next one.
2. **rendereg3** (1377.4–1404.9 s) — all 21,833 effects of the file are read by the current model, and on loading not one is "dropped"; the active events of the region are the nested reel "Reel 3" (three levels) under the morpher layers (fx 1 → 3). The media is not present on this machine.
3. **The graph draws straight lines between keys** — the shape of a segment is not shown at all; t698 checked the model, not the drawing, which is why it was green. "The shape of all segments" and its parameters edit the ACTIVE curve (a click on the colored name), not the curve of the field — hence "the second field does not change". The formula preview computes `t` from the left edge of the WINDOW, the render from the start of the material. The connection (view curve) has no door in the interface at all: the model and tests exist, but there is nothing to assign it with. There is no help on the formula's names.
4. **Self-recording keys**: a field with a live curve shows the value under the cursor; a `DragValue` with limits clamps what goes beyond the passport (a zigzag ±0.1 around 1.0) and reports "changed" → the field writes a key at the cursor every frame of playback. In `paintitblack_v26.09.26` there are exactly such keys: 66.1→0.20, 69.7→0.98, 69.9→0.07, 72.3→0.99, 74.7→0.16.
5. **Composition parameters**: the formula is rejected by validation (`CurveSourceUnsupported`), the window silently erases the text; there is no "shape of all segments" block on the instance (a duplicate instead of a shared block). In NOT_WORKING_KEYS there are two instances of the reel one above the other (V1 and V4), the curve is on the upper one.
6. **The head of the instance strip** — `📌 Composition parameters` in the accent color, without truncation.
7. **The frame in the preview** is off by default (`GeomMode::Off`), is not remembered, and is turned on only from a right-click on the preview; the diagram in the widget is clipped by its own rectangle.
8. **Reel previews**: the write to the sidecar happens only after the offline audio; a cancelled task lands in the negative cache as "failed"; the owner's sidecar holds a strip of 40 identical frames of 2237 bytes each.
9. **Stills**: the path `sources/a3566951817_10.jpg` relative to the project file gives `sources/sources/…` — the file does not exist; the placeholder for a missing file must show the name, not "Loading…"; there are no fixtures with stills in the project at all.

### The owner's decisions (AskUserQuestion, 2026-09-26)

- **Item 1 — do not fix the reader, but create the item "File → import an old-version project", and fix the reading precisely in the import.** Verbatim: "make an item in the File menu for importing old versions, and fix precisely in the import so the reader can read it normally. And fix it so that other agents also automatically make fixes into the import of old versions; gradually these fixes will be removed as old versions go away". → Rule for agents: if you broke the format for an old file, the fix goes into the import, not into the reader; the fixes die off together with the versions.
- **Item 2 — "run it yourself, for example, on the version tested-48 and find the differences, I feel that something is wrong"; the VIDEO FX dropped out (not visible in the frame).** The tag `tested-48` (19.09, R.56/57) exists; comparing the frames of the region between it and the current build is the round's task with a named cause.
- **Item 6 — a formula layer over the keys PLUS the formula as the SHAPE OF A SEGMENT**, combinable: keys (by mouse/table) → a segment has a shape OR a formula → to all segments at once → on top a formula layer (it sees `k` — the value of the keys with their shapes) and/or a connection; everything is visible on the graph and can be removed separately. The owner's first answer: "item 1 but this is all still not flexible, I want more flexible control" — more flexible exactly this way.
- **Item 5 — only fix**: a key is set only by a human gesture; no recording mode is to be created during playback.
- **Item 9 — the frame is on by default for the selected one; turning it off is remembered** (the user's storage).
- **Item 12 — "I don't remember" where the hill was** → both paths are checked: self-recording overwrote the keys (one defect with item 5), and a clean hill on a composition parameter plays.
- **Item 7 — both mechanisms, but the run is NOT on his projects**: (1) every UI scenario in the spec has a "user path" (mouse steps and what is seen), without it the check is red; (2) before the report — a live run on OUR scenarios/projects with screenshots in the report. Verbatim: "item 1 but not on my projects".

## Round R.68 (2026-09-27): speed of verification — "they run for a very long time, something must be figured out"

The owner's claim, verbatim:

> tests in finish.sh or something like that — they take a very long time to run, we need to think of something

Measurement before the work began (machine with 32 cores, warm tree, commit 5c7dc09):

| Stage | Time |
|---|---|
| live scenarios (50 of them, sequentially) | **1012 s ≈ 17 min** |
| `cargo test --workspace` (warm run, separate from finish.sh) | 259 s |
| build cascade from an edit in kt-model (`--no-run` of the whole workspace) | 61 s |
| build of kt-app + kt-cli (the finish.sh stage) | 43 s |
| kt-gpu stage in finish.sh (duplicate of what the workspace run already did) | 10 s |
| four SIMD stages in finish.sh | ~2 s |

That is, 85 % of the pain is the live scenarios, and inside them the longest is `nestparam` — 136 s alone.
The second-heaviest item is the run of the workspace tests. The SIMD stages, of which there are four in the script, cost two seconds for all of them.

The owner's decisions (AskUserQuestion, 2026-09-27) — verbatim:

> item 1 + mold + cut down the restarts of kt (after all, you can open projects one after another) + part of the tests
> moved to non-gui, since the tests partly repeat each other's actions; a large simple part can calmly be moved
> to headless, and the complex moments or those never clicked through even once are clicked through

Breakdown by item:

- **Item 1 — the set of checks is NOT weakened** (the chosen option was "only the mechanics, do not touch the set"): the full run remains mandatory, a "green" does not become cheaper.
- **mold** — the workspace linker is changed to mold (version 2.30.0 is already installed on the system).
- **Reduce restarts of kt-app**: one process per worker, projects are opened one after another inside it, rather than 50 cold starts of 3–5 s each. Scenarios that check the START itself (recovery after SIGKILL, the question about unsaved work, the lock, importing an old file) keep their own process.
- **Part of the checks — in headless mode**: the live scenarios partly repeat one another; the simple mass part moves to the harness/model tests, while the complex places and those that have NOT been clicked through EVEN ONCE are clicked through with the mouse (for this a report of "what was not touched live" is needed).
- **Parallelism is allowed** — the answer to a separate question: "Yes, with a limit on load and automatic re-run of a red one." This is a revision of the rule of R.59 §468 "live scenarios are not run in two threads" (it appeared after a second live check under load of ~14 turned green scenarios red): each worker has its own Xvfb and its own storage, the number of workers is limited, a red scenario is automatically re-run ALONE once, and only a second red counts as red.
- **The target — `finish.sh` ≤ 5 minutes**, printed at the end together with the time of each stage; a stage over budget is marked in the report.

Noticed during the measurement (not part of the claim, but red): `fxfield` and `fxpicker` on commit 5c7dc09 fail in a single run — before the round they need to be looked into separately.

## Round R.69 (2026-09-29): the mechanics of a gesture in a live run — chosen by the owner from the queue

There is no new claim. After R.68 was closed, the owner asked verbatim:

> what else can we do next?

He was shown the list of items deferred from the specs (the mechanics of a gesture in a live run; the growth of `.otio` on reels §464; the list of encoders versus what actually opens; re-linking of sources; frame-rate conforming of the source; color management; ui-reach debts), and he answered:

> 1

That is — **the mechanics of a gesture in a live run**, the first item of the list. This is a continuation of R.68 and the only thing that moves `finish.sh` toward the 300 s target that the owner set in R.68 and that a revision of the set (tasks 5.2/5.3) does NOT reach: the floor with the current mechanics is ≈330 s (215 s of the workspace plus ≈115 s of the fixed costs of the live scenarios).

Measurement before the work (commit cbc5a3e, warm tree, 32 cores; live run 1582 s sequentially, 424 s on the clock across four workers, 45 scenarios):

| source of fixed sleep | count | in `kt_live.py` | in `ui-probe.py` | total |
|---|---|---|---|---|
| `click` | 202 | 111 s | 47 s | 158 s |
| `key` (including typing a string character by character) | 102 | 61 s | 17 s | 79 s |
| `drag` | 30 | 41 s | 36 s | 77 s |
| `right_click` | 43 | 28 s | 10 s | 38 s |
| `double_click` | 16 | 11 s | 9 s | 20 s |
| `hover`, `wheel`, `close_window` | 17 | 13 s | 4 s | 17 s |
| **total** | **410** | **265 s** | **123 s** | **387 s (24 % of the run)** |

Plus ≈341 calls to `xwd`+`convert` per run: each pixel read is a full screenshot of 1920×1080 and two ImageMagick processes, and `pixel_settled` repeats this until the picture changes.

Two conclusions, which are the subject of the round:

- **The guard `live-sleeps.py` does not see `ui-probe.py`** — 123 s of sleep live outside its area, and the rule "a pause only with a reason" does not touch them;
- **a reason in words turned out to be not enough.** All 265 s in the harness are legitimate pauses with an explanation longer than forty characters. As long as a sleep has no MEASURED budget, prose will not hold it: this is the same lesson as the registry of windows (R.66) and the guard of the PATH (R.67).

## Round R.70 (2026-10-02): growth of `.otio` on reels — the next item in the queue

There is no new claim. After R.69 was closed the owner said verbatim:

> let's go on

The next item of the same queue of deferred work was taken — the one he saw in R.69 and from which he chose the first: **growth of `.otio` on reels (§464)**. This is the only item in the queue that the owner feels in HIS OWN projects right now: three reels gave **15,220 clips and 115 MB** per file, and autosave writes this volume regularly. The decision "fix it separately" was made by him already in R.68.

Measurement before the work (synthetic scene: reel L1 of 10 clips, L2 made of instances of L1, the root made of instances of L2 — two levels of nesting):

| clips in L1 | instances of L1 in L2 | instances of L2 in root | clips in the MODEL | clips in the FILE | size |
|---|---|---|---|---|---|
| 10 | 1 | 1 | 12 | 10 | 33 KB |
| 10 | 2 | 2 | 14 | 40 | 127 KB |
| 10 | 4 | 4 | 18 | 160 | 496 KB |
| 10 | 8 | 8 | **26** | **640** | **1.97 MB** |

The growth is MULTIPLICATIVE in the number of instances at each level, not in the depth: the body of the reel is written anew into EACH of its instances.

**The owner's decision (AskUserQuestion, 2026-10-02) — verbatim:**

> almost item 1. — we are moving away from the otio format, we leave otio only for export, and we make breaking changes so that our format becomes a hundred times better

That is, to the question "what do we pay for the file size" he answered more broadly than the question: OTIO does not need to be fixed, we need to STOP keeping the project in it. OTIO remains a door outward (export) and a door inward for already existing files (import, the rule of R.67), while the project lives in ITS OWN format, and breaking changes are allowed outright.

**The owner's clarification (2026-10-02) — verbatim:**

> I am against binary blobs — our file must be indexable in git

This caught a hole in the first plan: for the sake of size a COMPACT JSON was chosen, but compact means one line for the whole file, that is, git shows no diff at all. Measurement: with 50 events the compact file is 1 line, and editing one event "changes" the whole file; the line-by-line (pretty) file is 628 lines, and editing one event changes **1 line**, the write is deterministic. The price of line-by-line is 1.7× in size (11 KB against 7 KB) — still hundreds of times less than `.otio`. Decided: the project file is TEXTUAL, LINE-BY-LINE and DETERMINISTIC; "indexable in git" is a measurable rule: editing one event changes a few lines, not the file.

**The repair plan written in R.68 rested on a wrong premise and cannot be executed.** The `project-format/design.md` said: "The bodies already lie once in `kt.sequences`, expansion is needed only for other readers", and from this came the option "expand by ONE level". The check showed: `KtSequence` has NO field with clips at all — the bodies of reels live ONLY inline, and truncating the depth would take content away from our own reader too. What was promised to the owner ("our reader loses nothing") requires another solution — the body is written ONCE per reel, and repeated instances travel as an empty `Stack` with their own address (`kt.seq`), by which the reader already collapses them today.

## Round R.71 (2026-10-12): the claims file `ktprompts_v26.10` — indexing into the nearest plan

The owner put the claims file (a verbatim copy — `docs/ktprompts/v26.10.txt`, 36 KB, about 75 items in twelve areas) and said:

> Index /media/e/developer/kt.9/ktprompts_v26.10 into the nearest plan of edits

His rules from the file itself, verbatim:

> My claims — they need to be divided into stages, discussed and decided.
> These points need to be finished thoroughly. Work through each point separately, one by one. Backward compatibility is not needed.
> On any items I may be wrong; ask questions where I am wrong and on subtle points, do not limit yourself in questions.
> Make important notes for the agents, tests, etc., so that we do not return to the same kinds of problems, nuances, approaches.

And from the CORE section — a requirement for agents that is not yet in the rules:

> any agent that adds any new feature that breaks otio must record it in a separate list, and only conditionally in a year will we return to it, with the import handled similarly.
> The new format is called ktproj.

**What was done at the proposal stage** (`openspec/changes/r71-ktprompts-index`): each item was checked against the code and the specs; some are already closed (generator automation — R.59, generator and effects in one window — R.59, freeze frame — R.57, crash report — R.49, leaving OTIO and git — R.70); some contradict his own decisions of R.67/R.70 (the format name, the import door); two claims are repeats of earlier topics (resize/the seek zone steal clicks — r.12/r.13; "obvious labels" — checklist item 17 of r.7). The registry with states and eleven questions are in the change's `design.md`; the owner's answers will be appended here verbatim.

## Round readme-showcase (2026-10-21): README, screenshots, docs; English from now on

From here on entries are written in English; the owner's words stay verbatim.

> Right now the repository's readme needs to be finished in full, take photos, throw together documentation.
> We work in master. Commits and text from now on are in English.
> Use the sources from kt.9/sources/README_SOURCES — so that the timeline is full of all sorts of editing.
> Some things (the automation curves of slowed-reverb) should also be stuck in and shown in the readme.
> All of this will be uploaded to github by me.

Same day, about history and authorship:

> Also — all the commits need to be translated into English, and those that don't translate (like "w" or a single Cyrillic letter) —
> write "wip". The commit times must be kept.

> put the authorship log@kinetren.com everywhere and remove claude co-author from the commits (and messages) —
> these are junk lines
> going forward, agents are forbidden to add co-authorship

> I will push it myself

**Decisions taken:**
- Complete README + `docs/guide/` + `CONTRIBUTING.md` in English; screenshots from the owner's
  footage via a reproducible pipeline (`kt-cli showcase`, `readme-shots.py`), the footage is
  never committed. Change `openspec/changes/readme-showcase`.
- **Standing rules:** commit messages and all new text in English (existing Russian documents
  are not translated); author `log <log@kinetren.com>`; **no `Co-Authored-By`/AI attribution
  lines, ever** — root `CLAUDE.md` and `openspec/config.yaml`.
- History rewritten twice (2026-10-21): messages translated (meaningless ones → `wip`), dates
  and trees unchanged; then every commit and tag re-authored as log, co-author trailers removed.
  Hash references in docs remapped. The owner does the push.

**Second round, same day (after the first README):**

> Corrections:
> 1. kinetren, not kinotren
> 2. the first languages: English, Spanish, Simplified Chinese
> 3. and do you think people's sources will be substituted into your showcase? Something neutral should be included there
> 4. the readme has no program icon, and it's for github, after all. and
> 5.  there are no badges of any kind; the project will be at github.com/kinetren/kinetren
> 6. after all, write that this is a free editor with open source code..
> 7. let's think and find free videos on the internet for samples (not bigbugbunny — that is very old and overused, and in normal quality)

**Answers (AskUserQuestion, 2026-10-21):** name — *full rename* to Kinetren (including the
`.kt` root marker and internal names; old files then come in through the import door — a
separate change); open source — *"free, source-available"* (the license is
non-commercial, so the README does not claim "open source" in the OSI sense); languages —
*list order*: English, Spanish, Simplified Chinese first, then Russian and Arabic. Free
footage is being researched (Big Buck Bunny rejected as old and overused).
Footage answer: the recommended mix from Wikimedia Commons (Blender open movies Charge,
Spring, Coffee Run; 4K drone Lake Bled and Charlotte By Night; NASA vertical and square
clips with their soundtracks removed; music "Driving to the Night" and "Far Away"), about
0.9 GB. Download — *"Write script, I run it"*: `src/scripts/fetch-showcase-media.py` is
committed, and the owner runs the download and the re-shoot.

**Third round (2026-10-22):**

> 1. the logo at the top of readme.md is huge — will it look normal on github?
> 2. you also somehow hid the nested reels.. I think this needs to be shown right away in the timeline screenshot
> 3. you supposedly downloaded the footage — but the pictures are still with the old samples; old samples must not be there. The readme doesn't need junk about the footage, it can be put into the footage folder. Writing that this is a port from c++/qt is also not needed. Also don't write about the crates — there is a lot of junk in your readme; the readme is about the product, how to run it, not about technical junk

Rule taken from this: **the README is about the product and how to run it.** No
architecture, crates, invariants, porting history or footage credits in it (credits go to
`CREDITS.txt` next to the downloaded footage). Developer material lives in
`CONTRIBUTING.md`. The footage had not been downloaded (the owner had chosen to run the
script); asked again before downloading.

## Pre-release: GitHub CI and Docker build (2026-10-22)

> we are doing pre-release preparation for github, and I need the following:
>
> 1. automatic build in github ci (it's a free project there — there is a limit on space, we store only releases for now, if I understand correctly)
> 2. a build in docker (so people don't suffer as in the readme — change that there and make simple make commands)

**Answers (AskUserQuestion, 2026-10-22):** the repository is *public* when CI starts;
on every push and pull request — *build + tests* (workspace tests, clippy, guards; no GPU,
no Xvfb; nothing uploaded but a short-lived failure log); a release is made by *pushing a
`v*` tag* (tarball + sha256 attached to a GitHub Release); make commands — *build in
Docker, run on the host* (`make`, `make install`, `make run`, `make test`, `make clean`).
Change `openspec/changes/github-ci-docker-build`.

> kinetren/kinetren is final
> also — when building and distributing, it must be called kinetren, not kt-app as it is now

**Decisions:** the final repository is `github.com/kinetren/kinetren` (the current remote
`kvazi-team/kinetren` is not). The GUI binary is called `kinetren` in the build output, the
tarball, `~/.local/bin`, the `.desktop` file and `make run`; the cargo package stays `kt-app`
internally, and `kt-cli` keeps its name (assumption, easy to change).

**Fourth round (2026-10-22):**

> 1. the first photo in the readme — the media bin is very narrow, it needs at least two rows (and further in the readme too, insert a normal photo, not a narrow one), and it's even better to show it in free layout,
>
>  and also show the sound monitor in action on this photo.
>
> 2. the photo of the timeline — the nested reels are at the beginning and it's impossible to tell that it's a reel; there should even be shading on the sides; you gave a bad example: the reel should be shown expanded in the middle in the second photo, and in the first photo the timeline should not expand the reel
> 3. show the ctrl+f search
> 4. publishing of reel parameters
> 5. your morpher is shown meagerly, it's not visible what it does and the source replacement; in general you need to show what it looks like before and after it

Taken as: the hero shows a wide media bin in free layout and the meters during playback,
and no expanded reel; the timeline shot shows a reel expanded in the MIDDLE of the edit,
with its veiled tails on both sides; new shots for Ctrl+F search and for published reel
parameters; the morpher shot shows what it does — a before/after of the same frame,
including a source swap.

## Telemetry: launches, time in the program, clean close (2026-10-22)

> need to hook up posthog — the free version. or any other free telemetry service, to collect primary information about the number of program launches, the time spent in it, and its successful closing

**Answers (AskUserQuestion, 2026-10-22):** consent — *on by default, can be switched off*
(opt-out); abnormal sessions — *only the event at exit* (no crash marker on disk: a start
without a close is the signal). Service: the owner asked

> can't we determine us / eu by timezone? or do I have to create two accounts?

— answered: two clouds mean two projects, two keys and the data split in half, so one region,
EU. Then:

> are you able to write your own super simple client for posthog, or tear out the most important part of their client, so there is no dangerous uncontrolled dependency?
> and oh, I really don't care about tls — do it the simplest way; I thought the posthog library would be heavy

**Decisions:** our own capture client (one POST of JSON), no PostHog SDK; TLS through
`ureq` + `rustls` as the simplest. The owner supplied the project key and confirmed the
project is in EU. After the Audacity finding of F-COL-046:

> regarding audacity — we should then list in the notice exactly what we transmit, and a reassuring line that we do NOT transmit

— the first-start notice and the menu checkbox's hover text list what IS sent and what is
NOT sent. Change `openspec/changes/telemetry-posthog`.

## Windows build (2026-10-22)

> we need a build for windows, it touches many things.. the same search and work with libavcodec.dll? how to find it..

**Answers (AskUserQuestion, 2026-10-22):** FFmpeg on Windows —

> item 1. + make it possible to download the shared build from https://www.gyan.dev/ffmpeg/builds/#release-builds

that is, the rule "no FFmpeg build of our own" stays: the program *finds* the user's installed
shared FFmpeg (the folder he chose, `KINETREN_FFMPEG_DIR`, `ffmpeg\` next to the exe, `PATH`
including winget's links, scoop/winget/`C:\ffmpeg`) and, on a click, downloads gyan.dev's shared
release build; no DLLs in our zip. Toolchain — *cross-build in Docker (mingw)*. Delivery — *a
portable zip* (no installer, no signing yet).

**Found before the plan:** gyan.dev's "latest release" link already serves FFmpeg 9.0.2, which
the loader (majors 6/7/8) does not support. The download is therefore pinned to
`ffmpeg-8.1.2-full_build-shared.7z` with a SHA-256 committed in our source; FFmpeg 9 is a
separate change (Linux needs it too). Change `openspec/changes/windows-build`.

**First push (2026-10-25):**

> I pushed — some tests died there

The first real CI run: Linux 1128/1129 (t809 needs `xdpyinfo`, absent on the runner),
Windows 1086/1126. The Windows failures were real, not only the runner: FXC (wgpu's D3D12
shader compiler) rejects writes through a dynamic vector index (greenscreen, and the same
pattern in noise/composite/sharpen), and the error escaped the validation scope as an
INTERNAL one — the program would have crashed instead of falling back to the CPU; mingw's
`round` returns 1 for `round(0.49999999999999994)`, which every `.round()` of the CPU
reference used. The rest were test paths baked in at compile time (kt-cli's binary) and
Linux-only test paths.

## FFmpeg 9 (2026-10-26)

> ffmpeg 9 (windows / linux)

The change deferred by windows-build: FFmpeg 9.0 "Lei" (avutil 61, swresample 7, swscale 10,
avcodec 63, avformat 63; the current point release is 9.0.2) is refused on both OSes as "not
supported yet", and gyan.dev's "latest" already serves it.

**Answers (AskUserQuestion, 2026-10-26):** the Windows Download button — *9.0.2, CI tests 9
only* (the pin moves from 8.1.2 to 9.0.2, the windows-latest job runs the test archive against
9.0.2; 8.x on Windows stays covered by the pure search-plan tests); the Linux proof — *build
9.0.2 in Docker*: a test-only stage of the build container builds shared FFmpeg 9.0.2 from the
official source tarball (SHA-256 pinned), and the FFmpeg tests run against it — never shipped.
Change `openspec/changes/ffmpeg-9`.

## Shortcuts reference (2026-10-26)

A brief written in the site repository (kt.9.site, change `remove-landing-keys`), in the
owner's words, translated:

> Rewrite `docs/guide/shortcuts.md` so that it is a complete and tidy reference of keyboard
> shortcuts and mouse gestures. The site shows `docs/guide/*.md` as the `/docs/` pages. I
> removed the "Keyboard" section from the landing page: a reference does not belong there.
> Now `shortcuts.md` is the only key reference. Right now it is poorer and uglier than the
> landing was: the keys are written in bold text, there are no groups for the region and the
> view, and some shortcuts are not mentioned at all.

What he asked for: the list comes from the CODE, not from the landing; every landing row is
checked against the code, and where the program behaves differently the documentation says
what the program does, every difference listed in the report; tables grouped by what a person
looks up together, `| Keys | Action |`; keys as keys (`<kbd>`), alternatives with ` / `, holds,
double presses, wheel and clicks in words; a key is described only where it really does that
(the `compositions` rule); the site's Markdown subset (anything else breaks the site's build);
the program's name in lower case — kinetren; English documentation; contradictions in
`timeline.md` / `getting-started.md` fixed; the file name stays.

**Answers (AskUserQuestion, 2026-10-26):** a defect found while mapping the keys — `Alt+S`
(solo) also splits the event under the playhead — is *fixed here*, test first; the site's
Markdown subset is *guarded* by a `markup` class of `readme-check.py`. Change
`openspec/changes/guide-shortcuts-reference`.

## Split menu label (2026-10-26)

Found by `guide-shortcuts-reference` while checking every key against the code: Edit →
"Split / cut region" (chord `S`) promises a cut-out of the region that neither the item nor `S`
ever does — both only split at the playhead (item 5: "S: always a cut at the cursor"); the
cut-out is "Cut out the region interior" in the track's empty-space menu.

> label lies

**Answer (AskUserQuestion, 2026-10-26):** *Rename to "Split at playhead"* — `S` and the action
stay as they are. Change `openspec/changes/split-menu-label`. A second answer the same day:
the toolbar scissors button's hint says the same "Split / cut region (S)" over the same
action — *yes, same fix* ("Split at playhead (S)"; a hint may carry its key, a menu may not).

**Wine (2026-10-26):**

> I run the windows version through wine and there the whole interface is dark, while some parts are white, and also — the top titlebar disappeared and was replaced by the system bar, i.e. the context menus are gone, in effect..

Asked what wine should get (our row through a write into wine's registry / the system frame /
documentation only):

> the main thing for me: 1. that undecorated works correctly in real windows, that there is no windows bar there, and that in wine it at least somehow works.

Reproduced in the windows image (wine 11 + Xvfb + openbox): by default a system frame covers our
22 px title row and cuts the status bar, egui's light style paints the standard parts white;
with `KT_DECORATIONS=1` the menus are visible below the system bar; with wine's `Decorated=N`
the look is exactly Linux's. Decisions: the theme fixed to dark on every OS; under wine the
system frame (nothing written); real Windows stays borderless. Change
`openspec/changes/wine-frame-and-dark-theme`, done in the branch `wine-frame-and-dark-theme`
(the owner asked for a separate branch).

## Menu labels without keys (2026-10-26)

The follow-up proposed at the end of `split-menu-label`: five `[tl.menu]` labels carry their key
in the translated text ("Copy (Ctrl+C)", "Paste (Ctrl+V)", "Delete morpher (Del)", "Delete clips
in the region (Del)", "Region over this event (Shift+double click)").

> Do this task here: "Menu labels: move keys out of the text"

**Answer (AskUserQuestion, 2026-10-26):** the gesture of "Region over this event" goes to the
*chord column, translated* — like the reel items' "Enter, 2×click". Found while checking the
handlers: the context-menu Paste pastes at the click, not at the playhead, so it gets no
`Ctrl+V`; Delete morpher deletes one block while `Del` deletes the whole selection, so `Del` is
shown only when that morpher is alone. Change `openspec/changes/menu-chords-out-of-text`.

## What the user repeated more than once

| Topic | Rounds | Resulting rule |
|---|---|---|
| The width of the media bin is limited | 3 (r.9, r.12, r.13) | the limit stands with a comment "set on a complaint — do not squeeze"; rule "a class of problems ≠ squeeze everything" |
| Resize/edges steal clicks | 2 (r.12, r.13) | `no_resize_rects` + its own separator strip; t101 |
| Track labels | 2 (r.12, r.13) | the label as a line below the clip name; t102 |
| The look of the meters | 2 (r.13 twice) | the v1 layer order + strengthened alphas; a brightness measurement by pixels |
| "Check what I launch" | 2 (r.12, r.13) | `finish.sh` + the version in the report; the check in the EXPANDED window |
| Rules about UI convenience | 5+ (r.8–r.12) | a checklist of 15 items in apps/kt-app/CLAUDE.md |
| **Dead ends: you can create it, you cannot control it** | **7 (r.33–r.40)** | the checklist did not work — the rules were written down and broken. The R.38 tests did not work either: they checked the REACHABILITY of what was made, not its consequence. R.40: a test asserts the observable consequence AND probes from the point where the person acts; for each item — a live scenario |
| **"You did not check"** | **3 (r.12, r.13, r.40)** | the harness and the model prove nothing: `finish.sh`, a live scenario for each item, a check of the consequence |
| **One claim sent twice (R.43)** | **2** | the first attempt fixed the symptoms: there was nothing to look at in the model. `kt-cli probe` appeared; for the complaint "sound from another event" it is run FIRST |
