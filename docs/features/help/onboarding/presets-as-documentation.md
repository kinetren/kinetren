---
id: F-HLP-031
title: A preset as documentation — "this is how it is set up"
status: researched
priority: B
cost: S
category: help/onboarding
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-model, kt-cli]
invariants: []
ui_rules: [4, 18, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-004, F-HLP-025, F-AUT-006]
checked: 2026-09-17
---

# F-HLP-031 — A preset as documentation — "this is how it is set up"

## Essence

The editor does not read a list of effects — he takes a ready-made setting with a name ("VHS 90s", "interview", "for YouTube"), puts it on a clip and LOOKS at which sliders have moved. A setting made by the developer teaches two things at once: what the program can do and how these handles are customarily turned. The question is not "are the values saved" (this is F-AUT-004), but "is it visible WHAT the preset did": a preset into which one cannot look gives the result and teaches nothing.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Preset (Video FX window) | **the preset is the unit of viewing, the effect is a folder**: on the left a list of effects, on the right "thumbnail images … represent each of the existing presets for the selected effect"; hovering plays an animated example, the thumbnail is DRAGGED onto an event, a track or the preview window. Shift+drag applies a preset at all without opening a dialog. The chain is saved as a separate entity — the FX package (F-AUT-004) |
| Adobe Premiere | 26.x | Effect Preset (the Presets folder in the Effects panel) | the supplied sets lie in the same panel as the effects, in a separate folder. The Save Preset dialog asks for a NAME and a **description** — that is, a preset has a field where the author writes why it exists. An applied preset unfolds in Effect Controls into real effects and keyframes: it can be taken apart. A separate pole — Lumetri Look: a LUT-preset, where the result and the strength slider are visible, but the contents are opaque [some details NOT VERIFIED: `helpx.adobe.com` does not open from the environment, neither directly nor through a text proxy] |
| DaVinci Resolve | 21 | Still / PowerGrade in the Gallery + ResolveFX presets | "You can also save grades by right clicking anywhere in the viewer and creating a "still." Stills can be organized into albums and are stored in the gallery"; "Apply Grade" puts the saved grade onto another frame. A still carries the WHOLE tree of nodes, and to look into it means opening the nodes one by one: the most analyzable preset of the five. The drop-down list of presets in ResolveFX and the Deliver presets, named by purpose — [NOT VERIFIED against the primary source]; export presets as such are the subject of F-EXP-001 |
| Final Cut Pro | 12.3 | Effects Preset (the effects browser) | "You can save any combination of video effects and video effect parameter settings as a _video effects preset_, **which appears in the Effects browser and functions like any other video effect**" — the preset is deliberately INDISTINGUISHABLE from an effect. The composition is marked by checkboxes by attributes; if there are keyframes — Stretch to Fit / Maintain Timing; it is distributed as a file in `~/Library/Application Support/ProApps/Effects Presets/`. Separately from the effects — the menu item **Final Cut Pro > Download Demo Project**: a library with two ready-made projects built on Keywords and Smart Collections is downloaded |
| After Effects | 26.x | Animation Preset `.ffx` (the Effects & Presets panel) | the standard animation presets are laid out by categories in the same panel as the effects; a `.ffx` carries the effects, values, keyframes and expressions exactly for one layer. An applied preset is visible in the layer's timeline in full and is taken apart by keyframe — the most "readable" format and the least portable [NOT VERIFIED against the primary source: `helpx.adobe.com` is unavailable; the composition of `.ffx` is taken from F-AUT-004, where it is confirmed by secondary sources] |

**Outside the core.** Only in Blender is the "preset" the WHOLE default document, not a set of values of one effect, and only there is there a named road back; the five NLEs do not answer at all the question "how does the default differ from a preset and how to return to the factory state", and this is exactly the question the owner asked (R.38 item 16).

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Startup File + Load Factory Settings | "This menu manages the startup file which is used to store the default scene, workspace, and interface displayed when creating a new file. Initially this contains the startup scene included with Blender." The startup scene (cube, camera, light) is a supplied PRESET of the document, and it is also the "minimal working scene" as an assertion of the authors. `Save Startup File` makes its own default, `Load Factory Settings` "restores the default startup file and preferences" — the factory state is addressable by a menu item. Above it — Application Templates: "a re-usable configuration that can be selected to replace the default configuration, without … overwriting your personal settings", with its own startup file, theme, add-ons and layout

