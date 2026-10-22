---
id: F-HLP-006
title: Explanation of a parameter where it is turned
status: candidate
priority: B
cost: M
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: no
crates: [kt-model, kt-app]
invariants: [12]
ui_rules: [7, 9, 10, 13]
round: null
point: null
tests: []
depends_on: [F-HLP-019, F-HLP-025, F-HLP-033]
checked: 2026-09-07
---

# F-HLP-006 — Explanation of a parameter where it is turned

## Essence

The effect window shows the label "Amount:" and the number "0.40". Neither the label nor the number says what 0.40 actually is: a fraction, pixels, or times — and what will grow at 1.0.
This is about ONE phrase for ONE slider, shown where that slider is being turned. This is not "what this effect is" (the effect description — F-HLP-025) and not "what the field is called" (the label already exists and is already translated).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Plug-In Help | There is no parameter description in the FX window; the window has a BUTTON that opens help for this plugin ("click the Plug-In Help button in the Video FX window"). The automatability of the effect is shown directly in the UI — with an icon in the Plug-In Chooser |
| Adobe Premiere | 26 | none | Effect Controls: parameter name, value, stopwatch. The explanation of a parameter lives in the online help for the effect; there is no link to it from the panel |
| DaVinci Resolve | 21 | none | The program has tooltips, but not about parameters: the node name under the cursor, the date of the last save, the frame delta while dragging, the level when trimming automation. The Inspector fields have no descriptions |
| Final Cut Pro | 12.3 | none | The inspector shows the name and the value; the explanation is in a separate user guide |
| After Effects | 26 | none | Tooltips are enabled by the Show Tool Tips preference and carry names; the Info panel shows "information about items under the pointer" — values, not descriptions |

It is worth noting separately that in this group of five, the parameters of THIRD-PARTY plugins are explained better than the editors' own. The OFX standard (Resolve and VEGAS are OFX hosts) provides for a string property `kOfxParamPropHint` on each parameter — "A hint to the user as to how the parameter is to be used". That is, the industry has already agreed that the author of a parameter writes the one phrase; the hosts simply did not apply this to their own parameters. Whether Resolve shows this string and how exactly — [NOT VERIFIED], there was no live check.

**Outside the core.** Without Blender this feature cannot be described: five NLEs have no mechanism for it at all (four "none" and one "open help" button), while "the trick behind it" is taken entirely from HOW the description gets into the interface in Blender.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Property description (RNA) | The description of a property is declared in the code as a string next to the range; the same string is shown on hover and the same string gets into the API reference. The tooltip, besides the description, carries the value, a hotkey, and if the field is unavailable — a red line with the reason |

## The trick behind it

**What the mechanism carries, which a single status bar line cannot hold: not text, but a BINDING.** A status bar line can show any phrase, but it cannot guarantee that the phrase has not drifted from the range and the unit — because the phrase lives in a catalog and the range lives in code, and there is no link between them. At Blender the description physically lives in the same declaration as the range, and there is no room for them to drift apart. For us this payload will go to exactly the same place — into the line of the `params_for!` macro, where `min/max/default/step/unit/kind` already stand.

This is how it looks verbatim at them (`rna_sequencer.cc`, the Gaussian Blur effect):

```c
prop = RNA_def_property(srna, "size_x", PROP_FLOAT, PROP_UNSIGNED);
RNA_def_property_ui_text(prop, "Size X", "Size of the blur along X axis");
RNA_def_property_ui_range(prop, 0.0f, FLT_MAX, 1, -1);
```

The label and the description are one call; the range is the next line. In the API reference the same string is printed already with the range and the default attached automatically: "Size of the blur along X axis (in [0, inf], default 0.0)". One declaration serves the hint, the documentation, and the API.

The second detail, which is absent from every list of features: at Blender the machine explains not only WHAT the parameter does, but also why it cannot be touched. The list of tooltip elements has a separate element "Disabled (red) — The reason why the value is not editable", and the sign "the property is animatable" is moved into the interface as a decorator — a dot to the right of the field ("Decorators may appear next to number fields, menus, and checkboxes to indicate the property can be animated"). Animatability is also a flag of the same declaration (`RNA_def_property_clear_flag(prop, PROP_ANIMATABLE)`), not knowledge in the author's head. VEGAS solves exactly the same thing with an icon in the Plug-In Chooser.

