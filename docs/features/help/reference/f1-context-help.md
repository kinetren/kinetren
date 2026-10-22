---
id: F-HLP-022
title: Context help on F1: a page about what you are looking at
status: researched
priority: C
cost: M
category: help/reference
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, jetbrains-2026.2]
we_have: no
crates: [kt-app]
invariants: [12]
ui_rules: [10, 13, 14, 16, 17, 18, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-025, F-HLP-032, F-HLP-033, F-UI-002]
checked: 2026-09-07
---

# F-HLP-022 — Context help on F1: a page about what you are looking at

## Essence

The cursor sits on the fade handle, the person presses F1 — and a page about fades opens, not the manual's table of contents. The program picks the topic: the user is already looking at the right place and does not have to name it in words. If there is no page about that place, the program says so — aloud, with the name of the missing topic — instead of offering "something similar".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Application Help (F1), What's this help (Shift+F1) | the only one in the core with real context binding. The output help catalog carries `Data/Alias.xml`: **675 entries** `IDH_ADJUST_EVENT_LENGTHS` → `topics/adjusting_eventslength.htm` + numeric `ResolvedId="2796"`. The numbers live in the code, the topic names live with the author of the help; this is the classic Windows scheme "map file + alias file" |
| Adobe Premiere | 26 | F1 → Help | the key is taken, there is no context: F1 is the accelerator of the Help menu item, it opens the help home page in a browser. In a neighboring product of the same company this is visible right in the key editor: users are told "you will notice that the F1 is assigned to Help" (Illustrator). **[NOT VERIFIED]** for 26.x live: `helpx.adobe.com` does not open from this environment |
| DaVinci Resolve | 21 | Help → DaVinci Resolve Reference Manual | offline and without context: the item opens a **local PDF of 4444 pages** placed by the installer. The link "interface → help" is moved to the end of the PDF itself by the "Menu Description" appendix, where each menu item has its PAGE NUMBER printed ("Bus Format – Page 3880"). Bare F1 is not taken: across the whole manual only `Option-F1…F8` occur — track toggles |
| Final Cut Pro | 12 | none | neither F1 nor context help. In the guide (2000+ pages) the string "F1" does not occur EVEN ONCE, and the Help menu is mentioned twice: `Help > Download Additional Content` and "the Mac User Guide, available from the Help menu when the Finder is active". Help is an online guide plus a PDF on the site, updated 30.06.2026, the same day as 12.3 |
| After Effects | 26 | F1 → Help | as with Premiere: one key for the whole product, leads to the browser, to the general help; there is no binding to a panel, effect or parameter. **[NOT VERIFIED]**, secondary source |

Summary for the core: **context help exists in exactly one of the five**, and it is an inheritance of the Windows help of the nineties (numeric context id, "What's This?" on Shift+F1), not a decision of today. Two others open the table of contents in the browser, two do nothing. In the NLE world help is detached from the interface: it knows everything about the program, the program knows nothing about it except one address for the whole product.

**Outside the core.** Without these two rows the mechanism cannot be described: Blender is its origin, and only there it is visible that the link is BUILT FROM THE CODE, not maintained as a table; JetBrains shows the second important piece — the documentation root is separated from the identifier and can be configured, down to a local copy.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | F1 / RMB → "Online Manual"; "Online Python Reference" — under Developer Extras | the id of a widget is derived from its BINDING: `Struct.property` or the python idname of the operator. The manual — via a template table generated from the built help; the Python reference — with no table at all, the URL is assembled from the id. The prefix carries the version and the language; if there is no entry — a warning with the text "No reference available" |
| IntelliJ IDEA | 2026.2 | Shift+F1 "External Documentation" | "To view documentation for a symbol at the caret in a browser, press Shift+F1". The address = the documentation root (configurable per library/SDK, a downloaded copy is fine) + the identifier of the symbol under the caret. Next to it a second, non-browser level: Ctrl+Q, documentation inside the IDE |

## The trick behind it

The payload of this mechanism is **THE ADDRESS OF SOMEONE ELSE'S PLACE**. A status-bar line carries text, and however long it is made, it cannot carry a destination: a link has a version, a language, a page and a point within the page, while a line has only words. For us this payload lands in `site/dist/<lang>/index.html#<anchor>`, and all the complexity moves there too: the address must survive the renaming of a page, a change of interface language and the release of a new program version.

