## ADDED Requirements

### Requirement: A failed cell of a gallery is one cell
A panic while one cell of a gallery is computed SHALL be that cell's failure: it stays Loading and is not
asked again until the windows close, and the cells behind it come.

Why: found-defects 843 — one cell's panic poisoned the lock of the scene, and every later cell of the
session panicked on it.

#### Scenario: One broken cell
- **PATH** An event's effects window → + effect… → one cell stays Loading → the cells beside it fill → pick another effect → its gallery fills too
- **WHEN** the computing of one cell panics
- **THEN** it is marked failed and the next cell comes
- **TEST** t3264 (`src/apps/kt-app/src/fx_thumbs.rs`)
