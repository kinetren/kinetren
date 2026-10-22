# Tasks

## 0. Records before code

- [x] 0.1 Append the owner's words and decisions of 2026-10-22 (PostHog free, opt-out, exit event only, one EU region after the time-zone question, own client not a heavy SDK, the notice lists what is sent and what is NOT sent) to `docs/user-feedback.md`; verify the entry names this change
- [x] 0.2 Feature note in `docs/features/` plus a `posthog` row in `docs/features/_services.md`; correct the "no HTTP crate in Cargo.lock" sentence of rule 13 in `docs/features/README.md`; verify `python3 src/scripts/features-index.py --check` is green

## 1. Core module (no UI)

- [x] 1.1 Add `ureq` (rustls + webpki-roots, no default native-tls) to `src/apps/kt-app/Cargo.toml` only; verify `cargo tree -p kt-app -i openssl-sys` finds nothing and libs/ crates gained no dependency
- [x] 1.2 `src/apps/kt-app/src/telemetry.rs`: `resolve_consent`, install id in `<data_dir>/telemetry-id`, event builder with the closed allow-list, `Sink` trait (HTTP sink + recording sink for tests); verify t837, t838, t839
- [x] 1.3 Sender thread with `sync_channel(4)`, `try_send`, 1 s connect / 2 s total timeouts, exit wait `recv_timeout(2 s)`, one journal line on failure, `Drop` sends nothing; verify t836 and t841 (local `TcpListener`: black hole and `200`)
- [x] 1.4 Key only via `option_env!("KINETREN_POSTHOG_KEY")` and the tree guard against `phc_` literals in `.rs`; verify t842 (its `release-build.sh` assertion lands with 3.1)
- [x] 1.5 Commit stage 1 (`cargo test -p kt-app` green, `cargo clippy --all-targets` clean)

## 2. Wiring into the app

- [x] 2.1 `KtApp` holds `Telemetry`; resolve consent in the `run_native` closure from env + `cc.storage` `telemetry`; `app_started` on the first `update`; `save` stores `telemetry`; move the value across every `*self = app` / `*self = Self::new_empty(..)` (File → New/Open, recovery) so New/Open never sends a second start; verify t836 and a harness check that File → New keeps the session id
- [x] 2.2 `on_exit`: after the recovery write, enqueue `app_closed` and wait within the 2 s budget; verify the durability tests stay green (t351…, `openspec show durability --type spec`) and t841
- [x] 2.3 First-start notice through `KtApp::warn`-style notices (info, once) with the "sent" / "NOT sent" lists and the same lists as the checkbox hover text and the View menu checkbox next to Language (disabled with hover text when the build has no key); strings in all five catalogs `src/apps/kt-app/i18n/{en,ru,es,ar,zh-Hans}.toml`; verify t840 (`reach_tests.rs`, with the ПУТЬ line), `font_tests.rs`, `i18n_tests.rs`, `layout_tests.rs`
- [x] 2.4 Commit stage 2

## 3. Release build and docs

- [ ] 3.1 After `github-ci-docker-build` has committed its `release-build.sh` stage: add `src/packaging/posthog-key` with the key the owner supplied (design.md, Open Questions) and pass `-e KINETREN_POSTHOG_KEY="$(cat …)"` into the container `docker run` in `src/scripts/release-build.sh`; verify t842 and that `release-build.sh` with an empty key still builds (no key → sends nothing)
- [ ] 3.2 README "Privacy" paragraph: what is sent, what is NOT sent (the same two lists as the notice), where to switch it off (View menu, `KINETREN_TELEMETRY=0`, `DO_NOT_TRACK=1`); verify `readme-check.py --check` if the README guard covers it
- [ ] 3.3 Owner steps listed in the report (not done by the agent): create the PostHog EU project, enable "Discard client IP data", confirm the project is in EU; a dashboard with launches, mean `duration_s`, and starts without `app_closed` by `$session_id`
- [ ] 3.4 Commit stage 3

## 4. Finish

- [ ] 4.1 `cargo test --workspace` and `cargo clippy --all-targets` green; live check with a local capture server: run `target/debug/kinetren` built with `KINETREN_POSTHOG_KEY=phc_test` and the endpoint overridden by the test-only env `KT_TELEMETRY_URL` (debug builds only), start → close, the server log holds `app_started` and `app_closed` with `duration_s`
- [ ] 4.2 `src/scripts/finish.sh`; version `vYY.MM.DD-hash` in the report
