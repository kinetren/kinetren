# Tasks

## 1. The CI jobs install and mount what they need (distribution)

- [x] 1.1 `src/scripts/cargo-registry.sh`: the registry a container mounts — the host's, or the named volume chowned to the caller; `release-build.sh`, `windows-test.sh`, `ffmpeg-test.sh` take it from there. Verify: a fresh volume refuses `mkdir` to the user, the one the script hands over accepts it (Docker, local reproduction of the ffmpeg9 failure)
- [x] 1.2 `dist-check.py` `check_registry`: a script mounting the registry without the helper, or a helper without the chown, is refused — t1064
- [x] 1.3 `elfutils` in the flatpak installs of `ci.yml` and `release.yml`; `libfile-mimeinfo-perl` in the test job of `ci.yml` and the Dockerfile's base stage; `dist-check.py` `RECOMMENDS_NEEDED` + `check_recommends` — t1060
- [x] 1.4 t935 says why it did not run without `mimetype` — t935 green on the owner's machine

## 2. Live guards and canonical paths (durability)

- [x] 2.1 t801 and t802 each pass their own `KT_LIVE_OUT` (`target/live-guard-t801`, `-t802`) — t801, t802 green in one run
- [x] 2.2 `kt_dirs::plain_path` + `kt_dirs::canonical` (the `dunce` rule, pure over `Os`) — t1062
- [ ] 2.3 `user_store::project_key` uses `kt_dirs::canonical`; t986 compares the folder in its canonical spelling and refuses a `\\?\` label — t986 under wine (`scripts/windows-test.sh`) and on Linux

## 3. A forced encoder outside the cascade (export)

- [x] 3.1 The muxer names a forced encoder this system's cascade never tries, with the cascade — t1063 (red on the old muxer: "last error: ")
- [ ] 3.2 t962 expects that error where the cascade has no VAAPI — t962 under wine and on Linux

## 4. Records and finish

- [x] 4.1 What's new (`docs/guide/whats-new.md`, kt26.11): the welcome cards on Windows name the folder without `\\?\`
- [x] 4.2 Registry row C1 of `docs/ktprompts/kkt-26.11.01-index.md` moves to done; `python3 src/scripts/ktprompts-index.py --check` green
- [ ] 4.3 `cargo test --workspace` and `cargo clippy --all-targets` green; `src/scripts/finish.sh` — version recorded here

Not in this change, waiting for the owner: Windows t973c, t1019, t1048 — frozen hashes printed on
Linux, MinGW's `powf`/`tan` round differently (invariant 11).
