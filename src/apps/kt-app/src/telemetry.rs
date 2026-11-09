//! Anonymous usage telemetry (change `telemetry-posthog`, capability `telemetry`).
//!
//! Two events per process go to one PostHog Cloud EU project: `app_started` after the first
//! frame and `app_closed` from `on_exit` with the session length. A session whose
//! `app_closed` never arrived did not close cleanly — there is no crash marker on disk
//! (owner, 2026-10-22).
//!
//! Our own capture client: one JSON POST per event, no PostHog SDK. The payload is a closed
//! allow-list (`properties`, t837). Sending happens on one background thread fed by a bounded
//! channel; the UI thread only enqueues, and the exit waits at most a fixed budget (t841).
//!
//! The project key enters the binary only at compile time (`KINETREN_POSTHOG_KEY`, set by
//! `release-build.sh`): `cargo run`, tests and the live scenarios never report (t842).
//!
//! One `Telemetry` per process (`install`/`global`): File → New/Open rebuild `KtApp` in place,
//! and a per-app value would send a second `app_started` for each of them.

use eframe::egui;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// PostHog Cloud EU ingestion endpoint (one region, owner's choice).
pub const EU_CAPTURE_URL: &str = "https://eu.i.posthog.com/i/v0/e/";
/// The exit waits for `app_closed` at most this long (spec: 2 s after the recovery write).
pub const EXIT_BUDGET: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(2);
/// Name of the install id file inside the data directory.
pub const ID_FILE: &str = "telemetry-id";
/// The start counter of this install (`instance_seq`).
pub const SEQ_FILE: &str = "telemetry-seq";
/// One `<session>.lock` per running process (`instances_running`).
pub const INSTANCES_DIR: &str = "instances";
/// eframe storage key of the person's choice (`on`/`off`), next to `lang`.
pub const STORAGE_KEY: &str = "telemetry";

/// The project key of this build, if the release build provided one.
pub fn key() -> Option<&'static str> {
    option_env!("KINETREN_POSTHOG_KEY").map(str::trim).filter(|k| !k.is_empty())
}

/// A debug build's local target (D11): `KT_TELEMETRY_URL` and `KT_TELEMETRY_KEY`, BOTH set,
/// send to a stand-in of PostHog (the live scenario `telemetry.py`). A release build ignores
/// both, so a shipped binary can neither be redirected nor keyed from outside (t842).
pub fn local_target_from(
    debug: bool,
    url: Option<String>,
    key: Option<String>,
) -> Option<(String, String)> {
    let (url, key) = (url?.trim().to_string(), key?.trim().to_string());
    (debug && !url.is_empty() && !key.is_empty()).then_some((url, key))
}

fn local_target() -> Option<(String, String)> {
    local_target_from(
        cfg!(debug_assertions),
        std::env::var("KT_TELEMETRY_URL").ok(),
        std::env::var("KT_TELEMETRY_KEY").ok(),
    )
}

/// Why telemetry cannot be on, whatever the person chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    /// The build carries no project key (every dev build).
    NoKey,
    /// `KINETREN_TELEMETRY=0|off|false` or `DO_NOT_TRACK=1`.
    Env,
}

/// The environment variables that decide consent, read once.
#[derive(Debug, Clone, Default)]
pub struct EnvView {
    pub telemetry: Option<String>,
    pub do_not_track: Option<String>,
}

impl EnvView {
    pub fn from_env() -> Self {
        EnvView {
            telemetry: std::env::var("KINETREN_TELEMETRY").ok(),
            do_not_track: std::env::var("DO_NOT_TRACK").ok(),
        }
    }

    fn forbids(&self) -> bool {
        let off = |v: &Option<String>, words: &[&str]| {
            v.as_deref()
                .map(|s| words.contains(&s.trim().to_ascii_lowercase().as_str()))
                .unwrap_or(false)
        };
        off(&self.telemetry, &["0", "off", "false", "no"])
            || off(&self.do_not_track, &["1", "true", "yes"])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consent {
    /// The person's choice (default on — opt-out).
    pub choice: bool,
    /// Nothing stored yet: the notice is due once.
    pub first_start: bool,
    pub block: Option<Block>,
}

impl Consent {
    pub fn on(&self) -> bool {
        self.choice && self.block.is_none()
    }
}

/// Consent from the key, the environment and the stored choice (t839). The environment can
/// only turn telemetry OFF: re-enabling what the person disabled would be a dark pattern.
pub fn resolve_consent(key: Option<&str>, env: &EnvView, stored: Option<&str>) -> Consent {
    let choice = stored != Some("off");
    let block = if key.is_none() {
        Some(Block::NoKey)
    } else if env.forbids() {
        Some(Block::Env)
    } else {
        None
    };
    Consent { choice, first_start: block.is_none() && stored.is_none(), block }
}

/// A random UUID v4 (no machine data in it).
pub fn new_uuid() -> String {
    uuid_text(random_16())
}

/// A UUID v7: 48 bits of Unix milliseconds, then random (D4). PostHog groups sessions only
/// by v7 ids. The wall clock is read for the time prefix only — not an offline path.
pub fn new_uuid_v7() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    uuid_v7_at(ms)
}

fn uuid_v7_at(ms: u64) -> String {
    let mut b = random_16();
    b[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
    b[6] = (b[6] & 0x0f) | 0x70;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

fn random_16() -> [u8; 16] {
    let mut b = [0u8; 16];
    if getrandom::fill(&mut b).is_err() {
        // no entropy source: a hash of time and pid is still not tied to the person
        let seed = format!("{:?}{}", Instant::now(), std::process::id());
        let h = xxhash_rust::xxh3::xxh3_128(seed.as_bytes());
        b = h.to_le_bytes();
    }
    b
}

fn uuid_text(mut b: [u8; 16]) -> String {
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_hexdigit() }
        })
}

/// A UUID whose version nibble is 7 (the stand-in checks `$session_id` with it, t888).
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_uuid_v7(s: &str) -> bool {
    is_uuid(s) && s.as_bytes()[14] == b'7'
}

/// The install id from `<dir>/telemetry-id`, created on first need; a malformed file is
/// replaced (t838).
pub fn install_id_in(dir: &std::path::Path) -> String {
    let path = dir.join(ID_FILE);
    if let Ok(s) = std::fs::read_to_string(&path) {
        let s = s.trim();
        if is_uuid(s) {
            return s.to_string();
        }
    }
    let id = new_uuid();
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(&path, format!("{id}\n"));
    id
}

// ---- instances (D4) -------------------------------------------------------------------------

/// This process among the others of the same install: the n-th start and how many other
/// Kinetren processes hold their lock right now. The lock file is held for the process's life
/// (`File::try_lock`, as `project_lock.rs`): the kernel releases it on any death.
pub struct Instance {
    pub seq: u64,
    pub running: u64,
    _lock: Option<std::fs::File>,
}

/// Register this process in `<dir>` (t881). A lock file nobody holds is a leftover of a dead
/// process and is removed. A race of two starts on the counter may give one number twice —
/// accepted, it is a statistic.
pub fn register_instance(dir: &std::path::Path, session_id: &str) -> Instance {
    let seq_path = dir.join(SEQ_FILE);
    let seq = std::fs::read_to_string(&seq_path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(&seq_path, format!("{seq}\n"));
    let inst = dir.join(INSTANCES_DIR);
    let _ = std::fs::create_dir_all(&inst);
    let mine = inst.join(format!("{session_id}.lock"));
    let mut running = 0;
    for e in std::fs::read_dir(&inst).into_iter().flatten().flatten() {
        let p = e.path();
        if p == mine || p.extension().is_none_or(|x| x != "lock") {
            continue;
        }
        let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(&p) else {
            continue;
        };
        match f.try_lock() {
            Ok(()) => {
                drop(f);
                let _ = std::fs::remove_file(&p);
            }
            Err(std::fs::TryLockError::WouldBlock) => running += 1,
            Err(std::fs::TryLockError::Error(_)) => {}
        }
    }
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&mine)
        .ok()
        .filter(|f| f.try_lock().is_ok());
    Instance { seq, running, _lock: lock }
}

// ---- the schema (D1) ------------------------------------------------------------------------

/// Keys every event carries (`$os_version` only when the probe could read it).
pub const COMMON_KEYS: &[&str] = &[
    "$session_id",
    "$app_version",
    "$app_build",
    "$os",
    "$os_version",
    "arch",
    "lang",
    "$geoip_disable",
];

/// The environment probe's keys (D9): they go into the person's `$set` on every start.
pub const ENV_KEYS: &[&str] = &[
    "$os_version",
    "cpu_cores",
    "ram_gb",
    "gpu_vendor",
    "gpu_name",
    "gpu_type",
    "gpu_backend",
    "software_render",
    "screen_scale",
    "screen_w",
    "screen_h",
    "ffmpeg_major",
    "ffmpeg_source",
];

/// The person's properties in `$set` (D2): the build, the system and the environment.
#[cfg_attr(not(test), allow(dead_code))]
pub const PERSON_KEYS: &[&str] = &["$app_version", "$os", "arch", "lang"];
/// `$set_once`: what the install was first seen with.
#[cfg_attr(not(test), allow(dead_code))]
pub const PERSON_ONCE_KEYS: &[&str] = &["first_app_version", "first_os"];

/// The activity counters of heartbeat and close (D5).
pub const COUNTER_KEYS: &[&str] = &[
    "active_s",
    "playback_s",
    "playback_starts",
    "media_imported",
    "clips_added",
    "edits",
    "undos",
    "redos",
    "projects_opened",
    "saves",
    "exports",
    "fx_added",
];

/// Each event and its own keys besides `COMMON_KEYS`. Nothing else can leave the machine:
/// `event_json` drops (and in debug asserts on) any other key (t837).
pub const SCHEMA: &[(&str, &[&str])] = &[
    ("app_started", &["$set", "$set_once", "instance_seq", "instances_running"]),
    ("app_heartbeat", &["uptime_s", "#counters"]),
    ("app_closed", &["duration_s", "#counters"]),
    (
        "project_opened",
        &[
            "how",
            "clips",
            "tracks",
            "media",
            "duration_bucket",
            "has_reels",
            "has_morphers",
            "has_automation",
        ],
    ),
    (
        "export_finished",
        &[
            "outcome",
            "container",
            "video_codec",
            "audio_codec",
            "width",
            "height",
            "fps",
            "duration_bucket",
            "backend",
            "speed_x",
        ],
    ),
    ("app_crashed", &["crashed_build", "thread", "location", "crashed_session_id"]),
];

/// The full key row of an event: the common keys, its own, `#counters` unfolded.
pub fn schema_row(event: &str) -> Option<Vec<&'static str>> {
    let (_, own) = SCHEMA.iter().find(|(e, _)| *e == event)?;
    let mut row: Vec<&'static str> = COMMON_KEYS.to_vec();
    for k in own.iter() {
        if *k == "#counters" {
            row.extend_from_slice(COUNTER_KEYS);
        } else {
            row.push(k);
        }
    }
    Some(row)
}

/// What this binary is: the values of `$app_version`, `$app_build`, `$os`, `arch`.
#[derive(Debug, Clone, Copy)]
pub struct Build {
    pub version: &'static str,
    pub build: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
}

impl Build {
    pub const THIS: Build = Build {
        version: kt_version::SHORT,
        build: kt_version::FULL,
        os: if cfg!(windows) { "Windows" } else { "Linux" },
        arch: std::env::consts::ARCH,
    };
}