## The trick behind it

**The payload that a preset carries and a status bar line does not hold is a STATE.**
Not a paragraph, not a picture and not a link: a dozen numbers brought into a meaningful combination, plus an unspoken assertion "this is the right way". Our line carries
160 characters of text and not a single parameter value; a preset carries only
values and not a single character of text — they do not compete, they are of different kinds. For us this payload ALREADY lies, and lies where it should: in `video_fx_templates()`. What is missing is a name for it and a way to see how it differs from an empty one.

**1. The preset is the unit of viewing, and the effect is merely a folder, and from this the size of the catalog of capabilities changes.** In VEGAS the Video FX window is arranged not as "a list of effects, each with settings", but as "a list of effects on the left, a GRID OF PRESETS on the right", with an animated example on hover and dragging the thumbnail directly onto an event. From this follows a consequence that is absent from every list of features: the list of what the program can do is a list of presets, and it is longer than the list of effects by an order of magnitude. Continuum has 47 transition filters and, in the words of a vendor employee, "hundreds of presets"; the question on the forum sounds verbatim "where can I see them all, I can't remember what they look like". The vendor's answer is the FX Browser: **yet another filter**, which is hung on a clip and iterates over the presets on your own frame. That is, the catalog of presets has grown to a size at which it itself required documentation, and the documentation became an effect. This is not a curiosity but a regularity: an analyzable preset scales as knowledge, and an unanalyzable one scales as noise.

**2. Only a preset that can be taken apart teaches, and this is a verifiable property of the construction, not a style of presentation.** The dividing line runs between "the preset moves the same sliders that the user can move himself" (VEGAS, Premiere Effect Preset, AE `.ffx`, a Resolve still) and "the preset substitutes an opaque table" (Lumetri Look and LUTs in general). In the first case applying a preset is a demonstration of how to solve the task; the user pauses, reads the values and gets an answer to a question that does not fit into the help: "HOW MUCH do they set here". In the second only the result is visible, and there is nothing to learn — one can only choose from someone else's list. The forum quote below is a direct confirmation: a person says that everything he knows about the particle engine he learned by taking the supplied presets apart, and advises the asker to do the same.

**3. A preset that does not name itself stops teaching even when it is analyzable.** Thread 13225: the user applies a Sapphire preset, the sliders are set correctly, but the drop-down list in the FX window again shows "Default" — "i've still no idea which fx is on the timeline". The mechanism is intact, the documentation is destroyed: the RELATION between the default and the preset has been erased, and exactly that is the content of the lesson ("here is the empty state, here is the configured one, here is the difference"). Without this relation the configured state is indistinguishable from a random one, and the user gets exactly the feeling the owner described in R.38 item 16 verbatim: "a feeling that effects have some separate 'default' state of values". The phrase "some separate" is more precise here than it seems: for us there are literally three such states (see "How it fits").

## Pros

- A name by TASK works as a table of contents of capabilities: the user learns that the program can do "interview" or "VHS" without reading the list of effects and without knowing the word "Sobel". A list of parameters describes the tool, a list of presets describes tasks, and a person comes with a task.
- A preset from the developer is the only form in which one can convey taste: the help is entitled to say what `threshold` does, but is not entitled to say that 48 is a good value and 200 an ugly one. A preset says this silently and without obligations.
- An analyzable preset does not diverge from the code the way prose does: it is EXECUTED. Rename a parameter — the preset breaks loudly and at once, whereas a paragraph of help continues to lie confidently (our own case: `structure.toml` still suggests "CamMotion", removed from the picker in R.34 — F-HLP-032, item 230 of `docs/porting-notes.md`).
- A thumbnail and an animated example move the cost of choice from reading to looking: "hover your cursor over a preset to see an animated example" replaces a paragraph of description and is translated into five languages for free (cf. the cost of a line in F-HLP-033).
- The "description" field of a preset (Premiere) makes the author of the preset a co-author of the documentation, moreover in the only place where it is convenient for him — at the moment of saving, when he still remembers why.
- A factory state named by a menu item (Blender `Load Factory Settings`) closes rule 19 of the checklist for the whole program with one line, and not with one plate per decision.

