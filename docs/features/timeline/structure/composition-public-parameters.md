---
id: F-TL-012
title: Public composition parameters
status: done
priority: A
cost: XL
category: timeline/structure
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-engine, kt-otio, kt-app]
invariants: [3, 4, 7, 11]
ui_rules: [13, 16, 17, 20]
round: R.59, R.60
point: 469
tests: [t671, t672, t673, t674, t675, t676, t677, t678, t679, t680, t681, t682, t683, t684, t685, t686, t687, t688, t689, t690, t691, t692, t693, t694]
depends_on: [F-TL-004, F-TXT-002]
checked: 2026-09-22
---

# F-TL-012 — Public composition parameters

## Essence

An assembled piece of editing — the intro of a news programme, the lower third, a scene with a screen and a chat — receives a LIST OF HANDLES, which the author has declared outwards: the title, the logo, the colour of the plate, whether the running text is on. After that this composition is placed on the timeline ten times, and each instance differs from its neighbour exactly by the values of these handles, not by its content. Nobody climbs inside: editing inside is a separate action, editing from outside is filling in the declared form.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Essential Graphics panel · Master Properties · `.mogrt` export | THE AUTHOR'S SIDE, and the only one of the five where the mechanism is closed on nesting: layer properties are DRAGGED into the panel, there they are renamed, grouped and change the control type. When a composition with such a panel is nested into another, the exposed properties surface on the parent layer as Master Properties — that is, the instance of the nesting receives its OWN values for the declared handles, without touching the content. Protected time zones are marked with composition markers (Responsive Design – Time) |
| Adobe Premiere | 26.x | Motion Graphics Template (`.mogrt`), Essential Graphics | THE CONSUMER'S SIDE. The template is placed as a clip, there is not a single keyframe in Effect Controls — only the handles declared by the author, and keyframes cannot be set on them. On insertion the file is COPIED into the project folder "Motion Graphics Template Media", and each instance thereafter lives on its own. Your own `.mogrt`, made in Premiere itself, remains fully editable — it has no boundary "author ≠ consumer" |
| DaVinci Resolve | 21 | Fusion Macro Editor · `.setting` in `Templates/Edit/Titles` | The author assembles a node composition on the Fusion page and turns it into a macro; the Macro Editor is literally a table of inputs of all nodes with a tick "expose" and a rename field. In 21 the editor gained a control order and a direct "Save to Edit Template" with a choice of Title / Generator / Effect / Transition. The consumer on the Edit page sees the chosen inputs in the Inspector. The editing compound clip has no public parameters at all — they exist only in the node macro |
| Final Cut Pro | 12.3 | Motion templates, the Publish command, rigs | The author works in Motion: Publish exposes a parameter outwards by name. Beyond this — the RIG: one published widget (a slider, a checkbox, a menu) which, through state snapshots, moves a batch of parameters at once, that is, the public handle is not obliged to correspond to an internal property one to one. Some elements cannot be published at all. The Build In / Build Out markers keep the intro and the outro at their original tempo at any clip length |
| VEGAS Pro | 2026.0 build 66 | none | There is no mechanism of "the author declares a set of handles — the consumer fills them in" in any form. A nested timeline is a separate `.veg` file, and the instance has not a single field of its own beyond the ordinary event; titles came in as a Boris FX plugin (Continuum Title Pack) with its own window and its own preset browser, where a preset is a WHOLE composition without a limited public set. Vegas does not read `.mogrt` |

## The trick behind it

**1. The public interface is a SEPARATE layer, and it is assembled by hand, not derived
from the content.** None of the four editors where the mechanism exists has a command
"expose everything interesting": the Essential Graphics panel is a set of LINKS to
layer properties which were dragged into it with the mouse, the Macro Editor is a table of inputs with
a tick beside each, Publish in Motion is a named action on a single
parameter. And in the same place the author does three things, for the sake of which everything is undertaken:
renames ("Position" → "Plate offset"), groups, and SUBSTITUTES THE CONTROL TYPE — a slider with limits is exposed instead of an XY point, so that the consumer
physically cannot drive the title out of frame. Two consequences follow, which are in no list of features. First: the internal structure can be rearranged entirely, and
the public name does not change — the interface is decoupled from the content. Second: the consumer
sees NOT ONE keyframe, and the question "why can't I move this" runs into not a
bug, but the fact that the author did not expose the property. The furthest point of this axis is the
FCP rig: one handle "style: strict / festive" moves colour, thickness, delay and
a curve through state snapshots, that is, the public parameter stops being a projection of the internal one at all.