**1. The link is built from the widget's binding, not from a "widget → page" table.** In Blender this is eleven lines (`interface_utils.cc:780`): if the button has an RNA pointer, the id is `Struct.property`; if it is an operator button, the id is its python name; if neither — the function returns nothing, and the "Online Manual" item does NOT APPEAR in the menu (`interface_context_menu.cc:1285`). A consequence that is absent from every list of features: nobody sets the topic identifier by hand, so it cannot be forgotten on a new button. The same call, but "for the active button" (`button_online_manual_id_from_active`), is what fires on F1 — the key is bound in the default layout by one line (`wm.doc_view_manual_ui_context`, `blender_default.py:772`). Subtler still: before issuing the id the code climbs the base RNA classes until the property stops matching the parent, and addresses the PLACE where the property is declared — otherwise one and the same topic would have as many addresses as there are subclasses.

**2. The "id → page" table is not written, it is extracted from the built help.** `rna_manual_reference_updater.py` downloads `objects.inv` — the standard Sphinx cross-reference index — from `docs.blender.org/manual/en/dev/` and prints `_rna_manual_reference.py`: **about 4.6 thousand pairs** of the form
`("bpy.types.fluiddomainsettings.sndparticle_potential_max_wavecrest*",
"physics/fluid/type/domain/liquid/particles.html#bpy-types-fluiddomainsettings-…")`.
Three details visible only in the generator's source. First: the pairs are sorted by identifier length in descending order, and the pattern ends with `*`, so a family of properties is covered by one entry, and a longer (more specific) id always matches before a shorter general one. Second: the anchor within the page is also made from the identifier (`#bpy-types-…`), so moving a paragraph within the page does not break the link, and moving a page is fixed by the next generation — and nothing in the code changes. Third: names that cannot be put into a URL are silently discarded by the generator, which checks each one with the regex `[a-zA-Z0-9._*]+` — that is, the requirement "the identifier must be machine-readable" is written into the pipeline itself. The formulation for which the id layer exists at all, in the words of the help vendor: "The key to this relationship is the sharing of IDs that connect the specific pieces of documentation with the software user interface" (MadCap, CSH Tutorial, 2026).

**3. A miss does not stay silent, and this is a working list of holes.** If no pattern is found, Blender reports `{'WARNING'}`: "No reference available '<id>', update info in '_rna_manual_reference.py' or callback to bpy.utils.manual_map()" — it names both the identifier and the file where it should be added. There is nothing to compare with: at Resolve the link between a menu item and the help is a FILE NAME in a folder, and nobody checks a substitution ("put a full 16.2 manual in the folder, rename it — and it will open from the menu"); in beta 17 the menu opened a 200-page "What's New" instead of the guide; this was found out on a forum, not in the program.
## Pros

- Topic search moves from the person to the program. The user is already looking at the right place; the only thing required of them is not to look away. This is exactly what neither menus nor search can do: there one must first name the sought item in a word (the boundary with F-HLP-015 — the palette requires a word, F1 does not).
- An identifier derived from the widget's binding cannot lag behind the code: either the element has an address, or it has no help item. A manual list "widget → page" always lags — this is visible even on our only help line (`apps/kt-cli/src/main.rs:354`).
- A table extracted from `objects.inv` survives renaming and moving a page; an anchor made from the id survives reordering of paragraphs.
- The prefix (version + language) is separated from the suffix (topic): the version is resolved once for the whole program, the language once per user, and neither is smeared across calls.
- The extension point sits exactly where it is needed: `register_manual_map` replaces the prefix and the table as a whole, so a local copy of the help is not a separate feature but a different prefix. JetBrains does the same by configuring the documentation root to a downloaded set.
- A miss is a message, not silence (rule 16), and at the same time a free report of "which topic is missing from the help", collected by real users.
- Cheap where the identifier already exists for another reason. For us it does exist: parameter names live in the project file.

## Cons