## Cons

- A catalog of presets requires a catalog itself. At 600 entries neither search by name nor categories help, and the vendor is forced to write a separate display tool (FX Browser), while the community makes third-party preview galleries. Presets become numerous far earlier than a way to look at them appears.
- A set of presets goes stale more slowly than code — and that is exactly why it is more dangerous. The values remain valid, the tests are green, nothing falls; only in a year does the supplied set teach a technique that is done differently in the new version. Prose of the help at least gets read by a person when edited, while a preset is never re-read.
- A preset as a black box (LUT, Look) gives the result and takes away understanding: it scales to a hundred items, but a hundred LUTs do not add up to a skill, whereas a hundred analyzed presets do.
- A name by task lies faster than a name by parameter. "H.264 1080p25, 12 Mbit/s" is true forever, "for YouTube" becomes outdated with every change of the platform's requirements — and becomes outdated SILENTLY, because the name has remained the same.
- A preset indistinguishable from an effect (FCP does this deliberately: "functions like any other video effect") doubles the list and makes search ambiguous: is "Blur" in the results an effect or someone's blur setting? The cost is exactly the reverse of the benefit of "we do not split the way of application in two".
- The name of a preset is a translatable string, while the name of an effect we deliberately do not have. Two different rules in one list are a source of the eternal question "why did this get translated and that did not".

## What users say

> I can’t explain the little I do know that I learnt by taking these preset
> emitters to bits, I suggest you do the same, alter one parameter & see what
> it does, reset Ctrl+Z that action then alter another & so on
> — Gid.Joiner, Boris FX forum, 29.04.2023,
> https://forum.borisfx.com/t/other-emitters/16296

> if i load a preset from Load Preset option, the preset doesn’t show the name
> back in Vegas FX’s window of which i’ve chosen, the drop down at the top just
> says & only has Default … so i’ve still no idea which fx is on the timeline
> — Gid.Joiner, Boris FX forum, 03.02.2022,
> https://forum.borisfx.com/t/load-preset-doesnt-show-name-in-fx-window-returns-to-default-when-reopening-load-preset/13225

> Is there a website where I can preview all of Continuum’s 600+ transition
> effects? … I can’t possibly remmeber what they all look like.
> — peter2, Boris FX forum, 03.06.2020,
> https://forum.borisfx.com/t/continuum-has-over-600-transitions-is-there-a-place-i-can-preview-them-all/8230

## How it fits Kinotrain

### What already exists

**Parameter presets from the developer — yes, ten of them, and the user does not know that they are presets.** `video_fx_templates()`
(`src/apps/kt-app/src/fx_editor.rs:528`) — ten video templates; `registry()`
(`src/apps/kt-app/src/fx_registry.rs:47`) adds `Gain` and `Slowed+Reverb`,
for a total of 12 entries (11 lines in the video picker, 2 in audio — t48). The picker
(`fx_picker_with_head`, same file, line 114) draws the name and the category and on click
does `Picked::Fx(e.template.clone())`. The words "preset", "template", `preset`,
`template` **are not in any value of any of the five catalogs**
`src/apps/kt-app/i18n/*.toml` — checked by grep. That is, from the user's point of view applying an effect and applying a setting is one and the same
action, because the setting is exactly one.

**An effect has not one but THREE separate "default states", and they live in
different crates.** This is the literal answer to R.38 item 16 ("a feeling that effects have some separate 'default' state of values"); the round's diagnosis — the field edited the BASE while showing the curve's value — is correct and already fixed, but it explained items 15–16 together, and the second half of the feeling has its own cause:

| Declaration | Where | Who reads it |
|---|---|---|
| `#[serde(default = "…")]` on each field | `src/libs/kt-model/src/project.rs:2094+` | the file reader: what the field will get when it is absent from an old project |
| `ParamSpec.default` in `params_for!` | `src/libs/kt-model/src/project.rs:2529+` (the type — line 2424) | **nobody** |
| the template in `video_fx_templates()` / `FxInstance::glow(0.65)` | `src/apps/kt-app/src/fx_editor.rs:528`, the constructors in `project.rs:2937+` | the picker: what a NEW effect will get |

