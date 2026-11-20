## ADDED Requirements

### Requirement: The shapes and plates
The Shapes group SHALL offer a polygon, a star, an arrow, a border frame, a lower-third plate and a progress
bar, transparent, drawn where their rows say in fractions of the frame. A star SHALL keep its valleys empty
by its inner radius; a plate's reveal SHALL hide its part right of the revealed fraction; a progress bar
SHALL fill with the material time over its fill time. Every colour SHALL have its eyedropper with its own
alpha, and every row SHALL be captioned in all five catalogs.
Why: `v26.10` G5 — the owner's shapes-and-plates group: "star/polygon/arrow, lower thirds, frames,
progress bar".

#### Scenario: Each stands where its rows say
- **WHEN** each shape is drawn at its defaults; a star with inner radius 0.2; a plate revealed to half; a progress bar at a quarter of its fill time and at its end
- **THEN** each draws over a transparent frame; the star's valley is empty where the polygon is full; the plate's right part is hidden; the bar is filled to a quarter, then fully
- **PATH** Edit → Create a generator… → Shapes → Lower-third plate → a bar with an orange stripe at the lower third → a curve on Reveal → it slides in
- **TEST** t3709 (`src/libs/kt-media/tests/gen_shapes.rs`), live scenario `src/scripts/live/gen_shapes.py` (`./scripts/live-check.sh`)

#### Scenario: Reached from Edit, captioned, each colour its own alpha
- **WHEN** each is created from Edit › Create a generator › Shapes; the passports and colour groups are read
- **THEN** each is one picture event of its kind; every row is captioned in five languages; every colour component, the alphas of two-colour kinds included, lands in its own eyedropper
- **PATH** Edit → Create a generator… → Shapes → Progress bar → its window: Track and Fill, each with its eyedropper and transparency
- **TEST** t3710 (`src/apps/kt-app/src/gen_audio_tests.rs`), t508 (`src/apps/kt-app/src/reach_tests.rs`)
