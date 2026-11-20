## ADDED Requirements

### Requirement: Proxies are made, attached and switched where the media is
The bin card's right-click menu (and a selection of cards) SHALL offer "Make proxy", "Attach proxy…" and
"Remove proxy"; a card SHALL show its proxy state (ready, making with percent, none); a "Proxies" switch
next to the preview quality SHALL say how many of the project's files have a ready proxy; a menu item
that cannot apply (a generator, a still, a file without video) SHALL be disabled with its reason.
Why: UI rule 13 — on places already occupied; rule 16 — an impossibility shown as an error.

#### Scenario: Make a proxy and play from it by mouse
- **WHEN** a card's right-click menu → "Make proxy" is clicked, the job finishes, the "Proxies" switch is clicked, and a bin card of a generator is right-clicked
- **THEN** the card shows a percent, then "proxy" badge; the switch reads "Proxies 1/1" and the preview plays from the proxy; the generator's menu has "Make proxy" disabled with its reason
- **PATH** right click on the clip's bin card → "Make proxy" → the card shows 0…100 % → badge "proxy" → click "Proxies" beside the quality → the switch lights, "1/1" → play → the frame-time readout drops → right click on a generator's card → "Make proxy" greyed with "a generator has nothing to decode"
- **TEST** t3447 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/proxies.py` (t3448)
