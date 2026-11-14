## MODIFIED Requirements

### Requirement: The UI frame's cost is a number the person can see
The program SHALL measure each UI frame's own processor time on the UI thread, by stage — the frame,
the timeline (and its events), the media bin, the centre (and the preview's composite), the floating
windows — without charging a wait. Diagnostics → Performance SHALL show the last half second's
average as "UI frame N ms" and the stages; the journal's LOAD record SHALL carry it per second after
the work (", ui frame N ms"); the bench dump SHALL carry the totals for live scenarios. The measurement
steers nothing (invariant 11).
Why: the owner's report showed a thread at 100 % and the number of frames, never what a frame cost
or where: "АГЕНТЫ НЕ ДУМАЛИ" about the tool he has to bring the proof with (kkt-26.11.07b item 5).

#### Scenario: The stages add up per frame
- **WHEN** two readings of the totals are taken around some frames
- **THEN** the per-frame average of every stage is their difference over the frames, and no frame gives no number
- **TEST** live `ui_frame` (`src/scripts/live/ui_frame.py`)
