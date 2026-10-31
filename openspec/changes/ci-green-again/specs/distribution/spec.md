## ADDED Requirements

### Requirement: A package installed without its Recommends names what the checks need
Every `apt-get install --no-install-recommends` in `.github/workflows/ci.yml`,
`.github/workflows/release.yml` and every Dockerfile SHALL also install, by name, each Recommends
that a check run after it needs. The pairs live in one table of `src/scripts/dist-check.py`
(`RECOMMENDS_NEEDED`): `flatpak-builder` → `elfutils` (it strips with `eu-strip`) and `xdg-utils`
→ `libfile-mimeinfo-perl` (xdg-mime without a desktop asks `mimetype`). A new pair is a new row.
Why: the CI run of 94e36e9e lost two jobs to dropped Recommends — the Flatpak build had no
`eu-strip`, and t935 got `application/json` for our project, because xdg-mime fell back to
libmagic's `file`, which never reads the MIME database. Both were silent on the owner's desktop,
which has the full packages.

#### Scenario: Each dropped Recommends a check needs is named
- **WHEN** `dist-check.py --check` runs on the repository; then on temporary copies where, one at
  a time, `ci.yml` or `release.yml` installs `flatpak-builder` without `elfutils`, or `ci.yml` or
  the Dockerfile installs `xdg-utils` without `libfile-mimeinfo-perl`
- **THEN** the repository passes; each copy fails, naming the file, the package and the one it
  must install
- **TEST** t1060 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The MIME check runs only where it asks the database
- **WHEN** t935 runs on a machine with `update-mime-database` and `xdg-mime` but no `mimetype`
- **THEN** it says it did not run and names `libfile-mimeinfo-perl`, instead of failing on
  libmagic's answer; on CI and in the build container it runs (t1060 holds the packages)
- **TEST** t935 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: A registry volume is handed to the container's user
A script that mounts the crates.io registry into a build container
(`release-build.sh`, `windows-test.sh`, `ffmpeg-test.sh`) SHALL take it from
`src/scripts/cargo-registry.sh`: the host's `~/.cargo/registry` when it exists, otherwise the
named volume `kinetren-cargo-registry`, chowned to the calling user by a root container on every
run. Why: the containers run as the caller (`-u UID:GID`), and a fresh named volume belongs to
root. The ffmpeg9 CI job of 94e36e9e died in `cargo metadata` ("failed to create directory
`/usr/local/cargo/registry/cache`: Permission denied") — the runner has no `~/.cargo/registry`,
and only `release-build.sh` had the chown; the two test scripts carried their own copy without it.

#### Scenario: Every registry mount goes through the one script
- **WHEN** `dist-check.py --check` runs on the repository; then on temporary copies where
  `ffmpeg-test.sh` names the volume itself, `cargo-registry.sh` lost its chown, or
  `cargo-registry.sh` is missing
- **THEN** the repository passes; each copy fails, naming the script
- **TEST** t1064 (`src/apps/kt-cli/tests/dist_guard.rs`); the run itself is the `ffmpeg9` CI job
  (reproduced locally: a fresh volume refuses `mkdir` to the user, the handed one accepts it)
