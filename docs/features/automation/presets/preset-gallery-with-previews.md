---
id: F-AUT-006
title: Preview gallery in the picker — effect thumbnail on your own frame
status: done
priority: B
cost: M
category: automation/presets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app, kt-fx, kt-engine]
invariants: [8, 12]
ui_rules: [1, 3, 10, 12, 13]
round: "R.54"
point: 431
tests: [543, 544, 545, 546, 547, 548, 549, 550, 551]
depends_on: [F-AUT-004, F-HLP-031, F-HLP-025]
checked: 2026-09-17
---

# F-AUT-006 — Preview gallery in the picker — effect thumbnail on your own frame

## Essence

The editor opened the list of effects and reads the names: `Edges`, `Halftone`,
`Kaleidoscope`. The name does not say how it looks on HIS frame — and that is exactly
what he wants to know, because he is choosing not a tool but a look. The question of
the note is not "are the values saved" (F-AUT-004) and not "is there a description text"
(F-HLP-025), but "is the result visible BEFORE applying it, and on what is it shown": on
someone else's reference clip, on your frame — or not shown at all, and then the choice
goes by trial and error with undo.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Video FX window, grid of presets | effects as a list on the left, "The thumbnail images in the right pane represent each of the existing presets for the selected effect"; hovering plays an ANIMATED EXAMPLE — a ready-made clip from the vendor, not the user's material. The thumbnail is dragged onto an event, a track or the preview window; Shift+drag applies the preset without any dialog |
| Adobe Premiere | 26.x | Effects panel | a tree of folders and names with no pictures at all: no static icon, no preview. The result is visible only after applying, in the Program Monitor, so trial-and-error goes through undo. A separate pole is Lumetri Look with a visible result and a strength slider [NOT VERIFIED against the primary source: `helpx.adobe.com` does not open from this environment — the same unavailability recorded in F-HLP-031] |
| DaVinci Resolve | 21 | Effects Library + **Hover Scrub Preview** | the thumbnail in the cell is static, but the preview lives IN THE VIEWER: with "Hover Scrub Preview" enabled in the three-dots menu, moving the cursor over a cell plays the effect on the clips at the playhead (for a transition — on two clips at the nearest cut). Animated Fusion titles are animated in the cell itself on hover. The option is DISABLEABLE, and people turn it off for smoothness |
| Final Cut Pro | 12.3 | Effects browser, skimming | "A preview of the effect appears in the viewer" — the cell also serves as an axis, but the picture goes to the large viewer on the SELECTED clip. A unique detail: "For some effects, you can press and hold the Option key while you skim to preview changing the effect's primary control" — the horizontal position of the cell stops being time and becomes the VALUE of the primary parameter |
| After Effects | 26.x | Effects & Presets panel | names and categories as a list; for animation presets the preview is viewed not in the panel but in a separate browser (Adobe Bridge), where there is a long-standing complaint that "effects don't preview, thumbnails are shown". Inside AE itself the choice is made by applying it to a layer [NOT VERIFIED against the primary source: see the caveat about `helpx.adobe.com`] |

## The trick behind it

**1. A preview almost nowhere lives in the cell — it lives in the viewer, and this is a
consequence of cost, not taste.** Resolve and FCP arrived at the same solution
independently: the cell remains a static picture, and the result is shown in a large
viewer on the user's material. The reason is constructive: to draw the effect INTO EVERY
cell means keeping as many simultaneous pipelines as there are cells on screen, and doing
it on a frame that is not in the player's cache; to show it in the viewer means asking the
already existing player once to compute another frame. The cost shows in WHAT Resolve paid
for it: "Hover Scrub Preview" is not a behaviour but a toggle in the three-dots menu, and
the first thing forums advise when the interface stutters is to untick it. That is, even
the single additional pipeline turned out to be expensive enough to need a switch.

**2. In FCP the axis of the thumbnail is not time but VALUE.** Ordinary skimming scrubs
the frame, while `Option`+skimming scrubs the effect's "primary control": from left to
right what moves is not the material but the strength. From this follows something absent
from every list of features: an effect has the notion of a PRIMARY parameter, one per
effect, and it is precisely this that answers the question that fits neither in the name
nor in the description — "how much do people usually set here". A preset answers the same
question discretely (one point with a name); Option-skimming answers it continuously and
without a single line of text, that is, for free in five languages.

**3. Vegas shows the opposite pole: a preview on SOMEONE ELSE's frame.** The animated
preset example is a vendor clip recorded in advance, and it does not lie only as long as
your material resembles it. For an effect whose strength is set in PIXELS (blur radius,
raster step, channel offset), a foreign frame of a different resolution shows a different
strength at the same numbers — and this discrepancy is not visible in the gallery at all;
it surfaces only on application.

## Pros

- The gallery moves the cost of choice from reading to looking: a description line must be
  written, translated into five languages and then maintained (the cost — F-HLP-033), while a
  thumbnail is drawn by the same code that already executes the effect.
- A preview on YOUR OWN frame answers a question that someone else's example does not close:
  "how will it sit on my material" — a dark frame and a light frame with the same `Glow` look
  different, and the editor chooses not an effect but a combination.
- The value axis (FCP) gives what neither a preset nor a description gives: the range of
  meaningful magnitudes, shown rather than named.
- Dragging the thumbnail straight onto an event (Vegas) removes an extra step: choice and
  application are one gesture, and no intermediate dialog arises.
- Choosing BEFORE applying does not clutter the history: today browsing effects is a chain of
  "applied → looked → undid", that is, undo is used as a viewing mode.

## Cons

