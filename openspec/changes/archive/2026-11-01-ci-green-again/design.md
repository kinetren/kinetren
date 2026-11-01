# Design: ci-green-again

## Context

The six failures are read from the owner's archive of the CI run on 94e36e9e (jobs `test`,
`flatpak`, `ffmpeg9`, `windows-test`; `windows-build` was green). GitHub CI cannot be run from
here, so each cause is reproduced where it can be: the registry volume with Docker, t1063 on
Linux (red on the old muxer: "last error: "), t801/t802 in parallel, t962/t986 under wine
(`scripts/windows-test.sh`).

## Goals / Non-Goals

**Goals:** every cause fixed at its root, with a guard that turns red when the cause comes back.

**Non-Goals:** GPU bytes across machines (held by tolerances, not touched); the live output
resampler of kt-app (`FftFixedInOut`, playback only, not frozen).

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

### D6. One math for the frozen paths: pure-Rust libm, `f32` through `f64`

The owner's choice (01.11) between per-platform reference tables and our own kernel math was the
latter: "own math, close to Linux, where Linux is the more accurate one". `kt_plugin_api::math`
is the one place; `FrozenMath` gives its functions as methods (`x.fsin()` for `x.sin()`), so ~150
call sites in kt-model, kt-fx, kt-media, kt-engine, kt-plugin-api and the eleven families changed
mechanically. It lives in kt-plugin-api because that is the one crate every kernel already sees.

The implementation is the `libm` crate (pure Rust, FreeBSD's routines via musl; already in the
lock and the Flatpak sources). Measured on t1065's sweep, its own `f32` functions were a ulp off
glibc on ~10 % of `expf`/`powf` inputs: glibc's are ARM's near-correctly-rounded routines, libm's
are older. So the `f32` functions evaluate the `f64` one and round once — correctly rounded but
for ~2⁻²⁸ of inputs. Afterwards `expf`/`powf` differ from glibc on 0.05 %, and the larger shares
left (`asinf` 7 %, `atan2f` 17 %, `tanf` 4 %, `sinf` at thousands of radians 1.3 %) are where
glibc's routines are the less accurate ones. The `f64` functions are libm's, within one ulp of
glibc (3–10 % of inputs differ); on the frozen paths their results are rounded to `f32` or to a
pixel afterwards, which hides a last-bit difference but for ~2⁻²⁹ of values. A correctly rounded
`f64` set (CORE-MATH) would close that too; rejected for now as a large port with no hash that
needs it.

Rejected: interposing `sinf`/`powf` symbols for the whole binary — it would also catch third-party
code, but which definition the linker takes depends on archive order and the linker (mold, lld,
ld), and it silently changes egui's and wgpu's math too. Per-platform tables — two references to
keep, and a third CPU or CRT would need a third.

### D7. The warp's sinc interpolator is ours

rubato builds the 128 × 128 windowed-sinc table with `f32::sin`/`cos` and chooses AVX+FMA, SSE or
scalar for the dot product at run time — three different roundings, so Slowed+Reverb depended on
the OS AND the CPU. `FrozenSinc` (`kt-fx/src/audio/sinc.rs`, through rubato's
`new_with_interpolator`) is rubato's `make_sincs` (BlackmanHarris2, periodic, normalised the same
way, operations in its order) on our math, and the AVX+FMA product as the definition: eight lanes
of fused multiply-adds and rubato's horizontal sum. A fused multiply-add is exact by IEEE 754, so
the portable path (`fmaf`) and the AVX path give the same bits (t1067); the AVX path is taken only
behind kt-fx's `simd_level()` (`KT_SIMD=0` forces the portable one).

rustfft's twiddles (the pitch vocoder) are `f64` cosines rounded to `f32`: a platform difference
would have to sit on an `f32` midpoint, ~2⁻²⁹ per twiddle. Left as is and named here.

### D8. Eight values of t973c re-based, and why they are the Windows ones

With D6 and D7 the Linux run of t973c moved on exactly eight values — slowed_reverb (all four
cases), phaser (default, 70 %, reversed), pitch_shift (70 %) — and each new value equals what
windows-latest printed for it on 94e36e9e. On those inputs glibc was the one off by an ulp
(`sinf`/`cosf` of the sinc table, `tanf` in the phaser's all-pass, `atan2f` in the vocoder's
phases) and Windows' CRT was correctly rounded; our correctly rounded math agrees with the more
accurate side, which is the owner's rule. No other value of t973c, t1019 or t1048 moved. The test
is not weakened: the table is now a reference both platforms must give, where before it was
glibc's.

The export reference t11 tells the same: its Linux audio moved from glibc's 0x5a83eef245ab43f0
to 0x0a9c8e22f9f1e8c2 — the value windows-latest had measured on 2026-10-25 and committed as the
Windows row ("the C runtime's transcendentals are not glibc's"). The per-platform table keeps
both rows (t859), now with one audio value; the video value did not move.

## Risks / Trade-offs

- A future frozen call site that forgets `FrozenMath` is caught by t1066 only in the crates it
  scans; a new crate on a frozen path must be added to its list.
- The Dockerfile's base layer changes, so the `ffmpeg9` job rebuilds its cached image once
  (~4 min of FFmpeg) and local images rebuild on their next use.
- The Windows half (t962, t986, D6–D8) is confirmed under wine, not on windows-latest; the next CI run
  is the verdict.
