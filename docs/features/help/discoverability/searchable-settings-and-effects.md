---
id: F-HLP-019
title: Search over settings and effects: find by what it does
status: candidate
priority: B
cost: M
category: help/discoverability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2, blender-5.2, photoshop-27]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [13, 14, 17, 18]
round: null
point: null
tests: []
depends_on: [F-HLP-015, F-HLP-025]
checked: 2026-09-07
---

# F-HLP-019 — Search over settings and effects: find by what it does

## Essence

The editor remembers WHAT they want to do, and does not remember HOW it was named. Search within the program itself answers "remove the green", "softer", "where is smoothing turned on" — and leads either to an effect or to a setting. Half of this already works for us: the effect picker searches by invisible keywords in five languages. The second half — settings — is not searched at all, and this is a separate task, not an unfinished first one.

## Who has it and how

| Editor | Version | How it is named | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 build 66 | a filter in the Video FX / Plug-In Chooser window | the filter line at the top narrows the list of EFFECTS, and a match in the preset name keeps the whole effect in the list — the filter reaches one level below the name. Effects have "favorites". Settings (Options → Preferences) — tabs without search [NOT VERIFIED] |
| Adobe Premiere | 26.x | a search field in the Effects panel | by the names of effects, transitions and presets at once across all folders of the panel; Preferences has no search, a search line of its own exists only in Keyboard Shortcuts [NOT VERIFIED] |
| DaVinci Resolve | 21 | a magnifier in the Effects Library | by names; Project Settings and Preferences — a tree of pages without a search line [NOT VERIFIED] |
| Final Cut Pro | 12.3 | a search field in the Effects browser | by names, but the main answer to "what does this do" here is not textual: the thumbnail plays the effect on YOUR frame when the mouse is moved over it. There is no search over settings [NOT VERIFIED] |
| After Effects | 26.x | a search field in Effects & Presets | by the names of effects, categories and animation presets; Preferences has no search [NOT VERIFIED] |

The result for the core: search over effects exists for all five, and for all of them — by NAME (VEGAS also by the name of the preset). Search over settings — for nobody. Five rows of the core answer "no" to half of the note, therefore the second table is mandatory.

**Outside the core.** The mechanism of searching settings was not born in video editing: the five NLEs have nothing to describe it with, and the question "does the search look at the DESCRIPTION" has a meaningful answer only where a description exists at all.

| Program | Version | How it is named | How it works |
|---|---|---|---|
| VS Code | 1.136 | Settings editor and the `@` filters | matches the query against the key, the label, the DESCRIPTION, the declared `keywords` and the enum values, and for non-enum settings — against the CURRENT value; the description and the value stand in a fallback rank, the key and the label win. Filters `@modified`, `@tag:`, `@feature:`, `@lang:` |
| IntelliJ IDEA | 2026.2.2 | search in the Settings dialog, Find Action | a field in the upper left corner of the dialog searches options; Find Action puts a setting into the same list as a command [NOT VERIFIED: switching directly from the list of results] |
| Blender | 5.2 LTS | `F3` — search, RMB → "Online Manual" | `F3` searches OPERATORS (commands), not settings; but on any control the right button opens the manual page of exactly this control — the search is replaced by a direct link from the point where the person already stands (see F-HLP-032) |
| Photoshop | 27.x | Search (`Ctrl+F`) | one field for three sources at once: the program interface, help articles, Stock [NOT VERIFIED for 27.x] |

## The trick behind it

**The load of this mechanism is a FOREIGN WORD: the one by which the user would name the function, not the author.** A single status-bar line carries exactly one name, the author's, and carries it only to the one who has already hovered the mouse. Keywords carry two dozen readers' names for one subject and work BEFORE hovering. For us this load already lies in the catalog: `[fxreg.kw]`, 12 keys × 5 languages = 60 values, 287 words; "blur", "blur", "softness", "blur", "soft" — all of these are found by the same Blur, and none of these words is drawn anywhere.

Three details that are not in any list of features.

