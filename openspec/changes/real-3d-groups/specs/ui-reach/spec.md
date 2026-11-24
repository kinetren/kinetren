## ADDED Requirements

### Requirement: 3D is switched on a track and parallax is made in one gesture
A video track's header SHALL carry a 3D switch; neighbouring 3D tracks SHALL be bracketed as one group on the
headers; a selected member SHALL show a depth handle on the preview; "Split into depth layers…" on an event with masks
SHALL make a 3D group of its pieces in one undo step.
Why: Q2 (a); the owner's parallax with custom shapes.

#### Scenario: Make a parallax from a photo
- **WHEN** a photo event with two masks gets "Split into depth layers…", the far piece's depth handle is dragged back, and the camera is moved
- **THEN** three 3D tracks bracketed as one group appear; the far piece shrinks into depth; moving the camera shows the pieces shift against each other
- **PATH** right click the photo event → "Split into depth layers…" → three new tracks with a "3D" bracket on their headers → click the sky piece → a depth handle on the preview → drag it upward → the sky recedes → move the camera block's position → the pieces slide against each other
- **TEST** t3955 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `parallax3d`

#### Scenario: A 3D group at rest costs nothing
- **PATH** the heavy showcase with two of its tracks switched to 3D → Space plays a lap → Space stops → the mouse rests over the preview, moves over a member, and the effects window is open: nothing redraws by itself
- **WHEN** a paused project with a 3D group stands with a member selected and its effects window open
- **THEN** no frame is composed at rest and a hover composes none
- **TEST** t4150 (`src/apps/kt-app/src/idle_tests.rs`)
