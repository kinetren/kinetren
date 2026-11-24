## ADDED Requirements

### Requirement: The memory numbers are in GB, say what they count and name the whole process
The status bar SHALL show the program's resident memory and the video memory in GB, nothing else of memory, with a hint that
says what each is made of; it SHALL NOT show a pair a person reads as used/limit. The Diagnostics window SHALL show the
decoded frames' occupancy of their bound now, the program's memory with the rest no section counts, and the Memory
section; the rest named in the first row and "not counted" in the section SHALL be one number. The process's and the
devices' memory SHALL be read at most once a second, in frames drawn anyway, and no figure appearing or growing a digit
SHALL change the window's height.
Why: found-defects 397 — "cache: 39/535 MB" was audio/video and read as used/limit (kkt-26.11.10 J4); the owner,
kkt-26.11.20b B12 — "к текущему добавить vram, и убрать лишнее, ну и в gb показывать": the frames pair left the bar.

#### Scenario: The status bar says the program's memory and the video memory
- **WHEN** the editor is open with a project, on a machine whose devices count their memory and on one without
- **THEN** the status bar shows "P GB · VRAM V GB" with one decimal, "≥" where the devices do not count, and P GB alone before any device; no "frames: V/cap" stands in the bar; the width of the item does not shift the items after it
- **PATH** look at the status bar → "0.6 GB · VRAM 0.2 GB"; hover it → the line expands what each is made of; play a loop → both grow
- **TEST** t2397 (`src/apps/kt-app/src/diag_tests.rs`)

#### Scenario: Diagnostics names the whole process
- **WHEN** the Diagnostics window is open
- **THEN** the first performance row gives the program's memory and the rest, the rest a part of it and equal to the section's "not counted"; hovering it, the status line names the rest; the Memory section stands under the occupancy bar
- **PATH** open View → Diagnostics… → the first row reads "Program 7254 MB · rest 897"; hover it → the status line: "Program: all the memory it holds. Rest: what no section counts…"; under the occupancy bar the Memory section, its last grey segment 897 MB
- **TEST** t2398 (`src/apps/kt-app/src/diag_tests.rs`)

## REMOVED Requirements

### Requirement: The memory numbers say what they count and name the whole process
**Reason**: the owner's B12 (kkt-26.11.20b lines 25–26) changes the status bar's numbers; the requirement "The memory numbers
are in GB, say what they count and name the whole process" holds what stays of it, with t2397 rewritten and t2398 kept.
**Migration**: none (interface only).
