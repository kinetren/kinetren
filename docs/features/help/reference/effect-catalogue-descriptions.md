---
id: F-HLP-025
title: Effect description where it is chosen
status: candidate
priority: B
cost: S
category: help/reference
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, photoshop-27]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [9, 10, 13]
round: null
point: null
tests: []
depends_on: [F-HLP-002, F-HLP-032, F-HLP-033]
checked: 2026-09-07
---

# F-HLP-025 — Effect description where it is chosen

## Essence

The editor has opened the effects picker and reads a list of names: `Edges`, `EventGrid`, `Pan 3D`. An effect name is an industry term; it is not translated and promises nothing. To find out what an effect does, today you have to apply it, look, and undo. A one-sentence description ("Sobel outlines — the material turns into hatching") closes this cycle before it starts. For us such phrases are already written and proofread — but they lie in the catalog of the landing page, not in the program.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 build 66 | Video FX / Plug-In Chooser | a list of plugins with a filter and "favorites" (the parsing of the filter — F-HLP-019). There is no text "what this does" in the chooser; third-party OFX plugins carry their own description in a plugin property, but the host does not show it [NOT VERIFIED] |
| Adobe Premiere | 26.x | Effects panel | a tree of categories and search by names; there is no effect description in the panel, the explanation lives in the web help [NOT VERIFIED: `helpx.adobe.com` did not open from the environment] |
| DaVinci Resolve | 21 | Effects Library + Hover Scrub Preview | there is no description; instead there is a DEMONSTRATION: hovering over a thumbnail plays the effect in the viewer on the SELECTED clip with default parameters, and if no clip is selected — on the one under the cursor. Enabled by default |
| Final Cut Pro | 12.3 | Effects browser | there is no description; dragging the mouse over a thumbnail plays the effect in the viewer, and with Option held the drag runs the MAIN parameter of the effect (for Radial it is Amount) |
| After Effects | 26.x | Effects & Presets panel | name and category, search by names; there is neither a description nor a preview of the action in the panel [NOT VERIFIED] |

The result for the core: **none of the five shows an effect description**. Two of the five answer the same question by showing; three do not answer at all. Therefore the questions "where does this text live" and "who writes it" cannot be sorted out on the core alone — a second table is needed.

**Outside the core.** Programs where the description of a UI element is not decoration but a declared field next to the code (Blender), and where showing the action is taken to a video clip in the hint (Photoshop); in the five NLEs there is neither.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | tooltip + `F1` → Online Manual | the hint is assembled from declared fields: a short description, the hotkey, the value, the Python identifier. The description is the attribute `bl_description` next to `bl_label` and `bl_idname` in the definition of the operator itself; `F1` on a control opens the manual page of EXACTLY this control, and the manual honestly says that the coverage is not one hundred percent |
| Photoshop | 27.x | Rich Tool Tips | the most expensive end of the scale: in a tool's hint a short video of what the tool does is played (`_editors.md`; [NOT VERIFIED] for 27.x) |

## The trick behind it

**The cargo of this mechanism is the ACTION of an effect, shown as a moving picture on your own frame.** One status-bar line will never hold it: it fits a name and a phrase, not thirty frames. For us this cargo is deliberately shrunk to one sentence, because showing requires a run of `frame_plan` on every hover, while the phrase is already written and translated — the only question is from which catalog it is read.

Three details that are absent from every list of features.

1. **In OFX the description is a property of the PLUGIN, not of the host.** `kOfxPropPluginDescription` in `ofxCore.h` is "a potentially verbose description of the effect", UTF-8, one dimension; next to it lie THREE lengths of one name: `kOfxPropLabel`, `kOfxPropShortLabel` ("typically 13 character glyphs or less. Hosts should use this if they have limited display space") and `kOfxPropLongLabel` ("typically 32 character glyphs or so"); and the category — `kOfxImageEffectPluginPropGrouping`, "purely a user interface hint for the host". That is, the name, the short name, the long name, the category and the description are five properties of ONE object, arriving together with the code that draws the pixels. Renaming an effect and forgetting the description is impossible in principle in this scheme. For us it is exactly the opposite: the name is a literal in `fx_registry.rs`, the category is a key of the program's catalog, the description is a value of the LANDING PAGE catalog, and it has already gone stale (`cammotion`, F-HLP-033).
2. **The upper end of the scale answers not with text but with a demonstration — and it demonstrates not the effect but its MAIN parameter.** In FCP dragging over a thumbnail plays the effect in the viewer, and Option+drag "shows a preview of the effect of dragging the slider for the effect's primary control" — that is, every effect in the browser has a declared primary control, and the browser knows how to turn exactly it. Resolve's demonstration is simpler (default parameters), but it is enabled by default and tied to the SELECTED clip. Both replace a paragraph with a movement and write not a single word: the effect description is what you write when showing is impossible.
3. **Inside the same Resolve there is a description — but for PRESETS, not for effects.** In the color management settings: "Whenever you choose a preset, a brief description explains the workflow that preset is intended to facilitate". The boundary runs not between a "serious" and a "non-serious" program, but between what can be shown in a frame and what cannot. Our picker is the second case: we are not going to show 11 effects on hover (that is a separate subsystem and a separate cost), but we already have the words.

