//! The first contact with a libva driver happens in a throwaway process (hw-encode-context D10).
//!
//! `h264_vaapi` and Linux `h264_qsv` load the machine's VA driver into whoever opens them, and a
//! driver that faults takes that process with it: no return code reaches the cascade, so the render
//! and the unsaved project end there (ktprompts v26.10 T16). So our own binary is started once more
//! to open the device, open the encoder and encode a few frames into `/dev/null`; only when it
//! says so does the driver enter this process. A signal, a hang or a refusal in it is a skip with
//! a reason. Nothing is remembered: every export asks again (the export spec's "no cached verdict").
//!
//! A binary that hosts the trial calls [`serve`] at the top of `main`, before it does any work. A process that did not (a
//! test harness) has no way to run it, and opens in-process as before: [`Verdict::NotHosted`].

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const TRIAL_ARG: &str = "--kt-hw-trial";
/// What the throwaway process prints its verdict after; a driver may print to stdout too.
const ANSWER: &str = "kt-hw-trial: ";
/// Frames of the trial: enough to get past the first picture, where a driver that opens and cannot
/// encode refuses. Three, for the P frames after the IDR.
const TRIAL_FRAMES: usize = 3;
/// The output of the trial: nothing is left behind, whatever happens to the process.
pub(crate) const TRIAL_SINK: &str = "/dev/null";
const ENCODER_TIMEOUT: Duration = Duration::from_secs(20);
/// The device is asked on the UI thread when the render window opens (`available_encoders`).
const DEVICE_TIMEOUT: Duration = Duration::from_secs(5);

static HOSTED: AtomicBool = AtomicBool::new(false);
static IN_TRIAL: AtomicBool = AtomicBool::new(false);

/// What the throwaway process found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// This process cannot run the trial: open in-process.
    NotHosted,
    /// It worked there; the text is the driver's vendor for a device, empty for an encoder.
    Passed(String),
    /// It did not, and why: the failing part, or how the process died.
    Refused(String),
}

/// Call at the top of `main` of a binary that exports: marks it as hosting the trial, and when this
/// process IS a trial, runs it and exits.
pub fn serve() {
    HOSTED.store(true, Ordering::Relaxed);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some(TRIAL_ARG) {
        return;
    }
    IN_TRIAL.store(true, Ordering::Relaxed);
    let answer = match args.get(1).map(String::as_str) {
        Some("device") => args.get(2).ok_or("no device".to_string()).and_then(|p| trial_device(p)),
        Some("encoder") => trial_encoder(&args[2..]),
        _ => Err("unknown trial".to_string()),
    };
    match answer {
        Ok(text) => println!("{ANSWER}ok {text}"),
        Err(why) => println!("{ANSWER}fail {why}"),
    }
    std::process::exit(0);
}

fn trial_device(path: &str) -> Result<String, String> {
    match crate::hw_open_device(path) {
        Some(r) => r,
        None => Err("FFmpeg is not loaded".into()),
    }
}

/// `<name> <width> <height> <fps> <quality> <bitrate, 0 = none>`
fn trial_encoder(args: &[String]) -> Result<String, String> {
    let [name, width, height, fps, quality, bitrate] = args else {
        return Err("bad trial arguments".into());
    };
    let number = |s: &String| s.parse::<u32>().map_err(|_| format!("bad number {s}"));
    let (width, height) = (number(width)?.max(2) & !1, number(height)?.max(2) & !1);
    let cfg = crate::MuxConfig {
        path: TRIAL_SINK.into(),
        width,
        height,
        fps: number(fps)?,
        quality: number(quality)?,
        video_bitrate: Some(number(bitrate)?).filter(|b| *b > 0),
        audio_bitrate: 128_000,
        force_encoder: Some(name.clone()),
        threads: 0,
    };
    let mut muxer = crate::Muxer::open(&cfg).map_err(|e| failing_part(&e.to_string(), name))?;
    let grey = vec![128u8; width as usize * height as usize * 4];
    for _ in 0..TRIAL_FRAMES {
        muxer.write_video(&grey).map_err(|e| format!("first frames: {e}"))?;
    }
    // `finish` flushes what the encoder still holds, so the count is taken after it
    let bytes = muxer.finish_counted().map_err(|e| format!("first frames: {e}"))?;
    if bytes == 0 {
        return Err("first frames: the encoder produced no data".into());
    }
    Ok(String::new())
}

/// The failing part of a cascade error: "no VAAPI device (…)", "code N", "not in this FFmpeg build".
pub fn failing_part(error: &str, encoder: &str) -> String {
    error
        .split_once(&format!("{encoder}: "))
        .map(|(_, rest)| rest.strip_suffix(')').unwrap_or(rest).to_string())
        .unwrap_or_else(|| error.to_string())
}

/// Ask whether `name` opens and encodes at the export's settings, in a throwaway process.
pub fn ask_encoder(name: &str, cfg: &crate::MuxConfig) -> Verdict {
    let args = [
        "encoder".to_string(),
        name.to_string(),
        cfg.width.to_string(),
        cfg.height.to_string(),
        cfg.fps.to_string(),
        cfg.quality.to_string(),
        cfg.video_bitrate.unwrap_or(0).to_string(),
    ];
    ask(&args, ENCODER_TIMEOUT)
}

/// Ask whether `path` opens as a VAAPI device, in a throwaway process.
pub fn ask_device(path: &str) -> Verdict {
    ask(&["device".to_string(), path.to_string()], DEVICE_TIMEOUT)
}

fn ask(args: &[String], timeout: Duration) -> Verdict {
    if !HOSTED.load(Ordering::Relaxed)
        || IN_TRIAL.load(Ordering::Relaxed)
        || crate::search::Os::NATIVE != crate::search::Os::Linux
    {
        return Verdict::NotHosted;
    }
    // a test shortens the wait for a driver that hangs
    let timeout = std::env::var("KT_HW_TRIAL_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(timeout, Duration::from_millis);
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return Verdict::Refused(format!("no test process: {e}")),
    };
    let mut command = Command::new(exe);
    command.arg(TRIAL_ARG).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    // the same FFmpeg: the folder chosen in the app is not in the environment
    if let Some(dir) = crate::user_ffmpeg_dir() {
        command.env("KINETREN_FFMPEG_DIR", dir);
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => return Verdict::Refused(format!("no test process: {e}")),
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Verdict::Refused(format!("the driver did not answer in {} s", timeout.as_secs_f32()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => return Verdict::Refused(format!("no test process: {e}")),
        }
    };
    let mut out = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = std::io::Read::read_to_string(&mut pipe, &mut out);
    }
    let answer = out.lines().rev().find_map(|l| l.strip_prefix(ANSWER));
    match answer {
        Some(a) if a == "ok" || a.starts_with("ok ") => Verdict::Passed(a[2..].trim().to_string()),
        Some(a) => Verdict::Refused(a.strip_prefix("fail ").unwrap_or(a).to_string()),
        None => Verdict::Refused(death(&status)),
    }
}

#[cfg(unix)]
fn death(status: &std::process::ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    match status.signal() {
        Some(s) => format!("the driver crashed in a test process (signal {s})"),
        None => format!("the test process ended without an answer ({status})"),
    }
}

#[cfg(not(unix))]
fn death(status: &std::process::ExitStatus) -> String {
    format!("the test process ended without an answer ({status})")
}
