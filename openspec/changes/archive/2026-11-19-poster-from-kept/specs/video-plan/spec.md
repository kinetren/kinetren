## ADDED Requirements

### Requirement: An offline job takes the moment the preview already holds
A background job that wants a picture of a moment of the current project version — the project's poster
— SHALL first ask the player for what the preview holds for it (`Player::kept_picture_reader`): the
composite the display's executor keeps under the moment's plan key, read back off the UI thread, or the
disk tier's picture of it; only when neither exists, or the build is not the version asked, SHALL it
compose the moment itself.

Why: found-defects 767 — after every Ctrl+S the poster walked a sharp Datamosh's history again, 22.6 s of
a software device, for the moment the preview had just shown (poster-from-kept D1).

#### Scenario: A kept moment is read back without its history
- **WHEN** the preview has presented a moment over a cut mosh, the stream memo and the decoded frames are dropped, and the reader is run for that moment and for one never composed
- **THEN** the first gives the picture at the preview's size with no stream run, walk or decode request; the second gives nothing
- **TEST** t2978 (`src/libs/kt-engine/tests/decode_storm.rs`)
