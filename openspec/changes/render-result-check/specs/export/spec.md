## ADDED Requirements

### Requirement: The finished render's numbers are the announced ones and the file's
An export's result SHALL carry the frames and the samples it announced beside those it wrote. The sound written SHALL be
exactly the range's samples at 48 kHz, its last burst cut to the range's end. Where a pair differs, the result window's
final block and the telemetry journal SHALL name both numbers of the pair; the journal's line of a finished render SHALL
name both pairs always. The file's frame count SHALL be the frames written, and its sound SHALL be as long as the range
within one frame of the audio codec.
Why: the owner, v26.10 line 55 — "в окне рендера может быть в конце video 517/516 audio 412672/413364 - не понятно как
так".

#### Scenario: Frames and samples end at their totals
- **WHEN** ranges on the frame grid, off it, shorter than a burst and ending inside a burst are exported at 24 and 30 fps
- **THEN** the written frames and samples equal the announced ones each time; a result whose written numbers are lowered by hand reads both numbers of the pair in the final block
- **PATH** draw a region by hand → File → Render… → Render… → wait for "Done" → the final block reads one number of frames and no line about a difference
- **TEST** t3998 (`src/libs/kt-engine/tests/export_totals.rs`), live scenario `renderregion` (`src/scripts/live/renderregion.py`: the journal's two pairs are equal; the file's packets and sound length are the journal's)
