## ADDED Requirements

### Requirement: A window the window manager never took is asked for again
On X11 the program SHALL look for its own window until a window manager has taken it, and SHALL ask to map
it again while it is unmapped and no `WM_STATE` stands on it, at most once a second for two minutes; the
journal SHALL say how many times it asked. A window on screen, minimized or managed is never asked for.
Why: found-defects 830, 954 — openbox drops a map request that comes while it starts, and the program then
ran for ten minutes with no window on screen.

#### Scenario: What counts as lost
- **WHEN** the window is missing, mapped, unmapped but managed, or unmapped and unmanaged on the root
- **THEN** only the last is lost
- **PATH** minimize Kinetren's window from the panel → it stays minimized: the program asks for it no more
- **TEST** t3726 (`src/apps/kt-app/src/map_watch.rs`)

#### Scenario: A window manager that drops the first request
- **WHEN** the window manager ignores the program's first map request
- **PATH** start Kinetren while the desktop's window manager is still starting → within a second or two the window is on screen
- **THEN** the window comes on screen and the journal names the asks
- **TEST** live `map_lost` (`src/scripts/live/map_lost.py`)
