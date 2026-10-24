//! Test 158 (item 158, R.25): CROSS-RENDER conformance — foreign
//! .otio files are rendered by us and by melt (MLT, the engine of Kdenlive/Shotcut), and the results
//! are compared frame by frame.
//!
//! A match of the JSON (test 149) does not yet mean a match of the EDIT: a shifted
//! splice, a lost track or a hole in place of a misunderstood construct
//! are visible only in the picture. The synthetics are solid colors from a spaced-apart
//! palette, so “what plays at the moment t” turns into a classification
//! that is stable to the difference of codecs and BT.601/709 × limited/full conventions.
//!
//! The gate follows the pattern of t149: no melt or a venv with the adapter — the test is green
//! with a clear reason; KT_REQUIRE_CROSS_RENDER=1 (CI) — absence = failure.
//! Environment setup: `src/scripts/otio-mlt-venv.sh`.

use kt_media::MediaSource;
use kt_time::Tick;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The comparison tier. Strict — pixel by pixel; smoke — “both rendered and the
/// durations matched” for files with constructs that not both render.
enum Tier {
    Strict,
    Smoke,
}

const SAMPLES: &[(&str, Tier)] = &[
    ("simple_cut.otio", Tier::Strict),
    ("multiple_track.otio", Tier::Strict),
    ("preflattened.otio", Tier::Strict),
    ("multitrack.otio", Tier::Strict),
    // transitions: the MLT adapter expands them into separate segments and lengthens
    // the timeline — comparison on a common time scale is impossible (see item 158)
    ("transition_test.otio", Tier::Smoke),
    // Nested Stacks. The hole of item 158 is closed (R.50): we read them as real
    // reels and draw the content — there are no more placeholders in this sample
    // (t437). But the strict tier still did not become one, and the reason is the OPPOSITE of the
    // former: a strict run showed that nested Stacks
    // DROP the reference. Its render is shorter than ours by exactly the sum of their
    // lengths (31 + 29 + 4 frames), and everything after the first nested one
    // shifts by 31 frames — the picture matches only with this shift.
    // There is nothing to compare on a common time scale: it is not we who diverge.
    ("nested_example.otio", Tier::Smoke),
    ("generator_reference_test.otio", Tier::Smoke),
    // The Premiere export and the EDL edit — the most “real” of the samples; their
    // markers (Marker.3) are not read by the reference library itself, so for
    // the render prepare strips the markers — they do not affect the picture
    ("premiere_example.otio", Tier::Strict),
    ("screening_example.otio", Tier::Strict),
];

/// Acceptance threshold in classification (Euclidean in RGB): quantization ±2 per channel,
/// x264 crf15 noise on a flat fill ±3, rounding of a double conversion ≤6 —
/// a fivefold margin. The palette is spaced by 96 > 2·32, so the nearest
/// variant is guaranteed to be “its own”.
const ACCEPT: f64 = 32.0;

struct Env {
    python: PathBuf,
    samples: PathBuf,
    scripts: PathBuf,
}

fn env() -> Option<&'static Env> {
    static ENV: OnceLock<Option<Env>> = OnceLock::new();
    ENV.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let python = root.join("target/otio-venv/bin/python3");
        let marker = root.join("target/otio-venv/.kt-ok");
        let samples = root.join("fixtures/otio-samples");
        let scripts = root.join("scripts");
        let melt = Command::new("melt").arg("-version").output().is_ok_and(|o| o.status.success());
        if !melt {
            return fail("melt not found (apt install melt)");
        }
        if !python.exists() || !marker.exists() {
            return fail("no otio venv (run src/scripts/otio-mlt-venv.sh)");
        }
        if !samples.join("simple_cut.otio").exists() {
            return fail("no fixtures/otio-samples (run src/scripts/fetch-otio-samples.sh)");
        }
        Some(Env { python, samples, scripts })
    })
    .as_ref()
}

fn fail(why: &str) -> Option<Env> {
    if std::env::var_os("KT_REQUIRE_CROSS_RENDER").is_some() {
        panic!("cross-render conformance required: {why}");
    }
    eprintln!("skipped: {why}");
    None
}

