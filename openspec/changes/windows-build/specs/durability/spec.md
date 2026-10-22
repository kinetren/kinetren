# Spec Delta

## MODIFIED Requirements

### Requirement: Замок на проект — flock на соседнем файле, вторая копия деградирует честно

Открытый проект SHALL заниматься замком ядра на файле в каталоге данных: `flock` на Linux,
`LockFileEx` на Windows (ключ — хэш абсолютного пути; путь внутри). Ключ на Windows MUST
считаться от нормализованного пути: без префикса `\\?\`, с одним видом разделителя и без
учёта регистра, потому что `C:\Proj\a.kt` и `c:\proj\A.kt` — один файл. Номер процесса
держателя SHALL лежать в отдельном файле рядом с замком: замок Windows обязательный, и
заблокированный файл вторая копия прочитать не может. Замок ставится не на самом проекте и
не глобально на программу. Вторая копия MUST NOT запрещаться: она получает предупреждение с
номером чужого процесса (или без номера, если его файл не прочитался), СВОЙ файл
восстановления, СВОЙ временный файл и один вопрос при первом Ctrl+S за сессию. kt-cli перед
записью предупреждает о занятости, но не отказывает.
Почему: глобальный замок убил бы живые сценарии и «открыл второй раз посмотреть»; замок
на inode проекта отваливался бы после первого атомарного `rename`; файл с pid без замка
ядра оставлял бы вечную блокировку после `SIGKILL` (файл pid здесь только справка, держит
замок ядро); общий tmp двух копий подменял бы проект наполовину записанным содержимым
другой. На Windows замок был заглушкой «всегда свободно», и две копии писали один проект.

#### Scenario: Замок именной и переживает атомарное сохранение
- **WHEN** проект занят, затем атомарно перезаписан
- **THEN** второй дескриптор получает `Busy` с pid, соседний проект берётся свободно, после перезаписи замок держится, после drop отпускается
- **TEST** t362 (`src/apps/kt-app/src/project_lock.rs`), t362b (`src/apps/kt-app/src/project_lock.rs`)

#### Scenario: У второй копии свои tmp и recovery
- **WHEN** временное имя и цель восстановления считаются при занятом проекте
- **THEN** в tmp есть pid и полное имя проекта; recovery второй копии отличается от владельца и остаётся `.otio`
- **TEST** t363 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Первый Ctrl+S на занятом проекте спрашивает один раз
- **WHEN** проект занят соседом и пользователь сохраняется
- **THEN** уведомление с pid, вопрос; «не сохранять» оставляет файл и звёздочку; «всё равно» пишет и больше не переспрашивает
- **TEST** t364 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Ключ замка нормализован, pid читается при занятом замке
- **WHEN** ключ замка считается для `C:\Proj\a.kt`, `c:\proj\A.kt`, `\\?\C:\Proj\a.kt` и `C:/Proj/a.kt` по правилу Windows (чистая функция, проверяется на любой ОС); на Windows проект занят одним дескриптором и берётся вторым
- **THEN** все четыре ключа равны, а по правилу Linux `/p/a.kt` и `/p/A.kt` различны; второй дескриптор получает `Busy` с pid держателя, прочитанным из файла pid
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