1. **The searched text is the only text of the program that cannot be prepared for display.** All 522 values of the catalog are passed through `bidi::visual` at BUILD time (`src/apps/kt-app/build.rs:132`), that is, the Arabic table already holds the visual order of words, and at runtime this costs zero. Exactly two of the thirty-four sections are excluded: `NO_BIDI_SECTIONS = ["fxreg.kw.", "lang."]` (`build.rs:37`) — because `filter_entries` compares the value with what the person TYPED, and they type in logical order. Hence the consequence: as soon as a label becomes searchable, it acquires TWO values — the displayed one and the comparable one. Both tables are already generated by us (`TABLES[Ar]` and `AR_LOGICAL`), but the reader function of the second one, `i18n::ar_logical` (`src/apps/kt-app/src/i18n/mod.rs:135`), has NOT A SINGLE call and is held by `#[allow(dead_code)]`; its comment promises that it "is consumed by test 126", while t126 reads the table directly. Search over settings would be the first real consumer of this function.
2. **The reference searches by VALUE, not only by description.** In VS Code the query is matched against the key, the label, the description, `keywords` and — for non-enum settings — the current value, but the description and the value stand in a fallback rank. This is the exact answer to the question "by description or by name": by both, but with DIFFERENT weight, otherwise a ten-word description drowns an exact name match. We have no rank at all today: `filter_entries` is a bare `contains`, while `search_hits` ranks (exact → from the start → substring), and these are two different answers to one question within one program.
3. **Keywords are the only line of the catalog with an infinite width budget.** No one sees them and no one ever will: t128 does not measure them (it measures the menus and the performance panel), no widget shows them. At the same time they pay the full price of translation: a glyph for every non-ASCII character (t100), the font subset, the ceiling of 160 characters, an extra key breaks the build (t125). The catalog's author pays once in words; the user never pays in memory. This is the only feature of the domain that scales with the work of the TRANSLATOR, not of the programmer.

## Pros

- The load moves from the user's memory to the author of the catalog: for "remove the green" to find Greenscreen, the programmer has nothing to do — the translator adds the word to `[fxreg.kw]`.
- Synonyms do not take up a single pixel: invisible text does not take part in the layout, the 420 px limit of the left column (t127) does not touch it, rule 13 is not violated by anything.
- A search that names the PLACE ("Project resolution — the strip under the preview") teaches navigation: next time the person goes there by hand. This is rule 18 of the checklist ("a menu item says WHAT and WHERE"), extended from the menu to the result line; for us it has already been lived through and is fixed by t171.
- The index is a pure function, like `search_hits` and `filter_entries`: zero UI, zero engine, a test without a harness.
- A setting, unlike a command, has a VALUE — therefore it can be "modified" and "default". The VS Code `@modified` grows exactly from here, and this is also the boundary with F-HLP-015: a command palette cannot have such a filter in principle, since there is nothing in a command to compare with the default.

## Cons

- A synonym that is not in the catalog does not exist. If one typed "chroma key" — found it; "green screen" — not found, and one concludes that the function does not exist. A negative search result lies more confidently than the absence of search.
- The price is honest and is multiplied by five: the description of effects is 12 × 5 = 60 values, the index of settings is about as many, and each passes through the 160-character ceiling, the glyphs and the subsets (F-HLP-033).
- Merging our two searches into one means choosing ONE rank and rewriting the second; today they rank differently, and "identical behavior of paired ways" (rule 17) will have to be restored by hand during the merge.
- What is found must also be SHOWN. A jump to a label is the cursor's position; a transition to a setting in a closed render dialog is the opening of a window, that is, taking focus, while rule 14 requires giving it back.
- Search over a description makes every description a contract: rewriting the text means changing the set of queries that find the subject. An effect name has no such property; it is not translated at all.

## What users say

> Currently, typing in the top box for VideoFX lets users quickly filter the effect to just the ones which have a VFX or a preset (within one of those VFX) matching what is typed.
> — RedRob, Boris FX forum (VEGAS Pro), 19.06.2026, https://forum.borisfx.com/t/enhancement-filter-videofx-presets-not-just-the-fx/24206

