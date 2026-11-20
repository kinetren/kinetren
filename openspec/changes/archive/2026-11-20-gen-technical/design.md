# Design

## D1. One event, one tick for the picture and the beep

The flash period is the kind's one float (`interval` in seconds → ticks, in the preamble); the flash
length is `flash` frames of `fps`. The frame is white while `pts mod period < length`; the beep's
gate asks the same question of the sample's material tick (48 kHz → 90 kHz, exact), through the host's
`tone_1k` — no second sine, the phase is the global sample number. Between flashes a 1/60-height bar
fills over the interval: the next flash is seen coming. Integer pixels.

## D2. Guides as a transparent generator

Every editor keeps safe areas as a viewer overlay. Here they are a transparent kind on a track: seen
in the preview over the real picture, muted with their track, and burnt into an export only when a
delivery asks for a check copy. The outlines are fractions of the frame, so the preview quality does
not move them; the thickness is a fraction of the height, at least one pixel. The colour is a
`color_r/g/b/a` group with the eyedropper (white at 2/3 opacity).

## D3. The timer/counter goes to the Text generator

His G3 decision (2026-10-30, question 5): the timecode becomes a MODE of the Text generator so that the
whole text passport works on it. A counter is the same thing; a pixel-font counter kind would repeat the
defect G3 names. It is the next change, `text-counter-source`.

## Class walk

The mechanism: a kind that is a picture and a sound at once must change both on one tick — checked:
SMPTE's tone (continuous), the leader's beep (the same gate rule), the flash (new, same rule). A guide that
must not depend on the preview size — checked: the shapes (fractions), the table (columns by count),
the window (area fraction), the guides (fractions).