- **The link leads outward — and takes focus away for good.** Rule 14 requires that everything which takes focus gives it back; the browser gives nothing back, and it opens on top of the window at the very moment when the context on screen was the value. That is exactly why JetBrains keeps two levels: a short answer inside the program and an external one in the browser.
- **Without the internet nothing remains.** Blender, Premiere, After Effects and JetBrains open a browser; the page either exists or does not — offline it is simply absent, and a local copy is reached only through an extension (`register_manual_map`) or by manual setting of the root. The only offline answer in the core is Resolve, and its price is 187 MB on disk (v21) plus the loss of context: the PDF opens on the first page, and "where to look" is printed as a page number in an appendix to the PDF itself.
- **The URL is versioned by whoever happens to do it.** Blender — the program itself (`bpy.app.version[:2]` in the prefix), so an old build goes to the old manual. Vegas does not version at all: the help address has no version, and the header of its pages is built from the variable `VersionNumber` with the value **21** — for the 2026 product. Adobe serves "the latest". Resolve versions by a file on disk, and in beta the file lagged behind the program.
- **A fine id does not give a fine answer.** In Vegas 675 context identifiers lead to 265 pages: 35 different controls of the Trimmer open one and the same `window_trimmer.htm`. Without anchors the promise "the page about what you are looking at" turns into "the page about the window you are in".
- **The mechanism is blind where there is no binding.** No RNA property and no operator — no item either; a gesture, an empty space, a zone without a widget remain without an address. Our own dead ends of R.38 (the length of a morpher event — a drag of the edge) fall exactly into this hole.
- **The language of the address is not the language of the interface.** The manual is translated worse than the program: Blender keeps a whitelist of languages right in the code, half of the lines in it are commented out, and `manual_language_code(default="en")` silently leads to English. That is, F1 sometimes opens a page in a foreign language and does not warn about it.
- **The list of identifiers has to be shared by two people.** The vendor of the help says this in plain text: "Both you and the software developer need access to this file". For us the author of the help and the developer are one person, but there are already two lists (see F-HLP-033), and this is worse than two people and one list.

## What users say

On the fact that the help is tied to a version, and the tie is to a file name (a thread about the "Reference Manual" item in the Help menu):

> What I get from the HELP menu from within the app is a 200 page long pdf
> document; I see some of you mentioning a 2000 page document online?
> — thread author, Blackmagic Forum, 20.02.2021,
> https://forum.blackmagicdesign.com/viewtopic.php?p=729016

There — a complete description of the whole "interface → help" link mechanism in one of the five core editors:

> You could put a full 16.2 manual in the folder, re-name it and it would open
> via the menu. It does seem an odd way of going about it.
> — John Holt, Blackmagic Forum, 20.02.2021,
> https://forum.blackmagicdesign.com/viewtopic.php?p=729073

The reverse side: a key taken by help that still opens the wrong thing is perceived as a stolen key:

> I'd like to use the F1 key for something other than the Discover panel. Can
> I do that?
> — rcweburg, Adobe Support Community, 02.09.2024,
> https://community.adobe.com/questions-652/f1-811848

## How it fits Kinotrain

### What is already there

**The key is free, and not only that one.** `Key::F1` is not mentioned a single time; moreover, there is not a single function key anywhere in `src/` — `handle_keys` (`src/apps/kt-app/src/app.rs:3879`) knows letters, arrows, Space, Del, Backspace and Esc, and nothing from the F1–F12 row. There are three menus: File, Edit, View (`src/apps/kt-app/src/toolbar.rs:246,279,306`), Help is not among them. There is no "?" icon either: in `src/apps/kt-app/src/icons.rs` there are 39 constants, and the closest in meaning are `WARN` and `BELL`.

**Topic identifiers already exist for us — three generations, of different strength.**

1. `ParamSpec.name` (`src/libs/kt-model/src/project.rs:2424`): the snake_case name of the field, "the same as the key in the file and in `ParamCurve.param`". There are 78 such names for FX and transitions plus 27 for the camera — `Blur.radius_y`, `Glow.threshold`, `SlowedReverb.predelay_ms`. This is literally Blender's `Struct.property`, and for us it is STRONGER than in Blender: renaming breaks reading of old projects, that is, the address is protected by format compatibility.
2. `FxEntry.name` (`src/apps/kt-app/src/fx_registry.rs:15`) — the decision "the name of an effect is NOT translated" has already been taken and recorded in a comment. The trap is in plain view: `Pan 3D` and `Slowed+Reverb` contain a space and a plus sign, and to put them into an address they must be normalized, and normalization is a second name (Blender has no such trouble: its identifiers are machine-readable from birth).
3. Catalog keys (`src/apps/kt-app/i18n/en.toml`, 522 values in 34 sections): every label already has a stable name that depends neither on the text nor on the language — `tl.hint.area`, `trn.kind.*`, `mrph.*`.