/// The per-run part of every event: the session and what the probe found.
#[derive(Debug, Clone, Default)]
pub struct Common {
    pub session_id: String,
    pub os_version: Option<String>,
    pub lang: &'static str,
}

pub type Props = serde_json::Map<String, serde_json::Value>;

/// One event as the JSON body PostHog's capture endpoint takes: the common keys, then the
/// event's own `props` filtered by its schema row (an unlisted key is a bug — debug asserts,
/// release drops it).
pub fn event_json(
    api_key: &str,
    event: &str,
    install_id: &str,
    build: &Build,
    common: &Common,
    props: Props,
) -> String {
    let row = schema_row(event).unwrap_or_default();
    debug_assert!(!row.is_empty(), "telemetry: event {event} is not in SCHEMA");
    let mut out = Props::new();
    out.insert("$session_id".into(), common.session_id.clone().into());
    out.insert("$app_version".into(), build.version.into());
    out.insert("$app_build".into(), build.build.into());
    out.insert("$os".into(), build.os.into());
    if let Some(v) = &common.os_version {
        out.insert("$os_version".into(), v.clone().into());
    }
    out.insert("arch".into(), build.arch.into());
    out.insert("lang".into(), common.lang.into());
    out.insert("$geoip_disable".into(), true.into());
    for (k, v) in props {
        debug_assert!(row.contains(&k.as_str()), "telemetry: {k} is not in the row of {event}");
        if row.contains(&k.as_str()) && !COMMON_KEYS.contains(&k.as_str()) {
            out.insert(k, v);
        }
    }
    serde_json::json!({
        "api_key": api_key,
        "event": event,
        "distinct_id": install_id,
        "properties": out,
    })
    .to_string()
}

/// `app_started`'s own keys (D2): `$set` with the person's current properties (the build,
/// the system, the environment probe's `env`), `$set_once` with what it was first seen with.
pub fn start_props(build: &Build, common: &Common, env: &Props, inst: (u64, u64)) -> Props {
    let mut set = Props::new();
    set.insert("$app_version".into(), build.version.into());
    set.insert("$os".into(), build.os.into());
    set.insert("arch".into(), build.arch.into());
    set.insert("lang".into(), common.lang.into());
    for (k, v) in env {
        if ENV_KEYS.contains(&k.as_str()) {
            set.insert(k.clone(), v.clone());
        }
    }
    let mut once = Props::new();
    once.insert("first_app_version".into(), build.version.into());
    once.insert("first_os".into(), build.os.into());
    let mut p = Props::new();
    p.insert("$set".into(), set.into());
    p.insert("$set_once".into(), once.into());
    p.insert("instance_seq".into(), inst.0.into());
    p.insert("instances_running".into(), inst.1.into());
    p
}

/// Where a finished body goes. The HTTP sink in production, a recording one in tests.
pub trait Sink: Send {
    fn send(&mut self, body: &str) -> Result<(), String>;
}

pub struct HttpSink {
    agent: ureq::Agent,
    url: String,
}

impl HttpSink {
    pub fn new(url: impl Into<String>) -> Self {
        let url = url.into();
        let mut b = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(TOTAL_TIMEOUT));
        // the environment's proxy is never right for the machine itself (the live check's
        // local capture server; a container's forced proxy broke t841 under wine)
        if ["http://127.0.0.1", "http://localhost", "http://[::1]"]
            .iter()
            .any(|p| url.starts_with(p))
        {
            b = b.proxy(None);
        }
        HttpSink { agent: b.build().into(), url }
    }

    /// The EU endpoint; a debug build honours the local target (`local_target_from`) for the
    /// live check, a release build ignores it, so a shipped binary cannot be redirected.
    pub fn production() -> Self {
        Self::new(local_target().map(|t| t.0).unwrap_or_else(|| EU_CAPTURE_URL.to_string()))
    }
}

impl Sink for HttpSink {
    fn send(&mut self, body: &str) -> Result<(), String> {
        self.agent
            .post(&self.url)
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

// ---- milestones (D7) ------------------------------------------------------------------------

/// How a project came into the editor (`project_opened.how`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenHow {
    Open,
    Import,
    Recovery,
    New,
}

impl OpenHow {
    fn as_str(self) -> &'static str {
        match self {
            OpenHow::Open => "open",
            OpenHow::Import => "import",
            OpenHow::Recovery => "recovery",
            OpenHow::New => "new",
        }
    }
}

/// One bucket function for counts: dashboards group without exact sizes (D7).
pub fn count_bucket(n: u64) -> &'static str {
    match n {
        0 => "0",
        1..=9 => "1-9",
        10..=99 => "10-99",
        100..=999 => "100-999",
        _ => "1000+",
    }
}

/// And one for durations in seconds.
pub fn duration_bucket(secs: u64) -> &'static str {
    match secs {
        0..=59 => "<1m",
        60..=599 => "1-10m",
        600..=3600 => "10-60m",
        _ => ">60m",
    }
}

/// `project_opened`'s own keys: sizes in buckets and three flags — no name, no path.
pub fn project_props(how: OpenHow, p: &kt_model::Project) -> Props {
    let clips = p.clips.len() + p.sequences.iter().map(|s| s.clips.len()).sum::<usize>();
    let end = p.clips.iter().map(|c| (c.start + c.duration).0).max().unwrap_or(0).max(0);
    let curves = |c: &kt_model::ClipEvent| {
        c.fx_auto.iter().any(|v| !v.is_empty())
            || !c.gen_auto.is_empty()
            || !c.geometry_auto.is_empty()
    };
    let all_clips = p.clips.iter().chain(p.sequences.iter().flat_map(|s| s.clips.iter()));
    let mut out = Props::new();
    out.insert("how".into(), how.as_str().into());
    out.insert("clips".into(), count_bucket(clips as u64).into());
    out.insert("tracks".into(), count_bucket(p.tracks.len() as u64).into());
    out.insert("media".into(), count_bucket(p.media.len() as u64).into());
    out.insert("duration_bucket".into(), duration_bucket(end as u64 / 90_000).into());
    out.insert("has_reels".into(), (!p.sequences.is_empty()).into());
    let morphers = !p.morphers.is_empty() || p.sequences.iter().any(|s| !s.morphers.is_empty());
    out.insert("has_morphers".into(), morphers.into());
    let auto = {
        let mut clips = all_clips;
        clips.any(curves) || p.tracks.iter().any(|t| t.fx_auto.iter().any(|v| !v.is_empty()))
    };
    out.insert("has_automation".into(), auto.into());
    out
}

/// What an ended export was, in the terms of `export_finished`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportFacts {
    pub outcome: &'static str,
    pub video_codec: Option<&'static str>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// The length of what was to be exported.
    pub total_s: f64,
    /// Media seconds written, and the wall seconds it took.
    pub media_s: f64,
    pub wall_s: f64,
    /// `None` until the job's thread chose the backend.
    pub gpu: Option<bool>,
}

/// The codec family of an encoder name (`libx264`, `h264_nvenc` → `h264`).
fn codec_family(encoder: &str) -> &'static str {
    let e = encoder.to_ascii_lowercase();
    if e.contains("264") {
        "h264"
    } else if e.contains("265") || e.contains("hevc") {
        "hevc"
    } else if e.contains("av1") {
        "av1"
    } else if e.contains("vp9") {
        "vp9"
    } else {
        "other"
    }
}

impl ExportFacts {
    /// From the render dialog's job, at the moment the dialog lets it go.
    pub fn of(job: &crate::render_dialog::RenderJob) -> Self {
        let outcome = match &job.result {
            Some(Ok(_)) => "ok",
            Some(Err(_)) if job.job.is_cancelled() => "cancelled",
            Some(Err(_)) => "failed",
            None => "cancelled",
        };
        let encoder = match &job.result {
            Some(Ok(stats)) => Some(stats.encoder.as_str()),
            _ => job.opts.force_encoder.as_deref(),
        };
        let fps = job.fps.max(1) as f64;
        ExportFacts {
            outcome,
            video_codec: encoder.filter(|e| !e.is_empty()).map(codec_family),
            width: job.out_res.0,
            height: job.out_res.1,
            fps: job.fps,
            total_s: job.progress.total_video_frames as f64 / fps,
            media_s: job.progress.video_frames as f64 / fps,
            wall_s: job.finished_at.unwrap_or_else(job.clock).duration_since(job.started).as_secs_f64(),
            gpu: job.backend.as_ref().map(|b| matches!(b, crate::render_dialog::BackendLine::Gpu(_))),
        }
    }

    /// `export_finished`'s own keys.
    pub fn props(&self) -> Props {
        let mut p = Props::new();
        p.insert("outcome".into(), self.outcome.into());
        p.insert("container".into(), "mp4".into());
        if let Some(c) = self.video_codec {
            p.insert("video_codec".into(), c.into());
        }
        p.insert("audio_codec".into(), "aac".into());
        p.insert("width".into(), self.width.into());
        p.insert("height".into(), self.height.into());
        p.insert("fps".into(), self.fps.into());
        p.insert("duration_bucket".into(), duration_bucket(self.total_s as u64).into());
        if let Some(g) = self.gpu {
            p.insert("backend".into(), if g { "gpu" } else { "none" }.into());
        }
        if self.wall_s > 0.0 {
            let x = (self.media_s / self.wall_s * 10.0).round() / 10.0;
            p.insert("speed_x".into(), x.into());
        }
        p
    }
}

// ---- crash places (D8) ----------------------------------------------------------------------

/// `app_crashed`'s own keys from the panic hook's report: the version line, the thread, the
/// place — and NOT the message, which often names the person's paths.
pub fn crash_from_report(report: &str) -> Props {
    let mut p = Props::new();
    for line in report.lines() {
        if let Some(v) = line.strip_prefix("kinetren ") {
            p.entry("crashed_build").or_insert_with(|| v.trim().into());
        } else if let Some(v) = line.strip_prefix("thread: ") {
            p.entry("thread").or_insert_with(|| v.trim().into());
        } else if let Some(v) = line.strip_prefix("session: ") {
            let v = v.trim();
            if is_uuid(v) {
                p.entry("crashed_session_id").or_insert_with(|| v.into());
            }
        } else if let Some(rest) = line.split_once("panicked at ").map(|(_, r)| r)
            && let Some(loc) = panic_location(rest)
        {
            p.entry("location").or_insert_with(|| loc.into());
        }
    }
    p
}

/// `file:line:col` from the text after "panicked at ", the path cut to the part after `src/`
/// (a relative workspace path is kept, an absolute one loses everything up to `/src/`, and
/// with no `src/` only the file name stays). Everything after the place is dropped.
fn panic_location(rest: &str) -> Option<String> {
    let digits = |t: &str| t.chars().take_while(|c| c.is_ascii_digit()).count();
    for (i, _) in rest.match_indices(':') {
        let tail = &rest[i + 1..];
        let nl = digits(tail);
        let Some(after) = tail[nl..].strip_prefix(':').filter(|_| nl > 0) else { continue };
        let nc = digits(after);
        if nc == 0 {
            continue;
        }
        let file = rest[..i].trim().replace('\\', "/");
        let absolute = file.starts_with('/') || file.get(1..2) == Some(":");
        let file = if let Some(f) = file.strip_prefix("src/") {
            f.to_string()
        } else if !absolute {
            file
        } else if let Some((_, f)) = file.split_once("/src/") {
            f.to_string()
        } else {
            file.rsplit('/').next().unwrap_or_default().to_string()
        };
        return Some(format!("{file}:{}:{}", &tail[..nl], &after[..nc]));
    }
    None
}

