//! Test 179 (item 179): cross-render of a REAL film.
//!
//! The user’s proposal: “somewhere on the internet I came across opensource OTIO
//! files of public films — I propose taking some film whose sources weigh less
//! and rendering it with both programs too”.
//!
//! Taken: Netflix Open Content “Sol Levante” (CC BY 4.0), an animatic: 63
//! splices on one track, 23.976 fps, the timecode starts at one hour, one
//! H.264 1080p source. Synthetics do not give this: a real decode with
//! B-frames, an NTSC rate and real shot lengths (7…347 frames).
//!
//! Neither the .otio nor the media are COMMITTED (320 MB, and the repository with the .otio has
//! no license): `src/scripts/fetch-oc-media.sh` puts them into
//! fixtures/otio-oc/, without them the test is silently skipped.
//! KT_REQUIRE_OC_RENDER=1 turns the skip into a failure.
//!
//! The comparison cannot be pixel-exact: two different encoders on a real
//! picture will never give identical bytes. Therefore the metric is
//! SELF-CALIBRATING: a frame of our render MUST resemble the frame of melt
//! at the same moment MORE than the frame of melt one and a half seconds away.
//! A shifted splice breaks exactly this ratio, while a codec difference does not.

use kt_media::MediaSource;
use kt_time::Tick;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// How many seconds of the edit we render. The whole film — 162 s of 1080p in two
/// encoders; for checking splices a window is enough, and there are ~12 splices in it.
const WINDOW_S: f64 = 30.0;
/// Offset from a splice when choosing a comparison moment (share of a shot).
const EDGE: f64 = 0.25;

struct Env {
    python: PathBuf,
    otio: PathBuf,
    scripts: PathBuf,
}

fn env() -> Option<&'static Env> {
    static ENV: OnceLock<Option<Env>> = OnceLock::new();
    ENV.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let python = root.join("target/otio-venv/bin/python3");
        let marker = root.join("target/otio-venv/.kt-ok");
        let dir = root.join("fixtures/otio-oc");
        let otio = dir.join("SolLevante_Animatics_ver1_H264.otio");
        let mov = dir.join("SolLevante_Animatics_ver1_H264.mov");
        if !Command::new("melt").arg("-version").output().is_ok_and(|o| o.status.success()) {
            return fail("melt not found (apt install melt)");
        }
        if !python.exists() || !marker.exists() {
            return fail("no otio venv (run src/scripts/otio-mlt-venv.sh)");
        }
        if !otio.exists() || !mov.exists() {
            return fail("no fixtures/otio-oc (run src/scripts/fetch-oc-media.sh)");
        }
        Some(Env { python, otio, scripts: root.join("scripts") })
    })
    .as_ref()
}

fn fail(why: &str) -> Option<Env> {
    if std::env::var_os("KT_REQUIRE_OC_RENDER").is_some() {
        panic!("open-content cross-render required: {why}");
    }
    eprintln!("skipped: {why}");
    None
}

struct Prep {
    prepared: PathBuf,
    mlt: PathBuf,
    rate: f64,
    duration_s: f64,
    cuts: Vec<f64>,
}

fn prepare(env: &Env, out: &Path) -> Result<Prep, String> {
    let o = Command::new(&env.python)
        .arg(env.scripts.join("oc-prepare.py"))
        .arg(&env.otio)
        .arg(out)
        .arg(WINDOW_S.to_string())
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    let m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap())
            .map_err(|e| e.to_string())?;
    let mlt =
        m["mlt"].as_str().ok_or_else(|| format!("MLT adapter refused: {}", m["mlt_error"]))?;
    Ok(Prep {
        prepared: PathBuf::from(m["prepared"].as_str().unwrap()),
        mlt: PathBuf::from(mlt),
        rate: m["rate"].as_f64().unwrap(),
        duration_s: m["duration_seconds"].as_f64().unwrap(),
        cuts: m["cuts"].as_array().unwrap().iter().filter_map(|v| v.as_f64()).collect(),
    })
}

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

fn render_ours(prepared: &Path, out: &Path) -> Result<(), String> {
    let project = import_prepared(prepared)?;
    let o = Command::new(std::env::var_os("NEXTEST_BIN_EXE_kt-cli").unwrap_or_else(|| env!("CARGO_BIN_EXE_kt-cli").into()))
        .arg("export")
        .arg(&project)
        .arg(out)
        .arg("libx264")
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(())
}

