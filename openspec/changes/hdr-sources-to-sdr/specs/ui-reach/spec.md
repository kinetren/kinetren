## ADDED Requirements

### Requirement: The picture precision is set in the project's and a composition's settings
Project settings SHALL offer "Picture: 8-bit (fast) / 16-bit float (HDR)". A composition's settings SHALL
offer the same with "As the project". The chain SHALL show an "8-bit" mark on an effect that runs as an
island in a 16-bit chain, with a tooltip saying that HDR highlights are limited through it.
Why: kkt-26.11.20 line 33; hdr-sources-to-sdr D8, D12.

#### Scenario: Switching a project to 16-bit
- **PATH** Project settings → Picture: "16-bit float (HDR)" → OK: the preview redraws, the HDR clip's highlights roll off instead of clipping → open an effect chain holding Glow: its row shows "8-bit"; hovering it says the highlights are limited
- **WHEN** a project with an HDR clip and a Glow effect is switched to 16-bit
- **THEN** the setting is saved, the preview shows the tone-mapped 16-bit composite, and the chain marks Glow as an island
- **TEST** t3898 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A 16-bit project at rest costs nothing
- **PATH** the heavy showcase switched to 16-bit → Space plays a lap → Space stops → the mouse rests, moves over a static place, and an effects window is opened → nothing redraws or works by itself
- **WHEN** the 16-bit heavy showcase stands paused with an effects window open
- **THEN** no frame is asked at rest and no job runs
- **TEST** t3899 (`src/apps/kt-app/src/idle_tests.rs`)
