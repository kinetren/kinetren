---
id: F-MSK-007
title: A colour taken from the screen with an eyedropper — the key colour from the preview's frame
status: done
priority: B
cost: S
category: masks/keying
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: [12]
ui_rules: [1, 10, 14, 16, 17]
round: features-slow
point: change chroma-key-eyedropper
tests: [t3120, t3121, t3122]
depends_on: [F-MSK-004]
trackers: []
services: []
checked: 2026-11-19
---

# F-MSK-007 — A colour taken from the screen with an eyedropper — the key colour from the preview's frame

## Essence

The chroma key's colour was three numbers and the system colour picker; the person had to guess
the screen's green. The owner (kkt-26.11.01b line 3, row B2): "greenscreen не имеет пипетки, чтобы
ткнуть в тот же кадр превью или еще где-либо взять цвет с экрана". Picking the key from the frame is
"the first five seconds of work" of every key (F-MSK-004) — and every other colour a person sets
(a title's fill, a tint, a transition's colour) is the same gesture.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Chroma Keyer — the eyedropper beside Key color | the eyedropper samples the Video Preview (a drag samples an average over a rectangle); the same eyedropper sits in every colour picker of the program |
| Premiere | 26 | Ultra Key — Key Color eyedropper | the eyedropper samples the Program Monitor; Ctrl-click averages a 5×5 area |
| Resolve | 21 | Qualifier pickers in the Color page, the 3D Keyer | picks are drawn on the viewer; the Edit page's colour swatches have a picker that samples anywhere on screen |
| Final Cut Pro | 12 | Keyer — Sample Color | the key samples itself when added; Sample Color draws rectangles on the viewer |
| After Effects | 26 | Keylight Screen Colour, every colour property | an eyedropper beside every colour property samples the Composition panel — or anywhere on the screen |

## The trick behind it

- An eyedropper is a MODE, not a widget: a click that would act on whatever is under it (the viewer
  toggles playback, a card is placed) must be taken by the mode and do nothing else.
- What a person points at is what they SEE: reading the screen's pixel works for the viewer, a
  thumbnail and any other window alike, without asking each widget for its colour.
- The same eyedropper on every colour of the program (Vegas, AE): a key-only pipette teaches a
  gesture the rest of the program then refuses.

## Pros

- The key colour is right at the first click, from the frame itself.
- One gesture for every colour of the program.

## Cons

- A single pixel, not an average: noise in the screen's green moves the key a little (an averaged
  rectangle is the next step, with B3's automatic analysis).
- Linux Wayland gives a program no screen pixel: the eyedropper says so and the colour button stays.
- Outside the program's window it does not reach (a pointer grab is not taken).

## What users say

NO QUOTES: no third-party complaint was searched; the owner's own words (quoted above) are the signal.

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-controls/ — the contract is in `spec.md`, the "why" in `design.md`.
`eyedropper.rs`: the pipette beside every colour button (`widgets::rgb_button`, `color_row`,
`color_row_rgba`, a composition's colour parameter), one transparent pick layer while armed, the
screen pixel read through X11 `GetImage` or Windows `GetPixel`; Esc puts it down.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage |
| Where the main work is | kt-app `eyedropper.rs`, `widgets.rs`, `fx_editor.rs` |
| Project format | does not touch |
| Tests that will have to be written | a pick from the preview lands in the key as one step and does not play; Esc; every colour button has it |
| Depends on | F-MSK-004 |
| Breaks compatibility | no |

## Verdict

Take: a screen eyedropper beside every colour of the program, the key colour first.

## Sources

- https://www.vegascreativesoftware.info/us/vegas-pro-help/ (Chroma Keyer, access date 2026-11-19)
- https://helpx.adobe.com/premiere/desktop/ (Ultra Key, access date 2026-11-19)
- https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_Manual.pdf (qualifier pickers, access date 2026-11-19)
- https://support.apple.com/guide/final-cut-pro/ (Keyer, access date 2026-11-19)
- https://helpx.adobe.com/after-effects/ (colour properties, Keylight, access date 2026-11-19)