## Pros

- Two thirds of the work is not composition but transfer: 9 of 12 values move verbatim from `site/content/en.toml` and `ru.toml`; their length is 36–104 characters against the catalog limit of 160, that is, they fit into the limit with a margin of more than half.
- It is read exactly at the moment of choice and closes the cycle "applied → looked → undid". For us this cycle is more expensive than in FCP: applying an effect is a model edit, an undo step, and a rebuild of the FX state by the diff of chains (invariant 3).
- The program's catalog can do what the landing page's catalog cannot: a key that nobody displays breaks t125 ("catalog keys nobody uses"). If an effect is renamed, the dead description turns red in a test on the same day, rather than living five rounds as `cammotion` did since R.34.
- Not a single new pixel and exactly one change in the display code: all four points of adding effects are merged into one widget `chain_strip` (t217), and the hint from the FX window already reaches the status strip by itself.

## Cons

- A word is not a demonstration. FCP and Resolve answer "what does this do" with a moving picture on your own frame; a phrase of 100 characters is cheaper for us and poorer for the user, and this difference is seen at once.
- The text stays separately from the effect's code: an enum variant in kt-model, a category in `video_meta`, a value in TOML. In OFX it is one property of one object; moving it into the program's catalog shortens the distance from two trees to one, but the guarantee is given not by distance but by a test.
- 42 values must be translated, of which 15 must first be invented: `Pan 3D`, `Gain` and `Slowed+Reverb` are not described anywhere, and the ready landing-page prose about Slowed+Reverb is 188 characters in English and 203 in Russian — it does not fit the limit of 160 and requires a rewording, not a transfer.
- The landing page becomes a consumer of the program's catalog: `site/build.py` will start reading `src/apps/kt-app/i18n/*.toml`, and its build will stop being autonomous. There is a precedent — `site/screenshots.py` already drives the real `kt-app` (F-HLP-032) — but the dependency is new.
- The eye, at the moment of reading, is in a popup in the middle of the screen, while the text appears in the strip at the bottom of the window. The price of this gap is measured by F-HLP-002, and until it is measured it is unknown.

## What users say

About the preview in the effects browser (BCC FX Browser — an effect-selection window with a preview that lives inside VEGAS and AE):

> BCC Fx Browser has never had an option to Apply, at the moment i believe it
> is just a viewing platform.
> — Gid.Joiner, forum.borisfx.com, 03.02.2022,
> https://forum.borisfx.com/t/choosing-and-applying-filter-from-bcc-fx-browser-and-background-preview-issue/13215

> for me i can only view the fx on the Background, the video i applied FX
> Browser to, not Checkerboard or Black, but that doesn't bother me, i want to
> see it on the video i'm working on.
> — same thread

The second matters more than the first: the person refuses to look at the effect on a checkerboard or a stock frame and wants to see it on HIS OWN material. This is the upper limit, which text cannot reach by definition. A targeted search for complaints of the kind "I don't understand what this effect does" did not succeed: the session's `WebSearch` budget is exhausted, and a search of the Boris FX forum (Discourse `/search.json`, four queries) returns threads about bugs and licenses.

## How it fits Kinotrain

### What already exists

The effect registry is `src/apps/kt-app/src/fx_registry.rs`, the function `registry()`: **12 entries** — 10 video templates from `video_fx_templates()` (`src/apps/kt-app/src/fx_editor.rs:528`) plus `Gain` and `Slowed+Reverb`. In the video picker there are 11 rows (the two-sided time warp, t48); in the audio picker, 2. `fx_display_name` knows 14 names: beyond the registry — `Layer mix` (the strength of the morpher layer) and `CamMotion` (migrated to `Pan` in R.34, lives only when reading old projects).

What the user sees: the name (not translated — effects are searched by it in any locale), the category `[fxreg.cat]` (10 values), and invisible keywords `[fxreg.kw]` (12 keys). There is no explanation at all: the string `fxdesc` does not occur in `src/` even once.