They praise not the search itself but its DEPTH: the filter reaches one level below the effect, into the presets. Further on in the same message comes a request to finish the job: the list of presets inside the found effect does not narrow, and the query "716" leads to the needed group, but the sought "716 Title" still has to be looked for by eye. That is, even where search over effects is considered solved, there is exactly one complaint: it stops one level earlier than the person needs.

## How it fits Kinotrain

### What already exists

- **The effect picker with invisible keywords** — `src/apps/kt-app/src/fx_registry.rs`. `FxEntry` (line 12) carries `name` (NOT translated: an industry term, searched by it in any locale), `category: Key`, `audio`/`video` and `keywords: Option<Key>` — precisely a catalog key, not text, therefore the registry's `OnceLock` does not depend on the language and is not rebuilt when switching (item 129). `filter_entries` (line 84) filters by kind and searches for a substring in the name, in `tr(kw)` of the current language AND in `tr_en(kw)` — otherwise someone who chose Chinese would not find "blur", although the effect's name is exactly that (items 48, 129; t48 runs this through all five languages).
- **The keyword catalog** — `[fxreg.kw]` in `src/apps/kt-app/i18n/*.toml`: 12 keys × 5 languages, 287 words. They are written by the translator, not by the programmer, and the values are deliberately mixed — "blur, blur in Russian, softness, blur, soft", the native word next to the English one. Plus `[fxreg.cat]` — 10 categories, which are also the group headers in the picker.
- **t126, the part "c"** (`src/apps/kt-app/src/i18n_tests.rs:146`): sections compared with user input lie in the Arabic table in LOGICAL order. For search this is not cosmetics but a condition of working: `filter_entries` takes `tr(kw)`, and `tr` for Arabic returns the table reordered at build time WORD BY WORD (item 126); were `[fxreg.kw]` not excluded, Arabic would type its word and find nothing — a substring match breaks by a permutation of words, not of letters. The test is the only guard of this exclusion, and it is also the model of a check that will have to be extended for every new searchable section.
- **Project search Ctrl+F** — `search_hits` (`src/apps/kt-app/src/app.rs:4543`, item 92, t92) plus the ready-made window `draw_search` (`app.rs:1335`): a field with a hint, arrows, Enter, Esc inside the window, rows of the form "tag · label". This is COMMON code with F-MED-001 (that one owns search over the CONTENT of the project, this note — search over the program itself). Reusable from it: the window as a whole, `HitKind` (`app.rs:4522`) as the extension point, and ranking 0/1/2 — but today the rank is a closure inside `search_hits`, and it would have to be pulled out. NOT reusable: `filter_entries`, which has no rank at all.
- **Half of the text of the settings index is already written and paid for**: the `[hint]` section, 11 values × 5 languages, of which five describe precisely settings: `hint.fps`, `hint.resolution`, `hint.preview_quality`, `hint.smoothing`, `hint.economy`. They already say WHAT the setting does, and are already translated; they are simply tied to mouse hover and therefore unreachable to someone who has not yet found the control.

### What is missing

