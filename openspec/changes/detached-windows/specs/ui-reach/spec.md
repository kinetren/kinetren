## ADDED Requirements

### Requirement: A tool window can live outside the main window in the program's own style
Every tool window of the registry SHALL offer a detach button in its title row; detached, it SHALL be an OS
window without the system's decorations, drawn with the shell's own title row, theme, cross and Esc behaviour,
its body the same as inside; the same button, or dropping its title onto the main window, SHALL dock it back.
Whether a window kind is detached and its size (and its position where the platform gives one) SHALL be kept in
the machine's settings, not the project. A detached window SHALL repaint only when its own content or input
changes, never because the main window plays. Dialogs and progress windows SHALL stay inside.
Why: the owner, v26.10 W2 — "за пределы основного окна, без потери производительности, и с полностью тем же стилем".

#### Scenario: Detach the effects window and play
- **WHEN** the effects window is detached, moved, a parameter changed in it, the project played for five seconds, the program restarted, and the window docked back with its button
- **THEN** the window stands outside the main window with the program's title row; the parameter change shows in the preview; during play the detached surface presents no frame while its chain is static and the main V-fps equals the run without it; after the restart it opens detached at its size (and place on X11); docked, it opens inside
- **PATH** double-click an event → the effects window → click the detach button in its title → the window becomes a separate window → drag its title onto the second monitor → drag a slider → the preview changes → Space → plays at full rate → restart the program → open the effects window → it opens outside → click the dock button → it returns inside
- **TEST** t3901 (`src/apps/kt-app/src/reach_tests.rs`), t3902 (`src/apps/kt-app/src/window_fill_tests.rs`), t3903 (`src/apps/kt-app/src/idle_tests.rs`), planned live scenario `detached`
