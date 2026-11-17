## ADDED Requirements

### Requirement: `trace gpu` reads a compute job's pieces as one job
`kt-cli trace … gpu` SHALL read the device records of a compute job's pieces as ONE job: the records a
job's span holds on its thread. The wait for the job's buffers SHALL be read from its LAST piece's device
end to when its records were written; a record outside any job's span is a job of its own.
Why: found-defects 693 — every piece's record carries its own instant, and keyed by the instant every piece
counted as a job's last one: the admission waits between a Datamosh job's pieces read as "buffers after end
p50 607 ms p95 1990 ms" on the owner's trace, where the job's buffers came back p50 37 ms after its end.

#### Scenario: A job's pieces read as one job
- **WHEN** two near jobs of five pieces each, 100 ms apart, end 3 ms before their buffers come back, and their records are written a microsecond apart
- **THEN** the report counts ten pieces on the device and reads the buffers after the end at 3 ms, p50 and p95
- **TEST** t2863 (`src/apps/kt-cli/src/trace_query/tests.rs`)
