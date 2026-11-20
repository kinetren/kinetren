## ADDED Requirements

### Requirement: The Memory section draws bars by section, and the status counter explains itself
The Diagnostics window's Memory section SHALL draw the resident memory as one bar of sections (sorted by size,
"not counted" last), the video memory as a second bar and the project's disk stores (sidecar index, cache folder,
recovery file) as a third, each segment named with its size on hover and listed under the bars; the status bar's
memory counter SHALL keep one number whose hover lists the largest sections and the video memory; nothing SHALL be
sampled while the section is closed.
Why: the owner, v26.10 Q1–Q3.

#### Scenario: Read where the memory went
- **WHEN** the person opens Diagnostics on a played project and hovers the largest segment, then hovers the status bar's counter
- **THEN** three bars show; the hover names "decoded frames 1 100 MB" with what it is; the status hover lists the top three and the video memory; closing the section stops the disk walk
- **PATH** click Diagnostics → the Memory section shows three bars: memory, video memory, project on disk → hover the widest segment → the status bar says "Decoded frames: 1 100 MB — frames kept to play without decoding" → hover the "memory: 3 200 MB" counter at the bottom → it lists the three largest sections and the video memory
- **TEST** t3964 (`src/apps/kt-app/src/reach_tests.rs`), t3965 (`src/apps/kt-app/src/layout_tests.rs`)