struct Prep {
    prepared: PathBuf,
    mlt: Option<PathBuf>,
    mlt_error: Option<String>,
    palette: Vec<[u8; 3]>,
    transitions: Vec<(f64, f64)>,
    rate: f64,
    width: u32,
    height: u32,
    duration_s: f64,
}

/// Synthetics + replacement of references + .mlt: made by the reference python (which
/// also upgrades ancient sample schemas to the modern ones).
fn prepare(env: &Env, sample: &str, out: &Path) -> Result<Prep, String> {
    let script = env.scripts.join("otio-cross-prepare.py");
    let o = Command::new(&env.python)
        .arg(&script)
        .arg(env.samples.join(sample))
        .arg(out)
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    let text = std::fs::read_to_string(out.join("manifest.json")).map_err(|e| e.to_string())?;
    let m: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let palette = m["media"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            let c = x["rgb"].as_array().unwrap();
            [
                c[0].as_u64().unwrap() as u8,
                c[1].as_u64().unwrap() as u8,
                c[2].as_u64().unwrap() as u8,
            ]
        })
        .collect();
    let transitions = m["transitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["start_s"].as_f64().unwrap(), t["end_s"].as_f64().unwrap()))
        .collect();
    Ok(Prep {
        prepared: PathBuf::from(m["prepared"].as_str().unwrap()),
        mlt: m["mlt"].as_str().map(PathBuf::from),
        mlt_error: m["mlt_error"].as_str().map(str::to_string),
        palette,
        transitions,
        rate: m["rate"].as_f64().unwrap(),
        width: m["width"].as_u64().unwrap() as u32,
        height: m["height"].as_u64().unwrap() as u32,
        duration_s: m["duration_seconds"].as_f64().unwrap(),
    })
}

/// Our render. The encoder is forced: auto-selection could go to nvenc, and the difference
/// of ranges would add noise out of nowhere.
/// The prepared exchange file — into a project file by the same door as
/// `File → Import` (R.70): `kt-cli export` reads only `.kt`.
fn import_prepared(prepared: &Path) -> Result<PathBuf, String> {
    let kt = prepared.with_extension("kt");
    let o = Command::new(std::env::var_os("NEXTEST_BIN_EXE_kt-cli").unwrap_or_else(|| env!("CARGO_BIN_EXE_kt-cli").into()))
        .arg("import")
        .arg(prepared)
        .arg(&kt)
        .arg("--force")
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(kt)
}

fn render_ours(prepared: &Path, out: &Path) -> Result<u64, String> {
    let project = import_prepared(prepared)?;
    let o = Command::new(std::env::var_os("NEXTEST_BIN_EXE_kt-cli").unwrap_or_else(|| env!("CARGO_BIN_EXE_kt-cli").into()))
        .arg("export")
        .arg(&project)
        .arg(out)
        .arg("libx264")
        .output()
        .map_err(|e| e.to_string())?;
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    if !o.status.success() {
        return Err(err.trim().to_string());
    }
    // “ok: N video frames, …” — the exact counter of our frames (the container’s duration is spoiled
    // by AAC priming). Import notices may precede it.
    err.lines()
        .find(|l| l.starts_with("ok: "))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|n| n.parse::<u64>().ok())
        .ok_or_else(|| format!("could not parse the kt-cli output: {err}"))
}

