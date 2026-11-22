## ADDED Requirements

### Requirement: The Library window lists everything the program can download
View → Library… SHALL list every library item. Each row SHALL show:
- its name, kind and version, its size and licence;
- what in the program needs it;
- its state: not installed, downloading with a percentage, installed, "needs …", or "broken: …";
- the buttons Download, Cancel, Remove, Open folder and Verify.

The FFmpeg row SHALL also show Locate folder… and the search plan's report. The header SHALL show the
total size and "Install from file…". The FFmpeg window's menu item SHALL open the library on its row.

A feature that lacks an item SHALL show one line naming the item and its size, with a button that opens
the library on that row. Such features are subtitles from speech, a title in a pack's font and a project's
plugin. The window SHALL run no job while open, apart from a download the person started.
Why: kkt-26.11.20 line 16; one place to see, get and reclaim what is downloaded (F-EXT-006; download-library D5).

#### Scenario: The rows and a download from the window
- **PATH** View → Library…: the FFmpeg row, two speech rows and two font rows with sizes → Download on the small speech model: the row shows a bar and a percentage, Cancel beside it → it ends: the row reads "installed" and the header's total grows → Remove: the row reads "not installed"
- **WHEN** the window opens with a test catalogue and a loopback server, a model is downloaded and removed
- **THEN** the rows match the catalogue, the states follow the job, the total follows the disk, and nothing else runs
- **TEST** t3857 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A feature's missing item opens its row
- **PATH** a title is set in a Chinese font from a pack that is not installed: the preview shows the title's box hatched with the pack's name → the line "Needs the CJK font pack (19 MB)" → its button: the Library opens with that row selected
- **WHEN** a feature lacks its item
- **THEN** its line names the item and its size, and the button opens the library on that row
- **TEST** t3858 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The FFmpeg window's item opens the library
- **PATH** View → FFmpeg…: the Library opens on the FFmpeg row, with Locate folder… and the search plan's report
- **WHEN** the former FFmpeg menu item is used
- **THEN** the library opens on the FFmpeg row and the search plan's report is the one the FFmpeg window showed
- **TEST** t3859 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The Library window open costs nothing at rest
- **PATH** the heavy showcase opens → View → Library… → the mouse rests over the list, then moves over it → nothing redraws or works by itself
- **WHEN** the window stands open with no download running
- **THEN** no job runs and no repaint is asked beyond the mouse's own
- **TEST** t3869 (`src/apps/kt-app/src/idle_tests.rs`)
