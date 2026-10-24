---
id: F-TXT-002
title: Title templates with public parameters
status: researched
priority: C
cost: XL
category: titles/text
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: no
crates: [kt-model, kt-engine, kt-otio, kt-app, kt-media]
invariants: [4, 5, 7, 8, 11]
ui_rules: [13, 16, 17, 20]
round: null
point: null
tests: []
depends_on: [F-TXT-001, F-AUT-004, F-TL-012]
checked: 2026-09-21
---

# F-TXT-002 — Title templates with public parameters

## Essence

One person (the designer) assembles a title in another program, and another (the editor)
places it on the timeline and edits ONLY what the designer permitted: the text, the color, the duration, sometimes a picture in the backdrop. Everything else — the order of layers, masks,
animation keys — is not visible to the editor and not available to them. This is not a "pretty preset" but a
mechanism for separating two roles in time: the author decides once what may be touched, and after that a hundred editors cannot break the composition, even if they really want to.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Essential Graphics panel → `.mogrt` export | THE AUTHOR'S SIDE. Layer properties are DRAGGED into the panel, there they are renamed, grouped and given limits (a slider instead of a point — so that the consumer cannot move the object out of the frame). When a composition is nested, the exposed properties surface on the parent layer as Master Properties. Protected time zones are marked with composition markers (Responsive Design – Time) |
| Adobe Premiere | 26.x | Motion Graphics Template (`.mogrt`), Essential Graphics | THE CONSUMER'S SIDE. The template is placed as a clip; in Effect Controls there are no keyframes at all — only the handles exposed by the author. On insertion the file is COPIED into the project folder "Motion Graphics Template Media". Premiere also has the authoring side — but a `.mogrt` made in Premiere itself remains fully parseable, it has no boundary "author ≠ consumer" |
| DaVinci Resolve | 21 | Fusion Titles, `.setting` macros in `Templates/Edit/Titles` | The author assembles a node composition on the Fusion page and turns it into a macro; the Macro Editor chooses which node inputs come out and renames them. In 21 the macro editor gained the order of controls and a direct "Save to Edit Template" with a choice of Title / Generator / Effect / Transition. The consumer on the Edit page sees the chosen inputs in the Inspector |
| Final Cut Pro | 12.3 | Motion templates (Titles / Generators / Effects / Transitions) | The author works in Motion: the Publish command exposes a parameter; one can publish a whole RIG — a single widget (slider, checkbox, menu) that moves many parameters at once via state snapshots. The Build In / Build Out markers keep the intro and outro at their original pace at any clip length. Some elements cannot be published at all |
| VEGAS Pro | 2026 | no "author → consumer" system of its own; Continuum Title Pack (Title Studio) | Titles came from Boris FX as a plugin with its own window, its own 3D engine and its own preset browser ("broadcast-ready templates"). A preset is a WHOLE composition: there is no limited public set of handles, the consumer opens Title Studio and changes anything. Vegas does not read `.mogrt` |

## The trick behind it

**1. A template's public interface is a separate layer of abstraction, and it is ASSEMBLED BY HAND, not derived from the content.** In AE the Essential
Graphics panel is not a list of checkboxes "what to show" but a set of REFERENCES to layer properties,
which were dragged there with the mouse: there they are also renamed ("Position" →
"Backdrop shift"), grouped and — most importantly — the control type is replaced, because the author exposes a slider with limits instead of a point, so that the consumer cannot
move the text out of the frame. After that the internal structure can be rearranged entirely, and the public name will not change. The reverse side is that the consumer does not see A SINGLE
keyframe: a template from AE shows emptiness in Effect Controls, and "why can't I move this" runs into not a bug but the fact that the author did not expose the property. In Resolve the same principle lives on nodes (Macro Editor: a "publish" checkbox + renaming of the input), in FCP — the Publish command, and there is also what nobody else has: a RIG, where one published widget moves a bunch of parameters through state snapshots.

