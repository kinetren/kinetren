# Design

## D1. One distance library, one shape per kind

The six texts share `sdf.wgsl` — a rounded box, a segment, a rotation, `cover` (a distance under a
feather) and `over` (straight-alpha compositing) — prepended at build time (`concat!`). A one-colour
shape ends in the prelude's `shape(d, feather)`; the plate and the bar composite their two colours with
`over`. Distances in pixels, positions as fractions of the frame, sizes as fractions of the height, so
the preview's quality does not move a shape (the rule of R.53 item 419).

## D2. A star is its own kind

The rule of a kind (R.53, item 419): a kind is its own passport when it has its own numbers. The star's
inner radius is a number the polygon does not have, so the star is a kind of its own (AE's Polystar
puts both in one tool and leaves the inner radius dead for a polygon — rule 9).

## D3. A reveal and a fill time, not animations

The plate's Reveal is a number 0..1 a curve animates, as the title's reveal is (R.59): the person keys
its slide in and out where the event needs them. The progress bar fills with the MATERIAL time over its
fill time (a live kind): "a bar that runs for N seconds" with no keyframes.

## D4. Each colour's own alpha (found by t508)

`gen_ui::color_alpha` returned the FIRST colour group's alpha, and the form passed it to every group:
with two colours with alpha, the second eyedropper wrote the first one's alpha row and its own alpha
stayed unreachable — t508 named `plate.color2_a`. `color_alpha_of(kind, names)` finds the group's own
`<stem>_a`; the form and t508 ask it per group. The shapes with one colour are unchanged.

## Class walk

The mechanism: a shared per-kind value handed to every group. Checked: the colour groups' alpha (fixed),
the colour groups' captions (per group already), the hex field's memory id (per `names[0]`, per group).
