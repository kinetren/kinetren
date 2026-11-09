## ADDED Requirements

### Requirement: A build resolves each reel instance once
Inside one build scope (`revisions_scope`: the player's build of a version, a fresh renderer build)
each reel instance — the reel and its override set — SHALL be resolved (morpher layers, links,
overrides) once and shared by the audio build, the video build, the decode step and the generator
sources.
Why: found-defects row 155 — each pass resolved every reel on its own, about five times per build;
the root's resolve is 7 ms on the owner's project.

#### Scenario: Three passes in one scope
- **WHEN** the generator sources, the audio and the video renderer of a project with a reel placed twice are built apart, then in one scope
- **THEN** apart they resolve at least six times, in one scope twice (the root and the reel)
- **TEST** t2005 (`src/libs/kt-engine/tests/shared_build.rs`)