// ---- the environment (D9) -------------------------------------------------------------------

/// The adapter, as `ui()` reads it for the software-render check.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuFacts {
    pub vendor: &'static str,
    pub name: String,
    pub kind: &'static str,
    pub backend: String,
    pub software: bool,
}

impl GpuFacts {
    pub fn new(
        vendor_id: u32,
        name: &str,
        kind: eframe::wgpu::DeviceType,
        backend: eframe::wgpu::Backend,
        software: bool,
    ) -> Self {
        use eframe::wgpu::DeviceType;
        GpuFacts {
            vendor: match vendor_id {
                0x10de => "nvidia",
                0x1002 | 0x1022 => "amd",
                0x8086 => "intel",
                0x106b => "apple",
                0x5143 => "qualcomm",
                0x13b5 => "arm",
                0x1414 => "microsoft",
                0x15ad => "vmware",
                0x10005 => "mesa",
                _ => "other",
            },
            name: name.chars().take(80).collect(),
            kind: match kind {
                DeviceType::DiscreteGpu => "discrete",
                DeviceType::IntegratedGpu => "integrated",
                DeviceType::VirtualGpu => "virtual",
                DeviceType::Cpu => "cpu",
                DeviceType::Other => "other",
            },
            backend: format!("{backend:?}").to_ascii_lowercase(),
            software,
        }
    }
}

/// Everything the probe reads, as data (t886 runs on samples).
#[derive(Debug, Clone, Default)]
pub struct EnvSources {
    /// The text of `/etc/os-release` (Linux).
    pub os_release: Option<String>,
    /// `(major, build)` from `RtlGetVersion` (Windows).
    pub windows: Option<(u32, u32)>,
    pub ram_bytes: Option<u64>,
    pub cores: Option<usize>,
    pub gpu: Option<GpuFacts>,
    pub scale: Option<f32>,
    /// The first monitor's logical size in points.
    pub monitor: Option<(f32, f32)>,
    /// `(major, source)`: `system`/`folder`/`downloaded`/`missing`.
    pub ffmpeg: Option<(Option<u32>, &'static str)>,
}

fn os_release_name(text: &str) -> Option<String> {
    let field = |k: &str| {
        text.lines().find_map(|l| {
            let v = l.strip_prefix(k)?.strip_prefix('=')?;
            let v = v.trim().trim_matches('"').trim();
            (!v.is_empty()).then(|| v.to_string())
        })
    };
    let name = field("NAME")?;
    Some(match field("VERSION_ID") {
        Some(v) => format!("{name} {v}"),
        None => name,
    })
}

/// Coarse facts only; a value that cannot be read is omitted, never guessed (t886).
pub fn probe_env(s: &EnvSources) -> Props {
    let mut p = Props::new();
    let os = if let Some((major, build)) = s.windows {
        // Windows 11 still reports major 10; its builds start at 22000
        let shown = if major == 10 && build >= 22_000 { 11 } else { major };
        Some(format!("Windows {shown} (build {build})"))
    } else {
        s.os_release.as_deref().and_then(os_release_name)
    };
    if let Some(v) = os {
        p.insert("$os_version".into(), v.into());
    }
    if let Some(c) = s.cores {
        p.insert("cpu_cores".into(), (c as u64).into());
    }
    if let Some(b) = s.ram_bytes {
        let gb = (b as f64 / (1u64 << 30) as f64).round() as u64;
        p.insert("ram_gb".into(), gb.into());
    }
    if let Some(g) = &s.gpu {
        p.insert("gpu_vendor".into(), g.vendor.into());
        p.insert("gpu_name".into(), g.name.clone().into());
        p.insert("gpu_type".into(), g.kind.into());
        p.insert("gpu_backend".into(), g.backend.clone().into());
        p.insert("software_render".into(), g.software.into());
    }
    if let Some(sc) = s.scale {
        p.insert("screen_scale".into(), ((sc as f64 * 100.0).round() / 100.0).into());
    }
    if let Some((w, h)) = s.monitor {
        let r = |v: f32| ((v / 10.0).round() * 10.0) as u64;
        p.insert("screen_w".into(), r(w).into());
        p.insert("screen_h".into(), r(h).into());
    }
    if let Some((major, source)) = s.ffmpeg {
        if let Some(m) = major {
            p.insert("ffmpeg_major".into(), m.into());
        }
        p.insert("ffmpeg_source".into(), source.into());
    }
    p
}

/// How the loaded FFmpeg was found; `missing` when the search found nothing.
fn ffmpeg_state() -> (Option<u32>, &'static str) {
    use kt_ffmpeg::search::Source;
    let Some(report) = kt_ffmpeg::search_report_data() else {
        return (None, "missing");
    };
    let Some((place, major, _)) = report.loaded.as_ref() else {
        return (None, "missing");
    };
    let line = report.lines.iter().find(|l| l.reject.is_none() && &l.place == place);
    let source = match line.map(|l| l.source) {
        Some(Source::UserChoice) => "folder",
        Some(Source::Download) => "downloaded",
        _ => "system",
    };
    (Some(*major), source)
}

#[cfg(windows)]
fn windows_version() -> Option<(u32, u32)> {
    use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    // SAFETY: a zeroed plain-data struct with its size set, filled by the call
    let mut v: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    v.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    // RtlGetVersion, not GetVersionEx: the latter lies without a manifest
    (unsafe { RtlGetVersion(&mut v) } == 0).then_some((v.dwMajorVersion, v.dwBuildNumber))
}

#[cfg(windows)]
fn ram_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: as above
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    (unsafe { GlobalMemoryStatusEx(&mut m) } != 0).then_some(m.ullTotalPhys)
}

#[cfg(not(windows))]
fn ram_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = text
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(kb * 1024)
}

/// The production sources: files and system calls, read on the sender thread.
fn gather_env(gpu: Option<GpuFacts>, screen: Option<Screen>) -> EnvSources {
    EnvSources {
        #[cfg(not(windows))]
        os_release: std::fs::read_to_string("/etc/os-release").ok(),
        #[cfg(windows)]
        os_release: None,
        #[cfg(windows)]
        windows: windows_version(),
        #[cfg(not(windows))]
        windows: None,
        ram_bytes: ram_bytes(),
        cores: std::thread::available_parallelism().ok().map(|n| n.get()),
        gpu,
        scale: screen.map(|s| s.0),
        monitor: screen.and_then(|s| s.1),
        ffmpeg: Some(ffmpeg_state()),
    }
}

// ---- the one door (D5) ----------------------------------------------------------------------

/// The heartbeat period (D5, D6): about twelve events per ACTIVE hour; one constant if the
/// volume ever has to shrink.
pub const HEARTBEAT: Duration = Duration::from_secs(5 * 60);
/// The grain of `active_s`: a 10-second slot with input or playback counts whole.
const SLOT_MS: u64 = 10_000;
/// Events waiting for the sender. A full queue drops the event instead of blocking.
const QUEUE: usize = 32;

/// What the application tells telemetry: FACTS, never counts or events (D5). Everything
/// else — classifying, counting, bucketing, building events — happens in this module.
pub enum Signal<'a> {
    /// A batch about to be applied (`apply_batch`), with the model it is applied to.
    Commands(&'a [kt_model::Command], &'a kt_model::TimelineModel),
    /// One command (`apply`).
    Command(&'a kt_model::Command, &'a kt_model::TimelineModel),
    /// One command of a live gesture (`apply_coalesced`): one edit per gesture.
    Gesture(&'a kt_model::Command, u64, &'a kt_model::TimelineModel),
    Undo,
    Redo,
    /// The person's save went to disk.
    Saved,
    /// A project came into the editor (opened, imported, recovered or new).
    ProjectOpened(OpenHow, &'a kt_model::Project),
    /// The render dialog lets an ended export go.
    ExportEnded(&'a crate::render_dialog::RenderJob),
    /// The wgpu adapter, read once in `ui()` with the software-render verdict.
    Gpu(&'a eframe::wgpu::AdapterInfo, bool),
    /// The previous session's crash report, handed over at start (`take_crash_report`).
    CrashReport(&'a std::path::Path),
    /// Every drawn frame. After the first one this is atomics only (invariant 12).
    Frame {
        ctx: &'a egui::Context,
        playing: bool,
    },
    /// `on_exit`, after the recovery write: `app_closed` within `EXIT_BUDGET`.
    Exit,
}

/// The door. A no-op when telemetry is off or not installed (`kt-cli`, the harness).
pub fn signal(s: Signal<'_>) {
    // the local telemetry journal hears the same door, whatever the consent (telemetry-journal)
    crate::tele_journal::on_signal(&s);
    #[cfg(test)]
    if let Some(t) = testing::bound() {
        return t.signal(s);
    }
    if let Some(t) = GLOBAL.get() {
        t.signal(s)
    }
}

/// Where time comes from: the process's `Instant` in production, a hand-moved one in tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
}

struct Since(Instant);

impl Clock for Since {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

type Tally = Mutex<std::collections::BTreeMap<&'static str, u64>>;

fn bump(t: &Tally, key: &'static str) {
    *t.lock().unwrap_or_else(|p| p.into_inner()).entry(key).or_default() += 1;
}

fn tally(t: &Tally) -> std::collections::BTreeMap<&'static str, u64> {
    t.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// The session's counters. Atomics for everything the frame touches; the three maps are
/// touched only by non-frame signals and by the sender's snapshot, never across I/O.
#[derive(Default)]
struct Counters {
    active_slots: AtomicU64,
    /// The last active slot + 1 (0: none yet).
    last_slot: AtomicU64,
    playing: AtomicBool,
    playing_since_ms: AtomicU64,
    playback_ms: AtomicU64,
    playback_starts: AtomicU64,
    clips_added: AtomicU64,
    edits: AtomicU64,
    undos: AtomicU64,
    redos: AtomicU64,
    projects_opened: AtomicU64,
    saves: AtomicU64,
    /// The gesture whose first command already counted as an edit (+1; 0: none).
    last_gesture: AtomicU64,
    media_imported: Tally,
    exports: Tally,
    fx_added: Tally,
}

/// The counters at one moment: what heartbeat and close send, and what "changed" compares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub active_s: u64,
    pub playback_s: u64,
    pub playback_starts: u64,
    pub media_imported: std::collections::BTreeMap<&'static str, u64>,
    pub clips_added: u64,
    pub edits: u64,
    pub undos: u64,
    pub redos: u64,
    pub projects_opened: u64,
    pub saves: u64,
    pub exports: std::collections::BTreeMap<&'static str, u64>,
    pub fx_added: std::collections::BTreeMap<&'static str, u64>,
}

impl Snapshot {
    /// The `#counters` keys of the schema, in its order.
    pub fn props(&self) -> Props {
        let map = |m: &std::collections::BTreeMap<&'static str, u64>| -> serde_json::Value {
            m.iter()
                .map(|(k, v)| ((*k).to_string(), serde_json::Value::from(*v)))
                .collect::<Props>()
                .into()
        };
        let mut p = Props::new();
        p.insert("active_s".into(), self.active_s.into());
        p.insert("playback_s".into(), self.playback_s.into());
        p.insert("playback_starts".into(), self.playback_starts.into());
        p.insert("media_imported".into(), map(&self.media_imported));
        p.insert("clips_added".into(), self.clips_added.into());
        p.insert("edits".into(), self.edits.into());
        p.insert("undos".into(), self.undos.into());
        p.insert("redos".into(), self.redos.into());
        p.insert("projects_opened".into(), self.projects_opened.into());
        p.insert("saves".into(), self.saves.into());
        p.insert("exports".into(), map(&self.exports));
        p.insert("fx_added".into(), map(&self.fx_added));
        p
    }
}

impl Counters {
    /// One frame: two atomics when nothing changes, no lock, no allocation.
    fn frame(&self, now_ms: u64, playing: bool, input: bool) {
        if input || playing {
            let slot = now_ms / SLOT_MS + 1;
            if self.last_slot.swap(slot, Ordering::Relaxed) != slot {
                self.active_slots.fetch_add(1, Ordering::Relaxed);
            }
        }
        if self.playing.swap(playing, Ordering::Relaxed) != playing {
            if playing {
                self.playback_starts.fetch_add(1, Ordering::Relaxed);
                self.playing_since_ms.store(now_ms, Ordering::Relaxed);
            } else {
                let since = self.playing_since_ms.load(Ordering::Relaxed);
                self.playback_ms.fetch_add(now_ms.saturating_sub(since), Ordering::Relaxed);
            }
        }
    }

    fn commands(
        &self,
        cmds: &[kt_model::Command],
        model: &kt_model::TimelineModel,
        gesture: Option<u64>,
    ) {
        use kt_model::Command;
        let one = |a: &AtomicU64| {
            a.fetch_add(1, Ordering::Relaxed);
        };
        match gesture {
            Some(g) => {
                if self.last_gesture.swap(g.wrapping_add(1), Ordering::Relaxed) != g.wrapping_add(1)
                {
                    one(&self.edits);
                }
            }
            None => {
                self.last_gesture.store(0, Ordering::Relaxed);
                one(&self.edits);
            }
        }
        for c in cmds {
            match c {
                Command::AddMedia { path, .. } => bump(&self.media_imported, media_kind(path)),
                Command::AddClip(_) => one(&self.clips_added),
                Command::SetFx { id, fx } => {
                    let old = model.clip(*id).map(|c| c.fx.as_slice()).unwrap_or(&[]);
                    for name in added_fx(old, fx) {
                        bump(&self.fx_added, name);
                    }
                }
                Command::SetTrackFx { id, fx, .. } => {
                    let tracks = &model.project().tracks;
                    let old = tracks.iter().find(|t| t.id == *id).map(|t| t.fx.as_slice());
                    for name in added_fx(old.unwrap_or(&[]), fx) {
                        bump(&self.fx_added, name);
                    }
                }
                _ => {}
            }
        }
    }

    fn snapshot(&self, now_ms: u64) -> Snapshot {
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let running = if self.playing.load(Ordering::Relaxed) {
            now_ms.saturating_sub(get(&self.playing_since_ms))
        } else {
            0
        };
        Snapshot {
            active_s: get(&self.active_slots) * SLOT_MS / 1000,
            playback_s: (get(&self.playback_ms) + running) / 1000,
            playback_starts: get(&self.playback_starts),
            media_imported: tally(&self.media_imported),
            clips_added: get(&self.clips_added),
            edits: get(&self.edits),
            undos: get(&self.undos),
            redos: get(&self.redos),
            projects_opened: get(&self.projects_opened),
            saves: get(&self.saves),
            exports: tally(&self.exports),
            fx_added: tally(&self.fx_added),
        }
    }
}

/// The kind of an imported medium, from its path; the path itself never leaves the module.
pub fn media_kind(path: &str) -> &'static str {
    if path.starts_with("gen:") {
        return "generator";
    }
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "mxf" | "mts" | "m2ts" | "ts" | "wmv"
        | "flv" | "mpg" | "mpeg" | "ogv" | "3gp" => "video",
        "wav" | "mp3" | "flac" | "ogg" | "opus" | "m4a" | "aac" | "aif" | "aiff" | "wma" => "audio",
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "tif" | "tiff" | "exr" | "tga" => "image",
        _ => "other",
    }
}

