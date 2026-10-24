---
id: F-AUT-004
title: Effect presets that carry animation
status: done
priority: B
cost: M
category: automation/presets
editors: [premiere-26, vegas-pro-2026, resolve-20, fcp-12, after-effects-26]
we_have: yes
crates: [kt-model, kt-app, kt-otio]
invariants: [3, 11]
ui_rules: [4, 17, 20]
round: "R.48"
point: 345
tests: [344, 345, 346, 347]
depends_on: [F-AUT-002, F-MPH-008]
checked: 2026-09-14
---

# F-AUT-004 — Effect presets that carry animation

## Essence

The editor once built an effect with keyframes — a push over the whole length of the frame, a
six-frame tremor at a cut, a fade toward the end — and wants to put this on forty other clips
without repeating the gesture forty times. He gives the setting a name, it appears in the same
list from which the standard effects come, and it survives closing the project. The question is
not "were the sliders saved" but "was the ANIMATION saved" — and what it will do to a clip that
has a different length.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | Effect Preset (Save Preset…) | right-click an effect in Effect Controls; the dialog requires choosing one of three modes for carrying the animation over — Scale / Anchor to In Point / Anchor to Out Point; everything is stored in a single profile file `Effect Presets and Custom Items.prfpset` |
| VEGAS Pro | 2026 | Preset + FX Package (a plug-in chain) | a plug-in preset is a snapshot of ONE set of values, it carries no keys; keys travel only through an FX Package ("Save As" in the Plug-In Chooser) — an `.sfv` file in the packages directory, together with the order of the chain; there is no policy for a foreign duration at all |
| DaVinci Resolve | 20 | Still / PowerGrade in the Gallery | a still carries the whole tree of nodes; the mode is chosen NOT when saving but when applying — right-click on the Gallery background → Apply Grades Using: Keyframes Aligning Source Timecode / Keyframes Aligning Start Frames / No Keyframes. None of the three has stretching |
| Final Cut Pro | 12 | Effects Preset (Save Effects Preset) | checkboxes mark which attributes go in; when keys are present — a switch Stretch to Fit (default) / Maintain Timing; the files live in `~/Library/Application Support/ProApps/Effects Presets/` and appear in the effects browser alongside the standard ones |
| After Effects | 26 | Animation Preset `.ffx` | packs effects, values, keys AND expressions, but for ONE layer only; it is applied to the layer from the current position of the time pointer — the layer's duration is neither asked nor taken into account |

## The trick behind it

**1. A preset is not "keys" but "keys PLUS a policy for a foreign duration", and the editors
differ in WHERE that policy lives.** Adobe and Apple bake it in at the moment of SAVING: in
Premiere there are three mutually exclusive modes (Scale, Anchor to In Point, Anchor to Out Point)
— you cannot pick both anchors at once, and this is not an oversight but a consequence of the
mode being a property of the preset, not of the application. Resolve did the opposite: a still in
the Gallery always carries its keys as they are, and the decision is made at the moment of
APPLYING, through the item "Apply Grades Using" — by the source timecode (for the same material
and its duplicates) or by the start of the event (for a foreign frame). The practical difference:
at Adobe a library of a hundred presets stores a hundred baked-in decisions, and "the same preset,
but differently" is a second preset; at Resolve one decision applies to everything applied after
it, and a forgotten switch quietly spoils the next twenty clips.

**2. Stretching an animation is possible, but correct for only half of the presets, and a third
variant does not exist.** Take a preset built on a 4-second clip: two seconds of movement from the
start. We place it on a clip that is half as long. _Scale_ — the movement is compressed into one
second: the values are intact, the tempo doubles. For "a push over the whole length" this is
exactly right; for "a six-frame tremor" it is a defect: the tremor becomes three frames and turns
into a click. _Anchor to In Point_ — the distance from the in point to the first key is kept, the
movement is an honest two seconds, but the clip ends in the middle of the gesture: the final value
is never seen by the viewer. _Anchor to Out Point_ — the mirror: the keys are counted from the out
point and move BEFORE the in point, the clip opens already in the middle of the movement. None of
the three is always right, because one and the same data type holds two different meanings: "the
animation over the WHOLE LENGTH of the event" and "a gesture of fixed duration at the edge".
The keys alone cannot tell them apart — only the author's intention can. Therefore "just copy the
keys" is not a solution: a bare set of keys is silent about what to do with a foreign length, and
any single default is wrong in exactly half of the cases. FCP admits this more honestly than the
others: the default is Stretch to Fit, but the Maintain Timing switch sits next to it in the same
window.