**2. A template has "protected zones" of TIME, and only what lies between them is stretched.** The editor places a lower third and drags it by the edge to the needed length. Without markup this would stretch the whole animation: the text's entry would slow down twice. Therefore the author in AE places a composition marker with the "Protected Region" checkbox (or
Composition → Responsive Design – Time → Create Intro/Outro), and when the duration changes these pieces go at their original pace, while ONLY the uncovered middle is stretched. FCP does the same with the Build In / Build Out markers, and each one has a "required" variant and a "by consumer's checkbox" variant. Three non-obvious consequences. The first: the markup does not compose — "if you change the protected zone AFTER the composition is nested, the protected-region marker on the nested composition's layer will not update automatically" (AE help). The second: the mechanism is designed for the head and tail, not for the middle — an attempt to stretch an animated middle also produces, as a side effect, a drift of the first frame. The third and nastiest: SOUND inside a template does not survive in this scheme — time stretching is not applied to it, and a click synchronized with the animation drifts away.

**3. An instance of a template is a VALUE, not a reference, and this turns the whole economy upside down.** On insertion Premiere copies the `.mogrt` into the project (the folder "Motion Graphics Template Media") and from then on "treats each `.mogrt` instance as a separate clip". If the designer fixes a typo in the source file, forty titles in the edited episode do not change; they must be fixed one by one (Alt-drag over the old one) or, with Premiere 12.1, using the command "Relink Ae MOGRT in place". So a template is a mechanism of DISTRIBUTION (one author → many editors → many copies), and not a mechanism of DISTRIBUTION OF AN EDIT (one edit → many targets). This is exactly the opposite end of the axis from the adjustment layer and from our morpher, and that is precisely why "template" and "morpher" are not competitors, although both are about "made once, applied many times".

**4. A font is not part of the template but a promise.** A `.mogrt` is a zip; the font is put into it only if the license permits embedding, therefore the normal scenario is different: authors are advised to take ONLY Adobe Fonts, because Premiere itself pulls them for the consumer (right-click on the thumbnail → Sync Missing Fonts).
A local commercial typeface is never synchronized — Premiere silently substitutes a replacement, and the title looks not as intended, with a formally valid template. The advice from the industry sounds cynical and precise: better that everyone gets a "99% correct" font from Adobe Fonts than that some titles fall into Comic Sans.

## Pros

- A public interface assembled by hand breaks the link between the internal structure and what the consumer sees: the author rummages through the composition, fixes layers, changes pre-compositions — the editor keeps the same three fields with the same names, and the updated file is put in place of the old one.
- The limits of an exposed parameter are part of the contract, not decoration: a slider 0..100 instead of an XY point makes "moved the title out of the frame" physically impossible, and support stops receiving this class of requests.
- Protected time zones resolve a conflict that otherwise is resolved only by conversation: the editor needs an arbitrary length of the lower third, the author needs an unchanged pace of entry. Markup of the head and the tail satisfies both without negotiation.
- A rig in FCP (one widget → many parameters through snapshots) gives the consumer a meaningful handle instead of twenty technical ones: "style: strict / festive" instead of color, thickness, delay and curve separately.
- An instance-copy makes the project self-contained: the template file can be deleted from disk, the edited episode will not fall apart and will render.

## Cons