**The third declaration is not read from a single live path.** The only read of `ParamSpec.default` in the whole workspace outside the tests is
`src/apps/kt-app/src/fx_editor.rs:418`: `f.get_param(n).unwrap_or(spec.default)`,
where `n` is taken from `f.param_specs()`, so `get_param` by construction returns
`Some`. A dead branch. There is no return to the default in the UI at all: the catalogs contain
`reset_gain` and `reset_height`, but these are the volume rail and the track height, not FX.

**Two divergences out of twelve — and both exactly where the template really teaches something.** The check is manual, by values:

| Effect | Picker template | `ParamSpec.default` | |
|---|---|---|---|
| `EventGrid` | `cols: 2, rows: 2` | `cols: 1, rows: 1` | **≠** |
| `Slowed+Reverb` | `rate 0.82, mix 0.1, delay 85` | `rate 1.0, mix 0.0, delay 60` | **≠** |
| the other ten | — | — | match |

Both divergences are meaningful, and that is the whole point. `rate 0.82, mix 0.1` is not a "default" but a genre by which the effect is named; `2×2` is a grid that is VISIBLE, against `1×1`, which is an identity. That is, in two places out of twelve the developer said "this is how it is customarily set up" — and said it silently, without a name, and no test knows what was said there.

**The opposite decision on the same question, also silent.** `Pan 3D` in `video_fx_templates()` has the comment "defaults are the identity (layer 1:1)", and `Pan` — essentially the same (`Fit`, 0, 0, ×1). That is, to the question "must a fresh effect change nothing, or must it show what it does", the program answers both "change nothing" (Pan, Pan 3D) and "show" (EventGrid, Slowed+Reverb). Both answers are defensible, and there is not one rule.

**The rule is already written, works, and is applied to the wrong things.** `t223`
(`src/libs/kt-model/tests/transitions.rs:507`) requires exactly this invariance for TRANSITIONS and explains why with a comment in the code: "the default of the view == the default of the spec: otherwise 'reset' in the UI gives something other than what creating a transition gives". For effects the same loop over the specs is written
(`t190_set_get_param_all_specs`, `src/libs/kt-model/tests/fx_auto.rs:137`) — range, step, set/get, clamp — and there is NO assertion about the default in it. This is precisely the mechanism of R.38 ("prose breaks nothing"), only this time the rule was written not even in prose but as a test — just a neighboring one.

**A precedent from our own history, more expensive than any foreign one.** Item 230 of
`docs/porting-notes.md`: "empty keyframes on CamMotion meant NOT static, but the v1 preset 'push 1.0 → 1.35' — it is materialized explicitly, otherwise old projects would lose the motion". A supplied preset hidden inside an "empty" state we already had, and it had to be dug out during migration.

**A project-preset is written by us and unreachable.** `kt-cli demo`
(`src/apps/kt-cli/src/main.rs:184`) assembles six named tracks, markers, a region with a loop, FX chains with automation, a morpher and bin groups — that is, a ready-made training edit at the level of FCP's "Download Demo Project". It is unreachable from the GUI and is not even listed in its own list of commands (the same place, line 354: "available: version, new, roundtrip, render, export"). The subject is owned by F-HLP-032 (the pipeline) and F-HLP-023 (the guide); here it is important as proof that a preset-document already exists with us and costs zero.

**A portable format of a preset — also already exists.** `FxChainBlock` / `FxSlotBlock`
/ `EnvelopeBlock` (`src/libs/kt-model/src/ops.rs`) are serialized to JSON and travel
through the system clipboard (t215, t205). This is not a library — see below.

### What is missing

- One main declaration of the default, and a test that the three are consistent.
- A way back to the default: there is no "reset parameter" anywhere.
- A name for the setting, and a marker that says "this is a setting, not an effect" — today the picker list does not distinguish "an effect" from "an effect brought into a meaningful state".
- Showing "what the preset changed": `params_grid`
  (`src/apps/kt-app/src/widgets.rs:209`) draws the values and does not mark those that differ from the default — and that mark is exactly the lesson.
- Presets named by TASK. We have twelve names, and all twelve are names of tools.

### What gets in the way

**1. The three declarations cannot simply be merged: one of them is part of the format.**
The serde default decides what a field missing from an OLD file receives; changing it means re-reading already saved projects differently. So "single source" does not mean "one instead of three" but "one main, two derived", and the serde branch cannot be derived — it can only be CHECKED. The practical conclusion: the work here is a test and, where a divergence is unintentional, a fix to the template, not a refactoring of the declarations.