**2. In all five the instance is a VALUE, not a reference, and this reverses the meaning
of the mechanism.** Premiere on insertion copies the `.mogrt` into the project and thereafter treats
each instance as a separate clip; if the author corrected a typo in the source file —
forty titles in the edited episode did not change, and they have to be fixed one by one
(Alt-drag over the old one, or "Relink Ae MOGRT in place", which appeared only in
12.1). Resolve distributes `.setting` files into `Templates/`, FCP — as template packages,
Vegas — as plugin presets: everywhere a COPY is distributed. So a template in the industry is a
mechanism of DISTRIBUTION (one author → many editors → many copies), and not a
mechanism of distributing an EDIT (one edit → many targets). Exactly here we
diverge from all five: our nesting is a reference to an object inside the project (the owner's decision R.50), an edit of the reel reaches all instances by itself, and
an override on an instance turns out to be not a "copy that was allowed to be touched up", but the ONLY lawful divergence from the reel. The two ends of one axis swap places: for them
the public parameter protects the composition from the consumer, for us it describes what instances are allowed to differ by at all.

**3. Part of the template is declared NOT STRETCHING with time.** The editor grabs the lower third by the edge and makes it twice as long; without markup, together with it the text on the fly would be slowed twice. Therefore the author in AE places a composition marker with the tick "Protected Region"
(Composition → Responsive Design – Time → Create Intro/Outro), and in FCP — the Build In / Build Out markers: the marked pieces go at their original tempo, and only the uncovered middle stretches. There are two non-obvious points here. The markup does NOT COMPOSE — a protected-zone marker on a layer of a nested composition does not update itself after an edit inside. And sound does not enter this mechanism in principle: the time stretch is not applied to it, and a click synchronous with the animation drifts away.

## Pros

- A public interface decoupled from the content makes the composition editable without negotiation: the author goes through the layers, swaps precompositions, repairs masks — the consumer keeps the same three fields with the same names.
- The limits of an exposed parameter are part of the declaration, not decoration: a slider 0..100 instead of a free point makes a whole class of breakages ("drove the title out of frame") physically impossible, and support stops receiving that class of requests.
- A handle that does not correspond to one internal property (the FCP rig) gives the consumer a meaningful choice instead of twenty technical fields — that is, a public set can be designed for the TASK, not for the construction of the composition.
- Protected time zones remove the conflict "the editor needs an arbitrary length of the plate, the author needs an unchanged tempo of the entrance" by marking the head and the tail.
- An instance-copy makes the project self-sufficient: the template file can be deleted from disk, the edited episode will not fall apart and will render.

## Cons

- An instance-copy makes correcting the template useless for material already edited: a fix to the source does not catch up with the inserted copies, and "update the template" becomes a manual operation on each instance.
- Either the whole parameter is exposed outwards, or nothing: there is no intermediate "can be moved, but within these limits and only horizontally" — a limit is expressed ONLY by the choice of control type, and the author fits the meaning to the range of widgets.
- The public set is assembled by hand, so it also goes out of date by hand: a property thrown out of the composition during a rebuild leaves in the panel a handle that does nothing, and the consumer finds out about it.
- A template renders as a standalone clip and does not see the editing around it: adjustment layers and blending modes INSIDE it do not reach the tracks beneath it in Premiere — so a whole class of techniques ("blur the background under the plate") cannot be packed into a template.
- The markup of protected zones does not survive nesting and, by numerous reports, behaves in Premiere differently from AE; sound is not included in it at all.
- The public name is invented by the author — which makes it document content, and it will never get into the program's translation catalogue.

## What users say

The quotes are taken from F-TXT-002 (reconnaissance of 2026-09-07) verbatim, with their own
original links and dates: they were gathered precisely on this subject, and rewriting them in new
words would be forgery. This note did not open any new sources.

> After Effects built mogrts do not show any keyframes in the Effect Controls
> panel. They're fully encapsulated motion graphics - only the controls that
> the artist builds into the Essential Graphics panel are available.
> — Theresa Rostek (Adobe Community Manager), Adobe Community, 12.12.2023,
> https://community.adobe.com/t5/premiere-pro-discussions/mogrts-not-fully-editable-in-24-0-0-and-24-0-1/td-p/14289234

> The answer is generally that the mogrt designer did not allow editing the
> animation.
> — Stan Jones (Community Expert), Adobe Community, 10.12.2023, same thread,
> https://community.adobe.com/t5/premiere-pro-discussions/mogrts-not-fully-editable-in-24-0-0-and-24-0-1/td-p/14289234

> no way to replace the .MOGRT file in order to update changes made and keep
> the dynamic content intact
> — Devyn Brugge (reply marked as the correct one), Adobe Community, 30.10.2017,
> https://community.adobe.com/t5/premiere-pro-discussions/updating-essential-graphics-templates-used-in-premiere/m-p/15017871

> Ae Adjustment Layers are ignored when they are part of an AeMoGRT.
> — Roland R. kahlenberg, Creative COW, 23.01.2024,
> https://creativecow.net/forums/thread/mgrt-created-on-ae-with-an-adjustment-layer/

> Responsive Design Time in MoGRTs produces iffy/maybe/meh results.
> — Roland R. Kahlenberg, Creative COW, 26.09.2025,
> https://creativecow.net/forums/thread/mogrt-responsive-design-time-functions-clarifications/

The first and the third quotes are the two halves of the trick, said by users:
only what has been declared is visible from outside, and the source cannot be fixed after the fact.

## How it fits Kinotrain
CAPABILITY: openspec/specs/compositions/ — the contract is in `spec.md`, the "why" and the paths in the code are in `design.md`.
Public parameters of the reel: the template value is not stored (it is read at the target); the instance overrides only what is published; the overrides lie beneath the morpher layers.

## Estimate

| What | Answer |
|---|---|
| Size | XL — a round plus a format version: a parameter on the reel, an override on the instance, six value types and curves for two of them |
| Where the main work is | kt-model (the public set on `Sequence`, the override on the instance, undo), kt-engine (where a value turns into content and what is then counted as the reel's edit), kt-otio (new fields in `kt.sequences` and in the event envelope), kt-app (the grid of handles on the instance, publishing from the reel's side) |
| Project format | a version IS needed; no new constructs — fields in already existing sections. Backward compatibility was lifted by the owner's permission R.59 |
| Tests to be written | an override reaches the picture and the sound; an instance without overrides is byte-for-byte equal to the reel (a continuation of the E-law, t434-t436); two instances with different values share neither the sub-composite, nor the memo key, nor the warmth strip; a round trip in the format; an edit of the reel reaches all instances, and the override survives it; deleting a published property inside the reel leaves no dead handle outside |
| Depends on | F-TL-004 (nesting — done), F-TXT-002 (reconnaissance of the public interface), the property model of R.59 (without it only a number can be public) |
| Breaks compatibility | no (old files are read; a reel without a public set behaves as it does today) |

## Verdict

**We take it, priority A.** The owner named the subject directly (R.59, item 1: a template project of a news programme, ten instances, a different title and logo each), and without this the compositions remain "collapsed so they don't get in the way" instead of a unit of reuse.

It is cheap for us for two reasons, both already paid for. The first: nesting is done and done as a REFERENCE — the thing for which five editors build file copying and a "relink in place" command, works for us by construction, and the public set only has to be a field of the reel, not an exchange format. The second: a declared set of handles with a name, limits, step and type already exists for our effects (`ParamSpec`, `params_for!`), and the UI must draw a parameter only from it — a public handle falls into the same grid, not into its own window (UI rules 13 and 17).

The difference from all five is one, and it must be named aloud, because it changes the meaning of the word "template". For them an instance is a copy, so the template distributes the CONTENT, and the author's edit does not reach the inserted copies; for us an instance is a reference, so the template distributes the EDIT, and an override turns out to be the only lawful way for an instance to differ from the reel. This is not a protection policy (there is no one to protect against: the editor is single-user, and this is precisely why F-TXT-002 justified its priority C) — it is a structural fact: in the model there is simply no other place to put an instance's difference from the reel.

## Sources

- R.59, `docs/user-feedback.md`, section "Round R.59" — the owner's verbatim statement of the task (a template project of a news programme, OBS scenes) and the decisions: six types of public parameters at once, curves for a number and a colour, names — free author text, the instance overrides ONLY what is published (date 2026-09-20)
- F-TL-004 `timeline/structure/nested-sequences.md` and F-TL-011
  `timeline/structure/inline-nest-expansion.md` — what exactly we have done in R.50/R.55/R.56/R.58 and which nesting laws are already fixed by tests
- F-TXT-002 `titles/text/title-styles-and-templates.md` — reconnaissance of the public interface, protected zones and the instance-copy (2026-09-07). The quotes and links below are taken from there without changes and were NOT re-verified this time
- The AE template shows no keyframes, only the controls exposed by the author are available:
  https://community.adobe.com/t5/premiere-pro-discussions/mogrts-not-fully-editable-in-24-0-0-and-24-0-1/td-p/14289234
  (access date 2026-09-07)
- The template copy in "Motion Graphics Template Media", the instance as a separate clip,
  "Relink Ae MOGRT in place" with Premiere 12.1:
  https://community.adobe.com/t5/premiere-pro-discussions/updating-essential-graphics-templates-used-in-premiere/m-p/15017871
  (access date 2026-09-07)
- An adjustment layer inside a `.mogrt` does not reach the tracks beneath it:
  https://creativecow.net/forums/thread/mgrt-created-on-ae-with-an-adjustment-layer/
  — the stronger claim "they are ignored altogether" was not checked live
  **[NOT VERIFIED]** (access date 2026-09-07)
- Responsive Design – Time, protected zones, "a marker on a layer of a nested composition
  is not updated automatically":
  https://helpx.adobe.com/after-effects/desktop/motion-graphics/add-responsive-design/responsive-design.html
  — **[NOT VERIFIED against the primary source]**, `helpx.adobe.com` does not
  open from this environment; the behaviour is confirmed by the Creative COW threads
  https://creativecow.net/forums/thread/mogrt-responsive-design-time-functions-clarifications/
  (access date 2026-09-07)
- FCP/Motion: publishing parameters and rigs —
  https://support.apple.com/guide/motion/about-publishing-parameters-motn5ba1a1e6/mac
  and https://support.apple.com/guide/motion/intro-to-rigging-motn7ce46f0b/mac; the markers
  Build In / Build Out — https://support.apple.com/guide/motion/intro-to-template-markers-motn141baf86/mac
  (access date 2026-09-07)
- Resolve: Macro Editor (choice and renaming of the exposed inputs), the catalogue
  `Templates/Edit/Titles`, the update of the macro editor in 21:
  https://jayaretv.com/fusion/macro-editor/ and
  https://mixinglight.com/color-grading-tutorials/resolve-fusion-21-macro-editor-update/
  (access date 2026-09-07)
- VEGAS Pro 2026: Continuum Title Pack and Title Studio,
  https://www.vegascreativesoftware.com/vegas-pro/whats-new/ — that Vegas has no
  "author → consumer" mechanism is a conclusion from the absence of such mentions in
  the official materials, not confirmed by a separate primary source
  **[NOT VERIFIED]** (access date 2026-09-07)
- Our code, on which the "How it fits" section is written: `src/libs/kt-model/src/project.rs`
  (`Sequence`, `MediaRef`, `MorphPatch::apply_to`, `effective_reel`, `ParamSpec`,
  `params_for!`), `src/libs/kt-engine/src/video.rs` (`content_hash`, `nests`,
  `nest_scales`, `nest_memo_key`), `src/libs/kt-media/src/index.rs`
  (`reel_strip`/`reel_frame`/`reel_peaks`), `src/libs/kt-media/src/factory.rs`
  (parsing of the `gen:` pseudo-paths) — read 2026-09-21
