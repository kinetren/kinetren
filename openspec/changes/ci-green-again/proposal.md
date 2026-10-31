# Proposal: ci-green-again

## Why

The CI run on master 94e36e9e died in four of its five jobs (the owner's log archive
`logs_111804447070.zip`, registry row C1 of `docs/ktprompts/kkt-26.11.01-index.md`). Each cause is
a defect of the tree, not of the runner: a red CI hides every later regression, so it has to be
green again before the other executors of kkt-26.11.01 merge on top of it.

## What Changes

Six causes, each read in the log and in the code:

1. **ffmpeg9 job** — `cargo metadata` died on "failed to create directory
   `/usr/local/cargo/registry/cache`: Permission denied". The runner has no `~/.cargo/registry`, so
   `ffmpeg-test.sh` mounted a fresh named volume, which belongs to root, while the container runs
   as the runner's user. `release-build.sh` already handed the volume to the user;
   `ffmpeg-test.sh` and `windows-test.sh` did not. The rule moves into one script,
   `src/scripts/cargo-registry.sh`, used by all three.
2. **flatpak job** — "Failed to execute child process `eu-strip`". `--no-install-recommends` drops
   elfutils, a Recommends of flatpak-builder. `ci.yml` and `release.yml` name it.
3. **t935** (test job) — the project came back as `application/json`. Without a desktop, xdg-mime
   asks `mimetype` (libfile-mimeinfo-perl, a Recommends of xdg-utils) and otherwise libmagic's
   `file`, which never reads the MIME database. `ci.yml` and the build container name it; t935
   says why it did not run on a machine without it.
   Items 2 and 3 are one class: a package without its Recommends whose check needs one of them.
   `dist-check.py` gets a table of such pairs, held by **t1060**.
4. **t801** (test job) — "the re-run scenario stayed red". t801 and t802 ran in parallel against
   one output folder `target/live-guard`, and `live-check.sh` deletes `verdicts/` at its start:
   t802 wiped t801's verdict. Each guard now has its own folder.
5. **Windows t962** — forcing `h264_vaapi` on Windows, where the cascade has no VAAPI, gave
   "no video encoder could be opened (last error: )". The muxer now names a forced encoder outside
   the system's cascade, and lists the cascade. t962 expects that error on such a system; the new
   **t1063** forces the OTHER system's encoder, so the Linux job proves the same path.
6. **Windows t986** — "each card names its folder" failed: `std::fs::canonicalize` returns
   `\\?\C:\…` on Windows, which the welcome card showed. `kt_dirs::canonical` drops the verbatim
   prefix where the plain spelling names the same file (the `dunce` rule). `user_store::project_key`
   uses it, and t986 compares the folder in its canonical spelling (the runner's temp folder is an
   8.3 name). **t1062** holds the rule; it is pure over `Os`, so it runs on Linux.

Item 1 gets a guard too: `dist-check.py` refuses a script that mounts the registry without
`cargo-registry.sh`, or a `cargo-registry.sh` that does not hand the volume over (**t1064**).

Not in scope: Windows t973c, t1019 and t1048 fail because their frozen hashes were printed on
Linux and MinGW's `powf`/`tan` round differently (invariant 11). The owner was asked to choose
between per-platform tables and our own deterministic kernel math; those three wait for that
answer.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `distribution`: the CI jobs install what their checks need when Recommends are dropped
  (t1060), and every container that mounts a registry volume hands it to the user first (t1064).
- `durability`: the live guards each run in their own output folder (t801, t802); a canonical
  path is shown and keyed without the Windows verbatim prefix (t1062, t986).
- `export`: a forced encoder outside the system's cascade is an error naming it (t1063, t962).

## Impact

- `src/scripts/cargo-registry.sh` (new), `ffmpeg-test.sh`, `windows-test.sh`, `release-build.sh`.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `src/packaging/Dockerfile` (base
  stage gains `libfile-mimeinfo-perl`: the container images rebuild once).
- `src/scripts/dist-check.py` (`check_recommends`).
- `src/libs/kt-ffmpeg/src/muxer_impl.rs`, `src/libs/kt-dirs/src/lib.rs`,
  `src/apps/kt-app/src/user_store.rs`.
- Tests: t935, t801, t802, t962, t986 adjusted; t1060, t1062, t1063, t1064 new.