- An instance-copy makes fixing the template useless for already edited material: an edit of the source file does not catch up with forty inserted copies, and "update the template" turns into a manual operation on each instance.
- A template renders as an autonomous clip and does not see the timeline around it: an adjustment layer and blend modes INSIDE it do not act on the tracks beneath it in Premiere. So a whole class of techniques (darkening the backdrop, blurring the background under a plate) cannot be packed into a template at all — the material itself has to be dragged inside the template.
- The author cannot expose "a little more" to the outside: either the parameter is exposed whole, or nothing is. There is no intermediate "can be moved, but within these limits and only horizontally" — the limit is expressed only by the choice of control type.
- The markup of protected zones does not survive nesting (a marker on the nested composition's layer does not update by itself) and, according to numerous reports, behaves in Premiere differently than in AE. Sound is not included in the protected zones at all.
- Opacity is one-sided: a template cannot be taken apart with the tools of an EDITING program in any of the five. But there is no "protection" here either — `.mogrt` is renamed to `.zip`, inside it lies `.aegraphic`, which is also a zip, and inside that an `.aep` (the method is described by an Adobe engineer on the official forum). The boundary is held not by the format but by the host program: the editor simply has no command "open inside".
- The font lives outside the template: a guarantee exists only for Adobe Fonts, everything else is a silent substitution on the consumer's machine.

## What users say

> After Effects built mogrts do not show any keyframes in the Effect Controls
> panel. They're fully encapsulated motion graphics - only the controls that
> the artist builds into the Essential Graphics panel are available.
> — Theresa Rostek (Adobe Community Manager), Adobe Community, 12.12.2023,
> https://community.adobe.com/t5/premiere-pro-discussions/mogrts-not-fully-editable-in-24-0-0-and-24-0-1/td-p/14289234

> The answer is generally that the mogrt designer did not allow editing the
> animation.
> — Stan Jones (Community Expert), same thread, 10.12.2023

> Ae Adjustment Layers are ignored when they are part of an AeMoGRT.
> — Roland R. kahlenberg, Creative COW, 23.01.2024,
> https://creativecow.net/forums/thread/mgrt-created-on-ae-with-an-adjustment-layer/

> no way to replace the .MOGRT file in order to update changes made and keep
> the dynamic content intact
> — Devyn Brugge (reply marked as the solution), Adobe Community, 30.10.2017,
> https://community.adobe.com/t5/premiere-pro-discussions/updating-essential-graphics-templates-used-in-premiere/m-p/15017871

> Responsive Design Time in MoGRTs produces iffy/maybe/meh results.
> — Roland R. Kahlenberg, Creative COW, 26.09.2025,
> https://creativecow.net/forums/thread/mogrt-responsive-design-time-functions-clarifications/

> the protected regions are slowed down or accelerated, and the outro animation
> happens at a random point in time
> — Alexis Fradier, Adobe Community, 03.02.2020,
> https://community.adobe.com/t5/after-effects-discussions/motion-graphic-template-responsive-design-time-protected-region-bug-in-premi%C3%A8re-pro/td-p/10898756

> A lot of these essential graphics don't let me customize the font, and the
> default fonts aren't being recognized by Premiere.
> — tom_15480754, Adobe Community, 02.09.2024,
> https://community.adobe.com/questions-729/missing-fonts-in-premiere-pro-s-essential-graphics-need-help-1415851

A live comparison of "whose templates are more convenient" among the five editors could not be found: `reddit.com` and `video.stackexchange.com` do not open from this environment, and on the available platforms (Creative COW, Adobe Community, the Blackmagic forum) people discuss their own editor, not others'.

## How it fits Kinotrain

### What already exists

**Titles do not exist at all** (F-TXT-001), so a "title template" has no subject for us. But the note is not about text: the mechanism "the author exposes a limited set of handles" is not tied to text, and it runs into something else.

**A public parameter passport exists, and it is a good one.** `params_for!`
(`src/libs/kt-model/src/project.rs:2308`) is the SINGLE declaration: from it the specs, reading and writing with clamping are generated. `ParamSpec`
(`src/libs/kt-model/src/project.rs:2203`) carries exactly what the template author sets by hand in
AE: the name, `min`, `max`, `default`, `step`, the unit and `kind`
(`Smooth` — can be automated, `Structural` — recreates the object). This is the
"public API of the effect", and the UI MUST draw a parameter ONLY from it
(`widgets::param_row` / `params_grid`, `src/apps/kt-app/src/widgets.rs:106`).
The difference from a template is one, but a decisive one: our passport is declared IN THE CODE and shared by all projects, while a template's is IN THE DOCUMENT and its own for each file.

**The mechanism "one edit → many targets" exists, and on its axis it is stronger than a template.** The morpher: `MorpherEvent` (`src/libs/kt-model/src/project.rs:569`),
`MorphPatch` (same file, 677), applied in `Project::effective_clips()`
(3013) — the patch rewrites the clips BEFORE the plan is built, cumulatively from the bottom up.
An honest comparison along three axes:

- *Reference versus value.* The morpher is a LIVE reference: move the slider in
  `src/apps/kt-app/src/morpher_editor.rs`, and forty targets change in the same
  frame, without forty undo steps. A template is a COPY as of the moment of insertion: an edit of the source file does not catch up with the inserted instances. These are opposite ends of one axis, and along it we are ahead.
- *Who is the author.* In a template the author and the consumer are DIFFERENT PEOPLE, and the whole point of the design is the boundary between them. The morpher has no boundary: `MorphPatch` sticks out whole, there is nothing to hide in it and nobody to hide it from. We have no roles at all — neither the notion of a "foreign document" nor the notion of "read-only".
- *What exactly is reused.* The morpher reuses the EDIT (substitution of `media`, shift of `source_in`, `video_fx`/`audio_fx` chains) on top of someone else's content. A template reuses the CONTENT — a ready-made composition with its own time and its own output. The morpher creates nothing: remove the clips from under it, and it will show nothing.

**The closest thing to "a piece of the timeline as an object" is named areas.**
`MarkedArea` (`src/libs/kt-model/src/project.rs:1163`, item 241): a rectangle of time × tracks that has a NAME, and the morpher refers to it by name
(`MorphTargets::Areas`). This is already an addressable piece of the edit — but only a selection: it has no time of its own, no output of its own and no parameters of its own.

**Foreign nesting is already encountered by us and is already marked as non-portable.**
`ForeignEvent` (`src/libs/kt-model/src/project.rs:441`) explicitly names a nested Stack among the constructs we do not render; they are put there by
`src/libs/kt-otio/src/convert.rs:914`, and the raw material travels in `Retained` untouched.
In item 158 this cost a cross-render test: `melt`
expands nested compositions and draws their content, "and with us there is emptiness by decision
R.24 item 5 — there is nothing to compare until nested compositions are supported", so `nested_example.otio` remained in the smoke tier
(`src/apps/kt-cli/tests/cross_render.rs:40`).

**We do not have fonts in the sense that titles need them.**
`src/apps/kt-app/src/icons.rs:54` — three vendored fonts, of which two are SUBSETS, built by `scripts/subset-fonts.py` strictly by the characters of the catalogs
`apps/kt-app/i18n/*.toml` (19 MB → 75 KB). The application does not see system fonts at all. Test 100 checks that every non-ASCII character of the interface has a glyph.
So the problem "the consumer has no font" for us would be not better but WORSE than in Premiere: we do not even have a substitution — a character outside the subset is drawn as a square.

### What is missing

- **Nesting.** `ClipEvent.media` is a `MediaId`
  (`src/libs/kt-model/src/project.rs:1514`), and `MediaRef { id, path }` (1497) is
  a PATH TO A FILE. A clip cannot refer to a piece of the timeline: `Project` is a flat list of clips plus tracks, and there is not a single recursion in the model.
- Parameters declared by the DOCUMENT: `ParamSpec.name` is `&'static str`,
  `param_specs()` returns `&'static [ParamSpec]`, and the effect registry
  `registry()` (`src/apps/kt-app/src/fx_registry.rs:47`) is `&'static [FxEntry]`
  behind a `OnceLock`. A user-defined entry with its own set of handles cannot physically be put there.
- Stretching of the container's time and of protected zones. We do have a time warp
  (`Slowed+Reverb`, `rate`, items 247–250), but it is an EFFECT
  ON A CLIP, not a property of a container, and a "protected zone" has nothing to be correlated with in it: the clip already maps the material linearly.
- A library living between projects. The same question as in F-AUT-004:
  a project is ONE `.otio` file (`src/libs/kt-otio/`), and at the application level there is only `$XDG_DATA_HOME/kinotrain/` with autosave.
- The roles of "author" and "consumer" and, accordingly, the notion of a "read-only piece of a project".

### What gets in the way

**1. The main thing: nesting is not a feature of titles but a change in the shape of the model, and it touches everything.** A template is in essence a nested composition with public parameters. To hand it to the engine, a piece of the timeline must be able to pretend to be a source, that is, implement `MediaSource`
(`src/libs/kt-media/src/lib.rs:64`): `read_video_frame(pts, quality)`,
`read_audio_burst(idx)`, `keyframe_before(pts)`. The first two mean a RECURSIVE CALL of the engine from inside the thread which, under **invariant 5**, owns the decoder and must not block; the third has no meaning at all for a timeline. Further down the chain: **invariant 4** — the cache is keyed by clip and quality, a nested composition has no "own" material, and there is nothing to count for the coverage bar; **invariant 7** — `VideoRenderer::frame_plan`
(`src/libs/kt-engine/src/video.rs:446`) builds ONE plan over a flat set of layers, and "a layer that is itself a plan" is a second composition code path, exactly what the invariant forbids; **invariant 8** — thumbnails and waveforms are taken from the sidecar DB `<project>.ktindex.db`
(`src/libs/kt-media/src/index.rs`), and a nested composition has no file from which the index is built, so either it is drawn as a placeholder or scrolling starts recomputing the picture. Each point is solvable, but this is a round, not a stage.

**2. The parameter passport will have to be taught to live at runtime, and it was deliberately made static.** `params_for!` exists precisely so that "a desync of the three tables is impossible" — specs, reading and writing are generated from one declaration. A template requires a set of `ParamSpec` built from the document on load, with checks of names, limits and conflicts. This is not "change `&'static` to `String`": behind the passport hang automation (`ParamCurve.param` — the field name), the i18n registry with catalog keys, keyword search, and the UI rule "a numeric parameter is drawn ONLY by `param_row`". A public name that the template author invented themselves will never get into the translation catalog — so we would get the first non-localizable text in the interface.

**3. The format, conversely, is ready — and this is a rare case where it is ahead of the model.** OTIO is nested by its nature: `Timeline.tracks` is a `Stack`, and nested `Stack`s inside a track already arrive to us from other people's files. We deliberately drop them into `ForeignEvent` (R.24 item 5). So the price of nesting is not "a new format version" but "stop throwing away what the format already carries"; however, backward compatibility will need as much attention as the crooked parts in v3.

**4. The roles "author ≠ consumer" in a single-user editor must still be justified.** The whole value of a template lies in the fact that the editor CANNOT break the composition. When the author and the editor are one person at one computer, this value falls almost to zero, and only "reuse a blank" remains, which is covered much more cheaply by a preset library (F-AUT-004). This is the main argument against a priority above C, and it has to be named aloud, not hidden.

### Which invariants it touches

- **4** (the cache is the only truth about coverage): a nested composition has no material of its own; either it has its own form of cache key, or the warmth bar lies.
- **5** (exactly one MediaWorker owns the decoder): reading a frame of a nested composition is a recursive render inside a thread that is forbidden to block.
- **7** (the composition code is ONE): "layer = nested plan" is a direct claim to a second composition path; it is admissible only through expanding the nesting INTO the plan, not beside it.
- **8** (scroll/zoom does not generate decode requests): the thumbnails and waveforms of a nested composition have nowhere to come from — it has no file and no record in
  `.ktindex.db`.
- **11** (determinism): "stretch the template to a length" is multiplication of time, and the rounding of ticks MUST be ONE and the same in preview and in export.
- **UI rule 13** (do not waste space): the public handles of a template MUST live in the already existing `params_grid`, not in their own panel.
- **UI rule 16** (impossibility — as an error): "the template does not open because the author did not expose this" and "there is no glyph in the subset" MUST be an explicit error, not a silent substitution of a square.
- **UI rule 17** (paired methods are the same): a handle of a template and an ordinary effect parameter are drawn by one widget and behave the same.
- **UI rule 20** (nothing grows by itself): the template library is replenished only by an explicit save.

## Estimate

| What | Answer |
|---|---|
| Size | XL — nesting is a separate round, a template on top of it is still a stage; without nesting the feature degenerates into a "named preset" (F-AUT-004) |
| Where the main work is | kt-model (recursion in the model, the runtime parameter passport), kt-engine (expansion of nesting in `frame_plan`, cache, prefetch), kt-otio (stop dropping a nested `Stack` into `ForeignEvent`), kt-app (public handles in the common grid), kt-media (what a thumbnail of a nested composition is to be) |
| Project format | a version IS needed, but there are almost no new constructs: OTIO already carries nested `Stack`s, the question is reading old files and what to do with the already saved `ForeignEvent`s |
| Tests to write | round-trip of nesting in kt-model; expansion of nesting == the flat equivalent byte for byte (after the pattern of morpher_render t36 "render with morpher == bake"); moving `nested_example.otio` from the smoke tier to the strict one (`src/apps/kt-cli/tests/cross_render.rs`); protection against cyclic nesting; a table "a template on a clip twice as short/long" for protected zones |
| Depends on | F-TXT-001 (there are no titles — a title template has nothing to wrap), F-AUT-004 (where the library living between projects lives) |
| Breaks compatibility | no (old files are read; `ForeignEvent`s from old saves will have to be left as they are) |

## Verdict

**We do not take it as "title templates" — and this is not a refusal but a redirection.** The reconnaissance
showed that the feature runs into neither text nor titles: a template is a nested composition with a public set of parameters, and we have no nested sequences AT ALL
(`ClipEvent.media` is a `MediaId` pointing to a file path, `Project` is flat, foreign
nested `Stack`s are dropped into `ForeignEvent`, and because of this `nested_example.otio`
is still not in the strict tier of the cross-render). While there is no nesting, a "template" for us can only be a named preset — that is, F-AUT-004, and it costs an order of magnitude less.

Second: half of a template's value lies in the boundary between two PEOPLE, which a single-user editor does not have. Therefore priority C, although nesting itself deserves a separate note in `project/` and a priority above: not only titles depend on it, but multicam, compound clip and all work with other people's projects.

A third thing to take regardless of the verdict: **protected time zones.**
This is a cheap and well-tested by life idea — pieces marked with markers that do not stretch when the duration changes. Our marks and named areas
(`MarkedArea`) already address time; when we have container stretching, the markup "do not stretch this" must be done at once, not five years later, as Adobe did.

**Revision R.59 (2026-09-21).** The text above is not rewritten and remains a historical
document; the revision is added alongside, because one of the two premises of the verdict
has stopped being true.

*What was removed.* The formulation "we have no nested sequences AT ALL" (and everything that follows from it in the section "What gets in the way", item 1) is no longer a fact. Nesting
was done in R.50, R.55, R.56 and R.58: F-TL-004 has status `done`, a reel is an
object inside the project, an instance refers to it in the same address space, nesting has become an input of a layer, and foreign nested `Stack`s are read as real reels, not dropped into `ForeignEvent`. The item "The format, conversely,
is ready" came true exactly as it was written. None of the observations about FOREIGN
editors is affected by the revision — they were checked on 2026-09-07 and stand as they are.

*What held.* The second half of the verdict — the roles "author ≠ consumer" in a single-user editor — is true today as well, and it is precisely this, rather than the absence of nesting, that explains the priority C of THIS note: there is no one to protect the composition from. Therefore the status remains `researched`, and the priority is C.

*Where the subject was redirected.* The note's own finding — "a template is a nested composition with a public set of parameters" — was redirected to the new note **F-TL-012 "Public parameters of a composition"**
(`timeline/structure/composition-public-parameters.md`, R.59, priority A). There the subject stands without titles and without roles: not "protect the composition from the editor", but "express ten releases of one bumper". The quotes and the reconnaissance of foreign editors were reused from there with a reference to this note.

*The second emphasis named here is also removed.* "The parameter passport will have to be taught to live at runtime, and it was deliberately made static" (item 2 of the section "What gets in the way") is a direct claim of the owner R.59 (item 2) and the subject of the foundation of the same round: the model of properties and the parameters of generators on an event. That is, the emphasis remains a correct description of TODAY'S code and ceases to be an argument against.

## Sources

- Creating Motion Graphics templates with the Essential Graphics panel (dragging
  properties, supported control types):
  https://helpx.adobe.com/after-effects/desktop/motion-graphics/work-with-motion-graphics-templates/creating-motion-graphics-templates.html
  — **the page did not open from this environment** (`helpx.adobe.com` is blocked);
  its content is known from the title in search results and from the secondary write-ups below
  (accessed 2026-09-07)
- Responsive Design – Time, protected zones and the quote "If you modify a protected
  region after you nest a composition, the protected region marker on the nested
  composition layer does not get automatically updated":
  https://helpx.adobe.com/after-effects/desktop/motion-graphics/add-responsive-design/responsive-design.html
  — **[NOT VERIFIED against the primary source]**: the line was obtained from a search-result
  snippet; the page itself is unavailable (accessed 2026-09-07)
- A practical breakdown of `.mogrt` (control types, a slider instead of a point, Adobe
  Fonts are pulled automatically, media replacement, protected zones, "MOGRTs
  cannot interact with other layers in the Premiere Pro timeline", "Audio
  doesn't work well with Responsive Design – Time"):
  https://blog.frame.io/2024/08/12/mogrt-guide-after-effects-2024-motion-graphics-workflow/
  and https://blog.frame.io/2023/05/15/smooth-mogrt-workflows/ (accessed 2026-09-07)
- A template from AE shows no keyframes; the handles are only those the author exposed:
  https://community.adobe.com/t5/premiere-pro-discussions/mogrts-not-fully-editable-in-24-0-0-and-24-0-1/td-p/14289234
  (accessed 2026-09-07)
- Adjustment layer inside `.mogrt`: https://creativecow.net/forums/thread/mgrt-created-on-ae-with-an-adjustment-layer/
  and https://community.adobe.com/t5/premiere-pro-discussions/creating-a-mogrt-with-an-automated-blur-adjustment-layer/m-p/11243212
  — **a refinement of the reconnaissance finding**: it is reliably confirmed that a template
  renders as an autonomous clip and its adjustment layers with blend modes do NOT
  reach the Premiere tracks beneath it. The stronger claim "AE Adjustment
  Layers are ignored altogether, even inside the template's composition" — these are the words of
  experienced practitioners in the thread, not checked live **[NOT VERIFIED]**
  (accessed 2026-09-07)
- A copy of the template in "Motion Graphics Template Media", the instance as a separate
  clip, "Relink Ae MOGRT in place" from Premiere 12.1:
  https://community.adobe.com/t5/premiere-pro-discussions/updating-essential-graphics-templates-used-in-premiere/m-p/15017871
  (accessed 2026-09-07)
- Protected zones in Premiere behave differently from AE (a bug thread) and "RDT is made for
  the head and tail, not for the middle":
  https://community.adobe.com/t5/after-effects-discussions/motion-graphic-template-responsive-design-time-protected-region-bug-in-premi%C3%A8re-pro/td-p/10898756
  and https://creativecow.net/forums/thread/mogrt-responsive-design-time-functions-clarifications/
  (accessed 2026-09-07)
- How protected zones are set (Composition → Responsive Design – Time →
  Create Intro/Outro, or a composition marker with the "Protected Region" checkbox):
  https://mikemurphy.ai/tutorials/responsivedesign/ (accessed 2026-09-07)
- Missing fonts in templates, "Sync Missing Fonts" by right-click:
  https://community.adobe.com/questions-729/missing-fonts-in-premiere-pro-s-essential-graphics-need-help-1415851
  (accessed 2026-09-07)
- Unpacking `.mogrt` back (rename `.aegraphic` to `.zip`, open the `.aep`
  in AE, repack) — an answer from an engineer of the AE/PR teams:
  https://community.adobe.com/questions-729/can-t-import-aegraphic-file-through-essential-graphics-1395488
  (accessed 2026-09-07)
- FCP/Motion: the Build In / Build Out markers — "instruct Final Cut Pro to play
  that section at the same speed as the original template … regardless of the
  duration of the clip", only the span between them is stretched:
  https://motion-5.helpnox.com/en-us/motion-5-user-manual/creating-templates-for-final-cut-pro-x/working-with-markers-in-templates/
  (a mirror of the Motion manual; the current help is
  https://support.apple.com/guide/motion/intro-to-template-markers-motn141baf86/mac,
  whose article text is not served from there) (accessed 2026-09-07)
- FCP/Motion: publishing parameters and rigs, "Template items that can't be
  published": https://support.apple.com/guide/motion/about-publishing-parameters-motn5ba1a1e6/mac
  and https://support.apple.com/guide/motion/intro-to-rigging-motn7ce46f0b/mac —
  the page did not give the list of unpublishable elements itself **[NOT VERIFIED]**
  (accessed 2026-09-07)
- Resolve: macros and the Macro Editor (selecting and renaming the exposed inputs,
  the catalog `Templates/Edit/Titles`): https://jayaretv.com/fusion/macro-editor/;
  in Resolve 21 the macro editor gained the order of controls and "Save to Edit
  Template" with a choice of Title/Generator/Effect/Transition:
  https://mixinglight.com/color-grading-tutorials/resolve-fusion-21-macro-editor-update/
  and https://davinciresolveclub.com/davinci-resolve-fusion-macros-templates/
  (accessed 2026-09-07)
- VEGAS Pro 2026: Continuum Title Pack and Title Studio ("a dynamic text & mograph
  engine packed with presets"), three editions:
  https://www.vegascreativesoftware.com/vegas-pro/whats-new/ and
  https://borisfx.com/news/a-new-home-for-vegas-pro-sound-forge-acid-pro/
  (accessed 2026-09-07). That Vegas does not read `.mogrt` and has no
  "author → consumer" mechanism is a conclusion from the absence of such mentions in
  official materials; it is not confirmed by a separate primary source
  **[NOT VERIFIED]**
- None of the five editors was checked live: everything above is official help (Apple, Vegas, Boris FX), official Adobe forums, Creative COW and
  specialist write-ups. `helpx.adobe.com`, `reddit.com` and
  `video.stackexchange.com` are unavailable from this environment.