1. **No setting is searchable.** `HitKind` knows Marker, Media, Track, Clip, Group — there is no kind "setting". We have eleven switchable settings, and they are laid out across three unrelated surfaces: the strip under the preview (`app.rs:3615` — fps, project resolution, preview quality, smoothing), the "View" menu (`toolbar.rs:306` — detach the preview, economy UI, language), and the render settings window (`render_dialog.rs:275` — encoder, CRF or bitrate, audio bitrate, output resolution). A twelfth field, `ProjectSettings::sample_rate` (`src/libs/kt-model/src/project.rs:1685`), has no control at all, and eight `KT_*` environment variables are not visible from the GUI by a single line.
2. **Four render settings nowhere say what they do.** There is not a single call of `hint()` in `render_dialog.rs`, and `[hint]` does not describe them — there would be nothing to search them by even if search existed.
3. **Effect descriptions do not get into the program.** `FxEntry` has no field for a description. `[fxdesc]` lives in `site/content/*.toml` and is drawn only on the landing page (`site/build.py:363`).
4. **"Translated into five languages" — is wrong, checked.** `[fxdesc]` exists in five files, but is translated into ONE language: `es.toml`, `zh-Hans.toml` and `ar.toml` contain the English text byte for byte. The coverage of these three catalogs is 0.0 %, and `site/build.py` as a matter of course gives them `noindex` (the same figure is recorded in F-HLP-032). Two languages out of five are paid for, not five out of five; in terms of moving the descriptions into the program this is 30 untranslated values, not zero. All 50 existing values nonetheless fit under the 160-character ceiling (the maximum is 104), and the glyphs are short by exactly one character (a hard sign in the Russian `cammotion`).
5. **The description is not tied to the registry by anything.** The keys of `[fxdesc]` — `green`, `grid`, `cammotion` — do not match the names of the registry, and the divergence is already alive: the landing page offers `CamMotion`, which left the picker in R.34 K2 (item 230, "new instances are not created"), while `Pan 3D`, added in R.29, is described in no language. Two audio effects, `Gain` and `Slowed+Reverb`, are described nowhere and never were. In total, of the 12 registry entries 9 are explained in words. Neither `site/build.py` nor the tests of kt-app check this link — the divergence could not help but appear.
6. **The case of the keywords is held by an honest word.** `filter_entries` lowercases the QUERY, but not the value: `tr(kw).contains(&q)`. One capital letter from the translator — and the effect silently stops being found in their language. Today all 60 values are lowercase, but only by convention in the catalog's comment; `build.rs` does not check this.

### What gets in the way

- **One setting is not like another, and search promises one thing.** We have three kinds of them: a property of the project (fps, resolution — the command `SetSettings`, goes into `.otio`, is undone by undo), the state of the session (preview quality, smoothing, economy UI), and a parameter of a job (the whole render window, lives until the job ends). "Found and changed" means different things for them, and exactly ONE survives a restart — the language, via `storage.set_string("lang", …)` (`app.rs:5182`). Smoothing and economy UI die together with the process; one cannot promise about them "left as you set it".
- **`@modified` has nothing to compare with in our case.** The filter "modified" relies on a settings file with defaults; we do not have one — there are two lines in egui-storage (`lang` and `last_project`). This is precisely conclusion No. 2 of the first wave about "user storage outside the project" (F-AUT-004, F-TXT-002, F-EXP-001, F-UI-002) — we quote, not reopen: until there is a place, `@modified` does not carry over and must not be promised.
- **The error with word order is visible only in Arabic.** The displayed and the comparable strings diverge in one language out of five, so "works" and "broken" look identical in the other four. The only protection is to extend t126 at the same time as registering each new searchable section.
- **The index cannot grow by itself.** Bypassing the UI and collecting the settings is a temptation, but rule 20 ("nothing grows on its own") and the precedent of R.38 say the opposite: the registry is written by hand, and completeness is guarded by a reachability test, just as t252 guards orphaned commands.
- **The economy UI lives in two places** — the "View" menu (`toolbar.rs:318`) and the performance panel (`app.rs:3763`). The search must bring it into ONE place and choose which (rule 17).

### Which invariants it touches

