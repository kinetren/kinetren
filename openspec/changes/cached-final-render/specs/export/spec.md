## ADDED Requirements

### Requirement: An export can write the index of what it holds
An export with the cached render on SHALL write, per its output file, an index: the fingerprint of
everything that decides the bitstream (container, codec, encoder and library version, profile, level,
pixel format, size, rate, rate control and its value, GOP rule, B-frames, colour tags, encoder threads,
the device identity and the program build), one line per GOP (first frame, frames, byte offset, packets)
and the durable key of every frame at the export's size and quality. An index that does not verify
against its file (size, content hash) SHALL not be used.
Why: `v26.10` X1, kkt-26.11.18b U5 — "среди уже отрендеренных файлов искать … по индексам каким-то что
можем переиспользовать"; cached-final-render D3, D5.

#### Scenario: The index lists what the file holds
- **WHEN** a three-second project is exported with the cached render on, and the output is then touched by one byte
- **THEN** the index names the fingerprint, a GOP line per GOP whose offsets and packet counts match the file, and one key per frame equal to `durable_hash` of that frame's plan at the export's quality; after the touch the index does not verify
- **TEST** t3322 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: The fingerprint follows the settings
- **WHEN** fingerprints are taken for one render's settings, then with another bitrate, encoder, size, build salt and device
- **THEN** each change gives another fingerprint and the window's reason names it
- **TEST** t3323 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: Without the option nothing changes
- **WHEN** the golden project is exported with the cached render off, twice
- **THEN** no index is written, the files are byte-equal and equal to today's (GOLDEN, t2805 unchanged)
- **TEST** t3324 (`src/libs/kt-engine/tests/cached_render.rs`)

### Requirement: A cached render places closed GOPs at edit points and carries its parameter sets
An export with the cached render on SHALL start a closed GOP wherever the timeline's edit identity changes
(the leaves or the layer set of the plan) and otherwise at the GOP rule's spacing counted from there, as a
function of the timeline; it SHALL write the parameter sets before every IDR, in MKV, or in MP4 with
`avc3`/`hev1` sample entries.
Why: cached-final-render D2, D4 — a small fix must invalidate only the GOPs it touches; X1: "не мешает
pps/sps/vps/etc перед сегментами класть".

#### Scenario: GOPs start at cuts
- **WHEN** a project of three shots is exported with the option on
- **THEN** every cut frame is an IDR of a closed GOP, the spacing restarts there, every IDR is preceded by its parameter sets, and the file decodes in a fresh decoder from any IDR
- **TEST** t3325 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: MP4 keeps parameter sets in the stream
- **WHEN** the same project is exported with the option on to MP4 H.264 and HEVC
- **THEN** the sample entries are `avc3` and `hev1` and the probe reads every GOP alone
- **TEST** t3326 (`src/libs/kt-engine/tests/cached_render.rs`)

### Requirement: A cached render copies the whole GOPs an earlier render of the same fingerprint holds
A cached render SHALL copy, re-stamped in decode order, every GOP of a verified earlier file with an equal
fingerprint whose frames' keys equal, in order, the keys of the new timeline at that place, and SHALL
compose and encode every other GOP; candidates SHALL be ordered by their index hash and the first match
SHALL win. Its bytes SHALL be a function of the project, the settings and the set of verified files. The
preview's disk prerender SHALL NOT be a source of frames for any export.
Why: cached-final-render D1, D7, D8; found-defects 773 — the preview's tier is lossy, at the preview's size,
possibly in a run form the export never uses.

#### Scenario: A fix inside one shot re-renders one shot
- **WHEN** a three-shot project is rendered, then an effect value is changed inside the second shot and it is rendered again with the option on
- **THEN** the first and third shots' GOPs are copied byte for byte from the first file, the second shot's are encoded, the file decodes frame for frame to the first render's frames outside the second shot, and the summary names the seconds copied
- **TEST** t3327 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: Timestamps keep the GOP's reorder
- **WHEN** GOPs with B-frames are copied to another position of the timeline
- **THEN** their pts and dts are shifted by the same offset, the decode order is kept, and a decoder shows them at the new times
- **TEST** t3328 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: A partial or foreign match is rendered
- **WHEN** an earlier file matches all but one frame of a GOP, and another file matches wholly but has another bitrate
- **THEN** neither GOP is copied, and the reasons are "partial GOP" and "other bitrate"
- **TEST** t3329 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: The same inputs give the same bytes
- **WHEN** the second render of t3327 is made twice, with the candidate files listed in two orders
- **THEN** the two outputs are byte-equal
- **TEST** t3330 (`src/libs/kt-engine/tests/cached_render.rs`)

#### Scenario: The preview's tier never reaches an export
- **WHEN** a range is fully prerendered by the preview's disk tier and then exported with the cached render on and off
- **THEN** no frame of the export is read from the tier (the tier's read counter does not move during the export)
- **TEST** t3331 (`src/libs/kt-engine/tests/cached_render.rs`)

### Requirement: The render window shows what is already cached
The render window SHALL offer the cached render as its own step: the switch, the candidate files with
their verdicts, a strip over the render range marking the GOPs found, and an estimate that counts copied
GOPs at the copy's speed; the finished render SHALL say how many seconds it copied from how many files.
`kt-cli export --cached` SHALL do the same headless and print the counts.
Why: X1 — "FCR можно и отображать как отдельный шаг настройки рендера, где показывается какая зона уже
откеширована"; cached-final-render D6.

#### Scenario: The step lists files and the cached zone
- **WHEN** the window opens on a project with one matching earlier render and one of another bitrate
- **THEN** the step shows both files, the second with "other bitrate", the strip marks the matching GOPs, and the estimate drops by their share
- **PATH** File → Render → step "Cached render" → the switch on; the list shows two files and their verdicts; the strip over the range turns bright where GOPs are found
- **TEST** t3332 (`src/apps/kt-app/src/render_dialog.rs`)

#### Scenario: The summary and the headless counts
- **WHEN** the cached render of t3327 finishes in the window and through `kt-cli export --cached`
- **THEN** both say the seconds copied and the files used
- **TEST** t3333 (`src/apps/kt-cli/tests/cli.rs`), t3334 (`src/apps/kt-app/src/render_dialog.rs`)