/// Foreign render: melt on our profile. `real_time=0` is mandatory — otherwise
/// melt plays “in real time” and DROPS frames (the test would start guessing).
fn render_theirs(mlt: &Path, p: &Prep, dir: &Path, out: &Path) -> Result<(), String> {
    let profile = dir.join("kt-profile");
    let (w, h) = (p.width, p.height);
    let g = gcd(w, h).max(1);
    std::fs::write(
        &profile,
        format!(
            "description=kt cross render\nframe_rate_num={}\nframe_rate_den=1\nwidth={w}\n\
             height={h}\nprogressive=1\nsample_aspect_num=1\nsample_aspect_den=1\n\
             display_aspect_num={}\ndisplay_aspect_den={}\ncolorspace=709\n",
            p.rate.round() as u32,
            w / g,
            h / g
        ),
    )
    .map_err(|e| e.to_string())?;
    let o = Command::new("melt")
        .arg("-silent")
        .arg("-profile")
        .arg(&profile)
        .arg(mlt)
        .arg("-consumer")
        .arg(format!("avformat:{}", out.display()))
        .args([
            "vcodec=libx264",
            "preset=veryfast",
            "crf=15",
            "g=15",
            "pix_fmt=yuv420p",
            "an=1",
            "real_time=0",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() || !out.exists() {
        return Err(format!("melt failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(())
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

// ---- color classification ----------------------------------------------------

/// All values into which a solid color can turn under ONE
/// confusion of conventions (matrix 601/709 × range limited/full on encoding
/// and on decoding). The classifier looks for the nearest VARIANT, so a foreign
/// convention does not give a false divergence.
fn variants(rgb: [u8; 3]) -> Vec<[f64; 3]> {
    const M: [(f64, f64); 2] = [(0.299, 0.114), (0.2126, 0.0722)];
    let mut out: Vec<[f64; 3]> = Vec::new();
    for (kr1, kb1) in M {
        for lim1 in [true, false] {
            let yuv = encode(rgb, kr1, kb1, lim1);
            for (kr2, kb2) in M {
                for lim2 in [true, false] {
                    let v = decode(yuv, kr2, kb2, lim2);
                    if !out.iter().any(|u| dist(*u, v) < 1e-9) {
                        out.push(v);
                    }
                }
            }
        }
    }
    out
}

fn encode(rgb: [u8; 3], kr: f64, kb: f64, limited: bool) -> [f64; 3] {
    let [r, g, b] = [rgb[0] as f64 / 255.0, rgb[1] as f64 / 255.0, rgb[2] as f64 / 255.0];
    let kg = 1.0 - kr - kb;
    let y = kr * r + kg * g + kb * b;
    let pb = (b - y) / (2.0 * (1.0 - kb));
    let pr = (r - y) / (2.0 * (1.0 - kr));
    if limited {
        [
            (16.0 + 219.0 * y).round().clamp(0.0, 255.0),
            (128.0 + 224.0 * pb).round().clamp(0.0, 255.0),
            (128.0 + 224.0 * pr).round().clamp(0.0, 255.0),
        ]
    } else {
        [
            (255.0 * y).round().clamp(0.0, 255.0),
            (128.0 + 255.0 * pb).round().clamp(0.0, 255.0),
            (128.0 + 255.0 * pr).round().clamp(0.0, 255.0),
        ]
    }
}

fn decode(yuv: [f64; 3], kr: f64, kb: f64, limited: bool) -> [f64; 3] {
    let (y, pb, pr) = if limited {
        ((yuv[0] - 16.0) / 219.0, (yuv[1] - 128.0) / 224.0, (yuv[2] - 128.0) / 224.0)
    } else {
        (yuv[0] / 255.0, (yuv[1] - 128.0) / 255.0, (yuv[2] - 128.0) / 255.0)
    };
    let kg = 1.0 - kr - kb;
    let r = y + 2.0 * (1.0 - kr) * pr;
    let b = y + 2.0 * (1.0 - kb) * pb;
    let g = (y - kr * r - kb * b) / kg;
    [
        (r * 255.0).round().clamp(0.0, 255.0),
        (g * 255.0).round().clamp(0.0, 255.0),
        (b * 255.0).round().clamp(0.0, 255.0),
    ]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Color class: palette index (`None` — emptiness/black, `Err` — resembles nothing:
/// this is also a divergence, if the other side has a class).
fn classify(obs: [f64; 3], table: &[Vec<[f64; 3]>]) -> Result<Option<usize>, String> {
    if dist(obs, [0.0, 0.0, 0.0]) <= ACCEPT {
        return Ok(None);
    }
    let mut best: Option<(usize, f64)> = None;
    for (i, vs) in table.iter().enumerate() {
        for v in vs {
            let d = dist(obs, *v);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
    }
    match best {
        Some((i, d)) if d <= ACCEPT => Ok(Some(i)),
        Some((_, d)) => Err(format!(
            "#{:02X}{:02X}{:02X} (nearest at {d:.1})",
            obs[0] as u8, obs[1] as u8, obs[2] as u8
        )),
        None => Err("palette is empty".into()),
    }
}

/// The average color of the CENTRAL half of the frame: letterboxing and edge artifacts
/// of scaling do not fall into it.
fn center_mean(f: &kt_media::VideoFrame) -> [f64; 3] {
    let (w, h) = (f.width as usize, f.height as usize);
    let (x0, x1) = (w / 4, w * 3 / 4);
    let (y0, y1) = (h / 4, h * 3 / 4);
    let mut acc = [0f64; 3];
    let mut n = 0f64;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * w + x) * 4;
            if i + 2 >= f.rgba.len() {
                continue;
            }
            for (c, a) in acc.iter_mut().enumerate() {
                *a += f.rgba[i + c] as f64;
            }
            n += 1.0;
        }
    }
    if n == 0.0 { [0.0; 3] } else { [acc[0] / n, acc[1] / n, acc[2] / n] }
}

/// The comparison moments: the middles of “calm” segments between splices, outside the windows
/// of transitions (fade curves differ by definition between us and melt)
/// and with a gap of ±1 frame from any boundary.
fn probes(project: &kt_model::Project, p: &Prep) -> (Vec<i64>, usize) {
    let fd = (90_000.0 / p.rate).round() as i64;
    let mut bounds: Vec<i64> = vec![0];
    for c in &project.clips {
        bounds.push(c.start.0);
        bounds.push((c.start + c.duration).0);
    }
    for f in &project.foreign {
        bounds.push(f.start.0);
        bounds.push((f.start + f.duration).0);
    }
    let end = (p.duration_s * 90_000.0).round() as i64;
    bounds.push(end);
    bounds.sort_unstable();
    bounds.dedup();

    let mut skip: Vec<(i64, i64)> = p
        .transitions
        .iter()
        .map(|(a, b)| ((a * 90_000.0).round() as i64 - fd, (b * 90_000.0).round() as i64 + fd))
        .collect();
    // zones of foreign constructs that we show with a placeholder (decision
    // R.24 item 5): there is nothing to compare there — we are empty by definition. The number of
    // skipped points is printed, there is no silent loss of coverage.
    skip.extend(project.foreign.iter().map(|f| (f.start.0 - fd, (f.start + f.duration).0 + fd)));

    let mut out = Vec::new();
    for w in bounds.windows(2) {
        let (a, b) = (w[0] + fd, w[1] - fd);
        if b - a < fd {
            continue; // a segment shorter than three frames — the gap from splices matters more
        }
        for num in [1i64, 2, 3] {
            let t = a + (b - a) * num / 4;
            if skip.iter().any(|(s, e)| t >= *s && t <= *e) {
                continue;
            }
            let frame = t / fd;
            if !out.contains(&frame) {
                out.push(frame);
            }
        }
    }
    out.sort_unstable();
    let total = out.len();
    // a cap on long timelines — deterministic thinning
    if total > 96 {
        let step = total.div_ceil(96);
        out = out.into_iter().step_by(step).collect();
    }
    let dropped = total - out.len();
    (out, dropped)
}

#[test]
fn t158_cross_render_conformance() {
    let Some(env) = env() else { return };
    let root = std::env::temp_dir().join(format!("kt-cli-t158-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut problems: Vec<String> = Vec::new();

    for (sample, tier) in SAMPLES {
        let dir = root.join(sample);
        std::fs::create_dir_all(&dir).unwrap();
        let prep = match prepare(env, sample, &dir) {
            Ok(p) => p,
            Err(e) => {
                problems.push(format!("{sample}: preparation failed: {e}"));
                continue;
            }
        };
        let Some(mlt) = prep.mlt.clone() else {
            let why = prep.mlt_error.clone().unwrap_or_default();
            if matches!(tier, Tier::Smoke) {
                eprintln!("{sample}: MLT adapter refused ({why}) — skipped");
                continue;
            }
            problems.push(format!("{sample}: MLT adapter refused: {why}"));
            continue;
        };

        let ours_path = dir.join("ours.mp4");
        let theirs_path = dir.join("theirs.mp4");
        let frames_ours = match render_ours(&prep.prepared, &ours_path) {
            Ok(n) => n,
            Err(e) => {
                problems.push(format!("{sample}: our render crashed: {e}"));
                continue;
            }
        };
        if let Err(e) = render_theirs(&mlt, &prep, &dir, &theirs_path) {
            problems.push(format!("{sample}: the foreign render crashed: {e}"));
            continue;
        }

        let mut ours = kt_media::ffmpeg::FfmpegSource::open(ours_path.to_str().unwrap()).unwrap();
        let mut theirs =
            kt_media::ffmpeg::FfmpegSource::open(theirs_path.to_str().unwrap()).unwrap();
        let ti = theirs.info().clone();
        if (ti.width, ti.height) != (prep.width, prep.height) {
            problems.push(format!(
                "{sample}: melt rendered {}×{} instead of {}×{}",
                ti.width, ti.height, prep.width, prep.height
            ));
            continue;
        }

        let fd = (90_000.0 / prep.rate).round() as i64;
        let frames_theirs = (ti.duration.0 as f64 / fd as f64).round() as u64;
        let expect = (prep.duration_s * prep.rate).round() as u64;
        if frames_ours.abs_diff(expect) > 1 {
            problems.push(format!(
                "{sample}: our render {frames_ours} frames versus the reference {expect}"
            ));
        }

        let text = std::fs::read_to_string(&prep.prepared).unwrap();
        let bundle = kt_otio::load_project(&text, prep.prepared.parent()).unwrap();

        match tier {
            Tier::Smoke => {
                eprintln!(
                    "{sample}: smoke — ours {frames_ours} frames, melt {frames_theirs}, \
                     placeholders {}",
                    bundle.project.foreign.len()
                );
                continue;
            }
            Tier::Strict => {
                if frames_ours.abs_diff(frames_theirs) > 1 {
                    problems.push(format!(
                        "{sample}: durations diverged — ours {frames_ours}, melt {frames_theirs}"
                    ));
                }
            }
        }

        let table: Vec<Vec<[f64; 3]>> = prep.palette.iter().map(|c| variants(*c)).collect();
        let (frames, dropped) = probes(&bundle.project, &prep);
        assert!(!frames.is_empty(), "{sample}: nothing to build the probe grid from");
        eprintln!(
            "{sample}: strict — {} frames, {} probe points, placeholders {}{}",
            frames_ours,
            frames.len(),
            bundle.project.foreign.len(),
            if dropped > 0 { format!(", thinned {dropped}") } else { String::new() }
        );
        for f in frames {
            let t = Tick(f * fd);
            let (Ok(a), Ok(b)) = (ours.read_video_frame(t, None), theirs.read_video_frame(t, None))
            else {
                problems.push(format!("{sample}: frame {f} cannot be read"));
                continue;
            };
            let (ma, mb) = (center_mean(&a), center_mean(&b));
            let (ca, cb) = (classify(ma, &table), classify(mb, &table));
            let same = match (&ca, &cb) {
                (Ok(x), Ok(y)) => x == y,
                _ => false,
            };
            if !same {
                problems.push(format!(
                    "{sample} t={:.3}s frame {f}: ours {} (RGB {:.0},{:.0},{:.0}), melt {} (RGB {:.0},{:.0},{:.0})",
                    f as f64 / prep.rate,
                    show(&ca, &prep.palette),
                    ma[0], ma[1], ma[2],
                    show(&cb, &prep.palette),
                    mb[0], mb[1], mb[2],
                ));
            }
        }
    }

    assert!(problems.is_empty(), "cross-render diverged:\n{}", problems.join("\n"));
    std::fs::remove_dir_all(&root).ok();
}

fn show(c: &Result<Option<usize>, String>, palette: &[[u8; 3]]) -> String {
    match c {
        Ok(None) => "empty".into(),
        Ok(Some(i)) => {
            format!("#{:02X}{:02X}{:02X}", palette[*i][0], palette[*i][1], palette[*i][2])
        }
        Err(e) => format!("not recognized {e}"),
    }
}