/// The registry name of an effect ("Blur"), never a user-given one; `None` for what is not a
/// user effect (the layer mix).
fn fx_name(fx: &kt_model::FxInstance) -> Option<&'static str> {
    let tag = fx.tag();
    crate::fx_registry::registry().iter().find(|e| e.template.tag() == tag).map(|e| e.name)
}

/// The effects `new` has more of than `old`, by kind (a chain is replaced whole).
fn added_fx(old: &[kt_model::FxInstance], new: &[kt_model::FxInstance]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (i, f) in new.iter().enumerate() {
        let tag = f.tag();
        let before = new[..i].iter().filter(|g| g.tag() == tag).count();
        if old.iter().filter(|g| g.tag() == tag).count() <= before
            && let Some(n) = fx_name(f)
        {
            out.push(n);
        }
    }
    out
}

/// Input that means a person is at the program (a repaint alone is not).
fn is_activity(e: &egui::Event) -> bool {
    matches!(
        e,
        egui::Event::Key { .. }
            | egui::Event::Text(_)
            | egui::Event::PointerButton { .. }
            | egui::Event::PointerMoved(_)
            | egui::Event::MouseWheel { .. }
            | egui::Event::Zoom(_)
            | egui::Event::Touch { .. }
            | egui::Event::Paste(_)
    )
}

enum Msg {
    /// The first frame: `app_started` is built on the sender thread (the probe reads files).
    Start,
    Event(&'static str, Props),
    Flush(SyncSender<()>),
    /// A counter moved while the sender slept with no timer (one-scheduler): arm the heartbeat.
    Wake,
}

/// Pixels per point and the first monitor's logical size, from the first frame.
type Screen = (f32, Option<(f32, f32)>);

/// What the UI thread and the sender thread share.
struct Shared {
    /// Block and first start; the person's choice lives in `choice`.
    consent: Consent,
    choice: AtomicBool,
    api_key: String,
    install_id: String,
    session_id: String,
    /// `(instance_seq, instances_running)` and the held lock (`for_process`).
    instance: Option<Instance>,
    clock: Box<dyn Clock>,
    period: Duration,
    started: AtomicBool,
    closed: AtomicBool,
    counters: Counters,
    /// The heartbeat found nothing new and the sender sleeps on its channel with no timer
    /// (one-scheduler): the next activity sends `Msg::Wake`.
    dormant: AtomicBool,
    /// The interface language at the last UI-side signal (the language is per thread).
    lang: Mutex<&'static str>,
    /// What the probe found, for every event after the start.
    os_version: Mutex<Option<String>>,
    /// The counters the last heartbeat carried: an unchanged snapshot sends nothing.
    last_beat: Mutex<Snapshot>,
    /// What the UI knows for the probe: the adapter, and the scale and monitor size.
    gpu: Mutex<Option<GpuFacts>>,
    screen: Mutex<Option<Screen>>,
    /// The probe's sources in tests (production gathers them on the sender thread).
    env_override: Option<EnvSources>,
}

impl Shared {
    fn on(&self) -> bool {
        self.consent.block.is_none() && self.choice.load(Ordering::Relaxed)
    }

    fn now_ms(&self) -> u64 {
        self.clock.now().as_millis() as u64
    }

    fn body(&self, event: &str, props: Props) -> String {
        let common = Common {
            session_id: self.session_id.clone(),
            os_version: self.os_version.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            lang: *self.lang.lock().unwrap_or_else(|p| p.into_inner()),
        };
        event_json(&self.api_key, event, &self.install_id, &Build::THIS, &common, props)
    }

    /// `app_started`, after the probe (it reads files: the sender thread does it).
    fn start_body(&self) -> String {
        let sources = match &self.env_override {
            Some(e) => e.clone(),
            None => gather_env(
                self.gpu.lock().unwrap_or_else(|p| p.into_inner()).clone(),
                *self.screen.lock().unwrap_or_else(|p| p.into_inner()),
            ),
        };
        let env = probe_env(&sources);
        let os_version = env.get("$os_version").and_then(|v| v.as_str()).map(str::to_string);
        *self.os_version.lock().unwrap_or_else(|p| p.into_inner()) = os_version;
        let common = Common {
            session_id: self.session_id.clone(),
            os_version: None,
            lang: *self.lang.lock().unwrap_or_else(|p| p.into_inner()),
        };
        let inst = self.instance.as_ref().map(|i| (i.seq, i.running)).unwrap_or((1, 0));
        let props = start_props(&Build::THIS, &common, &env, inst);
        self.body("app_started", props)
    }

    /// The heartbeat's properties, if a counter changed since the previous one (D5).
    fn beat(&self, now_ms: u64) -> Option<Props> {
        if !self.on()
            || !self.started.load(Ordering::Relaxed)
            || self.closed.load(Ordering::Relaxed)
        {
            return None;
        }
        let snap = self.counters.snapshot(now_ms);
        let mut last = self.last_beat.lock().unwrap_or_else(|p| p.into_inner());
        if *last == snap {
            return None;
        }
        let mut props = snap.props();
        *last = snap;
        props.insert("uptime_s".into(), (now_ms / 1000).into());
        Some(props)
    }
}

pub struct Telemetry {
    shared: Arc<Shared>,
    notice_due: AtomicBool,
    /// Milestones that came before the first frame: they follow `app_started`.
    pending: Mutex<Vec<(&'static str, Props)>>,
    /// One `app_crashed` per process.
    crashed: AtomicBool,
    /// The sink waits here until the first event: a telemetry that never sends never spawns
    /// its thread (t839).
    sink: Mutex<Option<Box<dyn Sink>>>,
    tx: Mutex<Option<SyncSender<Msg>>>,
    thread_spawned: AtomicBool,
}

impl Telemetry {
    /// `install_id` is read lazily by the caller only when consent may be on, so a build
    /// without a key does not even create the id file.
    pub fn new(consent: Consent, install_id: String, sink: Box<dyn Sink>) -> Self {
        Telemetry {
            notice_due: AtomicBool::new(consent.first_start),
            shared: Arc::new(Shared {
                choice: AtomicBool::new(consent.choice),
                consent,
                api_key: key().unwrap_or_default().to_string(),
                install_id,
                session_id: new_uuid_v7(),
                instance: None,
                clock: Box::new(Since(Instant::now())),
                period: HEARTBEAT,
                started: AtomicBool::new(false),
                closed: AtomicBool::new(false),
                counters: Counters::default(),
                dormant: AtomicBool::new(false),
                lang: Mutex::new("en"),
                os_version: Mutex::new(None),
                last_beat: Mutex::new(Snapshot::default()),
                gpu: Mutex::new(None),
                screen: Mutex::new(None),
                env_override: None,
            }),
            pending: Mutex::new(Vec::new()),
            crashed: AtomicBool::new(false),
            sink: Mutex::new(Some(sink)),
            tx: Mutex::new(None),
            thread_spawned: AtomicBool::new(false),
        }
    }

    /// The process-wide telemetry of a real start: env + stored choice + this build's key.
    pub fn for_process(stored: Option<&str>) -> Self {
        let key = key().map(str::to_string).or_else(|| local_target().map(|t| t.1));
        let consent = resolve_consent(key.as_deref(), &EnvView::from_env(), stored);
        let dir = crate::journal::data_dir();
        let id = if consent.block.is_none() { install_id_in(&dir) } else { String::new() };
        let mut t = Self::new(consent, id, Box::new(HttpSink::production()));
        t.setup().api_key = key.unwrap_or_default();
        // the crash report names the session it ended (D8)
        crate::journal::set_session(&t.shared.session_id);
        if consent.on() {
            let s = t.setup();
            s.instance = Some(register_instance(&dir, &s.session_id));
        }
        t
    }

