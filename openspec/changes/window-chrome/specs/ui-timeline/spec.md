## ADDED Requirements

### Requirement: Nothing of the main window is painted over a tool window
The glow of the preview and the meters, the status hint and the title text SHALL be painted in one layer above the
panels and below every window, each glowing thing as one mesh; a layer of the `Foreground` or `Tooltip` order SHALL
exist only for a use listed with its reason.
Why: the owner, v26.10 W1 and T9 — the glow and the title showed through an opaque window, the glow only partly.

#### Scenario: A window over the meters and the title shows neither
- **WHEN** the effects window is moved over the meters column and to the top edge over the title, with the glow on
- **THEN** inside the window's rectangle no pixel of the glow or the title differs from the window's own background; outside it the glow is unchanged
- **PATH** open the effects window → drag its title over the meters → the window is opaque, the meters' glow ends at its edge → drag it to the top over the project title → the title does not show through
- **TEST** t3936 (`src/apps/kt-app/src/layout_tests.rs`), t3935 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `window_over_glow`

### Requirement: A window started maximized drags at a normal size
A window the window manager maximized at start SHALL be known as maximized; a drag of its title strip SHALL first
restore it, to its remembered normal size or, when it had none, to 80 % of the work area centred under the pointer,
and then move it.
Why: the owner, v26.10 T5 — "оно начнет перемещаться в том же размере".

#### Scenario: Started maximized, dragged
- **WHEN** the program starts maximized with no remembered size and its title strip is dragged; then it is maximized again and dragged
- **THEN** both drags move a window of 80 % of the work area under the pointer
- **PATH** start the program maximized → drag the toolbar's empty space → the window becomes smaller and follows the pointer → double-click to maximize → drag again → the same normal size
- **TEST** t3937 (`src/apps/kt-app/src/layout_tests.rs`), planned live scenario `max_drag`

### Requirement: A narrow toolbar scrolls with the wheel
Over the toolbar row the wheel SHALL move the row sideways when it does not fit, with the "more" menu kept.
Why: the owner, v26.10 U10.

#### Scenario: Wheel over a narrow toolbar
- **WHEN** the window is 900 points wide and the wheel turns over the toolbar
- **THEN** the hidden groups slide into view; the "more" menu lists only what is still outside
- **PATH** narrow the window → hover the toolbar → turn the wheel → the row slides left, Render and Search come into view
- **TEST** t3938 (`src/apps/kt-app/src/layout_tests.rs`)
