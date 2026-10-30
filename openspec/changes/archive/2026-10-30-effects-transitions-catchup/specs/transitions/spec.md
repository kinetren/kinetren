# Spec Delta

## ADDED Requirements

### Requirement: The transition vocabulary grows only through the ranked catch-up waves
New transition kinds SHALL arrive only through a wave of the F-FX-007 list (today W5, "transitions
catch-up": a luma wipe with a user image, more iris shapes, warp transitions, motion-blurred
push/slide), each wave its own change built as transition plugin crates whose CPU reference and
WGSL piece live in one crate (capability `plugins`). The note SHALL list all built-in transition
ids, and the waves SHALL be ordered so that no wave depends on a wave ranked after it. A wave
that reaches for an idea the owner did not choose earlier (a transition progress curve, stylised
audio transitions, a gallery of live previews) SHALL carry an explicit `Revision:` line naming a
new request of the owner.
Why: the 33 kinds were the owner's "modern mix" choice (R.51) and already cover most of the
CapCut 2026 trend list; what is missing is specific and short, and the earlier refusals were the
owner's own decisions — they change only on a new request (openspec config, design rule).

#### Scenario: The transition list and the wave order are checked
- **WHEN** the guard compares the transition list of F-FX-007 with the transition registry, and
  reads the dependencies of every wave
- **THEN** the list equals the registry (33 ids today) and the message names any missing or
  extra id; every wave's dependency on another wave points to an EARLIER rank; each wave names the
  family crate paths under `src/plugins/` it adds kernels to (or says "no new kernel") and at least
  one capability
- **TEST** t978 (`src/apps/kt-cli/tests/fx_catalogue.rs`), t982 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: An idea the owner refused needs a new request
- **WHEN** a wave of the change design mentions a transition progress curve, a stylised audio
  transition or a gallery of live transition previews
- **THEN** the wave carries `Revision: … — …` naming the owner's new request; without it the guard
  is red
- **TEST** t981 (`src/apps/kt-cli/tests/fx_catalogue.rs`)
