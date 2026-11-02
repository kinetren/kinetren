# Proposal: ci-flatpak-screenshots

## Why

In the CI run of 21505ad2 (2026-11-02 21:45 UTC), `flatpak` was the only red job. Its
`appstreamcli compose`, the build and the install passed. Then `flatpak-builder-lint
org.flatpak.Builder repo` refused the repository with two errors:
`appstream-missing-screenshots` ("Catalogue file has no screenshots") and
`appstream-screenshots-not-mirrored-in-ostree`. The Flathub linter docs say that neither check
ever gets an exception. A Flatpak that the linter refuses would not pass Flathub either.

## What Changes

There are three causes. Each was reproduced in an `ubuntu:24.04` container (the runner's system)
and in flatpak-builder-lint's own source:

1. **Compose drops every WebP screenshot.** `appstreamcli compose` downloads each screenshot and
   loads it through gdk-pixbuf. gdk-pixbuf has no WebP loader unless `webp-pixbuf-loader` is
   installed. The hint is `screenshot-save-error` "Could not load source screenshot for storing:
   Unrecognized image file format". Compose still says `Success!`, but the component has no
   `<screenshots>`, so the linter reports `appstream-missing-screenshots`. The URLs are reachable
   (HTTP 200, `image/webp`, tag v26.10.26). WebP is a format AppStream allows.
2. **No `screenshots/<arch>` ref.** flatpak-builder ran without `--mirror-screenshots-url`, so it
   stored no media and committed no screenshots ref. The linter wants that ref, holding the PNGs
   the catalogue names.
3. **Ubuntu's flatpak-builder cannot do what the linter wants.** The linter also requires every
   catalogue screenshot URL to start with `https://dl.flathub.org/media`
   (`appstream-external-screenshot-url`). Compose writes full URLs only with `--no-partial-urls`,
   which flatpak-builder passes for `--compose-url-policy=full`. That option came in 1.4.6, and
   flatpak-builder commits the screenshots ref by itself since 1.4.5. Ubuntu 24.04 ships 1.4.2.

The fix:
- CI and the release add `ppa:flatpak/stable` (the Flatpak project's own PPA: flatpak-builder 1.4.8
  for noble) and install `webp-pixbuf-loader`.
- `flatpak-build.sh` refuses a flatpak-builder older than 1.4.6 in milliseconds. It builds with
  `--mirror-screenshots-url=https://dl.flathub.org/media --compose-url-policy=full`, the arguments
  of Flathub's own `flathub-build`.
- `appstream-compose-check.sh` composes with the same mirror arguments. It fails unless every
  screenshot of the listing reaches the catalogue under the media URL as a stored PNG, so a host
  without the WebP loader fails in about a second, before the build.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `distribution`: the table of packages a check needs gains `webp-pixbuf-loader`. The Flatpak
  build mirrors the screenshots with a flatpak-builder new enough to do it, and the compose
  pre-flight checks the screenshots too.

## Impact

- `src/scripts/flatpak-build.sh`, `src/scripts/appstream-compose-check.sh`.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`: the PPA and the WebP loader.
- `src/scripts/dist-check.py`: a `RECOMMENDS_NEEDED` row, the mirror arguments and the PPA in
  `check_flatpak_ci`.
- `src/apps/kt-cli/tests/dist_guard.rs`: t1410 (the guard's red copies) and t1411 (the
  pre-flight keeps every screenshot, and an unreadable one fails it). t1381 now says it did not
  run when the screenshots cannot be fetched.
- `src/apps/kt-app/src/reach_tests.rs`: t850's `ffmpeg` binding is consumed on Windows, so the
  Windows build has no unused-variable warning.
- No program code and no user-visible behaviour change, so no What's new line (internal CI
  work).
