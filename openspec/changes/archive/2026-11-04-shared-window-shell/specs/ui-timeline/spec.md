# Spec Delta

## ADDED Requirements

### Requirement: A window does not grow by itself
The width and height of a window SHALL change only by the person's resize gesture or by content
that became larger in its own right (a slot added, a group expanded, a longer name). Repeated
frames with the same content — pointer motion, hover, playback, idle repaints — MUST NOT change the
size of any window by more than one physical pixel in total, at any display scale, including
fractional ones and the program's own zoom (1.1, 1.2, 1.25, 1.3, 1.5). A width once reached by such growth is not a "size the person
chose" and SHALL NOT be remembered as one.
Why: kkt item 2 (2026-10-27), the owner: "on my other system the morpher window endlessly tries
to grow in width; other windows don't do this. Move the mouse anywhere in the main window or the
morpher window — it widens fast; don't move it — slowly." Growth that follows the frame count
is a ratchet: every frame may only add, never take back (egui 0.36 `Resize` keeps the maximum of
the requested and the last content size). The R.67 lesson "one scroll per window" removed the
window-level scroll from the FX and transition windows, but the morpher, camera, notices and
diagnostics windows kept it, because nothing held the rule for all windows. Found 2026-10-28: the
window-level scroll under the program's style grows the morpher window one physical pixel per frame
at zoom 1.1 (the owner's first Ctrl+=), 1.3, 1.35, 1.85 — and a frameless button grows two points
while hovered, which a content-sized window keeps for good.

#### Scenario: Every window keeps its size over frames at fractional scales
- **WHEN** each window of the registry that the harness can open (the morpher, transition and camera windows in their own scenes) is opened at display scale 1.0, 1.1, 1.2, 1.25 and 1.5, and 300 frames are run with the pointer moving over the main window and over the window itself
- **THEN** the window's outer rectangle at frame 300 differs from the one at frame 3 by at most one physical pixel in width and height; a window that fails is named with its scale and both widths
- **PATH** open the morpher window from the morph block's double-click → move the mouse over the timeline and over the window for five seconds → the window's right edge stays where it was
- **TEST** t918 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The owner's morpher scene does not widen
- **WHEN** a morpher layer in batch mode with a chain of twelve slots, two area references and an open curve graph is opened at the program's zoom 1.1, 1.2, 1.25, 1.3 and 1.5 (the program's own style applied); 600 frames are run with the pointer moving over the main window, then the same over the morpher window, then the same with no pointer motion and a frame every step
- **THEN** the width at the end equals the width at frame 3 within one physical pixel in all three runs; the window-level scroll is absent (one body scroll); the test fails on the code before the change in the same scene that the first investigation stage recorded as growing
- **PATH** put a morpher layer on a morph track → double-click it → the morpher window → add effects until the strip wraps → open a curve → move the mouse around the main window → the morpher window's width does not change
- **TEST** t923 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/winshell.py` (the real program, Ctrl+= once and twice — zoom 1.1 and 1.2, the owner's case)
