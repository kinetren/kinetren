## ADDED Requirements

### Requirement: The Sound result is a table judged against the norm
The render window's result SHALL show each sound measurement on its own row with its unit, the chosen norm and a
state (within / tolerance / outside) by colour and mark; the section header SHALL count the rows outside the norm;
a hover SHALL say what the value means and what the norm is; the offset line SHALL say when raising to the target
would cross the ceiling, in the error style; the audio findings SHALL join the picture findings in one list with
jumps; changing the norm in the result SHALL re-judge without a new render.
Why: the owner — "на цифры вообще внимание не обратишь … оформить это получше надо"; rules 10, 16.

#### Scenario: A bad render is visible at a glance
- **WHEN** the project of t3919 is rendered with the norm Streaming and the result is read; then the norm is switched to EBU R128
- **THEN** the Sound header reads "4 outside the norm"; the Integrated, True peak, Balance and Silent channel rows are drawn in the outside colour; the findings list holds the three spans; after the switch the states change and no render starts
- **PATH** click "Render" → render → the result shows "Sound — 4 outside the norm" with red rows → hover "Balance" → the status bar explains it → click the silent-channel finding → the playhead jumps to it → pick "EBU R128" in the result's norm box → the rows re-colour
- **TEST** t3922 (`src/apps/kt-app/src/reach_tests.rs`), planned live scenario `render_norms`

#### Scenario: The norm is remembered
- **WHEN** the person picks Podcast, closes and reopens the render window
- **THEN** Podcast is selected
- **PATH** open the render window → pick "Podcast" in the norm box → close → reopen → "Podcast" is shown
- **TEST** t3923 (`src/apps/kt-app/src/reach_tests.rs`)
