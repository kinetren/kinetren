## ADDED Requirements

### Requirement: A frame's serial is unique across caches
The serial number of a frame in `BurstCache` SHALL be unique in the process, whichever cache gave it.
Why: the offline engine serves a fresh cache per job and addresses an uploaded frame by its serial;
with a serial per cache it could take a frame of one project for another's (design D6).

#### Scenario: Two caches share no serial
- **WHEN** frames are inserted into two caches
- **THEN** no serial appears in both
- **TEST** t1574 (`src/libs/kt-cache/src/lib.rs`)
