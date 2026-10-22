---
id: F-AUT-008
title: Linked parameters: one control moves a group
status: done
priority: B
cost: L
category: automation/keyframes
editors: [after-effects-26, fcp-12, resolve-21, premiere-26, vegas-pro-2026]
we_have: no
crates: [kt-model, kt-engine, kt-app]
invariants: [3, 7, 11]
ui_rules: [17, 20]
round: R.61
point: 463
tests: [t702, t702b, t703, t704, t705, t706]
depends_on: [F-AUT-007]
checked: 2026-09-22
---

# F-AUT-008 — Linked parameters: one control moves a group

## Essence

The editor does not want to turn eight sliders in three windows for one change. He wants ONE control — "intensity", "style variant", "day/night" — behind which stands a bundle of real parameters, and they must move in concert. The task is the same as that of expressions (F-AUT-007), but without a language: the driven parameters are listed in advance, and the control either recomputes them with a number or switches between prepared states.

## Who has it and how

The question for each one: can one handle be made, behind which several foreign parameters stand, and what can it do — only numbers, or whole states.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Final Cut Pro | 12.3 | **Rigs** (made in Motion, published in FCP) | the strongest line of the note. In Motion a "rig" is created, and widgets of three kinds are put into it — slider, pop-up menu, checkbox. A widget has SNAPSHOTS: the state of many parameters of many objects, recorded whole. The slider interpolates between snapshots, the menu switches them with a jump. Published widgets arrive in the Final Cut inspector, and the editor sees one handle instead of thirty. The official formulation of the goal is to restrict control, "so that junior compositors, editors and others in the pipeline stay within approved specifications" |
| After Effects | 26.x | pick-whip, parent/child, Essential Graphics | the "whip" is dragged from property to property and writes a link expression; in Essential Graphics a property is PULLED OUT onto the template panel under its own name. So the link in AE is a special case of an expression and inherits its price (F-AUT-007): one source can drive any number of driven properties, but each link is set with a separate gesture |
| DaVinci Resolve | 21 | Macro (Macro Editor on the Fusion page) | a group of nodes is selected, `Create Macro`, and in the window checkboxes mark which node inputs to EXPOSE OUTWARD. One exposed input can be mapped to several inputs inside — this yields exactly "one handle, many parameters". It lives only on the Fusion page and only for the node graph; on the editing page there is no such mechanism |
| Adobe Premiere | 26.x | Master Properties of a nested sequence | the closest, and not quite it: the Master Clip parameters of a nested sequence give an edit "on all instances at once" — one edit spreads across the copies, but it does not create a new, OWN control and does not combine several different parameters under one handle |
| VEGAS Pro | 2026.0 | none | no parameter linking and no public handles. The only way to move a bundle is a .NET script that walks through the events and sets values (F-EXT-002), that is, a one-off action, not a live link |

## The trick behind it

**1. A rig is the CHEAP HALF of expressions, and it has not a single expensive part.** A link has no language, so there is no interpreter, no version of it, and no question of "what this code can do when the file is opened" — the very reason Blender keeps `Auto Run Python Scripts` off (F-AUT-007). Nor is there the main risk to invariant 11: the list of driven parameters and the coefficients are data, and the computation reduces to linear interpolation, which we already have and which is already byte-for-byte. At the same time a large part of what expressions are created for is covered: "one handle — many parameters", "a parameter named in a human word", "a template that does not let you reach inside". This is how the order of implementation should be viewed: first the link, then the formula, because a formula without a recipient is meaningless, while a link without a formula is not.

**2. The FCP widget switches STATES, not numbers, and therefore takes what a formula cannot.** A snapshot is the values of many parameters of many objects at once, including non-numeric ones: a font, an alignment variant, the enabled state of an effect. The slider between two snapshots interpolates what can be interpolated and switches in steps what cannot; the menu simply selects a snapshot. From this a non-obvious consequence follows: the rig's axis is NOT time but a value, and it answers a question that neither a parameter's name nor its description answers — "how much do they usually set here". We have already applied the same trick in the effect gallery (F-AUT-006, R.54): the strength axis of a cell moves between two curated variants, not between "zero and maximum". A rig is the same mechanism, moved out of the picker onto a permanent control.

