## ADDED Requirements

### Requirement: A form uses the width it is given
Every form of parameter rows — effect, morpher, transition, camera, track, generator and title forms,
project settings and the render window — SHALL lay its rows into as many columns as its width holds without
a row falling below its readable minimum, reading top to bottom, with groups as cards that are never split;
widening a form SHALL never make it taller. A guard SHALL open every registered effect, transition and
generator kind and every registry window at a narrow, the default and a wide width and fail when the wide
form leaves more than a quarter of its width unused while it does not fit one screen, when a wider form is
taller, or when a row is narrower than its minimum; a form that cannot pass SHALL be listed with its reason.
Why: the owner, 2026-11-07: "половина ширины этой портянки - ПУСТАЯ … Я СТОЛЬКО РАЗ ПРОСИЛ НАВСЕГДА".

#### Scenario: The camera ISP form fills a wide window
- **WHEN** the camera ISP slot's form is opened at 420, at its default and at 1600 points wide
- **THEN** at 1600 its groups stand in several columns, unused width is at most a quarter, the form is shorter than at the default; at 420 every slider is at least 120 points
- **PATH** double-click an event with Camera ISP → its effects window → drag the window's right edge to the screen's width → the stage groups move side by side into columns, the sheet becomes short → drag it narrow → one column, every slider still usable
- **TEST** t3591 (`src/apps/kt-app/src/layout_tests.rs`), planned live scenario `form_width` (t3592)

#### Scenario: Every kind and window passes the width guard
- **WHEN** the guard lays out every registered kind's form and every registry window at the three widths
- **THEN** each passes the three checks or is listed with a reason of at least 25 characters; the list names only existing kinds and windows; on the code before this change the camera ISP and VHS tape forms fail
- **PATH** there is no gesture: the guard lays out every kind's form and every registry window at three widths and names each form that leaves width unused or grows taller when widened
- **TEST** t3593, t3594 (`src/apps/kt-app/src/reach_tests.rs`)