**The resolver of "what is under the cursor" is also already written and runs every frame.** `src/apps/kt-app/src/timeline.rs:2181-2256` parses the pointer position into a ZONE and picks a key from the section `[tl.hint]` — there are 21 of them (`area`, `clip`, `morpher`, `morph_edge`, `camera`, `fade_handle`, `level`, `trim_start`, `region_edge`, `ruler`…). This is literally `button_online_manual_id_from_active`, already implemented. One difference, and a decisive one: the result is immediately fed into `tr()`, and `KtApp::hint` (`app.rs:1505`) puts READY PROSE into `hover_hint: Option<String>` (`app.rs:269`). **The identifier lives until the end of the line and dies** — 45 calls of `hint`/`hint_into` across the crate, and not one of them preserves WHAT it was.

**The exit to the browser is already compiled into the binary.** `webbrowser 1.2.4` arrives transitively through `egui-winit 0.36.1` (`src/Cargo.lock`), and the strings `xdg-open`, `x-www-browser`, `BROWSER`, `only http urls supported` already lie in `src/target/debug/kt-app`. At the same time `egui::Context::open_url` and the `Hyperlink` widget are called nowhere in `src/`. The mechanism exists, the decision does not.

**The site has anchors, but not the ones that are needed.** `site/content/structure.toml` declares 12 sections, `site/build.py:439` prints `<section id=…>`, and in `site/dist/en/index.html` there are 11 of them: `why`, `edit`, `playback`, `effects`, `morpher`, `bin`, `export`, `care`, `keys`, `roadmap`, `get`. The twelfth, `hero`, is declared, but does NOT get an anchor — that is, a declared id already today is not equal to an address, and nobody notices it. Effects have no anchors at all: the `id` of an element of `[fx].items` (`pan`, `green`, `grid`…) is used exactly as the key `fxdesc.<id>` (`site/build.py:361-364`) and does not reach the HTML.

**The language in the address is already laid out:** `dist/<lang>/index.html`, `langs = ["en", "ru", "es", "zh-Hans", "ar"]`, `translated_min = 0.9` — an under-translated language is built, but gets `noindex`. This is our `manual_language_code`; what is missing is a fallback rule for when the page does not exist in the user's language.

**The only help line in the product today — the `usage` of kt-cli — is already wrong.** `src/apps/kt-cli/src/main.rs:354` prints "available: version, new, roundtrip, render, export", while `demo` (`:184`) and `convert` (`:155`) exist. The manual list has diverged from the code at the scale of seven commands; this is the price of "a correspondence table kept by hand", in our own execution and at the smallest possible scale.

### What is missing

Pages (the landing page is not a manual, and its 11 anchors are sections of advertising; the content is set up by F-HLP-025 and F-HLP-032). The rule "a catalog key and a parameter name are the address of a topic", and one function `id → URL` that knows the version, the language, the page and the anchor. A human answer "there is no page about this" (rule 16). And the decision about the exit to the browser — that is not a matter for reconnaissance.

### What gets in the way

**1. A link to an external site is an EXIT TO THE BROWSER, a separate decision.** In `src/apps/kt-app` and in `src/packaging/kinotrain.desktop` there is not a single address: the only occurrences of `http` in the whole crate and package are the XML namespace in `kinotrain.svg` and `kinotrain-mime.xml`. There is no string `kinotrain.app` in `src/` at all; we also have no appstream metadata where an application declares `url type="help"`. The program does not know that it has a site — and this state has to be changed consciously: the browser takes focus and does not return it (rule 14), an offline editor gets for the first time a reason to go to the network, and part of the interface starts to depend on a file in another folder of the repository.

**2. The version.** Our address would be `kinotrain.app/<lang>/#<topic>` — without a version, while the program is versioned by commit (`kt-version`, `vYY.MM.DD-hash`). What this leads to is shown by Vegas: an unversioned address and a help whose header still says 21. Blender's answer (the version from the program itself in the prefix) requires that the site keep versions, while our `dist/` is one and is overwritten in place.

**3. There are already two sets of identifiers, and they have already diverged.** `[fxdesc]` describes `cammotion`, which is not in the picker (`video_fx_templates` — `src/apps/kt-app/src/fx_editor.rs:528`; the effect was removed in R.34 and lives only as a conversion target), while `Pan 3D` has no description. This is the address of F-HLP-033, and the only sensible comparison of sets is there; here we record only what cannot be seen from that note: the divergence of names is half the trouble, the other half is that the page has no ANCHOR for the name to point to.

