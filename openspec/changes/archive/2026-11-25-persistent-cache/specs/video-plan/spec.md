## ADDED Requirements

### Requirement: The persistent cache keeps the final preview frame only
The persistent cache SHALL keep on disk two kinds of entry only: whole composites of moments as the preview shows
them (frames) and copies of heavy sources. It SHALL NOT write the picture of a layer, a node, a stream stage, a track
stage or a reel instance's output at any point inside a plan. A frame SHALL be keyed by the whole plan of its moment
(the durable key of the prerender, with the quality and the decode step), so an edit SHALL invalidate exactly the
moments whose plan it changes and keep every other held moment. Frames SHALL be written at the quality the preview
plays; a frame SHALL be read only at its own quality.
Why: the owner, kkt-26.11.22 D1 — "делай кеш имменно финального превью а не на каком-то конкретном этапе цепочки fx";
A10 — "как играем в превью так и храним".

#### Scenario: Only frames and copies lie in the store
- **WHEN** a project with a heavy Datamosh event, a heavy track stage and a reel instance is played and prerendered at rest until nothing is left to write, and the cache folder is scanned
- **THEN** every entry under `results/…/` written by the persistent cache is a frame segment whose manifest keys name whole moments, every other entry it wrote is a source copy under `media/<id>/`, and no `layers/` folder or stage entry exists
- **TEST** t3451 (`src/libs/kt-engine/tests/persistent_cache.rs`)

#### Scenario: An edit re-renders only the moments it touches
- **WHEN** a range 0–30 s holding a heavy event at 20–30 s and a title at 10–12 s is prerendered, the title is moved to 11–13 s, then the preview quality is raised from 1/2 to 1/1
- **THEN** after the move the frames of 10–13 s are misses and 0–10 s and 20–30 s are read from the store with no decode and no chain run; after the quality change no 1/2 frame is read at 1/1, the prerender writes 1/1 frames of the heavy moments at rest, and the 1/2 frames remain until the value order drops them
- **TEST** t3452 (`src/libs/kt-engine/tests/persistent_cache.rs`)

#### Scenario: A moment is judged heavy at the quality it is written at
- **WHEN** a moment is measured at 640×360 below the heavy threshold, and the same moment at 1920×1080 above it, then the preview quality is raised to 1920×1080 and the player rests
- **THEN** the prerender writes that moment at 1920×1080; a moment measured only at 640×360 is judged by that time scaled by pixels, and its own 1920×1080 time replaces the estimate once composed
- **TEST** t4106 (`src/libs/kt-engine/tests/persistent_cache.rs`)

### Requirement: Frames and source copies share one budget and one value order
Frames and source copies SHALL live in one store in the project's cache folder under one byte budget, evicted lowest
value per byte first (measured cost saved per read, times reads, over bytes), least recently used as the tie-break; a
frame this process used in the last minute SHALL not be evicted.
Why: one store, not several (the owner's B1 with B2's folder); invariant 4.

#### Scenario: A cheap copy goes before an expensive frame
- **WHEN** the store is full with a frame segment saving 40 ms per read and a source copy saving 3 ms per read of the same size, and a new entry arrives
- **THEN** the copy is evicted first; a frame read in the last minute is never evicted
- **TEST** t3453 (`src/libs/kt-engine/tests/persistent_cache.rs`)
