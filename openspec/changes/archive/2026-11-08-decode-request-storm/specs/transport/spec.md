## ADDED Requirements

### Requirement: A short loop over a stream stage plays, every decoded frame read
A loop over an event with a stream stage whose history does not fit the cache SHALL come to show frames
(V-fps above 0) and SHALL NOT drop a decoded frame before it is read; the decoders' work SHALL stay bounded
by a few times the history, not grow with the time the loop plays.
Why: the owner, kkt-26.11.08 line 14 (registry H8): "а loop-picture-throughput может связан с тяжелым
эффектом? типо я поменял там что-то и оно пошло считать?" — four loops in a row "V-fps 0.0, new frames 0"
after he selected Datamosh, ~20 000 requests a second (decode-request-storm).

#### Scenario: A cold loop over a cut mosh plays on model decoders
- **WHEN** an 8-frame loop 3 s into a cut mosh (144 lines, a 103-frame history) plays cold for 10 s through the real player with a 4 MB video cache, on model decoders that take 2 ms a picture and count their seeks
- **THEN** frames are shown, no entry is dropped before its first read, and the pictures decoded are at most four times the history plus eight GOPs
- **TEST** t2044 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A person plays a short loop over a Datamosh event
- **PATH** open the heavy showcase whose longest plain event carries a cut mosh at 1080 lines and a 0.4 s loop 3.3 s into it → the preview stands on the loop's start → press Space → the loop plays, the picture of the mosh comes and moves
- **WHEN** the live scenario runs on our fixtures
- **THEN** no 3 s window asks the decoders for far more than they decode, the loop shows frames, and the screenshots of each step are saved
- **TEST** t2045 live `decode_storm.py` (`src/scripts/live/decode_storm.py`, via `src/scripts/live-check.sh`)