The third detail is the honest reverse side, and it is visible only in the source: in one file, `rna_sequencer.cc`, there are 236 calls of `RNA_def_property_ui_text`, and 39 of them have an empty description (`""`). A field in the structure does not force anyone to write text. It makes the gap greppable — and this is the only reason Blender has descriptions at all: they are written by the PROGRAMMER at the moment the property is added, because the field is right next to it, and not by a technical writer a year later from a list.

## Pros

- The description cannot drift from the range and the unit: they are in one line of the declaration. Changing `0.0..1.0` → `0.0..4.0` physically passes by the phrase about "fraction", and this is visible in a one-line diff, not across two files.
- It is written at the moment the parameter is added, by the person who added it. This is not discipline, it is placement: an empty field next to the range is more visible than a task in a tracker.
- One text serves three surfaces: the hint at the field, the help, and search (F-HLP-019 searches effects and settings "by what they do" — the parameter description is exactly the text that search lacks).
- Exactly one phrase is shown per frame — the one under the cursor. The cost of a frame is one lookup in a table, not 63 (see "invariants").
- The mechanism also answers the question "why can't this parameter be automated": the sign is already in the declaration (`ParamKind`), only the phrase next to it is missing.

## Cons

- The field exists — the text does not. At Blender 39 of 236 descriptions in one file are empty; for us it will be the same if the key is not mandatory. A mandatory key means 63 values × 5 languages at once, otherwise the build will not come together (F-HLP-033).
- A 160-character phrase explains WHAT the slider changes, but not WHY to touch it. "Why" is the level of the effect, it stays with F-HLP-025, and this note does not solve it.
- A description is bound to a PAIR (effect, parameter), not to a word, and therefore is almost never reused: `softness` on chroma key (0..100, the blur threshold of the key) and `softness` on wipe (0..1, the width of the soft edge) are two different texts under one label. There will be no saving over 52 unique names: we will have to pay for 63.
- The text next to the field is read once, but always takes up space. Rule 13 of the checklist ("do not waste space") is satisfied only if the line is single and shows the hint ONLY for the field that is currently being looked at.

## What users say

About a bare number without a unit — a thread about the Blender remesher, where a user spent a week turning the parameter blindly:

> the thing that i didn’t know is the 0.1 in remesher is 10 cm ( checked and
> realized everything was right in front of me ), when i was testing the tool i
> was wondering about it.
> — erickBlender, blenderartists.org, 03.11.2019,
> https://blenderartists.org/t/the-big-blender-sculpt-mode-thread-part-1/1150731/2922

And the reply, in which both the cure and the reason it did not work are named:

> So this should be kind of obvious, no? Maybe it should be explained in the
> tooltip if someone doesn’t immediately get it or something.
> — Piotr_Adamowicz, ibid., 03.11.2019,
> https://blenderartists.org/t/the-big-blender-sculpt-mode-thread-part-1/1150731/2923

Our owner complains about the same thing, only in other words — R.38 item 16 (`docs/user-feedback.md`):

> a feeling that effects have some separate "default" state of values

The analysis of this complaint is important, because a parameter description does NOT cure it. Its cause is mechanical and was already found in R.38: a field with a live curve was editing the BASE of the effect, but the engine took the curve's value, so the base was invisible and surfaced when the curve was removed. It was fixed by making the field edit exactly what it shows (`widgets.rs`, `bound_value`/`bound_apply`). The complaint is valuable for something else: it shows that the user does not understand the SEMANTICS of the field and, in the absence of an explanation, builds a hypothesis himself — and the hypothesis was wrong, although the defect did exist. From the same series R.33 item 5, "it is not clear why we work in normalized time" — there the answer was not a hint but a change of the key's domain to the source material and showing seconds first. The conclusion for this note: a parameter description cures a misunderstanding of the UNIT AND RANGE; a misunderstanding of the fact that the field shows something other than what it edits is cured only by changing the behavior.

