# Spec Delta

## ADDED Requirements

### Requirement: Project settings window — one body with quick access
Project settings SHALL be edited in the "Project settings" window: the raster by two validated fields
(even, 16…8192), the frame rate — a list of divisors of 90 000 in 1…120 plus a field with the validation
`90000 % fps == 0` (an inexpressible value — a refusal with a reason in the error style, not silence), the audio rate, the default conform rule, the pitch engine, the segment shape.
The combo box in the preview panel SHALL remain quick access to the same commands (one body,
rule 17). The window SHALL open from the project menu and by the address in the status line, return
focus when closed, and open again. Applying the window SHALL set the project's explicit-settings flag.
Why: "at minimum there is no project settings window as in serious programs" (the owner,
R.62); five resolution presets express neither 1440×1080 nor a vertical project.

#### Scenario: Validated fields and reopening
- **WHEN** the window is opened twice in a row; 1441×1080, 29.97 fps, 1440×1080, 50 fps are entered
- **THEN** the odd width and 29.97 are rejected in the error style with a reason, 1440×1080/50 are applied with one command, the preview combo shows the same, focus is returned, the explicit-settings flag is set
- **TEST** t720b (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Source setup — the first clip sets the project, a mismatch is reported
Placing a video on the timeline of the root, where there are NO events and the project settings are not explicit, SHALL
adopt the project's raster equal to the source's view (with pixel aspect ratio, aligned to even) and the source's fps, if it divides 90 000; otherwise fps is not touched and the status line says why. Inside a reel the rule does not apply. Explicit settings MUST NOT be changed by
placement (rule 20). Placing a source whose view differs from the container's raster by more than 1 % SHALL report in the status line (one hint in one place) that the source is conformed by the project's rule and where this can be changed ("Event geometry / Project settings"); there MUST NOT be badges or questions (rule 4).
Why: Premiere asks "change the sequence settings to match the clip", Kdenlive — "adjust project settings to match the clip"; silence is the worst part of today's defect (rule 16).

#### Scenario: An empty project takes the settings of the first clip
- **WHEN** 1440×1080/25 is dropped into an empty project with implicit settings; then after applying the settings window 640×480/30 is dropped; then 29.97 is dropped into an empty project
- **THEN** the first case — the project 1440×1080/25 in one undo step together with the placement; the second — the settings are untouched; the third — the raster is adopted, fps is as before, the hint names the reason
- **TEST** t721 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A mismatch is reported with its address
- **WHEN** 4:3 is dropped into a 16:9 project; then 16:9
- **THEN** in the first case the hint names the rule and the address "where to change it", in the second there is no hint; the event is placed in both cases
- **TEST** t722 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Leading node in the chain strip and thumbnails in source proportions
The chain strip of a video event SHALL show the leading "Geometry" node first on creation,
with the leading flag; the cross on the leading node SHALL give an error with a reason, not be grayed out;
the catalog SHALL contain "Geometry" as an addable node; the conform rule SHALL be localized in all five catalogs. Filmstrip thumbnails SHALL be ordered as the source's view
in the thumbnail's frame and drawn with the row's color margins; scrolling and zoom still do not generate
decode requests.
Why: the combo `Fit/Fill/Stretch` showed `Debug` names and did nothing; the filmstrip of a vertical clip was squashed — the first place where the user could see the defect.

#### Scenario: The leading node in the editor
- **WHEN** the chain editor of a new video event is opened; the cross on the leading node is pressed; the node is dragged after Blur; a second "Geometry" is added
- **THEN** the leading node is first and marked; the cross is an error with a reason; reordering and the second node are one history step each; the rule labels come from the catalog, the glyphs are in the font
- **TEST** t716b (`src/apps/kt-app/src/layout_tests.rs`), `i18n_tests.rs`, `font_tests.rs`

#### Scenario: Changing the rule on pause rebuilds the preview
- **WHEN** the project is paused, and in the settings window the rule is changed from "fit" to "fill"
- **THEN** the preview is rebuilt without any cursor movement: a pixel of the margin becomes a pixel of the source; the project version is part of the preview key
- **TEST** t722b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/conform.py`

#### Scenario: Thumbnails of a vertical clip
- **WHEN** a vertical clip is on a track with a filmstrip; the timeline is scrolled and zoomed
- **THEN** the thumbnail is in the source's proportions with side margins (centered in the cell), the sidecar table `thumb_meta` carries the thumbnail size of the media (old squashed ones are not read), there are no decode orders on scroll/zoom
- **TEST** t719 (`src/apps/kt-app/src/thumbs.rs`, `src/apps/kt-app/src/layout_tests.rs`)
