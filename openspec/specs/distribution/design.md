# Distribution — why it is built this way

## What was missing and who asked

Before going public at `github.com/kinetren/kinetren`, nothing checked the repository
there: tests ran only on the owner's machine through `finish.sh`, and a release tarball
existed only when someone ran `release-build.sh` by hand. The README asked people to install
a Rust toolchain, clang and dev packages before they could try the editor (owner,
2026-10-22, `docs/user-feedback.md`, "Pre-release: GitHub CI and Docker build"). Change
`github-ci-docker-build`, archived 2026-10-26.

## Owner decisions (do not change without a new request)

- **Free plan, keep only releases**: CI uploads nothing that lingers; a release keeps its
  files; old releases are not deleted.
- **Build in Docker, run on the host**, with simple `make` commands.
- **The program is called `kinetren` on disk** (build output, tarball, `~/.local/bin`, the
  `.desktop` file, `make run`); the cargo package stays `kt-app`, `kt-cli` keeps its name.
- The final repository is `github.com/kinetren/kinetren`.

## Where it lives

- `src/packaging/Dockerfile` — the one build container (Ubuntu 22.04, glibc 2.35, the Rust
  toolchain pinned by `ARG RUST_TOOLCHAIN`).
- `src/scripts/release-build.sh` — the build and the delivery checks, inside the container.
- `Makefile` (repository root) — a thin front over the scripts.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`.
- `src/scripts/dist-check.py` + `src/apps/kt-cli/tests/dist_guard.rs` (t834, t835) — the
  tree stays consistent.

## Decisions and why

- **One recipe, checks inside it.** The image recipe used to be a heredoc inside
  `release-build.sh`, and the checks ran `ldd`/`objdump` on the host: a newcomer without
  binutils, or CI building the image another way, would get a different verdict from the
  owner's. Now the Dockerfile is a file, the checks run in the container, and a failed check
  removes `dist-out` so no tarball that looks good is left behind.
- **The toolchain is pinned in two places kept equal by the guard** (the Dockerfile `ARG`
  and `RUST_TOOLCHAIN` in `ci.yml`). Bump both together.
- **No host Rust needed**: the crate registry is the host's `~/.cargo/registry` if present,
  otherwise a named Docker volume; the target directory persists in
  `src/target/dist-container`, so a second `make` is incremental.
- **make is only a front.** No build logic in make, so `make`, `finish.sh --release` and the
  release workflow cannot drift apart.
- **CI runs natively on ubuntu-24.04**, not in the 22.04 image: the tests need FFmpeg ≥ 6,
  and native caching works. `RUSTFLAGS=""` because the host's mold is not on the runner;
  `CARGO_PROFILE_DEV_DEBUG=0` keeps the runner's disk; nothing changes `opt-level`, overflow
  checks or `debug_assert!`. Tests run through nextest, as in `finish.sh` (t663 times out
  under `cargo test`).
- **The release reuses `release-build.sh` as is** and refuses a tag whose commit is not on the
  default branch. The tarball's version stays the commit-date + hash of `kt-version`; the tag
  is only the release name — a moved tag would otherwise disagree with the binary's own version.
- **The binary is renamed in cargo** (`[[bin]] name = "kinetren"`), not only when packing:
  otherwise there would be two names for one program, and a script could start a stale
  `kt-app` left in `target/`.

## Rejected and why

- **`rust-toolchain.toml`** — it would also change the owner's local toolchain in every
  `cargo` call; the guard keeping two values equal is enough.
- **Tests inside a 24.04 variant of the image** — a second image recipe is exactly the drift
  the one Dockerfile removes, and building it on every push costs minutes for nothing.
- **`softprops/action-gh-release`** — `gh` is on the runners, and one command line is easier
  to audit than a third-party action holding `contents: write`.
- **Deleting old releases automatically** — release assets count toward no quota, and
  people may need an older build.
- **`clippy -D warnings` in CI** — about 25 existing warnings; making them fatal is a
  separate clean-up.

## Non-obvious details

- The workflows are checked with line patterns (`dist-check.py`): the standard library has
  no YAML parser, and these are shape guarantees, not a YAML validator.
- **The first runs on a clean runner found defects the owner's machine hid** (2026-10-25):
  tools the tests call but the runner lacks (t809 needs `xdpyinfo`, `xwd`, `convert` — the
  packages `x11-utils`, `x11-apps`, `imagemagick`), and tests racing over the real tree (t804
  wrote and removed files in `src/scripts/live` while t807/t808 globbed it). A test that
  needs to break the tree works on a copy (`KT_LIVE_DIR`); a test's external tools belong in
  the CI package list.

## Windows delivery (windows-build, 2026-10-26)

- **Cross-built in the same Dockerfile** (owner's choice): a `windows` stage (mingw-w64, the
  `x86_64-pc-windows-gnu` target, WineHQ wine, nextest) between the base and a final `linux`
  stage, so a plain build stays Linux. `make windows` packs a portable zip with no FFmpeg DLL;
  the PE import check runs inside the container. Each target cleans only its own packages in
  `dist-out`, so one release run carries both.
- **Tests run on real Windows without compiling there**: the Linux job cross-builds a nextest
  archive (the repository mounted at `/kt`, the build in `/kt/src`) and uploads it with the
  fixtures for a day; the windows-latest job links `D:\kt` to the checkout — the tests'
  compile-time paths (`env!("CARGO_MANIFEST_DIR")`) are `/kt/src/…`, which on Windows means the
  current drive — fetches the pinned FFmpeg (hash checked) and runs the archive. Binaries a test
  starts come from nextest's run-time path (`NEXTEST_BIN_EXE_*`), not the compile-time one.
- `make windows-test` runs the same tests under wine: the fast loop, not the verdict (wine's
  `round` agreed with mingw's bug, but its msvcrt differs from Microsoft's elsewhere).
- Rejected: native MSVC builds on a Windows runner (a second toolchain), an installer and code
  signing for the first delivery (later decisions).

## FFmpeg 9 proven on Linux (ffmpeg-9, 2026-10-27)

- **Why a source build.** The runner and the owner's machine have FFmpeg 6.1 only; nothing ever
  loaded 7, 8 or 9 on Linux. The owner chose a build from the official source in Docker
  (2026-10-26). The `ffmpeg-test` stage builds shared 9.0.2 from the tarball whose SHA-256 is
  committed next to the recipe (the same pin as the vendored headers — dist-check, t866), with
  libx264 (the cascade's usual encoder) and zlib (PNG — configure drops the decoder silently without
  it; t796 failed "no video decoder"). It is never the last stage, and release-build.sh never names
  it: nothing built there is shipped.
- **Found by SONAME, not by a folder.** *Revision (apply):* the plan loaded it through
  `KINETREN_FFMPEG_DIR` with the system search off — a path no user takes, and t849 overrides that
  variable on purpose. The libraries are registered with ldconfig, so the run finds FFmpeg 9 as a
  distribution's FFmpeg 9 is found. The folder path stays covered by the pure plan tests.
- **A run proves its major** (t865): `KT_EXPECT_FFMPEG_MAJOR=9` fails the run unless 9 loaded, so a
  fallback to another FFmpeg — or to none, with the FFmpeg tests skipping — is not a green run.
- **Tests run inside the container**: the libraries link the container's `libx264.so.163`, the
  runner has `.164`. The CI job caches the image (`docker save`) keyed by the Dockerfile's hash.
- **Windows tests only 9.0.2** (owner): the windows-latest job and `make windows-test` use the new
  pin; 6/7/8 on Windows stay covered by the pure plan tests.
- **Rejected:** BtbN prebuilt shared builds (git snapshots `n9.0.2-36-g…`, dated releases pruned —
  no stable hash); a distribution image with FFmpeg 9 (the version moves under the hash); copying
  the built libraries to the host runner (libx264 SONAME mismatch).

## The daily new-version check (update-check, 2026-10-28)

The program's side of the release channel: once per UTC day a release build asks
`api.github.com/repos/kinetren/kinetren/releases/latest` on a background thread, and a newer
release is one bell notice plus a short word after the version in the title. Owner,
2026-10-27 (`docs/user-feedback.md`, "Version check"): "remind him in the bell notice that the
version is old (but do not bother him with pop-up windows at startup), and also write at the
top in the title bar, somehow briefly, that the version is outdated". Research of other
editors: `docs/features/resilience/environment/update-available-notice.md` (F-RES-013).

**Owner decisions (do not change without a new request):** an off switch in View next to the
telemetry checkbox (on by default, `KINETREN_UPDATE_CHECK=0` too); the title says
"kt26.10 (outdated)" — our version and a translated word, not the new number; only normal
releases count (a `-` tag is a pre-release and is ignored); a notice only — nothing is
downloaded or installed.

Where it lives: `src/apps/kt-app/src/update_check.rs` (decision, day file, consent, fetch
thread, `install`/`global`); `app.rs` (first-frame start, the notice, the title word,
`save`, `run`); `toolbar.rs` (the painted word with its hover, the View checkbox);
`[update]` in the five catalogs; the live scenario `src/scripts/live/update.py`.

- **"Newer" comes from the asset name, not the tag.** The asset carries the binary's own
  commit-date version (`kinetren-<YY.MM.DD>-<hash8>-…`, from `release-build.sh`), while the tag
  is only the release's name and may be dated after its commit — compared with the tag, the
  very release a person just downloaded would call itself outdated. Newer = a strictly later
  date AND another hash; a same-day release is "not newer": silence costs one day, a false
  mark teaches people to ignore it. t872 also reads `release-build.sh` so that a rename of the
  asset form cannot silently make every release "not newer".
- **The day's answer is a file the thread writes, the choice is in eframe storage.**
  `<data_dir>/update-check` (`day=`, `latest=`; tmp + rename) is written by the thread as soon
  as the answer is in: eframe storage is saved every 30 s and on exit, and a crash in between
  would cost a second request. A failed request writes nothing, so the next start asks again.
  The on/off choice is a per-machine preference and sits where `lang` and `telemetry` do.
- **The day is the UTC day**: a local day needs a time-zone database; the boundary only moves
  WHEN the daily request goes out.
- **Nothing waits for it.** The UI spawns the thread once and reads an atomic per frame; the
  thread calls one `request_repaint` when its answer is in (invariant 12). Unlike telemetry's
  `app_closed`, `on_exit` does not wait: an unanswered check is simply asked again.
- **One checker per process** (`install`/`global`), for the same reason as telemetry: File →
  New/Open rebuild `KtApp` in place, and a per-app checker would ask and say it again.
- **Dev builds never ask.** A debug build checks only through `KT_UPDATE_URL` (the live
  scenario's local server, the day ignored); a release build ignores the variable, so a
  shipped binary cannot be pointed elsewhere. The request carries only
  `User-Agent: kinetren/<FULL>` (GitHub refuses one without it).

Non-obvious details:

- **The word is hit-tested by the pointer, not a widget.** The title text spills over the
  preview (the user's earlier decision for long names), and the preview's widgets, interacted
  later in the frame, won the hover of a widget placed on the word — found by t875.
- **The live scenario needs `OWN_PROCESS = True`.** The check runs on the first frame of a
  fresh start with `KT_UPDATE_URL` read at process start; without the flag the worker kept its
  stand running, two `kinetren` windows were on the screen, and the rig aimed at the stand's.
- **Literals shaped like probe names** (`v26.11.01`, `kt26.11.01`, `notes.txt`) in test code
  are counted by `live-coverage.py` as untouched places; the tests build those strings instead.
- **Checked against the real GitHub** (2026-10-28, release build `kt26.10.28-767c113a`, latest
  release `v26.10.25`): one request with the check on, the day recorded, no word (the build is
  newer); none with the stored choice off.

Rejected: the `releases.atom` feed (XML, no pre-release flag), listing tags (no flag,
paging), comparing with the tag or a build date (see above), a local-time day (a new
dependency), a "Check now" item and a check that re-runs past midnight (not asked for),
downloading or installing a release (F-RES-008: a self-updating binary is a separate class of
defects).

## ci-green-again (2026-11-01): what a check needs is named, and a registry volume is handed over

The CI run on 94e36e9e lost three jobs to the runner, not to the code under test. `--no-install-recommends`
is right (small runner, small image) and is what dropped `eu-strip` (elfutils, a Recommends of
flatpak-builder) and `mimetype` (libfile-mimeinfo-perl, a Recommends of xdg-utils — without it
xdg-mime asks libmagic, which called our project JSON). Instead of two one-line fixes,
`dist-check.py` keeps the pairs in `RECOMMENDS_NEEDED` and reads every `apt-get install` of the
workflows and Dockerfiles (t1060). The ffmpeg9 job died because a fresh named volume belongs to root
while the container runs as the caller; `release-build.sh` handed it over, the two test scripts had
their own copy without the chown. `scripts/cargo-registry.sh` is now the one body (t1064). Rejected:
`mkdir -p ~/.cargo/registry` on the host — it leaves an older root-owned volume broken. Details:
`openspec/changes/archive/2026-11-01-ci-green-again/design.md` (D1, D2).

## ci-red-again (2026-11-02): t935 asks GIO, compose has its SVG loader and runs first

The CI run on 9e1f1a45 reached two steps no run had reached. **t935** asked `xdg-mime`, which
without a desktop asks perl's `mimetype` (File::MimeInfo). Perl reads the priority-80 magic, then
keeps only the FIRST type `globs2` lists for an extension, so with our and Kotlin's equal `*.kt`
globs it called `Main.kt` ours. The order of those lines differs between shared-mime-info versions:
ours first on Ubuntu 2.1/2.4, Kotlin first on Fedora 2.3. The owner's Cinnamon asks GIO, so the
test was green there. GIO (2.72, 2.80, 2.82) and Qt 5/6 (`kmimetypefinder`) sniff and name all
three files right. t935 asks `gio info` (`libglib2.0-bin`) and asks perl only about our own files.
**The MIME file stays.** A lower glob weight was measured too: every content-based oracle,
perl included, then answers right. But every guess from the name alone (GIO's
`fast-content-type`, gvfs listings of network shares) would call our projects Kotlin everywhere,
where today it follows the `globs2` order. That is a user-visible trade, left to the owner.
**Compose:** flatpak-builder runs `appstreamcli compose` on the host. Compose renders the SVG icon
through gdk-pixbuf, whose SVG loader `librsvg2-common` is a Recommends of `librsvg2-2` and was
dropped (`file-read-error`, then `filters-but-no-output`). Both packages are rows of
`RECOMMENDS_NEEDED` (t1380). `appstream-compose-check.sh` composes the manifest's own install lines
with flatpak-builder's arguments BEFORE the ten-minute build (t1381; reproduced red and green in
`ubuntu:24.04`). The webp screenshots still give `screenshot-save-error` warnings (no WebP pixbuf
loader); they do not fail compose, which stores no media under flatpak-builder. Details:
`openspec/changes/archive/2026-11-02-ci-red-again/design.md` (D1–D3, the measured table).

## ci-flatpak-screenshots (2026-11-03): the screenshots are mirrored as Flathub's linter wants

The run of 21505ad2 got past compose, the build and the install. Then `flatpak-builder-lint repo`
refused it: `appstream-missing-screenshots` and `appstream-screenshots-not-mirrored-in-ostree`.
ci-red-again had seen the `screenshot-save-error` hints and judged them harmless, because compose
still said `Success!`. But compose had dropped all four WebP screenshots: Ubuntu's gdk-pixbuf has no
WebP loader (`webp-pixbuf-loader`, now a `RECOMMENDS_NEEDED` row). And flatpak-builder ran without
`--mirror-screenshots-url`, so no `screenshots/<arch>` ref. The linter also refuses partial media
URLs, and full ones need `--compose-url-policy=full`, which is flatpak-builder 1.4.6+. Ubuntu 24.04
has 1.4.2, so CI and the release take 1.4.8 from `ppa:flatpak/stable`. `flatpak-build.sh` refuses
an older flatpak-builder at once and builds with Flathub's own `flathub-build` arguments. The
compose pre-flight checks every screenshot too, and fails in about a second. Flathub's linter page
says these checks are "never granted" an exception, so none was added. Rejected: building with the
`org.flatpak.Builder` flatpak (nested bwrap and a session bus on the runner, not reproducible
here), a hand `ostree commit` on 1.4.2 (the URLs stay partial), PNG screenshots (a new tag, and the
silent drop would stay). A whole local run in a privileged `ubuntu:24.04` container is green. Run
it with HOME outside `/root`: `--filesystem=host` does not expose `/root`, and `flatpak-check.sh`
works in `$HOME`. Details: `openspec/changes/archive/2026-11-03-ci-flatpak-screenshots/design.md`.

## The version follows the commit in a worktree too (worktree-version-and-thumb-lock, 2026-11-01)

- **What hurt.** `libs/kt-version/build.rs` watched `../../../.git/HEAD` and `.git/refs/heads`. In a
  git worktree `.git` is a FILE (`gitdir: …/.git/worktrees/<name>`), neither path existed, cargo was
  told nothing about git and never reran the script after a commit. The crash window of an executor
  said `kt26.11.01-a498ecfe` while `finish.sh` printed `b6c8f121` — the "stale program" class
  `finish.sh` exists against, in every worktree of the round.
- **The rule.** git names the directories (`rev-parse --git-dir`, `--git-common-dir`); watched are
  this checkout's HEAD, the loose file of its branch (a packed-only branch: the directory under
  `refs` the next commit writes it into), `packed-refs` — and only paths that exist: cargo reruns a
  script on every build for a missing one, and a rerun of this one relinks every binary. One file,
  `git_watch.rs`, included by `build.rs` and its test t1170. Checked on the worktree itself: a commit
  moved the embedded version of `target/debug/kinetren` from `4c65c355` to `2fe66bb2`.
- **Rejected:** the whole `refs/heads` — shared by every parallel session's worktree, each of their
  commits would relink this tree; parsing the `.git` file by hand — git is already needed for the
  version string, and `rev-parse` also handles `GIT_DIR`, submodules and relative `gitdir:`.

## The off switch moved to File › Preferences… (welcome-window-and-settings, archived 2026-11-02)

The owner decision "an off switch in View next to the telemetry checkbox" is revised by his own
request of 2026-11-01 (kkt-26.11.01 item 13): both checkboxes moved together into File ›
Preferences…, the update check right after statistics (t878 checks the adjacency). The how-to-switch-
off text names the new path. When the state already says a newer release is out, the start window's
What's new pane is headed by the same line (no request of its own).