## How it fits Kinotrain

### What already exists

The infrastructure for a parameter description is in place in full; one field is missing.

- **A machine-readable spec for EVERY parameter.** `ParamSpec` in `src/libs/kt-model/src/project.rs` — `name/min/max/default/step/unit/kind`; declared by the macros `params_for!` (FxInstance, TransitionKind) and `params_for_struct!` (CameraParams). In total **78 declarations** (51 for effects, 20 for transition kinds, 7 for the camera) under **52 unique names**. There is no "description" field in the structure.
- **One drawing point.** `src/apps/kt-app/src/widgets.rs` declares as a rule: "a numeric effect parameter is drawn ONLY by these functions, and the range, step and unit are taken from `ParamSpec`". `params_grid_bound` → `param_row_bound` is the single path; it is used by the FX window (`fx_editor.rs:702`), the transition window (`transition_editor.rs:242`), the camera window (`camera_editor.rs:90`) and both sections of the morpher (`morpher_editor.rs:487`). One fix of the widget covers all the windows at once.
- **A ready wire for the hint from the windows.** `hint_into(out, r, text)` fills `hover_out`, the FX window puts it into `hover_hint` (`fx_editor.rs:323`), and `draw_status_hint` is drawn later (`app.rs:1032` against `app.rs:1162`), so the hint from the window reaches the status bar BEFORE it. The effect chain strip uses this (`widgets.rs:388-540`), while the parameter grid — **never**.
- **Effect descriptions are already written and translated** — `site/content/*.toml`, section `[fxdesc]`, ten video FX in five languages. The word `fxdesc` does not occur in `src/` at all: the translation is paid for and lies in the wrong program. This is a neighboring surface (F-HLP-025), not this one.
- **A precedent for a permanent explanatory line in the body of a window** already exists: `[trn] audio_note` — "The sound always crossfades — only its curve changes" — hangs in the transition window always, not on hover, and covers nothing.
- **Explanations that leaked into labels** are a sure sign of a missing field: `[fxp] radius_y = "Blur Y (0 = as X):"`, `[fx] keyframes_n = "Keyframes: {0} (empty = a 1.0 to 1.35 push as in v1)"`, `[fx] rate_linked`. The description text is already being written, just inside the label, where it is cramped and where it eats into the field's width budget (rule 7).

### What is missing

Three gaps, and all three are counted from the code, not by eye.

1. **The unit explains itself only for a minority.** `unit_suffix` (`widgets.rs:19`) prints a suffix only for `Db`, `Ms`, `Deg`, `Px` — that is 26 declarations out of 78. `Norm` (22), `Rgb8` (15), `Ratio` (9), `None` (4), `Count` (2) have no suffix at all. Fifteen `Rgb8` do not get into the grid (the color picker draws them), so on screen there are **63 numeric fields, of which 37 are a bare number**: `amount`, `softness`, `threshold`, `mix`, `damp`, `decay`, `spill`, `bleed`, `jitter`, `width`, `anchor_x/y`, `strength`, `turns`, `blur`, `rate`, `zoom`, `scale`, `time_scale`, `wet_gain`, `tolerance`, `cols`, `rows`.
2. **One label — up to seven different quantities.** `param_label` (`widgets.rs:32`) displays the NAME of the field, not the pair (effect, name). Of the 14 names that occur for more than one variant, five mean different things: `amount` — seven variants and three units (LayerMix/Noise/Glitch 0..1 Norm, Zoom/Spin 0..4 Ratio, BlurDissolve 0..64 px, Pixelate 1..256 px); `softness` — 0..100 without a unit for chroma key against 0..1 for five wipes; `threshold` — 1..255 for Edges against 0..255 for Glow; `strength` — 0..2 Ratio for Glow against 0..1 Norm for Flash; `radius` — 1..48 for Blur against 1..32 for Glow. On screen all of this is one word and a bare number.
3. **Nobody said why some parameters are automated and others are not.** `ParamKind::Structural` is set on exactly two declarations out of 78 (`reverb_delay_ms`, `predelay_ms`): changing the length of the comb lines rebuilds the reverb. `auto_params()` filters them out, and in the "+ curve" menu (`fx_editor.rs:394-415`) they are simply ABSENT — no line, no icon, no reason. At Blender this is shown by a decorator and explained by a red tooltip line; at VEGAS — by an icon in the Plug-In Chooser; at ours — not at all.

