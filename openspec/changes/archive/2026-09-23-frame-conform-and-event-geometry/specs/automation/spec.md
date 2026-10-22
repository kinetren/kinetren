# Spec Delta

## MODIFIED Requirements

### Requirement: One animation mechanism for all carriers
Camera curves and transition parameter curves SHALL be the same `ParamCurve` as for effects, and
drawn by the same widget; the automation widget MUST NOT know about the clip — it receives the
specs, the axis range in the curve's domain, and a pair of functions "curve domain ↔ timeline tick". The domains
of the carriers are their own and are recorded: a clip — material, a camera — tick from the start of the event, a transition — a
tick from the start of the window; a key outside the window is legal, editing the length opens/hides keys. A curve
on a nonexistent parameter fails the load. Legacy keyframes (CamMotion, cameras, slowed)
are converted on read and are never written.
Why: each carrier had its own keyframe editor (slowed — a graph, pan3d — a table), and they drifted apart; "CamMotion with keyframes" is the "Geometry" node with curves
(before R.62 — Pan with curves).

#### Scenario: A camera is animated with shared curves
- **WHEN** a camera has legacy keyframes, and then curves of the same kind as for effects
- **THEN** the conversion gives the same values as the old mechanism; transferring the camera carries the animation; an orphaned curve fails validation
- **TEST** t231 (`src/libs/kt-model/tests/cameras.rs`), t231 `t231_camera_editor_opens_and_edits` (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Legacy CamMotion migrates to Pan curves
- **WHEN** a project with CamMotion keyframes of the old format is read
- **THEN** (the scenario's heading is historical: Pan was abolished in R.62) they become the curves `pos_x`/`pos_y`/`zoom` of the "Geometry" node (not the leading one, on the same slot; `pos = pan_to_pos(pan, zoom)`), and rendering by the curves gives the same picture the old mechanism gave, within the resampling tolerance (mean deviation < 4/255; at zero with zoom 1 — byte for byte): the node stores the position and places the layer with a continuous affine; the legacy interpolated pan and zoom separately and cut an integer frame — byte-for-byte is unreachable by construction, and the owner did not require backward compatibility (R.62); CamMotion is not written back to the file
- **TEST** t230 (`src/libs/kt-engine/tests/fx_auto_render.rs`)

#### Scenario: Transition parameters are animated by the same widget
- **WHEN** a transition's parameter curve is of the kind (domain — tick from the start of the window)
- **THEN** the picture changes along the curve; at the start of the window the value equals the static one; a key beyond the window's edge is legal (the contract of the transition's kind — capability `transitions`)
- **TEST** t232 (`src/libs/kt-engine/tests/transitions_video.rs`)
