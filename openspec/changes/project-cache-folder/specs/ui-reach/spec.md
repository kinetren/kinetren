## ADDED Requirements

### Requirement: Settings show the caches on disk and their caps
Settings SHALL have a page "Caches on disk" that lists every cache folder the program made:
- its place;
- the projects it serves, a project no longer there marked so;
- its size, with a Delete button.

The page SHALL also show each folder's cap, the total the caches take now, and whether a folder is writing
in its cycle (full). Deleting the open
project's folder SHALL empty it and leave the work untouched. The page SHALL only read the registry and
the folders' sizes, and SHALL compute nothing while it is open.
Why: kkt-26.11.20 A14 moved the caches into the projects' folders; a cache the person cannot see or
reclaim breaks UI rule 20 (nothing grows by itself unseen); the owner's cyclic rule (kkt-26.11.20b B2) needs a place to see
and set it; project-cache-folder D3.

#### Scenario: The list, a deleted folder and a cap
- **PATH** Settings → "Caches on disk": two folders listed with sizes, one marked "no project here" → Delete on that row: the row goes and the total drops → the cap of the other row set to 2 GB: its size shows against it, marked full
- **WHEN** the page opens with two registered folders, one whose project is gone, then the orphan is deleted and the other folder's cap lowered below its size
- **THEN** the rows, sizes and marks match the registry, the deleted folder is gone from the disk and the list, the cap is saved, the folder is marked full and its next write drops its oldest entries, and the page runs no job while open
- **TEST** t3842 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The program at rest after cyclic writing costs nothing
- **PATH** the heavy showcase opens → Space plays a lap → Space stops → the mouse rests, then moves over the preview → nothing redraws or works by itself once the writes are done
- **WHEN** the heavy showcase stands paused after the prerender wrote into a full folder
- **THEN** no cache job runs and no frame is asked at rest
- **TEST** t3843 (`src/apps/kt-app/src/idle_tests.rs`)