### What gets in the way

- **kt-model has no right to know about language.** The comment on `ParamUnit` says it directly: "the translation of a label lives in the kt-app catalogs: kt-model knows nothing about the interface language". So we cannot literally repeat Blender: only a catalog KEY can go into `params_for!`, and the five translations will stay in `src/apps/kt-app/i18n/*.toml`. Blender has no such fork, because there the English string is in the declaration and is the source of the translation. The property "description next to the range" is still preserved: next to the range lies the address of the text, not the text, and an empty address is just as visible in a diff.
- **A second manual table already exists and already violates "one place".** `param_label` is a hand-written `match` with 46 arms and a silent fallback to `other => other.to_string()`. Today all drawn parameters are covered, but a new parameter arrives in the UI with its snake_case name and the build does not fail. A description added the same way would become a THIRD place to remember — which is exactly what `params_for!` was introduced against ("previously they were described by three manual tables… now all three are generated from one declaration").
- **The parameter grid has four columns of "label/value" pairs.** A description line under each pair cannot be inserted into it without breaking the layout and violating rules 7 and 13. So the surface is one permanent line per window, not a line per parameter.
- **A key per pair (effect, parameter), not per name**, makes the catalog more expensive: 63 values instead of 52, with no saving on matching words.

### Which invariants it touches

- **Invariant 12** (repaint only on movement, no allocations or clones per frame on the draw paths) — is touched, and in a good way, if the description is shown ONLY for the field under the cursor: this is one `tr()` per frame. A description line under each of the 63 fields would mean 63 extra lookups and 63 allocations. It is also worth noting that `param_label` already returns a `String` (an allocation per string per frame); this path must not be made worse.
- **Rule 10** ("exactly one hint, in one place") — is directly affected and requires a decision: the parameter field gets a second hint in addition to the status bar line. The answer: there remains ONE hint at a time, only the FX window has its own line instead of the shared one — the same technique as `[trn] audio_note`. The ban on pop-ups is not touched: no new `Order::Tooltip` layers appear, the guard **t83** (`layout_tests.rs`) stays green without a single edit, `theme.rs` does not change.
- **Rule 9** ("do not make obvious labels") — the boundary runs along the unit: for `radius: … Px` the suffix " px" already says everything, and a description there would be noise. The candidates for a description are those same 37 bare numbers, not all 63.
- **Rules 7 and 13** — the line is one, truncated by the window width, and room for it is reserved only if the window is wider than the minimum.
- New keys automatically fall under **t125** (no label bypasses the catalog, an extra key fails the build), **t100** (a glyph for each non-ASCII character, rebuild of the subsets) and **t128** (the label must fit the budget); the 160-character ceiling from `src/apps/kt-app/build.rs` applies to any string — the cost is not recalculated, see F-HLP-033.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages of the round |
| Where the main work is | kt-model (the description key in `params_for!`/`params_for_struct!` and `ParamSpec`), kt-app (`widgets.rs` — a line in the window body; 63 values × 5 languages in `i18n/*.toml`) |
| Project format | does not touch it: the description is a property of the code, it does not get into `.otio` |
| Tests that will have to be written | `reach_tests::every_param_row_explains_its_number`; t125/t100/t128 come along by themselves together with the keys; a check that "the description line does not shift the grid and is truncated" in `layout_tests.rs` |
| Depends on | F-HLP-033 (the cost of a line in five languages), F-HLP-019 (the same text must be searchable), F-HLP-025 (the description of the EFFECT — a neighboring line of the same window) |
| Breaks compatibility | no |

## Verdict

We take it. This is the cheapest item of the wave per unit of benefit: the payload is one phrase, the surface is a window the user came to deliberately and where there is already a precedent of a permanent explanatory line, and the machine-readable spec for it has existed since R.33 and covers 100 % of the program's numeric fields. The single substantive decision of the round is the key per PAIR (effect, parameter), not per name: without it "Amount" would remain one word for seven different quantities.