    /// The shared part before anyone else holds it (construction only).
    fn setup(&mut self) -> &mut Shared {
        Arc::get_mut(&mut self.shared).expect("telemetry: set up before the sender starts")
    }

    #[cfg(test)]
    pub fn with_key(mut self, k: &str) -> Self {
        self.setup().api_key = k.to_string();
        self
    }

    #[cfg(test)]
    pub fn with_clock(mut self, c: impl Clock + 'static) -> Self {
        self.setup().clock = Box::new(c);
        self
    }

    #[cfg(test)]
    pub fn with_env(mut self, e: EnvSources) -> Self {
        self.setup().env_override = Some(e);
        self
    }

    #[cfg(test)]
    pub fn with_heartbeat(mut self, period: Duration) -> Self {
        self.setup().period = period;
        self
    }

    pub fn consent(&self) -> Consent {
        Consent { choice: self.shared.choice.load(Ordering::Relaxed), ..self.shared.consent }
    }

    #[cfg(test)]
    pub fn thread_spawned(&self) -> bool {
        self.thread_spawned.load(Ordering::Relaxed)
    }

    /// The person's checkbox. Off during a session cancels that session's remaining events.
    pub fn set_choice(&self, on: bool) {
        self.shared.choice.store(on, Ordering::Relaxed);
    }

    /// The value to store under `telemetry`, or `None` when a block makes the choice moot
    /// (nothing is stored for a build that cannot send).
    pub fn stored_value(&self) -> Option<&'static str> {
        let c = self.consent();
        c.block.is_none().then_some(if c.choice { "on" } else { "off" })
    }

    /// `true` exactly once when the first-start notice is due (the bell entry).
    pub fn take_notice(&self) -> bool {
        self.notice_due.swap(false, Ordering::Relaxed)
    }

    /// The counters as they stand (tests read them where the person cannot).
    #[cfg(test)]
    pub fn snapshot(&self) -> Snapshot {
        self.shared.counters.snapshot(self.shared.now_ms())
    }

    pub fn signal(&self, s: Signal<'_>) {
        if !self.shared.on() {
            return;
        }
        if self.shared.dormant.swap(false, Ordering::AcqRel) {
            self.enqueue(Msg::Wake);
        }
        let c = &self.shared.counters;
        let one = |a: &AtomicU64| {
            a.fetch_add(1, Ordering::Relaxed);
        };
        match s {
            Signal::Frame { ctx, playing } => {
                if !self.shared.started.load(Ordering::Relaxed) {
                    let monitor = ctx.input(|i| i.viewport().monitor_size).map(|m| (m.x, m.y));
                    let screen = Some((ctx.pixels_per_point(), monitor));
                    *self.shared.screen.lock().unwrap_or_else(|p| p.into_inner()) = screen;
                }
                let input = ctx.input(|i| i.events.iter().any(is_activity));
                self.frame(playing, input);
            }
            Signal::Commands(cmds, model) => c.commands(cmds, model, None),
            Signal::Command(cmd, model) => c.commands(std::slice::from_ref(cmd), model, None),
            Signal::Gesture(cmd, g, model) => c.commands(std::slice::from_ref(cmd), model, Some(g)),
            Signal::Undo => one(&c.undos),
            Signal::Redo => one(&c.redos),
            Signal::Saved => one(&c.saves),
            Signal::ProjectOpened(how, project) => {
                one(&c.projects_opened);
                self.milestone("project_opened", project_props(how, project));
            }
            Signal::ExportEnded(job) => {
                let facts = ExportFacts::of(job);
                bump(&c.exports, facts.outcome);
                self.milestone("export_finished", facts.props());
            }
            Signal::Gpu(info, software) => {
                let g = GpuFacts::new(
                    info.vendor,
                    &info.name,
                    info.device_type,
                    info.backend,
                    software,
                );
                *self.shared.gpu.lock().unwrap_or_else(|p| p.into_inner()) = Some(g);
            }
            Signal::CrashReport(path) => {
                if let Ok(text) = std::fs::read_to_string(path) {
                    self.crash(&text);
                }
            }
            Signal::Exit => self.exit(EXIT_BUDGET),
        }
    }

    /// The frame body without a context: the first one starts the session.
    fn frame(&self, playing: bool, input: bool) {
        let s = &self.shared;
        if !s.started.load(Ordering::Relaxed) {
            *s.lang.lock().unwrap_or_else(|p| p.into_inner()) = crate::i18n::current().code();
            s.started.store(true, Ordering::Relaxed);
            self.enqueue(Msg::Start);
            let pending =
                std::mem::take(&mut *self.pending.lock().unwrap_or_else(|p| p.into_inner()));
            for (name, props) in pending {
                self.enqueue(Msg::Event(name, props));
            }
        }
        s.counters.frame(s.now_ms(), playing, input);
        if (playing || input) && s.dormant.swap(false, Ordering::AcqRel) {
            self.enqueue(Msg::Wake);
        }
    }

    /// A rare event: sent at once after the start, queued before it.
    fn milestone(&self, name: &'static str, props: Props) {
        if self.shared.started.load(Ordering::Relaxed) {
            self.enqueue(Msg::Event(name, props));
        } else {
            self.pending.lock().unwrap_or_else(|p| p.into_inner()).push((name, props));
        }
    }

    /// `app_crashed` from the previous session's report — once per process.
    fn crash(&self, report: &str) {
        if !self.crashed.swap(true, Ordering::Relaxed) {
            self.milestone("app_crashed", crash_from_report(report));
        }
    }

    /// `app_closed` with the session length and the final counters, then wait at most
    /// `budget`. Only once, only if this session's start was sent.
    fn exit(&self, budget: Duration) {
        let s = &self.shared;
        if s.closed.swap(true, Ordering::Relaxed) || !s.started.load(Ordering::Relaxed) {
            return;
        }
        *s.lang.lock().unwrap_or_else(|p| p.into_inner()) = crate::i18n::current().code();
        let now = s.now_ms();
        let mut props = s.counters.snapshot(now).props();
        props.insert("duration_s".into(), (now / 1000).into());
        self.enqueue(Msg::Event("app_closed", props));
        let (ack_tx, ack_rx) = sync_channel(1);
        if self.enqueue(Msg::Flush(ack_tx)) {
            let _ = ack_rx.recv_timeout(budget);
        }
    }

    /// What the sender thread does on its timer, driven by hand with a fake clock (t879).
    #[cfg(test)]
    pub fn beat_now(&self) {
        if let Some(p) = self.shared.beat(self.shared.now_ms()) {
            self.enqueue(Msg::Event("app_heartbeat", p));
        }
    }