**3. Vegas shows what the absence of such a policy costs.** There a plug-in preset physically cannot
hold more than one set of values, and keys travel only as a "package" — with the same button that
saves the whole chain. The package puts the keys in as they are, with no tie to length, and users
for twenty years have been getting around this by saving a fragment of the project to a separate
`.veg` file and using "Paste Event Attributes". A side effect: the package is the only way, and so
in Vegas the "animation library" and the "chain library" are one and the same list, from which one
effect cannot be extracted.

**4. AE's preset carries EXPRESSIONS and does not think about duration at all.** The `.ffx` packs
effects, values, keys and expressions — but only for one layer, and it lands from the time pointer.
A layer in AE is not a "piece of material", its length is a property of the layer rather than of the
preset, so the question "what to do with a foreign length" is simply not asked there: the fitting is
handled by an expression that the author of the preset wrote himself. This is the most powerful and
the least portable of the five variants.

## Pros

- A policy baked in at the moment of saving (Premiere, FCP) makes the preset self-sufficient: the file
  travels to another machine and behaves there the same way, with no "and which mode did you have set
  for applying".
- A policy chosen at the moment of applying (Resolve) lets one still cover two scenarios — returning a
  grade to the same material by its source timecode, and transferring it to a foreign frame by the start
  of the event; there is no need to keep two stills.
- A named library puts user animation into THE SAME list as the standard effects (FCP — into the effects
  browser; AE — into Effects & Presets as subfolders-categories): search, categories and the way of
  applying do not split into "our own" and "foreign".
- A preset as a separate file on disk (`.ffx` for AE, `.sfv` for Vegas, the `Effects Presets` directory for
  FCP), not an entry inside the project, is backed up, versioned and sold: an entire industry of ready-made
  packs grew around `.ffx`, which would not have happened around an entry inside the project.

## Cons

- The three Premiere modes are mutually exclusive: "anchor to the in point AND to the out point" — that is,
  "hold the edges, stretch the middle" — is not expressible at all, although that is exactly what is asked
  for titles and backgrounds.
- A monolithic store punishes updates: at Premiere the whole user library is one file, `Effect Presets and
  Custom Items.prfpset`, in the profile of the version, and a move to a new version (or syncing the catalogue
  via a cloud) regularly leaves people without their presets entirely, not one at a time.
- A decision at the moment of applying (Resolve) is a hidden global state: the switch lives in the context menu
  of the Gallery background, nothing in the interface reminds that it is set to "No Keyframes", and the error
  is discovered on viewing, not on applying.
- A preset with stretching breaks everything that must run in real time: a stretched "tremor" stops being a
  tremor, a stretched strobe changes its period. Editors do not distinguish parameters that can be stretched in
  time from those that have their own period.
- At Vegas the keys are not separated from the chain: to take one animated effect from the library, one has to
  drag in the whole package and clean out the excess.

## What users say

> preset doesnt do that… it only copies 1 position, not the whole animation
> — Graham Bernard, Creative COW (VEGAS Pro), 04.03.2017,
> https://creativecow.net/forums/thread/how-to-save-preset-for-crop-animationae/

> I don't want to select "Scale" type since it changes the duration between
> keyframes if the duration of the target clip is different.
> — Madaleine_tr, Adobe Community, 24.11.2022,
> https://community.adobe.com/t5/premiere-pro-discussions/how-to-save-a-preset-with-both-anchor-to-in-point-and-anchor-to-out-point-selected/td-p/13370740

> Scale and Anchor in will set key frames in the right spots. But Anchoring out
> or Scaled key frames on the end just simply don't appear.
> — m0gi, Adobe Community, 29.06.2023,
> https://community.adobe.com/t5/premiere-pro-discussions/subtitle-presets-are-not-keyframing-correclty/td-p/13900755

> all custom effects presets are gone and my custom bins are gone as well.
> — Anthony Paglia, Creative COW (Premiere Pro), 14.07.2014,
> https://creativecow.net/forums/thread/custom-effects-presets-disappeared/