Descriptions exist in the landing page, `site/content/*.toml`, the section `[fxdesc]` — **10 keys**: `pan`, `cammotion`, `mirror`, `blur`, `green`, `edges`, `noise`, `vhs`, `glow`, `grid`. Nine of the ten coincide with the registry entries; the tenth describes `CamMotion`. Three registry entries are not described: `Pan 3D`, `Gain`, `Slowed+Reverb` — the last two have landing-page prose in a different form (`fx.gain_body`, 24 characters; `fx.slowed_body`, 188 en / 203 ru).

**Does a description fit into the program's catalog — yes, with a margin.** The limit of a value is 160 characters (`MAX_VALUE_CHARS`, `src/apps/kt-app/build.rs:42`); the real lengths of `[fxdesc]`: English 36–104 (the maximum is `green`), Russian 32–93. More than a third of the room remains even for the longest phrase. Only `fx.slowed_body` does not fit, and it is not from this section.

**In how many languages the description is really translated — in two of five.** `ru` is translated; `es`, `zh-Hans` and `ar` are byte-for-byte English. This is not only about `[fxdesc]` in isolation: all 220 values of each of the three files coincide with the English ones, the coverage is 0.0% against `translated_min = 0.9` (`site/content/structure.toml:23`), that is, the three languages of the landing page are already under `noindex`. The catalogs of the PROGRAM are translated for real: out of 522 values, those coinciding with the English ones are 14 (ru), 36 (es), 8 (zh-Hans), 13 (ar) — these are proper names. A finding that did not exist before the reconnaissance: we have exactly one paid five-language catalog, and it is the program's catalog, not the landing page's.

The display surface is fully ready. `hint_into(out, r, text)` (`src/apps/kt-app/src/app.rs:4419`) writes a hint into the frame's accumulator; `chain_strip` (`src/apps/kt-app/src/widgets.rs:368`) accepts `hover: &mut Option<String>` and already passes it to the checkboxes, slots and copy-paste buttons; `draw_one_fx_editor` merges the accumulator into `hover_hint` (`src/apps/kt-app/src/fx_editor.rs:323`), and `draw_status_hint` draws the line last in the frame. The only place without a hint is the picker itself: `fx_picker_with_head` (`fx_registry.rs:114`) does not receive `hover` at all, and its rows (`ui.button(e.name)`) stay silent.

### What is missing

The sections `[fxdesc]` in `src/apps/kt-app/i18n/*.toml` — 12 keys × 5 languages = 60 values, of which 18 move verbatim and 42 must be written (27 translations of ready phrases into es/zh-Hans/ar and 15 values of three new keys). F-HLP-033 estimated the transfer at "30 values and 10 keys" — the reconnaissance refines the count: 42 and 12, because the registry knows about `Pan 3D` and two audio effects, while the landing page does not know about them.

There is also no single address of truth. Today there are two trees, and they have already diverged four times: `cammotion` describes a non-existent item; `Pan 3D` is missing from the landing page; `[fxcat]` knows 6 categories out of 10 (three items — F-HLP-033); and the caption under the screenshot promises a picker "listing all ten effects grouped by category", although the video picker shows eleven rows. The direction of the transfer is set rigidly and is not chosen: the program's catalog must be complete in five languages (otherwise `kt-app` does not build), while the landing page's catalog allows a partial delivery — so the source of truth can only be the programmatic one, and `site/build.py`, as a consumer of `[fxdesc]` from its own tree, becomes a consumer of the program's catalog.

### What gets in the way

- A key that nobody references by its full path `k::fxdesc::…` breaks t125 — that is, the 12 keys must be stitched into `FxEntry` at the same time as the catalog; a partial delivery does not happen (the price — F-HLP-033).
- New values in zh-Hans and ar drag along t100 (a glyph for every non-ASCII character), a rebuild of the subsets `python3 scripts/subset-fonts.py`, and word-by-word bidi on the build.
- Editing `fx_registry.rs` must leave t48 green: it checks the registry against the templates by name and forbids the morpher's pseudo-slot from appearing in the registry.
- The hint of the chain slot is already TAKEN: `k::fx::SLOT_HINT` ("click selects, drag reorders, cross removes", `src/apps/kt-app/src/widgets.rs:460`) — it is about the gesture, not about the effect. The description is attached to a picker row, where there is no hint at all; replacing the gesture hint with a description is not allowed, and showing both is a violation of rule 10.
- The FX window is an `egui::Window` with a default size of 580×520 and a minimum of 460×320 (`fx_editor.rs:167`), and it can be dragged over the status strip. This is not a prohibition, but a reason not to consider the strip guaranteed to be visible.