**2. Templates live in kt-app, specs in kt-model.** `video_fx_templates()` returns `Vec<(&'static str, FxInstance)>`, and `registry()` holds a `&'static [FxEntry]` behind a `OnceLock`; the specs come from `params_for!` in `project.rs`. A test that sees both sides at once has to live in kt-app — in the same place as `reach_tests.rs`, and for the same reason that one appeared there: a model unit test stays green while the application has not yet reached the model.

**3. The library of user presets does not run into this question.** The wave-one conclusion no. 2, verbatim: "Four features run into the absence of a 'user storage outside the project' … The project is one `.otio` file for us, and everything listed lives, in meaning, BETWEEN projects. Until such a place exists, each of the four features will invent it anew" (F-AUT-004, F-TXT-002, F-EXP-001, F-UI-002). Here this conclusion is QUOTED and not reopened: a shipped preset needs no storage at all — it is compiled into the binary, and that is exactly why it is an order of magnitude cheaper than a user library.

**4. A name by task is worth translating.** The effect name is deliberately not translated (the comment on `FxEntry.name`: "this is an industry term, effects are searched by it in any locale"), but a preset name by task will have to be translated — "interview" is not an industry term. The cost of the line is F-HLP-033; what matters here is only that the policy will be TWO in one list, and this has to be decided consciously.

