# Spec Delta

## ADDED Requirements

### Requirement: Mute and solo act on the file and are listed before start
Export SHALL apply the mute and solo of tracks the same way as the preview (the same renderers). The render window SHALL list the muted tracks and the active solo BEFORE the start, in the same place where unavailable sources are listed, with an address "where to change" (the track header); kt-cli SHALL print the same before rendering. There is no refusal — the owner's decision in R.62: mute and solo are project content.
Why: "forgot to clear solo — delivered one track" is a risk the owner accepted; the price is the duty to say so (rules 16, 19; Kdenlive #498560 asks for exactly this).

#### Scenario: The window and kt-cli list
- **WHEN** a project has one track muted and two in solo; the render window is open; `kt-cli render`
- **THEN** the list shows both things with the track names and the address; the render proceeds; kt-cli prints a notice; the file does not contain the muted track
- **TEST** t740 (`src/apps/kt-app/src/layout_tests.rs`), t740b (`src/apps/kt-cli/tests/cli.rs`)
