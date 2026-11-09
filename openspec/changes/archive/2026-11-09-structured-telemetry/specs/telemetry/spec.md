## ADDED Requirements

### Requirement: One structured trace of the run, typed and joined by ids
The program SHALL record a structured trace beside the text journal: typed fixed-size records (an event,
the time since the process started in nanoseconds, a duration for a span, an id, a cause and the
event's numbers), the names they use written once per file, in bounded ring files under
`<data>/trace` (segments of 8 MiB, all runs within 128 MiB, each segment readable alone). A preview
frame's id SHALL ride with every request it makes: its decode requests, its stream stage's run and its
compute jobs carry it as their cause, and a cache key's id is the same number in the frame's wait, the
request, the decoded entry and the eviction (whose cause is the key that pushed it out). Every
subsystem that waits or keeps SHALL record: the preview frame's life (asked, superseded, planned,
waiting — decode, lock, device —, executed with its kept-composite hit or store, its layers, done,
shown or dropped and why), the "Loading" scrim, playback state changes, the decode queue, cache
evictions, stream stages and their memo, the compute host, the executor's kept composites and pipeline
warm-ups, the audio render and the kept mix's decision, prefetch passes, jobs (thumbnails, effect
thumbnails, reel previews, posters, analysis, export), UI frames of 50 ms or more, edits and what they
invalidated, the load watch's busy seconds; and the machine: processor, cores, memory, OS, and for each
device a role opens its adapter, driver, features and limits. Recording SHALL never wait: a lock-free
ring, no allocation after a thread's first record, a full ring drops and the writer says how many; the
audio callback records nothing. At rest the trace SHALL cause no wake-up. It never leaves the machine.
Why: the owner, kkt-26.11.09 I6/I7: "у тебя в логах срач, очень не структурированные логи";
"телеметрия должна быть такая, чтобы ты мог понять что у тебя происходит, без мышиной возни туда-сюда
версиями или что ты там за бред мне прелагаешь с стендом".

#### Scenario: Records come back whole, in time order, threads and names resolved
- **WHEN** three named threads record requests with a frame as their cause, a span with an interned kernel name, and a journal line
- **THEN** the files read back every record with its fields, in time order, the threads and the name resolved, the span with its length, the journal line and the machine's facts present
- **TEST** t2140 (`src/libs/kt-time/src/trace/tests.rs`)

#### Scenario: The ring files are bounded and each reads alone
- **WHEN** records pass the segment size many times under a small budget
- **THEN** there are several segments within the budget plus one segment, each names its threads and names, and the newest record is kept
- **TEST** t2141 (`src/libs/kt-time/src/trace/tests.rs`)

#### Scenario: At rest the writer sleeps
- **WHEN** nothing is recorded for a second and a half
- **THEN** the writer thread made no context switch; one record then wakes it and is written
- **TEST** t2142 (`src/libs/kt-time/src/trace/tests.rs`)

#### Scenario: Recording never waits and never allocates
- **WHEN** the writer is held and more records are made than the ring holds; and, in a binary of its own, ten thousand records are made after a thread's first
- **THEN** recording returns at once, the lost records are counted and the writer records how many; the records made no allocation
- **TEST** t2143 (`src/libs/kt-time/src/trace/tests.rs`, `src/libs/kt-time/tests/trace_alloc.rs`)

#### Scenario: A frame is joined to the decode it waited for
- **WHEN** the preview asks for a frame whose layer is not decoded, through the real player on model decoders
- **THEN** the frame's wait names a key, the request of that key names the frame as its cause and went out from the compose thread, the media worker decoded the key on its own thread, and the plan, the executor and the answer name the frame
- **TEST** t2144 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: The trace answers questions through a query tool
`kt-cli trace [<dir|file>] [--run N] <question>` SHALL answer from one run's trace, with times in the
journal's seconds since the start: `summary`; `loading [from to]` — each stretch the preview waited for
a picture (the UI's scrim records, else a tick asked until a complete frame of it, 180 ms or more), its
waits ranked by time (the decodes waited for and whether they were evicted again, the stream history,
compute by kernel with device time, the stream memo's evictions, plans, the executor, locks, the device,
warm-ups, builds, prefetch passes), the critical path of its last composite and the journal around it;
`loop` — per lap of a loop region the frames kept and composed, the kept mix's replays, and for every
frame composed again on a later lap why (evicted, its key changed and which layer, a held stream frame,
the slow device, incomplete, never kept); `during <from> <to>`; `slow [n]` with each frame's critical
path; `frame <id>`; `machine`; `dump` as JSON lines; `chrome <out.json>` for Perfetto.
Why: the owner asked for telemetry the orchestrator reads to understand a run; a record nobody can
query is the ASCII log again.

#### Scenario: Why the preview was Loading
- **WHEN** a paused frame waits two seconds: three composites, a decode, a history of 60 positions computed by the prefetch thread with 60 compute jobs
- **THEN** one stretch with the three composites, the stream history ranked first, the compute kernel and the decode next, and the last composite's critical path; with the UI's scrim records, the scrim decides the stretch
- **TEST** t2146 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: Pass by pass over a loop
- **WHEN** three laps over three moments: kept on the first, one evicted and one whose key moved on the second, all kept on the third; the kept mix replays once and misses once in the lap's context
- **THEN** the laps' counts, "evicted from the kept composites", "the frame's key changed", "lap context never kept", and the third lap all kept
- **TEST** t2147 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The slowest frames carry their critical path
- **WHEN** the slow question is asked of a run whose slowest frame ran three compute jobs
- **THEN** that frame comes first with its length, its stream stage, its compute and its executor lines
- **TEST** t2148 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: A window, a frame, and the exports
- **WHEN** a window, one frame, a dump and a Chrome export are asked
- **THEN** the window names the busy threads and the jobs, the frame tells its decode's story, every dump line is JSON, and the Chrome export parses with its threads named and spans as complete events
- **TEST** t2149 (`src/apps/kt-cli/src/trace_query/tests.rs`)
