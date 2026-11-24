## ADDED Requirements

### Requirement: The Memory section draws bars by section, and the status counter explains itself
Diagnostics SHALL have a Memory section: the process's memory as one bar of sections by size with "not counted" last, the
video memory as a second (absent on a software rasterizer, whose bytes are in the first), and the open project's disk as a
third — the parts of its cache folder, its file and its recovery file — each segment named on hover in the status line and
listed under its bar. The status counter's hint SHALL expand the largest sections and the project's disk, and its click
SHALL open the section. There SHALL be no pop-up. Nothing SHALL be asked of the model, the disk or the textures while
neither is on screen.
Why: the owner, v26.10 lines 143–145 and kkt-26.11.20b B12.

#### Scenario: Read where the memory went
- **WHEN** a played project's Diagnostics is opened, the widest segment hovered, then the counter hovered and clicked with Diagnostics closed
- **THEN** the bars stand with their lists and each bar's segments add up to its total; the status line names the segment; the counter's hint lists sections of both and the disk; the click opens Diagnostics at the section; no layer of the Tooltip order exists
- **PATH** View → Diagnostics… → the Memory section: three bars → hover the widest segment → the status line "Decoded frames: 1.1 GB — kept to play without decoding" → close the window → hover "3.2 GB · VRAM 1.1 GB" at the bottom → the line expands "Memory 3.2 GB: decoded frames 1.1 · …" → click it → Diagnostics opens on the section
- **TEST** t3964 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `memory`

#### Scenario: The project's disk
- **WHEN** the section is opened on an untitled project, on a saved one whose folder is not counted yet, and on one sharing its folder with another project; a segment's link is clicked
- **THEN** it says in turn that there is no folder yet; "counting…", then the parts; the other project's name; the link opens Preferences at the persistent cache; with the section closed the disk is not read
- **PATH** Diagnostics → Memory → the third bar "Project on disk": waveforms and filmstrips, preview frames, copies… → hover → "…rebuilt when needed" → click "Persistent cache…" → Preferences opens there
- **TEST** t3963 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: Nothing is asked while nothing is shown
- **WHEN** the program plays and rests with Diagnostics closed and the pointer elsewhere, then the section is open on a pause
- **THEN** only the process's and the devices' two numbers are read, at most once a second and in frames drawn anyway; at rest none; the open section on a pause asks for no frame of its own
- **PATH** play with Diagnostics closed → the counter at the bottom changes once a second → pause and take the hand off → nothing is drawn → open Diagnostics → the section stands and does not flicker
- **TEST** t3965 (`src/apps/kt-app/src/idle_tests.rs`)
