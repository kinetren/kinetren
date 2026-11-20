## ADDED Requirements

### Requirement: A track's sound link raises the file by content
A video track's sound track SHALL be written as `"sound"` with the audio track's id in the project
file and in the track envelope of the exchange file. A project with a link SHALL be written as project
format 6 and as `metadata["kt"]` v17; a project without one SHALL keep its former version and text. An
older build SHALL refuse such a file naming its version. A file whose link names a missing or
non-audio track, or an audio track another video track names, SHALL be refused with the track's
address.
Why: an older build would drop the link on its next save and leave the halves unmirrored.

#### Scenario: The link in the project file and the exchange file
- **WHEN** a project with Video 1 linked to Audio 1 is written and read, exported to `.otio` and imported; the same project without the link is written; a file naming a video track as a sound track is read
- **THEN** the first reads back equal at format 6, the exchange file carries `sound` at v17 and imports equal; the second keeps format 1 and its text; the third is refused with the track's address
- **TEST** t3233 (`src/libs/kt-model/tests/sim_pairs.rs`), t3234 (`src/libs/kt-otio/tests/convert.rs`)