- Invariant 12 — only at the edge: the filter is computed on a change of the query line, not on every frame; the search window in our case already lives this way.
- UI checklist: 13 (do not set up a new strip for search — take the existing surface), 14 (the search window must give back focus), 17 (paired ways are identical — economy UI), 18 (say WHAT and WHERE).
- The project format is not touched: neither the keywords nor the settings index go into `.otio`, except the already existing `ProjectSettings`.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages: (1) the settings index and a sixth kind in `search_hits` with a column "where it lives"; (2) a description in `FxEntry` and the move of `[fxdesc]` into `i18n/` with the completion of three languages; (3) a comparable string for Arabic and an extension of t126 |
| Where the main work is | `src/apps/kt-app` as a whole: `fx_registry.rs`, `app.rs` (`search_hits`, `draw_search`), catalogs `i18n/*.toml`, `build.rs` |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::every_setting_and_effect_is_findable`; the extension of t126 to new searchable sections; the lowercase clause in `build.rs`; the link of `[fxdesc]` with the FX registry (today it exists neither in `site/build.py` nor in the kt-app tests) |
| Depends on | F-HLP-025 (the text of the descriptions itself), F-HLP-015 (the surface, if a palette appears earlier), F-MED-001 (the common `search_hits`); the price of translation — F-HLP-033 |
| Breaks compatibility | no |

## Verdict

We take it, but split in two and in the reverse order of the title. **Effects:** we complete what already exists — the rare part of the mechanism (invisible synonyms in five languages) already works for us, what is missing is a cheap one-line "what this does", and that is half-written and paid for. **Settings:** set up the index, but not a separate surface: eleven settings do not pay for their own window, they pay for a sixth `HitKind` in the already existing Ctrl+F and for a mandatory column "where it lives". Do not promise `@modified` until there is a user storage of settings (conclusion No. 2 of the first wave).

TEST: reach_tests::every_setting_and_effect_is_findable — today it fails twice. (a) `search_hits` (`src/apps/kt-app/src/app.rs:4543`) knows five kinds — Marker, Media, Track, Clip, Group — and not a single kind "setting": of the eleven switchable settings, laid out across three unrelated surfaces (`app.rs:3615`, `toolbar.rs:306`, `render_dialog.rs:275`), Ctrl+F finds none, and the four render settings, moreover, are not described by a single catalog line. (b) `filter_entries` (`src/apps/kt-app/src/fx_registry.rs:84`) searches by names and by the 60 values of `[fxreg.kw]`, but is called from exactly one place — `widgets::chain_strip` (`src/apps/kt-app/src/widgets.rs:490`) — and of the 12 registry entries 9 are explained in words, and the explanation lives in `site/content/*.toml` and does not get into the program.

CHECK: src/apps/kt-app/build.rs — a clause "the value of a searchable section is written in lowercase": `filter_entries` compares `tr(kw)` as it is with the lowercased query, so one capital letter from the translator silently removes the effect from the search in their language, and today this is held only by a comment in the catalog.

DEFERRED: until F-HLP-025 — the text of the effect descriptions is entered there; here the single requirement for it is recorded, which is visible from this note and not visible from that one: the description must be SEARCHABLE, that is, lie in `src/apps/kt-app/i18n/*.toml` next to `[fxreg.kw]` and have a comparable Arabic form, rather than remain display text of the landing page.

## Sources

- Boris FX forum (VEGAS Pro), thread "Enhancement: Filter VideoFX presets, not just the FX", 19.06.2026:
  https://forum.borisfx.com/t/enhancement-filter-videofx-presets-not-just-the-fx/24206
  (accessed 2026-09-07; via the Discourse API `/raw/24206/1`)
- VS Code, Settings editor and the `@` filters:
  https://code.visualstudio.com/docs/configure/settings (accessed 2026-09-07)
- VS Code, what exactly is matched against the query (the primary source is the code, not the documentation): `SettingMatches` in
  https://raw.githubusercontent.com/microsoft/vscode/main/src/vs/workbench/contrib/preferences/browser/preferencesSearch.ts
  (accessed 2026-09-07)
- IntelliJ IDEA, the Settings dialog:
  https://www.jetbrains.com/help/idea/settings-preferences-dialog.html
  (accessed 2026-09-07)
- **A live check for software outside the core is not assumed** (the rule of `_editors.md`). For the strings of the core: `helpx.adobe.com` did not open from this environment either directly or through the text proxy `r.jina.ai` (HTTP 422), `support.apple.com` gave only the table of contents of the manual, and the session's `WebSearch` budget was exhausted (200 of 200) before work on the note began. Therefore the claims about the ABSENCE of search in the settings of Premiere, Resolve, Final Cut Pro and After Effects are marked [NOT VERIFIED]; the claim about the presence of search over effects in all five did not need checking — it is confirmed both by the quote and by the vendors' documentation.
- Our numbers were obtained by grep and counting over the repository as of 2026-09-07: 522 values in `src/apps/kt-app/i18n/ru.toml` (34 sections), 60 values and 287 words in `[fxreg.kw]`, 50 values of `[fxdesc]` in `site/content/*.toml` (maximum 104 characters), translation coverage of `es`/`zh-Hans`/`ar` — 0.0 %.