/// melt on a profile with a FRACTIONAL rate: 24000/1001, not the rounded 24 —
/// otherwise the foreign render itself would drift by 0.1 % and the comparison would measure our drift
/// together with the foreign one.
fn render_theirs(mlt: &Path, rate: f64, dir: &Path, out: &Path) -> Result<(), String> {
    let (num, den) = ntsc_ratio(rate);
    let profile = dir.join("kt-profile");
    std::fs::write(
        &profile,
        format!(
            "description=kt oc render\nframe_rate_num={num}\nframe_rate_den={den}\n\
             width=1920\nheight=1080\nprogressive=1\nsample_aspect_num=1\n\
             sample_aspect_den=1\ndisplay_aspect_num=16\ndisplay_aspect_den=9\n\
             colorspace=709\n"
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
            "crf=18",
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

/// A fractional rate as a ratio of integers: 23.976→24000/1001, 24→24/1.
fn ntsc_ratio(rate: f64) -> (u32, u32) {
    let r = rate.round();
    if (rate - r).abs() < 1e-6 { (r as u32, 1) } else { ((r * 1000.0) as u32, 1001) }
}

/// Frame signature: brightness on a 16×9 grid. Stable to the difference of codecs,
/// but it breaks if a DIFFERENT shot is shown.
fn signature(f: &kt_media::VideoFrame) -> Vec<f64> {
    let (w, h) = (f.width as usize, f.height as usize);
    let mut out = vec![0.0; 16 * 9];
    let mut n = vec![0.0; 16 * 9];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            if i + 2 >= f.rgba.len() {
                continue;
            }
            let lum = 0.2126 * f.rgba[i] as f64
                + 0.7152 * f.rgba[i + 1] as f64
                + 0.0722 * f.rgba[i + 2] as f64;
            let cell = (y * 9 / h) * 16 + (x * 16 / w);
            out[cell] += lum;
            n[cell] += 1.0;
        }
    }
    for (v, c) in out.iter_mut().zip(n) {
        if c > 0.0 {
            *v /= c;
        }
    }
    out
}

fn diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f64>() / a.len() as f64
}

fn frame_at(src: &mut dyn MediaSource, t: f64) -> Option<kt_media::VideoFrame> {
    src.read_video_frame(Tick((t * 90_000.0).round() as i64), None).ok()
}

#[test]
fn t179_real_film_cross_render() {
    let Some(env) = env() else { return };
    let dir = std::env::temp_dir().join(format!("kt-oc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let prep = prepare(env, &dir).expect("preparing the real film");
    let ours_path = dir.join("ours.mp4");
    let theirs_path = dir.join("theirs.mp4");
    render_ours(&prep.prepared, &ours_path).expect("our render");
    render_theirs(&prep.mlt, prep.rate, &dir, &theirs_path).expect("melt render");

    let mut ours =
        kt_media::ffmpeg::FfmpegSource::open(ours_path.to_str().unwrap()).expect("our render");
    let mut theirs =
        kt_media::ffmpeg::FfmpegSource::open(theirs_path.to_str().unwrap()).expect("melt render");

    // duration: we compare in SECONDS — the number of frames is different for us and for melt
    // by construction (our grid is 24, melt’s is 24000/1001, item 178)
    let (od, td) =
        (ours.info().duration.0 as f64 / 90_000.0, theirs.info().duration.0 as f64 / 90_000.0);
    let frame = 1.0 / prep.rate;
    assert!(
        (od - td).abs() <= 2.0 * frame,
        "durations diverged: ours {od:.3} s, melt {td:.3} s (edit {:.3} s)",
        prep.duration_s
    );

    // comparison moments: inside each shot, with an offset from the splices
    let mut probes: Vec<f64> = Vec::new();
    let mut prev = 0.0;
    for &c in &prep.cuts {
        let (a, b) = (prev, c.min(prep.duration_s));
        if b - a > 4.0 * frame {
            probes.push(a + (b - a) * EDGE);
            probes.push(a + (b - a) * (1.0 - EDGE));
        }
        prev = c;
    }
    assert!(probes.len() >= 8, "too few check points: {}", probes.len());

    // How many frames of drift we tolerate. Our grid is 24, melt’s is 24000/1001
    // (item 178): by the end of the third minute about ~4 frames accumulate, on a 30 s window —
    // no more than two. More than that is already a shifted splice, not drift.
    const MAX_SHIFT: i64 = 2;

    let mut bad: Vec<String> = Vec::new();
    let mut worst = 0i64;
    for &t in &probes {
        let Some(a) = frame_at(&mut ours, t) else {
            bad.push(format!("{t:.3} s: our frame could not be read"));
            continue;
        };
        let sa = signature(&a);
        // we look for which frame of melt our frame is explained by BEST:
        // the offset in frames is the measured drift itself
        let mut best: Option<(i64, f64)> = None;
        for k in -4i64..=4 {
            let tt = t + k as f64 * frame;
            if tt < 0.0 || tt >= prep.duration_s - frame {
                continue;
            }
            if let Some(f) = frame_at(&mut theirs, tt) {
                let d = diff(&sa, &signature(&f));
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((k, d));
                }
            }
        }
        match best {
            None => bad.push(format!("{t:.3} s: melt frames could not be read")),
            Some((k, d)) if d > 24.0 => bad.push(format!(
                "{t:.3} s: no melt frame resembles ours (best {d:.1}/255 at shift {k})"
            )),
            Some((k, _)) if k.abs() > MAX_SHIFT => bad.push(format!(
                "{t:.3} s: our frame matched melt only at a shift of {k} frames — the splice drifted"
            )),
            Some((k, _)) => worst = worst.max(k.abs()),
        }
    }
    assert!(
        bad.is_empty(),
        "check points {}, mismatches {}:\n{}",
        probes.len(),
        bad.len(),
        bad.join("\n")
    );
    eprintln!(
        "t179: {} check points over {:.1} s of edit — matched, worst drift {worst} frame(s) \
         (our grid {:.0} against {:.3} in melt, item 178)",
        probes.len(),
        prep.duration_s,
        prep.rate.round(),
        prep.rate
    );
}
