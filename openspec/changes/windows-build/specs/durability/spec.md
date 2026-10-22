# Spec Delta

## MODIFIED Requirements

### Requirement: Project lock — flock on an adjacent file, a second copy degrades honestly
An open project SHALL take the core lock on a file in the data directory: `flock` on Linux,
`LockFileEx` on Windows (the key — a hash of the absolute path; the path inside). The key on Windows MUST
be computed from a normalized path: without the `\\?\` prefix, with one kind of separator and without
regard to case, because `C:\Proj\a.kt` and `c:\proj\A.kt` are one file. The process number of the holder
SHALL lie in a separate file next to the lock: the Windows lock is mandatory, and
a locked file cannot be read by a second copy. The lock is placed not on the project itself and
not globally on the program. A second copy MUST NOT be forbidden: it gets a warning with
the number of the other process (or without a number, if its file could not be read), ITS OWN
recovery file, ITS OWN temporary file, and one question at the first Ctrl+S of the session. kt-cli warns
about the busy state before writing, but does not refuse.
Why: a global lock would have killed live scenarios and "opened a second time to look"; a lock
on the project's inode would fall off after the first atomic `rename`; a file with a pid without a
kernel lock would leave a permanent block after `SIGKILL` (the pid file here is only a reference, the kernel holds
the lock); a shared tmp of two copies would replace the project with the half-written contents of
the other. On Windows the lock was a stub "always free", and two copies wrote one project.

#### Scenario: The lock is named and survives an atomic save
- **WHEN** the project is busy, then atomically overwritten
- **THEN** the second handle gets `Busy` with the pid, a neighbouring project is taken freely, after the overwrite the lock is held, after drop it is released
- **TEST** t362 (`src/apps/kt-app/src/project_lock.rs`), t362b (`src/apps/kt-app/src/project_lock.rs`)

#### Scenario: The second copy has its own tmp and recovery
- **WHEN** the temporary name and the recovery target are computed while the project is busy
- **THEN** the tmp contains the pid and the full project name; the recovery of the second copy differs from the owner's and remains `.otio`
- **TEST** t363 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The first Ctrl+S on an occupied project asks once
- **WHEN** the project is busy with a neighbour and the user saves
- **THEN** a notice with the pid, a question; "don't save" leaves the file and the asterisk; "save anyway" writes and no longer asks again
- **TEST** t364 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The lock key is normalized, the pid is read while the lock is busy
- **WHEN** the lock key is computed for `C:\Proj\a.kt`, `c:\proj\A.kt`, `\\?\C:\Proj\a.kt` and `C:/Proj/a.kt` by the Windows rule (a pure function, checked on any OS); on Windows the project is held by one handle and taken by a second
- **THEN** all four keys are equal, and by the Linux rule `/p/a.kt` and `/p/A.kt` differ; the second handle gets `Busy` with the holder's pid, read from the pid file
- **TEST** t851 (`src/apps/kt-app/src/project_lock.rs`)

### Requirement: Per-user data lives in `kinetren`; the former directory is moved once
The application's per-user data directory SHALL be `$XDG_DATA_HOME/kinetren` (falling
back to `~/.local/share/kinetren`) on Linux and `%APPDATA%\kinetren\data` on Windows (the
Roaming known folder, the same directory eframe uses for its own storage). It covers
recovery files, the journal and crash report, project locks, presets, the workspace and UI
state. Large regenerable downloads (the FFmpeg build) SHALL go to the Local known folder on
Windows (`%LOCALAPPDATA%\kinetren\`), not to Roaming. The resolver MUST NOT fall back to the
working directory: if no per-user directory can be resolved, the program SHALL say so once
and use the system temporary directory. On Linux, if the directory does not exist and the
former `kinotrain` directory does, the first start SHALL move the former one into place
before anything is read or written. This happens once, and nothing is lost or duplicated.
If the move fails (for example, across filesystems), the content SHALL be copied and the
former directory left in place. If both directories exist, the new one wins and the former
one is not touched. The move MUST NOT run on Windows, where no former install exists.
`kt-cli` SHALL resolve the directory with the same code as the app and look for project
locks there.

Why: the full rename must not lose the user's presets, workspace, recovery copies or
locks. On Windows `HOME` is usually unset, and the former fallback `"."` put the data
directory into the working directory. Started from Explorer, that can be `System32`, where
every write fails silently. Started from a source tree, the migration would have renamed
any `kinotrain` folder found there.

#### Scenario: First start moves the former directory; later starts do not touch it
- **WHEN** the data directory is resolved with only `kinotrain/` present (holding a preset file and a lock); then again; then with both present
- **THEN** the first call leaves `kinetren/` with the same files and no `kinotrain/`; the second changes nothing; with both present the new one is used and the former is untouched
- **TEST** t830 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: kt-cli and the app agree on the lock directory
- **WHEN** the app takes the lock of a project, and `kt-cli` checks the same project
- **THEN** `kt-cli` reports the holder's process, i.e. both use `kinetren/locks`
- **TEST** t831 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Directory per OS, never the working directory
- **WHEN** the resolver runs with injected environments: Linux with `XDG_DATA_HOME`; Linux with only `HOME`; Linux with neither; Windows with Roaming and Local known folders and no `HOME`; Windows with known folders unavailable; each time with a `kinotrain/` folder in the working directory
- **THEN** the results are `$XDG_DATA_HOME/kinetren`, `~/.local/share/kinetren`, the temp directory (with a warning), `%APPDATA%\kinetren\data` with downloads under `%LOCALAPPDATA%\kinetren`, and the temp directory (with a warning); the `kinotrain/` folder in the working directory is untouched in every case
- **TEST** t850 (`src/libs/kt-dirs/tests/dirs.rs`)

## ADDED Requirements

### Requirement: Atomic replace on Windows survives a briefly held target
On Windows the rename step of `kt_otio::write_atomic` SHALL replace the project with
`MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` and SHALL retry on a
sharing violation or access denied within a budget of about one second, with growing
pauses. Write-through SHALL count as the directory sync (`dir_synced == true`), so the
"not synced" notice does not appear on every Windows save. When the budget runs out, the
write MUST fail with the reason, and the temporary file is removed. The same replace is used
for the journal rotation and the user store.
Why: on Windows, replacing a file fails while another process holds it open without
delete sharing: an antivirus scanning the just-written file, a sync client, an editor.
Without a retry, Ctrl+S, autosave and the exit save fail at random. A directory cannot be
opened for `fsync` there; write-through is the platform's way to make the rename durable.

#### Scenario: Retry within the budget, then an honest failure
- **WHEN** the replace runs through an injected file system that answers "sharing violation" twice and then succeeds; then one that answers it for longer than the budget
- **THEN** the first write succeeds with `dir_synced == true` after exactly three attempts; the second returns an error that names the sharing violation, the project is unchanged and no temporary file remains
- **TEST** t852 (`src/libs/kt-otio/tests/io.rs`)
