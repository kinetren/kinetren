## ADDED Requirements

### Requirement: Settings show the caches on disk and their limit
Settings SHALL have a page "Caches on disk" that lists every cache folder the program made:
- its place;
- the projects it serves, a project no longer there marked so;
- its size, with a Delete button.

The page SHALL also show the limit the caches may take and the total they take now. Deleting the open
project's folder SHALL empty it and leave the work untouched. The page SHALL only read the registry and
the folders' sizes, and SHALL compute nothing while it is open.
Why: kkt-26.11.20 A14 moved the caches into the projects' folders; a cache the person cannot see or
reclaim breaks UI rule 20 (nothing grows by itself unseen); project-cache-folder D3.

#### Scenario: The list, a deleted folder and the limit
- **PATH** Settings → "Caches on disk": two folders listed with sizes, one marked "no project here" → Delete on that row: the row goes and the total drops → the limit field set to 2 GB: the total shows against it
- **WHEN** the page opens with two registered folders, one whose project is gone, then the orphan is deleted and the limit lowered
- **THEN** the rows, sizes and marks match the registry, the deleted folder is gone from the disk and the list, the limit is saved, and the page runs no job while open
- **TEST** t3842 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The program at rest after a limit pass costs nothing
- **PATH** the heavy showcase opens → Space plays a lap → Space stops → the mouse rests, then moves over the preview → nothing redraws or works by itself once the pass is done
- **WHEN** the heavy showcase stands paused after the prerender wrote and the limit pass ran
- **THEN** no cache job runs and no frame is asked at rest
- **TEST** t3843 (`src/apps/kt-app/src/idle_tests.rs`)