TEST: reach_tests::every_param_row_explains_its_number — today it fails on three independent grounds, and all three are visible by reading the files: `ParamSpec` (`src/libs/kt-model/src/project.rs`) has no description field in any of the 78 declarations; `unit_suffix` (`src/apps/kt-app/src/widgets.rs`) returns an empty suffix for `Norm`/`Ratio`/`None`/`Count`, because of which 37 of the 63 drawn fields are a bare number without a unit anywhere; `param_label` is keyed by NAME, and five labels (`amount`, `softness`, `threshold`, `strength`, `radius`) cover 2–7 different quantities each with different ranges and units. The grid `params_grid_bound` does not call either `hint` or `hint_into`, so hovering over a parameter gives NOTHING at all today.

ONE LINE: overlap — does not touch | latency — does not touch | capacity — one phrase ≤160 characters per PAIR (effect, parameter), declared next to the range in `params_for!`, and shown not in the status bar but in the body of the already open FX/transition/camera/morpher window — as one permanent line under the parameter grid, above the "Done/Cancel" buttons, for exactly the field the cursor is on.

## Sources

- Blender 5.2 Manual, Help System — tooltip elements (Short Description, Shortcut, Value, Disabled (red), Python) and F1 → Online Manual:
  https://docs.blender.org/manual/en/5.2/getting_started/help.html
  (accessed 2026-09-07)
- Blender 5.2 Manual, Decorators — "indicate the property can be animated":
  https://docs.blender.org/manual/en/latest/interface/controls/buttons/decorators.html
  (accessed 2026-09-07)
- Blender Python API, `GaussianBlurStrip.size_x` — a description with the range and default attached:
  https://docs.blender.org/api/current/bpy.types.GaussianBlurStrip.html
  (accessed 2026-09-07)
- The source of the declaration of the same property, `RNA_def_property_ui_text` next to `RNA_def_property_ui_range`; there also the 236 calls and 39 empty descriptions were counted:
  https://raw.githubusercontent.com/blender/blender/main/source/blender/makesrna/intern/rna_sequencer.cc
  (accessed 2026-09-07)
- blenderartists.org, thread "The big Blender Sculpt Mode thread", posts 2922 and 2923 — both quotes of the section "What users say":
  https://blenderartists.org/t/the-big-blender-sculpt-mode-thread-part-1/1150731/2922
  (accessed 2026-09-07)
- OFX, `ofxParam.h` — `kOfxParamPropHint`, "A hint to the user as to how the parameter is to be used":
  https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxParam.h
  (accessed 2026-09-07)
- DaVinci Resolve 21 Reference Manual (July 2026), a search over all mentions of tooltips: the node name, "Edited", the frame delta while dragging, the level when trimming automation — Inspector parameters are not among them:
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
  (accessed 2026-09-07)
- After Effects, General user interface items — Show Tool Tips and the Info panel ("information about items under the pointer"):
  https://helpx.adobe.com/after-effects/desktop/get-started/get-familiar-with-the-interface/general-user-interface-items.html
  (accessed 2026-09-07)
- VEGAS: the phrase "For more information about using specific plug-ins, click the Plug-In Help button" and "Effects that can be automated are indicated by the icon in the Plug-In Chooser" — the online help of VEGAS Pro (topic "Audio plug-ins", a local copy from the R.40 reconnaissance; the canonical URL was not saved in the dump); the same phrase verbatim is in the VEGAS Pro 13 manual in the Video FX section. VEGAS 2026 was not checked live.
- A live check for programs outside the core is not assumed (the rule "Software outside the core" in `docs/features/_editors.md`): Blender and OFX were analyzed from documentation and sources.
- Our side was counted from the code on 2026-09-07: `src/libs/kt-model/src/project.rs` (78 declarations, 52 names, 2 `Structural`), `src/apps/kt-app/src/widgets.rs` (`unit_suffix`, `param_label`, `params_grid_bound`), `src/apps/kt-app/src/fx_editor.rs`, `src/apps/kt-app/i18n/en.toml`, `site/content/en.toml`.
