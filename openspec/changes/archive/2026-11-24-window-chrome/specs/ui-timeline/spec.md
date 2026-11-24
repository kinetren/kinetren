## ADDED Requirements

### Requirement: Nothing of the main window is painted over a tool window
The glow of the preview and the meters, the resting status line and the project's title with its crumbs SHALL be painted
in one layer above the panels and below every window; a layer of the `Foreground` or `Tooltip` order SHALL exist only
for a use listed with its reason (a live status line among them: it may answer a control of the covering window).
Why: the owner, v26.10 W1 and T9 — the glow and the title showed through an opaque window, the glow only partly.

#### Scenario: A window over the meters and the title shows neither
- **WHEN** the effects window is moved over the meters column and to the top edge over the title, with the glow on
- **THEN** inside the window's rectangle no pixel of the glow or the title differs from the window's own background; outside it the glow is unchanged
- **PATH** open the effects window → drag its title over the meters → the window is opaque, the meters' glow ends at its edge → drag it to the top over the project title → the title does not show through
- **TEST** t3936 (`src/apps/kt-app/src/layout_tests.rs`), t988 `no_teaching_layer_over_the_ui` (`src/apps/kt-app/src/reach_tests.rs`), live `window_chrome`

### Requirement: A window started maximized drags at a normal size
A drag of a maximized window by any of its strips (the header, the top, the window buttons' row, the bottom toolbar)
SHALL first restore it and then move it, under the pointer; a window that has not been normal in this run SHALL be
given its remembered normal size (kept between runs) or, when it had none, 80 % of the monitor; the restore button and
the double click SHALL do the same; a window that comes up over the whole monitor without being maximized SHALL be
given its normal size and maximized in truth.
Why: the owner, v26.10 T5 — "оно начнет перемещаться в том же размере".

#### Scenario: Started maximized, dragged
- **WHEN** the program starts maximized with no remembered size and its title strip is dragged; then it is maximized again and dragged
- **THEN** both drags move a window of 80 % of the monitor under the pointer
- **PATH** start the program maximized → drag the toolbar's empty space → the window becomes smaller and follows the pointer → double-click to maximize → drag again → the same normal size
- **TEST** t3937 (`src/apps/kt-app/src/layout_tests.rs`), live `window_chrome`

### Requirement: A narrow toolbar scrolls with the wheel
Over the toolbar row the wheel SHALL move the row sideways when it does not fit, the transport and the timecode standing,
with the "more" menu holding what is not wholly in view; a window wide enough SHALL show the row unmoved.
Why: the owner, v26.10 U10.

#### Scenario: Wheel over a narrow toolbar
- **WHEN** the window is 900 points wide and the wheel turns over the toolbar
- **THEN** the hidden groups slide into view; the "more" menu lists only what is still outside
- **PATH** narrow the window → hover the toolbar → turn the wheel → the row slides left, Render and Search come into view
- **TEST** t3938 (`src/apps/kt-app/src/window_fill_tests.rs`), live `window_chrome`