**3. The cost of a link shows not in how it is set, but in how many times it is set.** In AE a link is set one at a time: "I know you can parent several layers at once, but if I want to pick-whip several color controls to a master controller, I have to pick-whip them one by one" (rossigoat, 04.10.2023). FCP has the opposite extreme: a rig is made once and reliably, but setting a snapshot to an EXACT value is a known pain, because the snapshot is set by a double-click under the slider ("hard to set the value precisely" — Alex4D, 08.08.2017; before version 5.3 exact values were edited as text in the file). Both symptoms point to the same thing: the mechanism of linking is almost always built on the existing editing interface, without giving it its own, and this is a mistake that becomes visible only at the second hundred links.


## Pros

- A link is data, not code: it is serialized, compared, copied, and does not depend on any runtime. Invariant 11 is not touched at all.
- One handle covers the case for which people most often reach for expressions, and covers it without a language — so the feature arrives sooner and cheaper.
- The driven parameters are enumerated in advance, so a cycle is caught by structure rather than by text analysis, and "what will break if this is moved" is answered by a list.
- A widget with snapshots takes NON-NUMERIC parameters — a flag, a choice, text, a reference to media — which a formula over numbers does not touch at all.
- A rig is a ready answer to "a template you should not reach inside" (F-TL-012): a published parameter driving three internal properties IS a rig.
- A restriction as a feature: Apple's official purpose of a rig is not convenience but a ban on going deeper, and for a template project that is exactly what is needed.

## Cons

