## ADDED Requirements

### Requirement: A title runs a timecode, a stopwatch or a countdown
A title SHALL have a source: its typed text, or a value that runs with the event's material time — a
timecode at its frame rate, a stopwatch counting up from its start, or a countdown counting down from
its start to zero and holding there. A stopwatch SHALL show what has passed and a countdown what is left
(rounded up). The typed text SHALL be a template where `{}` stands for the value; without `{}` the value
SHALL be the text. Every text property SHALL apply to it. The title's window SHALL choose the source and
the format, and SHALL show the start and the frame rate only where they count.
Why: `v26.10` G5 (the technical group's "timer/counter") and G3 (the owner: the timecode is text,
controlled as text).

#### Scenario: The value runs with the material time
- **WHEN** titles with a timecode from 1 h, a stopwatch "Lap {} s", a 5-second countdown and a typed text are read at several ticks
- **THEN** the timecode reads 01:00:01:15 at 1.5 s; the stopwatch keeps the words; the countdown reads 0:05 at 0.2 s, 0:01 just before 5 s, 0:00 at and after 5 s; the four formats write their forms; the typed title ignores the time
- **PATH** a Text generator → its window → Text: Countdown → the preview counts down from the start
- **TEST** t3703 (`src/libs/kt-model/tests/text_source.rs`), live scenario `src/scripts/live/text_counter.py` (`./scripts/live-check.sh`)

#### Scenario: A running title draws its value
- **WHEN** a 10-second countdown title is drawn at 0 and 3 s; a typed title at both
- **THEN** the countdown's two frames differ and each moment repeats its bytes; the typed title's frames are equal
- **PATH** play over a countdown title → the digits change frame by frame
- **TEST** t3706 (`src/libs/kt-media/tests/gen_technical.rs`)

#### Scenario: The window chooses the source
- **WHEN** a Text generator is created from Edit and its source set to Countdown in its window; then back to the typed text
- **THEN** the source is a countdown; the Format row and the Start field show; with the typed text the Start field is gone while the grid is in view; every new text exists in all five catalogs
- **PATH** Edit → Create a generator… → Titles and countdown → Text → Text: Countdown → Format: 1:05 … and Start (s) appear → Text: Typed → they go
- **TEST** t3705 (`src/apps/kt-app/src/gen_audio_tests.rs`)