- An additional render pipeline in the UI is the most expensive part, and the first consequence
  is a switch (Resolve) or abandoning the preview altogether (Premiere, AE). A gallery done
  carelessly steals frames from the player.
- A preview on someone else's reference (Vegas) silently lies about the strength of everything
  set in pixels, and a preview on your own frame lies about it too if it is computed at the cell
  size rather than at the working resolution.
- A static thumbnail says nothing about effects whose essence is motion (jitter, a camera pass):
  for them the "look" exists only in time.
- The gallery takes space the picker does not have: a list of forty-odd entries with a column of
  pictures stops fitting the screen and needs its own navigation — the very effect that led, in
  F-HLP-031, to a separate tool for showing presets from a third-party vendor.
- The "primary parameter" must be declared for each effect by hand: it cannot be derived from the
  spec, and a wrong choice makes the axis meaningless.

## What users say

> Is there a website where I can preview all of Continuum's 600+ transition
> effects? … I can't possibly remmeber what they all look like.
> — peter2, Boris FX forum, 03.06.2020,
> https://forum.borisfx.com/t/continuum-has-over-600-transitions-is-there-a-place-i-can-preview-them-all/8230

> if i load a preset from Load Preset option, the preset doesn't show the name
> back in Vegas FX's window of which i've chosen … so i've still no idea which
> fx is on the timeline
> — Gid.Joiner, Boris FX forum, 03.02.2022,
> https://forum.borisfx.com/t/load-preset-doesnt-show-name-in-fx-window-returns-to-default-when-reopening-load-preset/13225

Both quotes were found during the reconnaissance of F-HLP-031 and are reused here: the first
is about the size of a catalogue that stopped fitting in one's head (exactly the reason for a
gallery), the second is about losing the link to "what is currently applied". Fresh feedback
could NOT be FOUND: the Blackmagic forum returns 403 to direct reading, the Resolve manual
pages on the mirror return 404, `helpx.adobe.com` is still unavailable from the environment;
the "Sources" below list what was read from the primary source and what only through search
results.

## How it fits Kinotrain
CAPABILITY: openspec/specs/video-fx/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
The picker gallery: thumbnails are rendered by the same composition code; a LIFO queue with cancellation by generation, an LRU cap; the strength axis blends neighbouring variants.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages: showcase values in the registry, thumbnail rendering and its service, gallery layout |
| Where the main work is | kt-app (thumbnail service and picker), kt-fx (rendering a frame with a chain) |
| Project format | does not touch it: the thumbnail is derived and is not written to disk |
| Tests to write | golden test of thumbnail rendering; key/LRU/cancellation of the service; harness test of the picker (a click on a cell gives EXACTLY these parameters in the model); a live scenario with the real program |
| Depends on | F-AUT-004 (done, R.48 — preset library), F-HLP-031 (preset as documentation), F-HLP-025 (effect description) |
| Breaks compatibility | no |

## Verdict

**We take it.** The reconnaissance shifted the framing twice. First, "showing a preview" in
all five means the VIEWER, not the cell, and the reason is the cost of the pipeline; we do not
have that cost: the base frame already lies in the sidecar DB, and applying a chain to a ready
320×180 frame is a stateless pure function costing fractions of a millisecond. So the third
option — a picture IN THE CELL and on your own frame — is cheaper for us than for any of the
five, and precisely because our video FX do not live in a player. Second, "we have no presets"
is no longer true (R.48), and the gallery sits not on empty ground but on ready `PresetRow` and
a ready `ThumbService`.

The axis of value at the cell (FCP's Option-skimming) was made in the same round, but DIFFERENTLY
from FCP, and this is the main practical conclusion of the reconnaissance: declaring a "primary
parameter" for each of the 27 kinds turned out to be unnecessary altogether — the axis moves
between two curated points (variants), so the "useful range" does not have to be invented, it is
set by them. For kinds where choosing a primary parameter would be disputed (`VHS`, `Color Grade`)
or impossible (`Mirror`, `Corner Pin`), the axis works the same as for the others.

## Sources

- Final Cut Pro User Guide for Mac 12.3, "Add video effects" — "A preview of the
  effect appears in the viewer", "For some effects, you can press and hold the
  Option key while you skim to preview changing the effect's primary control":
  https://support.apple.com/guide/final-cut-pro/add-video-effects-ver4e33bc9/mac
  (accessed 17.09.2026, read from the primary source)
- VEGAS Pro Online Help, "The Video FX Window" — preset thumbnails in the right
  panel, the animated example on hover:
  https://help.magix-hub.com/video/vegas/22/en/content/topics/2-window/window_videofx.htm
  (17.09.2026; the same page in edition 23 was checked during F-HLP-031)
- DaVinci Resolve 18.6 Reference Manual, sections "Using the Effects Library" and
  "The Toolbox" — "Hover Scrub Preview" in the options menu, the preview in the viewer on
  clips at the playhead, the animation of Fusion titles right in the thumbnail:
  https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1151.htm
  (17.09.2026 — ONLY through search results: direct reading of the mirror gives 404, not
  confirmed by the primary source)
- Blackmagic Forum, thread "Media Thumbnail Hovering" (advice to untick the box for smoothness):
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=175707
  (17.09.2026 — the forum returns 403 on reading, the content is known from the results)
- Boris FX forum, threads 8230 and 13225 — the quotes above (checked during F-HLP-031,
  07.09.2026)
- Premiere and After Effects — secondary: `helpx.adobe.com` does not open from this environment
  (the same unavailability recorded in F-HLP-031). None of the five editors was checked live.

**Implemented in R.54** — `docs/porting-notes.md` items 427-433 (t543-t551), the live scenario
`src/scripts/live/fxpicker.py`; the further life of the feature is recorded there.