## How it fits Kinotrain
CAPABILITY: openspec/specs/automation/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
A preset carries curves; inserting a chain into a morpher and into a clip is one command, `append_chain_block`; the preset store is the user's file, checked against the registry.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages: a store of named blocks, a carry-over mode, a place in the picker |
| Where the main work is | kt-model (curve operation + a mode field in the block), kt-app (saving under a name, the picker, the mode-selection dialog) |
| Project format | does not touch the PROJECT: the preset lives outside `.otio`. But it introduces a new store at the application level — the question is open |
| Tests to write | round-trip of a block with a mode in kt-model; a table "preset on a clip half as long / longer" for the three modes (values and key positions are exact); a harness test of the picker with a user record; a regression for the morpher defect (curves get lost) |
| Depends on | F-AUT-002 (key domain), F-MPH-008 (loss of envelopes in the morpher) |
| Breaks compatibility | no |

## Verdict

**We take it, but not first.** Half of the mechanics already stands: the blocks in `ops.rs` are
serializable, `rebase` is a ready "Anchor to In Point", and `fx_registry` is a ready place for named
records. Three things are missing, and all three are separate decisions, not code: how to stretch a
curve (F-AUT-002), where to keep the library, and what to do about the silent loss of envelopes in the
morpher (F-MPH-008). Starting the library before F-MPH-008 is closed is not allowed: it would turn an
isolated loss of curves into a systematic one.

## Sources

- Effect presets in Premiere, the three carry-over modes (Scale / Anchor to In Point /
  Anchor to Out Point): https://www.premiumbeat.com/blog/create-custom-effect-presets-in-premiere-pro/
  (accessed 2026-09-07)
- Mutually exclusive anchors, a thread with an answer from a Community Expert:
  https://community.adobe.com/t5/premiere-pro-discussions/how-to-save-a-preset-with-both-anchor-to-in-point-and-anchor-to-out-point-selected/td-p/13370740
  (accessed 2026-09-07)
- Keys of a preset do not appear with Anchor to Out / Scale:
  https://community.adobe.com/t5/premiere-pro-discussions/subtitle-presets-are-not-keyframing-correclty/td-p/13900755
  (accessed 2026-09-07)
- "Scale Attribute Times" in Paste Attributes — a related Premiere mechanism:
  https://community.adobe.com/t5/premiere-pro-discussions/copy-and-paste-effect-relatively-to-clip-length/m-p/15079185/highlight/true
  (accessed 2026-09-07)
- Premiere's preset store (`Effect Presets and Custom Items.prfpset`) and their disappearance:
  https://creativecow.net/forums/thread/custom-effects-presets-disappeared/
  (accessed 2026-09-07)
- Vegas: a preset is a snapshot of one set of values, keys only as a package:
  https://creativecow.net/forums/thread/how-to-save-preset-for-crop-animationae/
  and https://creativecow.net/forums/thread/saving-multiple-keyframes-in-one-preset/
  (accessed 2026-09-07)
- Vegas, help on FX packages (the contents of a package; the help is silent about keys — the
  behaviour is known only from the forum):
  https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/creating_fx_packages.htm
  (accessed 2026-09-07)
- FCP: Stretch to Fit / Maintain Timing and the `Effects Presets` directory:
  https://support.apple.com/guide/final-cut-pro/save-video-effects-presets-ver462c2078f/mac
  (accessed 2026-09-07)
- After Effects: the contents of `.ffx` (effects, values, keys, expressions; one layer):
  https://helpx.adobe.com/after-effects/using/effects-animation-presets-overview.html
  (accessed 2026-09-07)
- Resolve: "Apply Grades Using" — Source Timecode / Start Frames / No Keyframes:
  https://davinciresolveclub.com/copy-color-grade-davinci-resolve/
  (accessed 2026-09-07)
- Resolve: PowerGrade as a static grade without keys and tracking:
  https://pixeltoolspost.com/blogs/resolve/powergrades (accessed 2026-09-07)
- Not checked live in the editors: Premiere/FCP/AE/Resolve — from the official help and forum
  threads, Vegas — from the help and Creative COW.

**Implemented in R.48** — items 342-346 (t342-t347); the further life of the
feature is recorded there. The carry-over modes other than "in point to in point" (Scale,
Anchor-Out) remain with F-AUT-002 — the round did only the one that was already the canonical
mechanic of copy-paste.