    /// Never blocks: a full channel drops the message.
    fn enqueue(&self, msg: Msg) -> bool {
        let mut tx = self.tx.lock().unwrap_or_else(|p| p.into_inner());
        if tx.is_none() {
            let Some(sink) = self.sink.lock().unwrap_or_else(|p| p.into_inner()).take() else {
                return false;
            };
            let (t, rx) = sync_channel::<Msg>(QUEUE);
            let shared = self.shared.clone();
            let spawned = std::thread::Builder::new()
                .name("kt-telemetry".into())
                .spawn(move || run(shared, sink, rx))
                .is_ok();
            if !spawned {
                return false;
            }
            self.thread_spawned.store(true, Ordering::Relaxed);
            *tx = Some(t);
        }
        tx.as_ref().map(|t| t.try_send(msg).is_ok()).unwrap_or(false)
    }
}

/// The sender thread: the queue, and the heartbeat timer on `recv_timeout` — the UI clock and
/// repaint are never involved, so an idle program stays idle.
fn run(shared: Arc<Shared>, mut sink: Box<dyn Sink>, rx: Receiver<Msg>) {
    let mut logged = false;
    let mut send = |b: String| {
        if let Err(e) = sink.send(&b)
            && !logged
        {
            logged = true;
            crate::journal::log("INFO", &format!("telemetry: not sent ({e})"));
        }
    };
    let mut next = shared.period;
    loop {
        let wait = next.saturating_sub(shared.clock.now());
        // a quiet session sleeps on its channel with no timer (one-scheduler): the heartbeat woke
        // every five minutes to find nothing; the next activity sends `Wake`
        let got = if shared.dormant.load(Ordering::Acquire) {
            rx.recv().map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected)
        } else {
            rx.recv_timeout(wait)
        };
        match got {
            Ok(Msg::Wake) => {
                let now = shared.clock.now();
                while next <= now {
                    next += shared.period;
                }
            }
            Ok(Msg::Start) => send(shared.start_body()),
            Ok(Msg::Event(name, props)) => send(shared.body(name, props)),
            Ok(Msg::Flush(ack)) => {
                let _ = ack.try_send(());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let now = shared.clock.now();
                match shared.beat(now.as_millis() as u64) {
                    Some(p) => send(shared.body("app_heartbeat", p)),
                    // nothing changed since the last beat: sleep until something does
                    None if shared.started.load(Ordering::Relaxed) => shared.dormant.store(true, Ordering::Release),
                    None => {}
                }
                while next <= now {
                    next += shared.period;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

static GLOBAL: OnceLock<Arc<Telemetry>> = OnceLock::new();

/// Install the process telemetry (once, at start in `run`).
pub fn install(t: Telemetry) -> Arc<Telemetry> {
    GLOBAL.get_or_init(|| Arc::new(t)).clone()
}

/// The process telemetry, if `run` installed one (the harness does not).
pub fn global() -> Option<Arc<Telemetry>> {
    GLOBAL.get().cloned()
}

/// The bell entry's one calm line: anonymous statistics help, and where they are switched off
/// (rule 19). The rest waits under the entry's collapsed "Details" (owner, 2026-10-28: the bell
/// only, nothing that frightens).
pub fn notice_text() -> String {
    use crate::i18n::{k, tr};
    tr(k::telemetry::SUMMARY).to_string()
}

/// Under "Details": what is sent in plain words, where it goes, what is NOT sent, and the
/// environment switch. The same catalog entries the README's lists are written from.
pub fn notice_details() -> String {
    use crate::i18n::{k, tr};
    [k::telemetry::SENT, k::telemetry::WHERE, k::telemetry::NOT_SENT, k::telemetry::HOW_OFF]
        .map(tr)
        .join("\n")
}

/// The checkbox hint (one status-bar line): why it is disabled, if it is, then the summary and
/// what is NOT sent — the two the person must not have to look for.
pub fn hint_text(block: Option<Block>) -> String {
    use crate::i18n::{k, tr};
    let text = format!("{} {}", tr(k::telemetry::SUMMARY), tr(k::telemetry::NOT_SENT));
    match block {
        None => text,
        Some(Block::NoKey) => format!("{} {text}", tr(k::telemetry::NO_KEY)),
        Some(Block::Env) => format!("{} {text}", tr(k::telemetry::ENV_OFF)),
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::{Clock, Sink, Telemetry};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    thread_local! {
        static BOUND: std::cell::RefCell<Option<Arc<Telemetry>>> = const { std::cell::RefCell::new(None) };
    }

    /// The telemetry `signal` reaches on this thread (the harness has no `install`).
    pub fn bound() -> Option<Arc<Telemetry>> {
        BOUND.with(|b| b.borrow().clone())
    }

    /// Bind `t` for this test thread until the guard drops.
    pub fn bind(t: Arc<Telemetry>) -> Bound {
        BOUND.with(|b| *b.borrow_mut() = Some(t));
        Bound
    }

    pub struct Bound;

    impl Drop for Bound {
        fn drop(&mut self) {
            BOUND.with(|b| *b.borrow_mut() = None);
        }
    }

    /// A local stand-in of PostHog's capture endpoint (D11): answers `200 {"status":1}` on
    /// `POST /i/v0/e/` and refuses what the ingestion would refuse (t841, t888).
    pub struct FakePostHog {
        pub url: String,
        accepted: Arc<Mutex<Vec<serde_json::Value>>>,
        rejected: Arc<Mutex<Vec<String>>>,
    }

    /// What the ingestion checks, on one request: method and path, a JSON body, a non-empty
    /// `api_key`, `event` and `distinct_id`, `properties` an object, `$session_id` a UUID v7,
    /// `$set`/`$set_once` objects when present.
    pub fn validate(request_line: &str, body: &str) -> Result<serde_json::Value, String> {
        if !request_line.starts_with("POST /i/v0/e/ ") {
            return Err(format!("not the capture endpoint: {request_line}"));
        }
        let v: serde_json::Value =
            serde_json::from_str(body).map_err(|e| format!("not JSON: {e}"))?;
        for k in ["api_key", "event", "distinct_id"] {
            if !v[k].as_str().is_some_and(|s| !s.is_empty()) {
                return Err(format!("{k} missing or empty"));
            }
        }
        let p = v["properties"].as_object().ok_or("properties is not an object")?;
        let sid = p.get("$session_id").and_then(|s| s.as_str()).unwrap_or("");
        if !super::is_uuid_v7(sid) {
            return Err(format!("$session_id is not a UUID v7: {sid:?}"));
        }
        for k in ["$set", "$set_once"] {
            if p.get(k).is_some_and(|x| !x.is_object()) {
                return Err(format!("{k} is not an object"));
            }
        }
        Ok(v)
    }

    impl FakePostHog {
        pub fn start() -> Self {
            use std::io::{Read, Write};
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/i/v0/e/", l.local_addr().unwrap());
            let (accepted, rejected) =
                (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
            let (a, r) = (accepted.clone(), rejected.clone());
            std::thread::spawn(move || {
                for mut s in l.incoming().flatten() {
                    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    // headers, then Content-Length bytes
                    let (head, body) = loop {
                        let n = s.read(&mut chunk).unwrap_or(0);
                        if n == 0 {
                            break (String::new(), String::new());
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        let text = String::from_utf8_lossy(&buf).to_string();
                        if let Some(i) = text.find("\r\n\r\n") {
                            let len = text[..i]
                                .lines()
                                .find_map(|l| {
                                    let l = l.to_ascii_lowercase();
                                    l.strip_prefix("content-length:")
                                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                                })
                                .unwrap_or(0);
                            if buf.len() >= i + 4 + len {
                                break (
                                    text[..i].to_string(),
                                    text[i + 4..i + 4 + len].to_string(),
                                );
                            }
                        }
                    };
                    let line = head.lines().next().unwrap_or("").to_string();
                    let answer: &[u8] = match validate(&line, &body) {
                        Ok(v) => {
                            a.lock().unwrap_or_else(|p| p.into_inner()).push(v);
                            b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"status\":1}"
                        }
                        Err(e) => {
                            r.lock().unwrap_or_else(|p| p.into_inner()).push(e);
                            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                        }
                    };
                    let _ = s.write_all(answer);
                }
            });
            FakePostHog { url, accepted, rejected }
        }

        pub fn accepted(&self) -> Vec<serde_json::Value> {
            self.accepted.lock().unwrap_or_else(|p| p.into_inner()).clone()
        }

        pub fn rejected(&self) -> Vec<String> {
            self.rejected.lock().unwrap_or_else(|p| p.into_inner()).clone()
        }

        /// Wait until `n` requests were answered (or a few seconds passed).
        pub fn wait(&self, n: usize) {
            let t0 = std::time::Instant::now();
            while self.accepted().len() + self.rejected().len() < n
                && t0.elapsed() < kt_time::TEST_WAIT
            {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    /// A clock moved by hand (t836, t879, t884).
    #[derive(Clone, Default)]
    pub struct FakeClock(pub Arc<AtomicU64>);

    impl FakeClock {
        pub fn set(&self, d: Duration) {
            self.0.store(d.as_millis() as u64, Ordering::Relaxed);
        }

        pub fn advance(&self, d: Duration) {
            self.0.fetch_add(d.as_millis() as u64, Ordering::Relaxed);
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            Duration::from_millis(self.0.load(Ordering::Relaxed))
        }
    }

    /// Test sink: keeps every body.
    #[derive(Clone, Default)]
    pub struct Recording(pub Arc<Mutex<Vec<String>>>);

    impl Sink for Recording {
        fn send(&mut self, body: &str) -> Result<(), String> {
            self.0.lock().unwrap_or_else(|p| p.into_inner()).push(body.to_string());
            Ok(())
        }
    }

    impl Recording {
        pub fn events(&self) -> Vec<serde_json::Value> {
            let v = self.0.lock().unwrap_or_else(|p| p.into_inner());
            v.iter().filter_map(|s| serde_json::from_str(s).ok()).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{FakeClock, Recording};
    use super::*;

    /// The 48-bit millisecond prefix of a v7 id.
    fn uuid_v7_ms(s: &str) -> u64 {
        u64::from_str_radix(&s.replace('-', "")[..12], 16).unwrap_or(0)
    }

    fn on() -> Consent {
        Consent { choice: true, first_start: false, block: None }
    }

    fn recorded() -> (Telemetry, Recording, FakeClock) {
        let rec = Recording::default();
        let clock = FakeClock::default();
        let t = Telemetry::new(on(), new_uuid(), Box::new(rec.clone()))
            .with_key("phc_test")
            .with_clock(clock.clone());
        (t, rec, clock)
    }

    fn wait_for(rec: &Recording, n: usize) {
        let t0 = Instant::now();
        while rec.events().len() < n && t0.elapsed() < kt_time::TEST_WAIT {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A frame through the door, as `draw_all` reports it (no input events).
    fn frame(t: &Telemetry, playing: bool) {
        let ctx = egui::Context::default();
        t.signal(Signal::Frame { ctx: &ctx, playing });
    }

    pub(crate) fn empty_model() -> kt_model::TimelineModel {
        kt_model::TimelineModel::new(kt_model::Project::new(kt_model::ProjectSettings {
            fps: 30,
            width: 1280,
            height: 720,
            sample_rate: 48000,
            pitch_engine: Default::default(),
            default_shape: Default::default(),
            conform: Default::default(),
            settings_explicit: false,
        }))
    }

    /// t836: one run gives exactly the pair, linked by the session id; a dropped session
    /// sends only the start.
    #[test]
    fn t836_start_and_close_pair() {
        let (t, rec, clock) = recorded();
        frame(&t, false);
        frame(&t, false);
        clock.set(Duration::from_millis(125_600));
        t.signal(Signal::Exit);
        t.signal(Signal::Exit);
        wait_for(&rec, 2);
        let ev = rec.events();
        assert_eq!(ev.len(), 2, "{ev:?}");
        assert_eq!(ev[0]["event"], "app_started");
        assert_eq!(ev[1]["event"], "app_closed");
        assert_eq!(ev[0]["properties"]["$session_id"], ev[1]["properties"]["$session_id"]);
        assert!(is_uuid_v7(ev[0]["properties"]["$session_id"].as_str().unwrap()));
        assert_eq!(ev[1]["properties"]["duration_s"], 125);

        let (t, rec, _) = recorded();
        frame(&t, false);
        // the start is on the sender thread's clock: only what comes AFTER it can be absent
        wait_for(&rec, 1);
        drop(t);
        std::thread::sleep(Duration::from_millis(50));
        let ev = rec.events();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0]["event"], "app_started");
    }

    /// t879: the heartbeat speaks only after activity, carries cumulative counters, and the
    /// close carries the same; the timer lives on the sender thread.
    #[test]
    fn t879_heartbeat_only_after_activity() {
        use kt_model::{Command, MediaId};
        let (t, rec, clock) = recorded();
        let model = empty_model();
        let min = |m: u64| Duration::from_secs(m * 60);
        frame(&t, false);
        clock.set(min(1));
        let import = Command::AddMedia { id: MediaId(1), path: "/media/someone/a.mp4".into() };
        t.signal(Signal::Command(&import, &model));
        clock.set(min(5));
        t.beat_now();
        clock.set(min(7));
        frame(&t, true);
        clock.set(min(7) + Duration::from_secs(40));
        frame(&t, false);
        clock.set(min(10));
        t.beat_now();
        clock.set(min(15));
        t.beat_now();
        clock.set(min(17));
        t.signal(Signal::Exit);
        wait_for(&rec, 4);
        let ev = rec.events();
        let names: Vec<&str> = ev.iter().map(|e| e["event"].as_str().unwrap()).collect();
        assert_eq!(names, ["app_started", "app_heartbeat", "app_heartbeat", "app_closed"]);
        let (b5, b10, closed) = (&ev[1]["properties"], &ev[2]["properties"], &ev[3]["properties"]);
        assert_eq!(b5["media_imported"]["video"], 1);
        assert_eq!(b5["playback_s"], 0);
        assert_eq!(b5["uptime_s"], 300);
        assert_eq!(b10["media_imported"]["video"], 1);
        assert_eq!(b10["playback_s"], 40);
        assert_eq!(b10["playback_starts"], 1);
        for k in COUNTER_KEYS {
            assert_eq!(b10[k], closed[k], "{k}: the close carries the final counters");
        }
        assert_eq!(closed["duration_s"], 17 * 60);
        assert!(!rec.0.lock().unwrap().iter().any(|b| b.contains("someone")));

        // the real timer: a short period on the real clock, no frame after the activity
        let rec = Recording::default();
        let t = Telemetry::new(on(), new_uuid(), Box::new(rec.clone()))
            .with_key("phc_test")
            .with_heartbeat(Duration::from_millis(40));
        frame(&t, false);
        t.signal(Signal::Undo);
        wait_for(&rec, 2);
        std::thread::sleep(Duration::from_millis(200));
        let ev = rec.events();
        let beats = ev.iter().filter(|e| e["event"] == "app_heartbeat").count();
        assert_eq!(beats, 1, "one beat for one change, then silence: {ev:?}");
        assert_eq!(ev[1]["properties"]["undos"], 1);
    }

    /// Test t2242 (one-scheduler): a quiet session's sender sleeps on its channel with no timer —
    /// after a beat with nothing new it is dormant — and the next activity re-arms the heartbeat,
    /// which then carries it.
    #[test]
    fn t2242_a_quiet_session_sleeps_until_activity() {
        let rec = Recording::default();
        let t = Telemetry::new(on(), new_uuid(), Box::new(rec.clone()))
            .with_key("phc_test")
            .with_heartbeat(Duration::from_millis(40));
        frame(&t, false);
        t.signal(Signal::Undo);
        wait_for(&rec, 2);
        let deadline = kt_time::test_deadline();
        while !t.shared.dormant.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline, "the sender never went to sleep");
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(rec.events().len(), 2, "a dormant sender sent something");
        t.signal(Signal::Redo);
        wait_for(&rec, 3);
        let ev = rec.events();
        assert_eq!(ev[2]["event"], "app_heartbeat");
        assert_eq!(ev[2]["properties"]["redos"], 1, "the activity that woke it is in the beat");
    }

    /// A project with the person's names everywhere a careless hook would look (t837).
    fn secret_project() -> kt_model::Project {
        let mut p = empty_model().project().clone();
        p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "/media/someone/secret.mp4"));
        let mut c = crate::layout_tests::sample_clip(kt_model::ClipId(1));
        c.fx = vec![kt_model::FxInstance::gain(0.0)];
        c.fx_auto = vec![vec![kt_model::ParamCurve {
            param: "db".into(),
            keys: vec![kt_model::AutoKey::at(kt_time::Tick(0), 1.0)],
            source: Default::default(),
        }]];
        p.clips.push(c);
        p.markers.push(kt_model::Marker::named(kt_time::Tick(0), "secret clip"));
        p
    }

    /// The report the panic hook writes for a panic whose message names a path.
    fn secret_report(session: &str) -> String {
        crate::journal::crash_report_with(
            "panicked at src/apps/kt-app/src/timeline.rs:812:9: cannot open /home/someone/cut.kt",
            None,
            Some(session),
        )
    }

    /// The environment sample of t886.
    fn sample_env() -> EnvSources {
        EnvSources {
            os_release: Some("NAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nID=ubuntu\n".into()),
            windows: None,
            ram_bytes: Some((16.4 * (1u64 << 30) as f64) as u64),
            cores: Some(12),
            gpu: Some(GpuFacts::new(
                0x10de,
                "NVIDIA GeForce RTX 3060",
                eframe::wgpu::DeviceType::DiscreteGpu,
                eframe::wgpu::Backend::Vulkan,
                false,
            )),
            scale: Some(1.25),
            monitor: Some((2560.0, 1440.0)),
            ffmpeg: Some((Some(7), "system")),
        }
    }

    /// Every event built by its real builder from data full of the person's names (t837).
    fn real_props(event: &str) -> Props {
        use kt_model::{Command, MediaId};
        let counters = || {
            let c = Counters::default();
            let model = empty_model();
            let import =
                Command::AddMedia { id: MediaId(1), path: "/home/someone/secret.mov".into() };
            c.commands(
                &[import, Command::AddClip(crate::layout_tests::sample_clip(kt_model::ClipId(1)))],
                &model,
                None,
            );
            c.frame(0, true, true);
            c.frame(5_000, false, false);
            bump(&c.exports, "ok");
            bump(&c.fx_added, "Blur");
            c.snapshot(10_000).props()
        };
        match event {
            "app_started" => {
                let common = Common { session_id: new_uuid_v7(), os_version: None, lang: "ru" };
                start_props(&Build::THIS, &common, &probe_env(&sample_env()), (2, 1))
            }
            "app_heartbeat" => {
                let mut p = counters();
                p.insert("uptime_s".into(), 300.into());
                p
            }
            "app_closed" => {
                let mut p = counters();
                p.insert("duration_s".into(), 42.into());
                p
            }
            "project_opened" => project_props(OpenHow::Open, &secret_project()),
            "export_finished" => export_sample().props(),
            "app_crashed" => crash_from_report(&secret_report(&new_uuid_v7())),
            e => panic!("no builder for {e}"),
        }
    }

    fn export_sample() -> ExportFacts {
        ExportFacts {
            outcome: "cancelled",
            video_codec: Some(codec_family("libx264")),
            width: 1920,
            height: 1080,
            fps: 30,
            total_s: 75.0,
            media_s: 75.0,
            wall_s: 50.0,
            gpu: Some(true),
        }
    }

    /// t837: every event carries exactly its row, nothing from the person's paths or machine.
    #[test]
    fn t837_payload_is_the_allow_list() {
        crate::plugins::install();
        let host = std::fs::read_to_string("/etc/hostname").unwrap_or_default();
        let common = Common {
            session_id: new_uuid_v7(),
            os_version: Some("Ubuntu 24.04".into()),
            lang: "ru",
        };
        for (event, _) in SCHEMA {
            let props = real_props(event);
            let s = event_json("phc_test", event, &new_uuid(), &Build::THIS, &common, props);
            let v: serde_json::Value = serde_json::from_str(&s).unwrap();
            let mut keys: Vec<&str> =
                v["properties"].as_object().unwrap().keys().map(|k| k.as_str()).collect();
            keys.sort();
            let mut want = schema_row(event).unwrap();
            want.sort();
            assert_eq!(keys, want, "{event}");
            let mut top: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
            top.sort();
            assert_eq!(top, ["api_key", "distinct_id", "event", "properties"]);
            assert!(v["properties"].get("$process_person_profile").is_none(), "{event}");
            assert_eq!(v["properties"]["$geoip_disable"], true);
            assert!(!s.contains("someone") && !s.contains("secret"), "{event}: {s}");
            let host = host.trim();
            if host.len() > 3 {
                assert!(!s.contains(host), "host name in the payload");
            }
        }
    }

    /// t882: a crash is reported at the next start by its place only, once.
    #[test]
    fn t882_crash_by_place_only() {
        let session = new_uuid_v7();
        let p = crash_from_report(&secret_report(&session));
        assert_eq!(p["location"], "apps/kt-app/src/timeline.rs:812:9");
        assert_eq!(p["crashed_session_id"], session.as_str());
        assert_eq!(p["crashed_build"], kt_version::FULL);
        assert!(p["thread"].as_str().is_some_and(|t| !t.is_empty()));
        // the forms the hook really writes: the place on its own line, workspace-relative,
        // a registry path, a Windows path
        let loc = |text: &str| panic_location(text);
        assert_eq!(loc("apps/kt-app/src/x.rs:3:4:").as_deref(), Some("apps/kt-app/src/x.rs:3:4"));
        assert_eq!(
            loc("/home/someone/.cargo/registry/src/index/egui-0.36.1/src/context.rs:10:5:")
                .as_deref(),
            Some("index/egui-0.36.1/src/context.rs:10:5")
        );
        assert_eq!(
            loc(r"C:\Users\someone\kt\src\apps\kt-app\src\app.rs:7:1:").as_deref(),
            Some("apps/kt-app/src/app.rs:7:1")
        );
        assert_eq!(loc("/opt/someone/lib.rs:1:2").as_deref(), Some("lib.rs:1:2"));

        // through the door, before the first frame: it follows app_started, and only once
        let (t, rec, _) = recorded();
        let dir = std::env::temp_dir().join(format!("kt-t882-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let report = dir.join("kt-app-crash.seen.txt");
        std::fs::write(&report, secret_report(&session)).unwrap();
        t.signal(Signal::CrashReport(&report));
        t.signal(Signal::CrashReport(&report));
        frame(&t, false);
        t.signal(Signal::Exit);
        wait_for(&rec, 3);
        let ev = rec.events();
        let names: Vec<&str> = ev.iter().map(|e| e["event"].as_str().unwrap()).collect();
        assert_eq!(names, ["app_started", "app_crashed", "app_closed"]);
        let body = rec.0.lock().unwrap().join("\n");
        assert!(!body.contains("someone") && !body.contains("cannot open"), "{body}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// t883: the buckets and the export's properties.
    #[test]
    fn t883_buckets_and_export() {
        crate::plugins::install();
        let counts: Vec<&str> = [0, 1, 9, 10, 999, 1000].into_iter().map(count_bucket).collect();
        assert_eq!(counts, ["0", "1-9", "1-9", "10-99", "100-999", "1000+"]);
        let durs: Vec<&str> = [59, 60, 3600, 3601].into_iter().map(duration_bucket).collect();
        assert_eq!(durs, ["<1m", "1-10m", "10-60m", ">60m"]);
        let p = export_sample().props();
        let mut keys: Vec<&str> = p.keys().map(|k| k.as_str()).collect();
        keys.sort();
        let mut want: Vec<&str> =
            SCHEMA.iter().find(|(e, _)| *e == "export_finished").unwrap().1.to_vec();
        want.sort();
        assert_eq!(keys, want);
        assert_eq!(p["outcome"], "cancelled");
        assert_eq!(p["backend"], "gpu");
        assert_eq!(p["speed_x"], 1.5);
        assert_eq!(p["video_codec"], "h264");
        assert_eq!(p["duration_bucket"], "1-10m");
        // the project's sizes and flags, without a name
        let pp = project_props(OpenHow::Recovery, &secret_project());
        assert_eq!(pp["how"], "recovery");
        assert_eq!(pp["clips"], "1-9");
        assert_eq!(pp["media"], "1-9");
        assert_eq!(pp["has_automation"], true);
        assert_eq!(pp["has_reels"], false);
    }

    /// t886: the probe reads coarse facts and nothing personal; a missing file is omitted.
    #[test]
    fn t886_environment_probe() {
        let p = probe_env(&sample_env());
        assert_eq!(p["$os_version"], "Ubuntu 24.04");
        assert_eq!(p["ram_gb"], 16);
        assert_eq!(p["cpu_cores"], 12);
        assert_eq!(p["gpu_type"], "discrete");
        assert_eq!(p["gpu_backend"], "vulkan");
        assert_eq!(p["gpu_vendor"], "nvidia");
        assert_eq!(p["screen_w"], 2560);
        assert_eq!(p["screen_h"], 1440);
        assert_eq!(p["screen_scale"], 1.25);
        assert_eq!(p["ffmpeg_major"], 7);
        assert_eq!(p["ffmpeg_source"], "system");
        assert!(p.keys().all(|k| ENV_KEYS.contains(&k.as_str())), "{p:?}");
        let none = probe_env(&EnvSources { os_release: None, ..sample_env() });
        assert!(none.get("$os_version").is_none(), "missing, not empty");
        let win =
            probe_env(&EnvSources { os_release: None, windows: Some((10, 22631)), ..sample_env() });
        assert_eq!(win["$os_version"], "Windows 11 (build 22631)");
        // the start carries the probe in $set, and every later event its $os_version
        let rec = Recording::default();
        let t = Telemetry::new(on(), new_uuid(), Box::new(rec.clone()))
            .with_key("phc_test")
            .with_env(sample_env());
        frame(&t, false);
        t.signal(Signal::Exit);
        wait_for(&rec, 2);
        let ev = rec.events();
        assert_eq!(ev[0]["properties"]["$set"]["gpu_name"], "NVIDIA GeForce RTX 3060");
        assert_eq!(ev[1]["properties"]["$os_version"], "Ubuntu 24.04");
        // the production gatherers run on this machine without panicking
        let real = probe_env(&gather_env(None, None));
        assert!(real.keys().all(|k| ENV_KEYS.contains(&k.as_str())), "{real:?}");
    }

    /// t880: the start identifies the install — `$set` with the current properties,
    /// `$set_once` with the first ones, no person-profile opt-out.
    #[test]
    fn t880_start_identifies_the_install() {
        // built from pieces: a dotted literal reads as a UI place to the live-coverage guard
        let (version, build) = (concat!("kt26", ".11"), concat!("kt26", ".11", ".02-abcdef12"));
        let now = Build { version, build, os: "Linux", arch: "x86_64" };
        let common = Common { session_id: new_uuid_v7(), os_version: None, lang: "en" };
        let mut env = Props::new();
        for k in ENV_KEYS {
            env.insert((*k).into(), 1.into());
        }
        let props = start_props(&now, &common, &env, (3, 1));
        let s = event_json("phc_test", "app_started", &new_uuid(), &now, &common, props);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let p = &v["properties"];
        assert!(p.get("$process_person_profile").is_none());
        let mut set: Vec<&str> =
            p["$set"].as_object().unwrap().keys().map(|k| k.as_str()).collect();
        set.sort();
        let mut want: Vec<&str> = PERSON_KEYS.iter().chain(ENV_KEYS).copied().collect();
        want.sort();
        assert_eq!(set, want);
        assert_eq!(p["$set"]["$app_version"], version);
        assert_eq!(p["$app_version"], version);
        assert_eq!(p["$app_build"], build);
        assert_eq!(p["$os"], "Linux");
        let mut once: Vec<&str> =
            p["$set_once"].as_object().unwrap().keys().map(|k| k.as_str()).collect();
        once.sort();
        assert_eq!(once, PERSON_ONCE_KEYS);
        assert_eq!(
            (p["instance_seq"].as_u64(), p["instances_running"].as_u64()),
            (Some(3), Some(1))
        );
    }

    /// t881: two processes of one install are told apart; a lock nobody holds is stale.
    #[test]
    fn t881_instances_are_told_apart() {
        let dir = std::env::temp_dir().join(format!("kt-t881-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ids: Vec<String> = (0..3).map(|_| new_uuid_v7()).collect();
        let a = register_instance(&dir, &ids[0]);
        assert_eq!((a.seq, a.running), (1, 0));
        let b = register_instance(&dir, &ids[1]);
        assert_eq!((b.seq, b.running), (2, 1), "A holds its lock");
        drop(a);
        // a leftover of a killed process: a lock file nobody holds
        let stale = dir.join(INSTANCES_DIR).join(format!("{}.lock", new_uuid_v7()));
        std::fs::write(&stale, "").unwrap();
        let c = register_instance(&dir, &ids[2]);
        assert_eq!((c.seq, c.running), (3, 1), "B only");
        assert!(!stale.exists(), "the stale lock is removed");
        let mut prev = 0;
        for id in &ids {
            assert!(is_uuid_v7(id), "{id}");
            assert!(uuid_v7_ms(id) >= prev);
            prev = uuid_v7_ms(id);
        }
        drop((b, c));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The doors of `telemetry::signal`, per file. A new door is a deliberate edit of this
    /// list (t887): the owner asked that telemetry not smear across the code.
    const DOORS: &[(&str, usize)] = &[("app.rs", 14), ("render_dialog.rs", 1)];
    /// At most this many doors in all (spec).
    const MAX_DOORS: usize = 16;

    /// t887: outside this module telemetry is reached only through the door and the switch's
    /// plumbing; the doors are counted; nobody else builds an event or names a PostHog key.
    #[test]
    fn t887_one_door_counted() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let allowed = [
            "telemetry::signal(",
            "telemetry::Signal::",
            "telemetry::install(",
            "telemetry::global()",
            "telemetry::notice_",
            "telemetry::hint_text(",
            "telemetry::Telemetry>",
            "telemetry::Telemetry::for_process(",
            "telemetry::STORAGE_KEY",
            "telemetry::OpenHow",
        ];
        let posthog = [
            "\"$session_id\"",
            "\"distinct_id\"",
            "\"api_key\"",
            "\"$set\"",
            "\"$set_once\"",
            "\"$os\"",
            "\"$app_version\"",
            "\"app_started\"",
            "\"app_heartbeat\"",
            "\"app_closed\"",
            "\"project_opened\"",
            "\"export_finished\"",
            "\"app_crashed\"",
        ];
        let mut doors: Vec<(String, usize)> = Vec::new();
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if !name.ends_with(".rs") || name == "telemetry.rs" || name.ends_with("_tests.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                // the code only: up to the file's test module, without comments
                let lines: Vec<&str> = text.lines().collect();
                let end = (0..lines.len())
                    .find(|&i| {
                        lines[i] == "#[cfg(test)]"
                            && lines.get(i + 1).is_some_and(|n| {
                                ["mod ", "pub mod ", "pub(crate) mod "]
                                    .iter()
                                    .any(|m| n.starts_with(m))
                            })
                    })
                    .unwrap_or(lines.len());
                let code: Vec<&str> = lines[..end]
                    .iter()
                    .copied()
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .collect();
                let mut n = 0;
                for (i, l) in code.iter().enumerate() {
                    for (at, _) in l.match_indices("telemetry::") {
                        if l[..at].ends_with("k::") {
                            continue; // catalog keys
                        }
                        let rest = &l[at..];
                        assert!(
                            allowed.iter().any(|a| rest.starts_with(a)),
                            "{name}:{}: telemetry reached past its door: {}",
                            i + 1,
                            l.trim()
                        );
                    }
                    n += l.matches("telemetry::signal(").count();
                    for k in posthog {
                        assert!(!l.contains(k), "{name}:{}: {k} outside telemetry.rs", i + 1);
                    }
                }
                if n > 0 {
                    doors.push((name, n));
                }
            }
        }
        doors.sort();
        let want: Vec<(String, usize)> = DOORS.iter().map(|(f, n)| (f.to_string(), *n)).collect();
        assert_eq!(doors, want, "the doors moved: update DOORS deliberately");
        let total: usize = doors.iter().map(|(_, n)| n).sum();
        assert!(total <= MAX_DOORS, "{total} doors, the spec allows {MAX_DOORS}");
    }

    /// t838: the install id survives restarts and heals.
    #[test]
    fn t838_install_id_persists_and_heals() {
        let dir = std::env::temp_dir().join(format!("kt-telemetry-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let a = install_id_in(&dir);
        let b = install_id_in(&dir);
        assert!(is_uuid(&a));
        assert_eq!(a, b);
        std::fs::write(dir.join(ID_FILE), "garbage").unwrap();
        let c = install_id_in(&dir);
        assert!(is_uuid(&c));
        assert_ne!(c, a);
        assert_eq!(std::fs::read_to_string(dir.join(ID_FILE)).unwrap().trim(), c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// t839: consent table; a telemetry resolved off never spawns its thread.
    #[test]
    fn t839_consent_resolution() {
        let k = Some("phc_x");
        let none = EnvView::default();
        let env = |t: Option<&str>, d: Option<&str>| EnvView {
            telemetry: t.map(Into::into),
            do_not_track: d.map(Into::into),
        };
        let c = resolve_consent(None, &none, None);
        assert!(!c.on() && c.block == Some(Block::NoKey) && !c.first_start);
        let c = resolve_consent(k, &none, None);
        assert!(c.on() && c.first_start);
        let c = resolve_consent(k, &env(Some("0"), None), None);
        assert!(!c.on() && c.block == Some(Block::Env));
        let c = resolve_consent(k, &env(None, Some("1")), None);
        assert!(!c.on() && c.block == Some(Block::Env));
        let c = resolve_consent(k, &none, Some("off"));
        assert!(!c.on() && c.block.is_none() && !c.first_start);
        let c = resolve_consent(k, &env(Some("1"), None), Some("on"));
        assert!(c.on() && !c.first_start);
        // the environment cannot re-enable what the person switched off
        assert!(!resolve_consent(k, &env(Some("1"), None), Some("off")).on());

        for c in [resolve_consent(None, &none, None), resolve_consent(k, &none, Some("off"))] {
            let t = Telemetry::new(c, String::new(), Box::new(Recording::default()));
            frame(&t, false);
            t.signal(Signal::Exit);
            assert!(!t.thread_spawned(), "{c:?}");
        }
    }

    /// t841: a black-hole server costs at most the budget; a live one gets two POSTs.
    #[test]
    fn t841_never_blocks_and_posts_json() {
        // black hole: accepts, never answers
        let hole = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/i/v0/e/", hole.local_addr().unwrap());
        std::thread::spawn(move || {
            let mut keep = Vec::new();
            for s in hole.incoming().flatten() {
                keep.push(s);
            }
        });
        let t = Telemetry::new(on(), new_uuid(), Box::new(HttpSink::new(url))).with_key("phc_test");
        // the black hole never answers: an enqueue that waited for it takes a whole request, and
        // an exit past its budget waits out both queued requests. The bounds are those waits
        let t0 = Instant::now();
        frame(&t, false);
        assert!(t0.elapsed() < TOTAL_TIMEOUT, "enqueue blocked: {:?}", t0.elapsed());
        let t1 = Instant::now();
        t.signal(Signal::Exit);
        assert!(t1.elapsed() < 2 * TOTAL_TIMEOUT, "exit held {:?}", t1.elapsed());

        // the local stand-in answers 200 and checks what the ingestion checks
        let ph = super::testing::FakePostHog::start();
        let t = Telemetry::new(on(), new_uuid(), Box::new(HttpSink::new(ph.url.clone())))
            .with_key("phc_test");
        frame(&t, false);
        t.exit(Duration::from_secs(10));
        ph.wait(2);
        assert!(ph.rejected().is_empty(), "{:?}", ph.rejected());
        let got = ph.accepted();
        assert_eq!(got.len(), 2, "{got:?}");
        for (v, ev) in got.iter().zip(["app_started", "app_closed"]) {
            assert_eq!(v["api_key"], "phc_test");
            assert_eq!(v["event"], ev);
            assert!(v["distinct_id"].as_str().is_some_and(is_uuid));
        }
        // and it refuses what PostHog would refuse
        let bad = super::testing::validate(
            "POST /i/v0/e/ HTTP/1.1",
            r#"{"api_key":"k","event":"e","distinct_id":"d","properties":{"$session_id":"x"}}"#,
        );
        assert!(bad.is_err());
    }

    /// t842: the key stays out of source and dev builds.
    #[test]
    fn t842_key_only_at_release_build() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut stack = vec![root.clone()];
        let marker = ["phc", "_"].concat();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                let name = e.file_name();
                if p.is_dir() {
                    if name != "target" && name != ".git" {
                        stack.push(p);
                    }
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let s = std::fs::read_to_string(&p).unwrap_or_default();
                    // a literal key: the marker followed by a key character
                    let bad = s.match_indices(&marker).any(|(i, _)| {
                        s[i + marker.len()..]
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_alphanumeric())
                            && !s[i + marker.len()..].starts_with("test")
                            && !s[i + marker.len()..].starts_with("x\"")
                    });
                    assert!(!bad, "a PostHog key literal in {}", p.display());
                }
            }
        }
        let me = std::fs::read_to_string(root.join("apps/kt-app/src/telemetry.rs")).unwrap();
        assert!(me.contains(&format!("option_env!(\"{}\")", "KINETREN_POSTHOG_KEY")));
        let rb = std::fs::read_to_string(root.join("scripts/release-build.sh")).unwrap();
        assert!(rb.contains("-e KINETREN_POSTHOG_KEY"), "release-build.sh does not pass the key");
        assert!(
            rb.contains("packaging/posthog-key"),
            "release-build.sh does not read the key file"
        );
        if cfg!(debug_assertions) {
            assert!(key().is_none(), "a debug build carries the key");
        }
        // the local target: debug only, and only with BOTH variables
        let s = |v: &str| Some(v.to_string());
        assert_eq!(
            local_target_from(false, s("http://127.0.0.1:1/"), s("k")),
            None,
            "release ignores both"
        );
        assert_eq!(local_target_from(true, s("http://127.0.0.1:1/"), None), None);
        assert_eq!(local_target_from(true, None, s("k")), None);
        assert_eq!(local_target_from(true, s(" "), s("k")), None);
        assert_eq!(
            local_target_from(true, s("http://127.0.0.1:1/"), s("k")),
            Some(("http://127.0.0.1:1/".to_string(), "k".to_string()))
        );
    }
}