**4. The identifier is not an index.** `Key(pub u16)` is the position in `en.toml` ("the index in it is the Key", `src/apps/kt-app/build.rs:64`), and inserting a key into the middle shifts all subsequent ones. Therefore only the dotted path, not a number, is suitable for the address of a topic, and no serialization of `Key` outward can exist. Vegas stores both forms (`IDH_*` and `ResolvedId`) for exactly this reason.

**5. Granularity.** There are two dozen timeline zones, one hundred and five parameters, and if pages are written there will be a dozen. The lesson of Vegas (675 → 265) says that without anchors inside a page F1 opens "the page in general", and the promise is not kept; our anchors so far exist only at the level of landing-page sections.

**6. Invariant 12 — in plus, not in minus.** `hint()` does `text.to_string()` on every hover frame; if the identifier (`Copy`, two bytes) travels along the hover path, and the prose is fetched by the status bar, the allocation from the draw path disappears. This is not a cost of the feature but a return; there is nothing to design here, but it is good to know.

### Which invariants it touches

Engine invariants — **none**: neither the model, nor the project format, nor rendering, nor determinism, nor the audio path. Invariant **12** is touched in the direction of improvement (see above). Rules of the UI checklist: **14** (main point — focus that went into the browser does not come back), **16** (no page is an error with the topic's name, not silence), **17** (F1 and the context-menu item must lead to the same place), **10** (help is not a second hint: the ban on pop-up hints from item 14 of round 8 and guard t83 are not touched; F1 is not hovering but a press), **13** (no new strip and no new panel is created), **18** (a menu item, if it appears, says WHAT and WHERE), **20** (nothing grows on its own: the list of topics does not replenish itself, it is derived from the names that already exist).

## Estimate

| What | Answer |
|---|---|
| Volume | M — 2 stages: the identifier rule + anchors; but without pages the output is zero, and the pages are not this note |
| Where the main work is | `kt-app`: the hover path carries `Key`, not `String` (`app.rs:1505`, `timeline.rs:2181`), one function `id → URL`, the F1 handler; `site/build.py` — topic anchors and the contract on them; `kt-model` is not touched at all |
| Project format | does not touch it; parameter names are already in format and already stable |
| Tests that will have to be written | a clause in `site/build.py --check` (every declared topic anchor is present in `dist` and does not vanish silently); `reach_tests`: F1 over the timeline zone names the topic or honestly refuses; "F1 and the menu item lead to one address" (rule 17); t100/t125/t128 — automatically for new lines |
| Depends on | F-HLP-025 (the text of descriptions), F-HLP-032 (the pipeline and content), F-HLP-033 (the price of a line and the check of name sets), F-UI-002 (the command registry) |
| Breaks compatibility | no |

A frame, not a specification: the address scheme, the set of topics and the order of stages are the subject of the round.

## Verdict

F1 itself is taken last, and its foundation first. **What this adds ON TOP of the command registry (F-UI-002): nothing essential — it is the same question asked about another class of objects.** The registry asks "how is an action stably called", the help asks "how is the thing a person is looking at stably called", and answering them separately means keeping two lists of names for the same things. The difference is only that part of the answer is already written and lies in a project file: 105 parameter names that cannot be renamed without breaking the reading of old projects.

Reconnaissance at the same time cancels the temptation: context help does not exist at anyone of the five except Vegas, and at Vegas it is a Windows mechanism thirty years old, where 675 identifiers lead to 265 pages. The real find is not in the key, but in the fact that the link MUST BE BUILT FROM THE CODE and must report a miss aloud.

CHECK: site/build.py — the clause "a declared topic is present in `dist` as `id=` and does not vanish silently". Today it would fail on the very first example: `hero` is declared as a section in `site/content/structure.toml`, and in `site/dist/*/index.html` it gets no anchor — of the twelve declared addresses eleven exist, and there is nothing to notice this with except one's eyes.

DEFERRED: until F-HLP-025 — while there are no pages, F1 has nowhere to point, and the whole feature reduces to one rule that has to be accepted BEFORE the texts are written: the address of a topic is the dotted key of the catalog or `<FxName>.<param>`, not the page title and not a number.

The exit to the browser as a separate line: today `kt-app` and `src/packaging/kinotrain.desktop` contain not a single address, and the first one to appear will change the class of the program. This is a decision of the owner, not a consequence of the help.

## Sources

- https://raw.githubusercontent.com/blender/blender/main/source/blender/editors/interface/interface_utils.cc (2026-09-07) — `button_online_manual_id`: the id from the RNA binding or from the python name of the operator, otherwise `nullopt`
- https://raw.githubusercontent.com/blender/blender/main/source/blender/editors/interface/interface_context_menu.cc (2026-09-07) — the "Online Manual" and "Online Python Reference" items, the second only under Developer Extras
- https://raw.githubusercontent.com/blender/blender/main/scripts/startup/bl_operators/wm.py (2026-09-07) — `WM_OT_doc_view_manual`, the message "No reference available", the assembly of the Python reference URL directly from the identifier
- https://raw.githubusercontent.com/blender/blender/main/scripts/modules/bpy/utils/__init__.py (2026-09-07) — `manual_map`, `register_manual_map`, the whitelist of manual languages, `manual_language_code(default="en")`
- https://raw.githubusercontent.com/blender/blender/main/tools/utils_doc/rna_manual_reference_updater.py (2026-09-07) — generation of the table from `objects.inv`, sorting by id length, checking the name with a regex
- https://raw.githubusercontent.com/blender/blender/main/scripts/modules/_rna_manual_reference.py (2026-09-07) — the table itself, ~4.6 thousand pairs, a prefix with version and language
- https://raw.githubusercontent.com/blender/blender/main/scripts/presets/keyconfig/keymap_data/blender_default.py (2026-09-07) — `("wm.doc_view_manual_ui_context", {"type": 'F1', "value": 'PRESS'})`; in the legacy layout F1 opens a file
- https://help.magix-hub.com/video/vegas/en/content/topics/shortcutkeys.htm (2026-09-07) — "Application Help — F1", "What's this help — Shift+F1"
- https://help.magix-hub.com/video/vegas/en/Data/Alias.xml (2026-09-07) — 675 records `IDH_*` → topic + `ResolvedId`, 265 unique pages
- https://help.magix-hub.com/video/vegas/en/content/topics/intro.htm (2026-09-07) — the variable `VersionNumber` with the value 21 in the header of the help for a 2026 product
- https://docs.madcapsoftware.com/flare2026/Flare-CSH-Tutorial.pdf (2026-09-07) — the structure of header/alias files, textual and numeric id, the call form `Default.htm#cshid=…`
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf (2026-09-07) — 4444 pages; the Help menu (p. 4443) and the "Menu Description" appendix with page numbers for menu items; `Option-F1…F8` — the only mentions of F1
- https://help.apple.com/pdf/final-cut-pro/en_US/final-cut-pro-user-guide.pdf (2026-09-07, file dated 30.06.2026) — not a single mention of F1; the Help menu only for "Download Additional Content"
- https://forum.blackmagicdesign.com/viewtopic.php?p=729016 and `?p=729073` (20.02.2021, accessed 2026-09-07) — quotes about the "Reference Manual" item and about the file name in the folder; opened through the text proxy `r.jina.ai`
- https://community.adobe.com/questions-652/f1-811848 (02.09.2024, accessed 2026-09-07) — F1 as the accelerator of the Help item in the Adobe key editor and the request to reassign it
- https://www.jetbrains.com/help/idea/viewing-reference-information.html (2026-09-07) — Shift+F1 "External Documentation", the documentation root is configurable, including to a downloaded copy

**What could not be verified.** No program was run live: for software outside the core this is stated in `_editors.md` as standard; for NLEs — an environment without Windows and macOS. `helpx.adobe.com` does not open from this environment either directly or through the text proxy (HTTP/2 framing error), the `WebSearch` budget is exhausted by parallel sessions, therefore the Premiere and After Effects rows rely on an expert's answer in the Adobe community (a related product, Illustrator) and on secondary cheat sheets — they are marked **[NOT VERIFIED]**. The claim "F1 in Vegas 2026 indeed calls help" relies on the official page of hotkeys, and the existence of an alias file on the output catalog of the online help; whether the installer places the same help locally cannot be verified from here. The negative rows ("there is no context help") are proven by the absence in the documentation — the absence cannot be proven by documentation, and this limitation is deliberate.