- A link is a second place where a value lives. You turn the handle, then go to edit a driven parameter by hand — and either the edit is rejected or the link silently breaks; there is no third option, and both have to be shown.
- A snapshot is fragile to template edits: you add a parameter inside — it is absent from every snapshot, and the rig drives it to its default. This is a silent loss, the reverse side of what people take a rig for.
- It is set one at a time. In AE this has been voiced by a user; a mass link is done by copying the expression, that is, the mechanism is propped up by the very mechanism one wanted to get away from.
- Precision of setting. In FCP a snapshot is set by a gesture, not a number, and this comes up for years as a separate task.
- The control asks for its own name, and the name is text: in a template it is free (the project's content), but everything nearby pays with five languages (F-HLP-033).
- The mechanism lives not where the editing is: in Resolve a macro exists only on Fusion, in FCP a rig is made in an entirely different application. That is, everyone who made the feature took it out of the editor, and this should be understood as a signal of its weight.

## What users say

On the price of a link set one at a time:

> I know you can parent multiple layers all at the same time, but if I
> wanted to pick whip multiple color controls to a master controller, I
> have to pick whip them one by one.
> — rossigoat, Adobe Community (After Effects), 04.10.2023,
> https://community.adobe.com/t5/after-effects-discussions/pick-whipping-multiple-values-at-once/td-p/14133239

On what the rig model pays with — the handle has points, and there is no way to set them precisely:

> The problem is that you add snapshots to slider widgets by
> double-clicking below the slider...but it is hard to set the value
> precisely.
> — Alex4D, 08.08.2017,
> https://alex4d.com/notes/item/apple-motion-tip-setting-precise-widget-snapshot-values

And Apple's own wording — why people build a rig: not for convenience, but for restriction.

> Rigging is especially useful in Final Cut Pro templates, allowing users to
> modify a complex group of parameters with a small set of controls.
> — Motion User Guide, "Intro to rigging in Motion",
> https://support.apple.com/guide/motion/intro-to-rigging-motn13f20610/mac
> (accessed 2026-09-21)

## How it fits Kinotrain
CAPABILITY: openspec/specs/automation/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
A link addresses any event of the reel (it does not cross the reel's boundary), time between events is the timeline, and non-numeric values are driven by snapshots on the value axis.

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round: the control as a row of the passport, the list of driven properties, snapshots, resolution in the plan, display of the driven list |
| Where the main work is | kt-model (the carrier of the control, the list of driven properties, the snapshot, the order "link ↔ curve"), kt-app (the widget of the handle, the driven-marker on a parameter), kt-engine (resolution in `layer_src`) |
| Project format | affected: the control and its driven properties are new data in the `metadata["kt"]` envelope; the version grows together with the public parameters of the composition (F-TL-012), no separate version is needed |
| Tests that will have to be written | one handle moves eight parameters in ONE undo step; the ends of the axis equal the snapshots byte-for-byte (like t550); a driven parameter is not silently edited by hand; the order "link → curve" is the same in preview and export; a project without links is byte-for-byte the same as before |
| Depends on | F-AUT-007 (the common half: addressing a foreign property), F-TL-012 (the carrier of a public parameter), F-AUT-006 (the ready value axis in the interface) |
| Breaks compatibility | not forward; backward yes — together with F-TL-012 |

## Verdict

**A candidate, and it should be assessed immediately AFTER the public parameters of the composition (F-TL-012), not separately.** The reason is simple: a published parameter of a template that drives three internal properties is exactly a rig, and if one introduces it without an answer to the question "one to many or one to one", the answer will have to be redone in the very first real template ("a scrolling line, logos, a news headline", R.59). The reconnaissance adds two practical conclusions. The first: a rig is the cheap half of expressions — it has no language, no interpreter, no security question, and it does not touch invariant 11, because the blending arithmetic is already written by us and already under the golden (`FxInstance::lerp`, t550). The second: for a rig the axis is a VALUE, not time, and it can handle non-numeric things — a snapshot switches a font and a variant just as it moves a number; that is how it takes what a formula over numbers is powerless to do.

The order of implementation follows from this: the link before the formula. A formula without a recipient degenerates into a function of time, that is, into an expensive way of drawing a curve that
`src/apps/kt-app/src/automation.rs` already draws; a link without a formula is useful from the first day.

### Decisions of round R.61

The link is taken, and before the formula, in exactly the order the note prescribed.
The owner's decisions that settle the note's open questions:

* **Any event of the PROJECT can be addressed**, not only one's own. A constraint derived from the invariants: the link does not cross the reel boundary. Outward, because the reel would stop being self-contained and law E (nesting is identical to the expanded content) would break. Inward, because "setting a value from outside, inside" is already done and is called the public parameters of a composition (F-TL-012).
* **Time between different events is timeline tick**: two events in different places of the edit have no shared material time. Conversion goes through the same pair of functions as the whole program; outside the source body the extreme value is taken.
* **Non-numeric values are handled too**: a flag by threshold, a choice, text, media and color, by snapshots at marks on the VALUE axis of the leading property. This confirms the note's main conclusion: for a rig, the axis is value, not time.
* **The cycle is searched by traversal from ALL vertices**, printing the path, using the same technique as the reel cycle check: a pair of properties that lead each other and are read by no one is still a cycle.

### THE DOOR APPEARED (R.67)

Before R.67 the link existed in the model and in the tests, but there was NOTHING to assign it to a person with. The owner wrote: "about view curves, I don't understand at all how to assign/insert them". Now the curve block has an item "Driven by…" (curve events → their properties, by passports); the address is visible in the curve name; "Unlink" returns the keys. Test t788 (`reach_tests.rs`).

## Sources

- https://support.apple.com/guide/motion/intro-to-rigging-motn13f20610/mac —
  what a rig is and why it is used in Final Cut templates (2026-09-21)
- https://support.apple.com/guide/motion/build-a-simple-rig-motn13f2099a/mac
  and https://support.apple.com/guide/motion/slider-widget-motn13f202eb/mac —
  three kinds of widgets, snapshots, interpolation between snapshots (2026-09-21)
- https://support.apple.com/guide/motion/publish-rigs-to-final-cut-pro-motn13f21017/mac
  — publishing widgets to the Final Cut inspector (2026-09-21)
- https://alex4d.com/notes/item/apple-motion-tip-setting-precise-widget-snapshot-values
  — snapshot value precision as a separate pain point, 08.08.2017 (2026-09-21)
- https://community.adobe.com/t5/after-effects-discussions/pick-whipping-multiple-values-at-once/td-p/14133239
  — the link is set one at a time, 04.10.2023 (2026-09-21)
- https://www.premiumbeat.com/blog/pick-whip-adobe-after-effects/ — pick-whip
  as a gesture that writes a reference expression (2026-09-21)
- Resolve, Macro Editor and "expose an input outward": from the analysis of F-EXT-002 and
  the Fusion help; the edit page has no such mechanism — **[NOT VERIFIED]**
  live (2026-09-21)
- Premiere, Master Properties of a nested sequence: from the analysis of F-TL-004
  (nested sequences); no separate check was made in this pass
- Ours: `src/libs/kt-model/src/project.rs:4573` (`FxInstance::lerp`),
  `src/libs/kt-model/tests/fx_lerp.rs` (t550),
  `src/apps/kt-app/src/fx_registry.rs` (strength axis, t551),
  `src/libs/kt-model/src/ops.rs` (chain blocks),
  `src/libs/kt-model/src/model.rs` (`apply_batch`),
  `src/libs/kt-engine/src/video.rs` (`layer_src`),
  `docs/user-feedback.md` section "Round R.59"

No other editor was run live: FCP/Motion from Apple's manual and the note's practice, AE from the Adobe help and forum, Resolve and Premiere from related notes in the base, VEGAS from the absence of the mechanism in the help (checked by searching the sections on envelopes and keyframes).
