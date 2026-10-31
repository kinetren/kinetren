# Design: ci-green-again

## Context

The six failures are read from the owner's archive of the CI run on 94e36e9e (jobs `test`,
`flatpak`, `ffmpeg9`, `windows-test`; `windows-build` was green). GitHub CI cannot be run from
here, so each cause is reproduced where it can be: the registry volume with Docker, t1063 on
Linux (red on the old muxer: "last error: "), t801/t802 in parallel, t962/t986 under wine
(`scripts/windows-test.sh`).

## Goals / Non-Goals

**Goals:** every cause fixed at its root, with a guard that turns red when the cause comes back.

**Non-Goals:** Windows t973c, t1019, t1048 (frozen hashes printed on Linux; MinGW's `powf`/`tan`
round differently — invariant 11). They wait for the owner's choice between per-platform tables
and our own deterministic kernel math.

## Decisions

### D1. One script picks the registry a container mounts

`release-build.sh` already handed a fresh named volume to the user; `ffmpeg-test.sh` and
`windows-test.sh` had copies of the first half only. The rule moved into
`src/scripts/cargo-registry.sh`, which prints the registry to mount, and all three call it.
The chown runs every time (a root `docker run` of a second), so a volume an older script made
root-owned heals by itself. Rejected: `mkdir -p ~/.cargo/registry` on the host — it would work on
the runner, but writes into the home of a machine that has no Rust, and leaves a root-owned
volume made earlier on the owner's machine broken. Guard: `check_registry` in `dist-check.py`
(t1064).

### D2. Dropped Recommends are a table, not two fixes

`--no-install-recommends` is right (it keeps the runner and the image small), and it is what
removed both `eu-strip` and `mimetype`. Instead of two one-line fixes, `dist-check.py` holds
`RECOMMENDS_NEEDED` — the Recommends a check needs — and reads every `apt-get install` of the
workflows and Dockerfiles (t1060). The base stage of the Dockerfile gets
`libfile-mimeinfo-perl` too: the `ffmpeg9` job runs kt-cli's tests, t935 among them, in that
container, and would have failed the same way once the registry was fixed. Cost: the images
rebuild once from the base layer.

t935 itself now skips with a reason when `mimetype` is missing: without it xdg-mime asks
libmagic, which reads the content — a failure there says nothing about our MIME file. The skip is
safe because t1060 keeps CI and the container from losing the package.

### D3. Each live guard owns its output folder

`run_live_check` takes the output folder; t801 uses `target/live-guard-t801`, t802
`target/live-guard-t802`. `live-check.sh` is unchanged: cleaning `verdicts/` at the start is right
for a run, wrong only for two runs sharing a folder.

### D4. A forced encoder outside the cascade is named before the loop

The muxer's loop `continue`s past every candidate whose name differs from the forced one, so a
name the cascade lacks ended with an empty `last_err`. Before the loop the muxer now checks the
forced name against the cascade and sets the error — the loop still runs and still fails, so no
second exit path appears. t962's device half needs VAAPI in the cascade, so on a system without
it the test asserts the named error and returns. t1063 forces the other system's encoder, so the
Linux job, which runs on every push, proves the path the Windows job hit.

### D5. `kt_dirs::canonical`, the `dunce` rule without the crate

`std::fs::canonicalize` on Windows returns the verbatim spelling `\\?\C:\…`. The card showed it,
and the key differed from the `std::path::absolute` spelling. `kt_dirs::plain_path` drops the
prefix where the plain spelling names the same file: a drive path or UNC path shorter than
`MAX_PATH`; `\\?\Volume{…}` and long paths keep it (the plain spelling would not open). The rule
is pure over `Os` (as `resolve` is), so t1062 holds it on Linux. Rejected: the `dunce` crate — a
dependency for twenty lines; `locks::key` and `kt-otio`'s `strip_verbatim` keep their own strip,
because they fold case or rewrite separators for a key that is never opened.

t986 compares the card with `kt_dirs::canonical(&projects)`, not `projects.display()`: the
runner's temp folder is an 8.3 spelling (`RUNNER~1`) that canonicalisation expands, and the card
shows the canonical one — the long name is the one a person reads.

Existing Windows Recent records keep working: `StartStore` re-keys every row through
`project_key` on load. A poster made under the old key is rendered again the next time its
project closes.

## Risks / Trade-offs

- The Dockerfile's base layer changes, so the `ffmpeg9` job rebuilds its cached image once
  (~4 min of FFmpeg) and local images rebuild on their next use.
- The Windows half (t962, t986) is confirmed under wine, not on windows-latest; the next CI run
  is the verdict.