**5. The effect description is a neighboring feature, not this one.** The text "what Sobel does" belongs to F-HLP-025 (`[fxdesc]`, moved from `site/content` into the program's catalog). This note is about VALUES, not prose; together they cover the picker completely, separately each covers its own half.

### Which invariants it affects

- **None of 1–12 of the root `CLAUDE.md`.** The template determines what a NEW effect receives; the goldens (GOLDEN_VIDEO/GOLDEN_AUDIO) are built on fixed projects with values already written, and are not touched by editing the template. This is a strong argument for "cheap".
- **Rule 4** (the default is useful): the question "is a fresh effect an identity or a demonstration?" has been answered in the code twice, and differently; the rule has to be chosen once and written down as a test.
- **Rule 18** (the name says WHAT and WHERE): a name given by task is the same rule, extended to the list of effects.
- **Rule 20** (nothing grows by itself): the shipped set is not replenished with "last used" items and does not grow on its own.
- **Format:** serde defaults must not be touched, see "What gets in the way", item 1. This is not an invariant from the list, but a constraint of the same weight.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage: a test of consistency of the three defaults, a fix of the divergences where they are unintentional, reset to the default in the parameter grid |
| Where the main work is | kt-app (the test, which sees both the templates and the specs; the "value ≠ default" mark in `params_grid`), kt-model (nothing except normalizing values) |
| Project format | not touched; serde defaults are frozen for compatibility |
| Tests to write | a cross-check of the three default declarations across all 12 registry entries; the rule "fresh effect — identity or demonstration" as an explicit decision for each entry; a "parameter differs from the default" mark in the harness test of the FX window |
| Depends on | F-AUT-004 (user library and storage outside the project), F-HLP-025 (description text in the same picker) |
| Breaks compatibility | no |

## Verdict

**We take it in the cheaper half.** The reconnaissance shifted the framing: "we have no presets" is wrong — they exist, there are twelve of them, they are compiled into the binary, and in two of the twelve cases they carry a real authorial claim ("this is how slowed+reverb sounds", "this is how the grid looks"). What is missing is not presets but three things around them: one default instead of three, a name, and a way to see the difference. A library of user presets is a different feature with a different price: it runs into the absent storage outside the project, and this conclusion of the first wave is quoted here, not rediscovered.

TEST: reach_tests::a_preset_is_distinguishable_from_a_default — fails today four times, and all four failures are visible from reading the files, without running the program: (1) the effect has THREE default declarations — `#[serde(default)]`, `ParamSpec.default` and the picker template — and nobody compares them; (2) two divergences out of twelve (`EventGrid` 2×2 versus the spec's 1×1, `Slowed+Reverb` 0.82/0.1/85 versus 1.0/0.0/60) are not marked as intentional anywhere; (3) `ParamSpec.default` is not read from any live path — the only read outside the tests is the `unwrap_or` in `src/apps/kt-app/src/fx_editor.rs:418` after `get_param`, which for a name from `param_specs()` is always `Some`; (4) exactly this assertion exists and is green for transitions (`t223`, `src/libs/kt-model/tests/transitions.rs:507`, with the comment "the default of a transition kind == the default of its spec: otherwise 'reset' in the UI gives something other than what creating a transition gives"), but it is absent from the neighboring loop over effects (`src/libs/kt-model/tests/fx_auto.rs:137`).

DEFERRAL LIFTED (cross-check of 17.09.2026). The former line "DEFERRED: until F-AUT-004" is out of date: F-AUT-004 was done in R.48 — storage outside the project has appeared (`src/apps/kt-app/src/user_store.rs`, file `fx-presets.json` in the autosave directory), a user setting is saved under a name, five factory presets are compiled into the binary and shown in the picker as a separate section (items 343-346, t344-t347). So of the three missing things of this note — "one default instead of three, a name, a way to see the difference" — the second is closed, and the third has been taken over by F-AUT-006 (a preview gallery in the picker, R.54): the difference is now visible as a picture. The FIRST one remains, and it is still the address of this note: nobody compares the three default declarations, `ParamSpec.default` is still not read from a live path, and `reach_tests::a_preset_is_distinguishable_from_a_default` is not written. The note stays in the status "researched" precisely because of this.

## Sources

- VEGAS Pro 23 Online Help, "The Video FX Window" — "The thumbnail images in the right pane represent each of the existing presets for the selected effect. Hover your cursor over a preset to see an animated example":
  https://help.magix-hub.com/video/vegas/23/en/content/topics/2-window/window_videofx.htm
  (accessed 07.09.2026)
- VEGAS Pro 23 Online Help, "Adding video effects" — Shift+drag of a preset onto a track/event/preview window:
  https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/addvideofx.htm
  (07.09.2026)
- Final Cut Pro User Guide for Mac 12.3, "Save video effects presets" — "appears in the Effects browser and functions like any other video effect", Stretch to Fit / Maintain Timing, the folder `Effects Presets`:
  https://support.apple.com/guide/final-cut-pro/save-video-effects-presets-ver462c2078f/mac
  (07.09.2026, via the text proxy `r.jina.ai`)
- Final Cut Pro User Guide for Mac 12.3, "Download a demo project" — the menu item `Final Cut Pro > Download Demo Project`, a library with two projects:
  https://support.apple.com/guide/final-cut-pro/download-a-demo-project-ver4dec14a61/mac
  (07.09.2026, via `r.jina.ai`)
- Blackmagic Design, the DaVinci Resolve Color page — stills, gallery albums, "Apply Grade":
  https://www.blackmagicdesign.com/products/davinciresolve/color
  (07.09.2026, via `r.jina.ai`)
- Blender 5.2 LTS Manual, Topbar → File → Defaults — "Save Startup File", "Load Factory Settings … Restores the default startup file and preferences":
  https://docs.blender.org/manual/en/latest/interface/window_system/topbar.html
  (07.09.2026, via `r.jina.ai`)
- Blender 5.2 LTS Manual, "Application Templates" — "a re-usable configuration that can be selected to replace the default configuration":
  https://docs.blender.org/manual/en/latest/advanced/app_templates.html
  (07.09.2026, via `r.jina.ai`)
- Premiumbeat, "Create Custom Effect Presets in Premiere Pro" — the Save Preset dialog asks for a name AND a description; three transfer modes:
  https://www.premiumbeat.com/blog/create-custom-effect-presets-in-premiere-pro/
  (07.09.2026)
- Boris FX forum (Discourse API `/t/<id>.json`), threads 16296, 13225, 8230 — the quotes above (07.09.2026)
- None of the five was checked live. VEGAS and FCP — by the official help, Resolve — by the official product page, Premiere and AE — secondhand: `helpx.adobe.com` does not open from this environment either directly or through `r.jina.ai` (an HTTP/2 error on the proxy side), searches on `community.adobe.com` return 403, and the session's `WebSearch` budget (200 of 200) was exhausted before work on the note began. For programs outside the core, live verification is not expected under the `_editors.md` rule.