### Which invariants it affects

Invariant **12** — a preview in the spirit of FCP/Resolve (a frame with the effect on hover) requires a run of the frame plan on every mouse movement and therefore does not fit within "the UI does not heat the system" without a separate subsystem; the note does not propose it. Rule **10** (exactly one hint, in one place) — precisely for this reason the description lives in ONE surface, not in two. Rule **13** (do not waste space) — it rejected a fixed block of description at the bottom of the popup: until the first hover it is empty. Rule **9** — the description does not duplicate the name, it answers a different question. The regression guards that must stay green: t48, t100, t125, t128.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round; the work is translation, the code is one field of `FxEntry`, one argument of the picker, and reading the section in `site/build.py` |
| Where the main work is | `src/apps/kt-app/i18n/*.toml` (42 new values), `fx_registry.rs`, `site/build.py` |
| Project format | not touched |
| Tests to write | one: `i18n_tests::every_effect_has_exactly_one_description` — one key per registry entry and one entry per key, plus a ban on a second copy of the text in `site/content` |
| Depends on | F-HLP-033 (the cost of five languages), F-HLP-032 (the site pipeline), F-HLP-002 (the real budget of the strip) |
| Breaks compatibility | no |

## Verdict

The work here is not "writing a help" but moving what is already written to where people look at it, and naming one address of truth. The reconnaissance changed two figures of the setup: the descriptions are translated not into five languages but into two (`es`, `zh-Hans`, `ar` in `site/content` are English placeholders, coverage 0.0%), and what must be moved is not 10 keys but 12 — the registry knows about `Pan 3D`, `Gain` and `Slowed+Reverb`, which the landing page does not know. Within the 160-character limit the descriptions fit with a margin: the longest is 104 characters.

TEST: i18n_tests::every_effect_has_exactly_one_description — fails today at the very first step: the section `fxdesc` does not exist in the program's catalog at all (0 descriptions for 12 registry entries), and the only section with descriptions lives in someone else's tree, where there are 10 keys, one of which describes `CamMotion`, removed from the picker in R.34, and three registry entries are not described anywhere.

CHECK: site/build.py — the clause "`[fxdesc]` is read from the program's catalog; the landing page has no section of its own": as long as the same text lies in two trees, it diverges, and this has already happened four times.

ONE LINE: overlap — not touched | delay — not touched | capacity — not a single new pixel: the description (today ≤104 characters against a limit of 160) travels into THE SAME status strip through the existing `hint_into`, and the new thing here is not the place but the MOMENT — the effect picker's row, the only element of the FX window that has no hint at all.

## Sources

- `ofxCore.h` (OpenFX, AcademySoftwareFoundation): `kOfxPropPluginDescription`, `kOfxPropShortLabel`, `kOfxPropLongLabel` —
  https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxCore.h
  (access date 07.09.2026)
- `ofxImageEffect.h`: `kOfxImageEffectPluginPropGrouping` —
  https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxImageEffect.h
  (07.09.2026)
- Final Cut Pro User Guide for Mac (the PDF of the manual, section "Add video effects"): "Preview an effect: Skim the video effect thumbnail" and the Option-drag over the main parameter —
  https://help.apple.com/pdf/final-cut-pro/en_US/final-cut-pro-user-guide.pdf
  (07.09.2026)
- DaVinci Resolve 21 Reference Manual (PDF), ch. 54 "Using the Effects Library" (Hover Scrub Preview) and ch. 9 (the description of color management presets) —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  (07.09.2026)
- Blender 5.2 LTS Manual, "Help System" (the composition of the hint, `F1` → Online Manual) — https://docs.blender.org/manual/en/5.2/getting_started/help.html;
  `bpy.types.Operator.bl_description` —
  https://docs.blender.org/api/current/bpy.types.Operator.html (07.09.2026)
- forum.borisfx.com, thread 13215 (quotes) —
  https://forum.borisfx.com/t/choosing-and-applying-filter-from-bcc-fx-browser-and-background-preview-issue/13215
  (07.09.2026)
- Our numbers were obtained by counting over the repository's files: `fx_registry.rs`, `fx_editor.rs`, `src/apps/kt-app/build.rs`, `src/apps/kt-app/i18n/*.toml`, `site/content/*.toml`, `site/content/structure.toml`, `site/build.py`.
- Not checked live: the VEGAS, Premiere and After Effects rows — `helpx.adobe.com` does not open from the environment either directly or through the text proxy, and the session's `WebSearch` budget is exhausted. For Blender and Photoshop no live check is expected (`_editors.md`, "Software outside the core").
