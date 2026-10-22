---
id: F-TXT-001
title: On-screen text: titles, captions, lower thirds
status: done
priority: A
cost: L
category: titles/text
editors: [premiere-26, resolve-21, vegas-pro-2026, fcp-12, after-effects-26]
we_have: yes
crates: [kt-media, kt-model, kt-engine, kt-app, kt-otio, kt-cli]
invariants: [4, 5, 7, 8, 11]
ui_rules: [3, 7, 8, 11, 13, 14, 17]
round: R.52
point: 405
tests: [t480, t481, t482, t483, t484, t485, t487, t488, t489, t490, t495]
depends_on: []
checked: 2026-09-16
---

# F-TXT-001 — On-screen text: titles, captions, lower thirds

## Essence

A title is the only image on the timeline that exists in none of the source files: the editor COMPOSES it. They place an event on the top
video track, write a line, choose a font, size, color, outline and shadow,
drag it with the mouse to the needed corner of the frame, and set the entry and exit. Three genres
cover almost all the work: a centered heading, the lower third (the speaker's name and title), and end credits crawling up. Everything else —
signage — is also made with this same tool.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Adobe Premiere | 26.x | Text tool + Essential Graphics, `.mogrt` templates | A title is a Graphic-type clip on a track, and INSIDE it there is its own stack of LAYERS (text, shape, image) with their own transforms. Style is moved into an object (Master Styles / Track Styles) and reused. The former tool — Legacy Titler: a separate modal window, the title is a bin item, not a clip; removed in version 23.0 in October 2022 [NOT VERIFIED against the primary source: `helpx.adobe.com` does not open from this environment, the date is taken from a snippet of the Adobe FAQ and confirmed by community threads] |
| After Effects | 26.x | Text Layer + Animators | THE REFERENCE. Text is a layer; on top of the ordinary properties it has a SEPARATE animation mechanism: Animator + Range Selector. The values of the animator's properties are not values but OFFSETS, applied to a subset of characters chosen by the selector (Start / End / Offset; units — percent or indices; the basis — characters / characters excluding spaces / words / lines). An animator can have several selectors, and there are non-gradient kinds — Wiggly and Expression |
| DaVinci Resolve | 21 | TWO tools: "Text" (a generator on the Edit page) and "Text+" (a Fusion node) | The official page describes both in one paragraph: "find the text generator or Fusion title template you want, and drag it into the timeline". "Text" is simple and lives on the Edit page; "Text+" is a Fusion node with per-character layout, layout modes and a stack of shading elements, and per-character animation is done with the Follower modifier [NOT VERIFIED: the set of elements and the Follower are taken from publicly available write-ups; the official Reference Manual was not downloaded]. Subtitles are a third, separate subsystem with its own track |
| Final Cut Pro | 12.3 | Titles (Motion templates), 3D Titles | A title is not a "clip with an effect" but a separate class of object: it is CONNECTED (connected clip) to a host clip and moves together with it on ripple. All titles, including the built-in ones, are Motion templates; your own title is made not in FCP but in Motion and placed in the template catalog. 3D titles have their own section of parameters: "Rotate titles in 3D space", "Modify depth and edges", Substance / Paint / Finish controls |
| VEGAS Pro | 2026 | "Titles & Text" (OFX generator), ProType Titler (legacy), Boris Continuum Title Studio | A title is a MEDIA GENERATOR: an event on a track whose source is not a file but a plugin, and which otherwise behaves like an ordinary event (trim, crossfade, FX chain). The second, "serious" titler is foreign: Title Studio from Boris Continuum, "our next-gen 3D vector titling and animation solution", with its own window and its own "hardware accelerated 3D engine". The status of ProType Titler in the 2026 lineup [NOT VERIFIED: the Vegas forum moved to `forum.borisfx.com` and lost the archive] |

## The trick behind it

**1. Everyone has TWO text tools, and the second is a foreign renderer in its own window.** Resolve: Text versus Text+. Premiere: Legacy Titler versus Essential Graphics. Vegas: "Titles & Text" versus Title Studio. FCP:
built-in titles versus Motion. This is not the vendor's laziness but a consequence of the design:
a simple title MUST be drawn by the same code as the rest of the timeline
(otherwise it does not take part in the cache and in scrubbing), while a "titling" tool is
in essence a mini-compositor, and it is cheaper to bring it in whole than
to rewrite the timeline. The price is paid by the user and it is concrete:
style does not carry over between tools, nobody has automatic conversion,
and a project where titles were made with the second tool does not open without it. Boris writes plainly about Title Studio that "the real work is
done in the custom user interface" and that inside is its own hardware 3D engine
— that is, the second tool has its own notion of a pixel, a font and
anti-aliasing, and it is not obliged to coincide with the first.

**2. Per-character animation is not keyframes on a layer but a separate mechanism.**
In AE "letters fly in one by one" is done NOT with N keyframes for N letters. An
Animator is created (say, Position), its value is a constant OFFSET, and
the Range Selector's parameter — Offset — is animated. One keyframe on Offset sweeps the
range along the line, and the offset in turn reaches each character.
From this follows a non-obvious practical consequence: **a word can be added to a finished
title, and the animation rebuilds itself** — it is bound to a range in percent of the line length, not to character numbers. The same property belongs to the
Follower modifier in Fusion. Tools without per-character animation
(Vegas "Titles & Text", Resolve "Text") force the string to be chopped
into events — and then any edit of the text means re-laying everything.

**3. A title is the only timeline object that is not sampled from a raster but is BORN at the composition's resolution.** A video clip has its own
frame, which the engine scales; a title has no source frame at all,
and rasterization happens at whatever size was requested. Two things follow from this, visible only in practice. The first: changing the project resolution
is free for a vector title (it is redrawn sharp), while for a title baked into a raster it is fatal (it is scaled and blurred); the classic complaints "the title got blurry" are exactly about the second case. The second, and for us more important:
since the frame is born at the moment of rendering, its bytes depend on the version of the font rasterizer, the version of the font file itself, and the hinting settings.
A title is the place where "the same project" stops meaning "the same bytes", and does so silently.

## Pros

- A title as an ordinary EVENT on a track (Vegas, Premiere, Resolve) gets the whole timeline for free: trim, split, crossfade, FX chain, level, copy-paste, undo. A separate class of object (FCP, Legacy Titler) requires each of these operations to be implemented anew.
- A connected title (FCP connected clip) travels with its clip on ripple: you cut out a line — the caption moved with it instead of hanging over someone else's shot.
- Style as an OBJECT, not a set of fields: editing one Master Style rewrites forty lower thirds without forty undo steps.
- A range selector instead of keyframes per character: the length of the text and the volume of the animation are decoupled, and editing the line does not break the animation.
- A title is born at the output resolution — at 4K it is sharp without reshooting, and one and the same project is suitable for several masters.
- A template (`.mogrt`, a Motion template, a Fusion Title) separates the author of the graphics from the editor: the designer hands over the file, the editor edits only the text.

## Cons

- Two tools = two incompatible models: style does not carry over, there is no converter, and "redo it in the new one" means typing it in again.
- A title depends on the font INSTALLED in the system. On another machine the font is substituted, the line is re-laid, and the render farm silently delivers a different picture. The project carries the font's name, not the font.
- Rasterization is not part of the project: an update of the application or of the system font libraries changes the frame in the lowest bits — that is enough to break a bit-exact comparison and smart render.
- A discontinued tool takes old projects with it. Legacy titles do not convert, and opening an archive a few years later yields not "a title that needs a slight fix" but an empty space.
- Safe zones are a direct inheritance from picture tubes (5% for action safe, 10% for title safe), but they live on to this day and still determine where text can be placed at all.
- The text of a title and the text of a subtitle are different subsystems: a line typed in a title gets into neither search, nor subtitle export, nor translation.
- RTL and CJK break first of all: drawing the glyphs "in order" is not enough, shaping (Arabic cursive, ligatures) and bidi reordering are needed, and vendors test this last.

## What users say

On the change of tool in Premiere — from 2017, five years before the actual removal of Legacy Titler:

> Thank you for taking away an easy, simple way to create titles in
> Premiere and replace it with TITLE HELL!
> — John21601, community.adobe.com, 20.09.2017, https://community.adobe.com/t5/premiere-pro/what-happened-to-the-legacy-title-tool/m-p/9334021

> Now using the legacy titler makes the program lag 3 seconds every time
> you try to change anything, it is indeed a massive step backwards to
> ruin a simple easy tool.
> — ml46074010, community.adobe.com, 19.08.2018, https://community.adobe.com/t5/premiere-pro/what-happened-to-the-legacy-title-tool/m-p/9334021

What the removal of the tool does to old projects — a thread from 2024, a year and a half after the removal:

> I lost all the titles in the preview, all is left was a thin strip (2)
> that moves according to the motion settings.
> — valeriansantosa, community.adobe.com, 14.03.2024, https://community.adobe.com/t5/premiere-pro-discussions/legacy-title-is-missing-in-premiere-pro-2023-only-a-thin-strip-was-left/td-p/14489469

On safe zones and why they outlived the picture tubes:

> Even today, we still need to pay attention to these issues because not
> everyone has yet upgraded to an LCD TV set.
> — Larry Jordan, 02.01.2012, https://larryjordan.com/articles/display-action-title-safe-zones/

What could NOT be found, and what the next wave should look for: live quotes about Text versus Text+ in Resolve and about Arabic/CJK in titles. The thread
"Combine Text and Text+ Please" exists on the Blackmagic forum
(`forum.blackmagicdesign.com/viewtopic.php?f=21&t=207775` — found through
search results), but the forum itself returns HTTP 403 from this environment for all
`viewtopic.php`; reddit is unavailable, Adobe Community search returns 403 on
`/t5/forums/searchpage`, and search engines in this session hit
429/CAPTCHA. The Vegas forum, meanwhile, has moved to `forum.borisfx.com` and contains 11 threads in the Vegas Pro section — the old archive at this address is unavailable, so there was simply nowhere to quote Vegas from.

## How it fits Kinotrain
CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
A title is a source (a generator), not a layer of the plan: it gets trim, transitions, FX and copy-paste for free; the font and pins — only with regeneration of the goldens.

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round. It is split: (1) the `title:` source + rasterization + the vendored font + golden; (2) the model and the format; (3) the editor in the UI, safe zones, text input; (4) the i18n path (bidi/shaping) and a test related to t100 |
| Where the main work is | kt-media (a new `MediaSource` next to `placeholder.rs`), kt-app (the title editor, the input field, safe zones), kt-model (how the string is stored), kt-otio (what goes into the file) |
| Project format | the envelope version `metadata.kt.v` need not move, if the title travels as `MediaRef.path` with a scheme (a live precedent: `synth:`); a structured title object will require v2 |
| Tests to write | determinism of rasterization (a relative of t208: the same text → the same bytes, a repeat and different machines); preview == export on a title project (a relative of t11/t67); a golden with a title scene; glyph coverage of the vendored font (a relative of t100); an Arabic/CJK title — order and cursive shaping (a relative of t126); a harness test of the editor window (a relative of t221/t227) |
| Depends on | nothing hard. Related: F-AUT-001 (Bézier handles — without them the title's entry is only linear/smoothstep), F-MSK-003 (a title as a stencil for a track matte), F-MPH-007 (what the layer sees at its input) |
| Breaks compatibility | no: old projects do not contain titles, `GOLDEN_VIDEO` does not move |

## Verdict

We take it, and it is the first candidate of the `titles/` category: the owner chose titles on the roadmap in a direct line (`docs/user-feedback.md`, item 6 of the section on the landing page —
"titles and transitions, color correction and LUT…"), and the transitions from the same line were already done in R.33. An editor without a single way to put text into the frame
looks unfinished more strongly than an editor without color correction.

It must be taken by the cheap path — **a title as a source, not as a new layer of the plan**: a `MediaRef` with a scheme in the path plus a `MediaSource` next to
`placeholder.rs`. Then kt-gpu, `video_plan.rs`, `ClipEvent` and the OTIO spine are not touched, and the title gets trim, transitions, the FX chain, level and
copy-paste for free.

The only thing that cannot be decided on the fly is determinism: the vendored font and the pinned rasterizer must be written down as a rule BEFORE
the first line of code, otherwise the first `cargo update` will break `GOLDEN_VIDEO` and nobody will understand why. Per-character animation (AE Animator / Fusion
Follower) is not taken in this pass — it is a separate note of the category.

## Sources

- https://community.adobe.com/t5/premiere-pro/what-happened-to-the-legacy-title-tool/m-p/9334021 (accessed 2026-09-07) — quotes by John21601 and ml46074010
- https://community.adobe.com/t5/premiere-pro-discussions/legacy-title-is-missing-in-premiere-pro-2023-only-a-thin-strip-was-left/td-p/14489469 (2026-09-07) — quote by valeriansantosa, the fate of old projects
- https://community.adobe.com/t5/premiere-pro-discussions/legacy-title-function-being-discontinued/td-p/12326704 (2026-09-07) — an answer from a Community Expert "Draw the shapes and text in the EGP and animate in the Effect controls"
- https://www.blackmagicdesign.com/products/davinciresolve/edit (2026-09-07) — the wording about the "text generator or Fusion title template"
- https://borisfx.com/products/title-studio/ (2026-09-07) — Title Studio as a Continuum plugin with its own UI and 3D engine, the list of hosts includes Vegas Pro
- https://support.apple.com/guide/final-cut-pro/intro-to-titles-ver2833f4e2/mac (2026-09-07) — titles and 3D titles in FCP
- https://larryjordan.com/articles/display-action-title-safe-zones/ (2026-09-07) — safe zones, 5% and 10%, their origin in picture tubes
- Unavailable from this environment, hence the `[NOT VERIFIED]` marks: `helpx.adobe.com`
  (the Premiere/AE help, including the FAQ on the removal of Legacy Titler),
  `forum.blackmagicdesign.com` (HTTP 403 on `viewtopic.php`),
  `reddit.com`, Adobe Community search (`/t5/forums/searchpage` — 403).
  Search engines in this session returned 429/CAPTCHA after the first
  requests, so the Resolve lines (the composition of Text+ and the Follower modifier)
  and the Vegas lines (the status of ProType Titler) rely on publicly available write-ups rather than on the primary source.
- The AE lines (Animator + Range Selector, the units and basis of the selector,
  Wiggly/Expression) were written from knowledge of the tool; the official help was not downloaded — to be checked against `helpx.adobe.com` on re-verification.
- Internal sources: `src/libs/kt-media/src/placeholder.rs`,
  `src/libs/kt-media/src/synthetic.rs`, `src/apps/kt-app/src/util.rs`,
  `src/apps/kt-app/src/render_dialog.rs`, `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-otio/src/schema.rs`, `src/libs/kt-otio/src/convert.rs`,
  `src/libs/kt-engine/src/video_plan.rs`,
  `src/libs/kt-engine/tests/golden_export.rs`,
  `src/apps/kt-app/src/icons.rs`, `src/apps/kt-app/src/i18n/bidi.rs`,
  `src/scripts/subset-fonts.py`, `src/apps/kt-app/assets/SUBSET-MANIFEST.txt`,
  `docs/file-format.md`, `docs/user-feedback.md`.
