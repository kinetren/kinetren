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

use std::sync::atomic::{AtomicBool, Ordering};
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
/// eframe storage key of the person's choice (`on`/`off`), next to `lang`.
pub const STORAGE_KEY: &str = "telemetry";

/// The project key of this build, if the release build provided one.
pub fn key() -> Option<&'static str> {
    option_env!("KINETREN_POSTHOG_KEY").map(str::trim).filter(|k| !k.is_empty())
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
    let mut b = [0u8; 16];
    if getrandom::fill(&mut b).is_err() {
        // no entropy source: a hash of time and pid is still not tied to the person
        let seed = format!("{:?}{}", Instant::now(), std::process::id());
        let h = xxhash_rust::xxh3::xxh3_128(seed.as_bytes());
        b = h.to_le_bytes();
    }
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

/// One event as the JSON body PostHog's capture endpoint takes. The property set is the
/// allow-list of the spec, and nothing else may be added here without a spec change (t837).
pub fn event_json(
    api_key: &str,
    event: &str,
    install_id: &str,
    session_id: &str,
    lang: &str,
    duration_s: Option<u64>,
) -> String {
    let mut props = serde_json::Map::new();
    props.insert("$session_id".into(), session_id.into());
    props.insert("app_version".into(), kt_version::display().into());
    props.insert("os".into(), std::env::consts::OS.into());
    props.insert("arch".into(), std::env::consts::ARCH.into());
    props.insert("lang".into(), lang.into());
    props.insert("$process_person_profile".into(), false.into());
    props.insert("$geoip_disable".into(), true.into());
    if let Some(d) = duration_s {
        props.insert("duration_s".into(), d.into());
    }
    serde_json::json!({
        "api_key": api_key,
        "event": event,
        "distinct_id": install_id,
        "properties": props,
    })
    .to_string()
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
        if ["http://127.0.0.1", "http://localhost", "http://[::1]"].iter().any(|p| url.starts_with(p)) {
            b = b.proxy(None);
        }
        HttpSink { agent: b.build().into(), url }
    }

    /// The EU endpoint; a debug build honours `KT_TELEMETRY_URL` for the live check, a
    /// release build ignores it, so a shipped binary cannot be redirected.
    pub fn production() -> Self {
        let url = if cfg!(debug_assertions) {
            std::env::var("KT_TELEMETRY_URL").unwrap_or_else(|_| EU_CAPTURE_URL.to_string())
        } else {
            EU_CAPTURE_URL.to_string()
        };
        Self::new(url)
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

enum Msg {
    Body(String),
    Flush(SyncSender<()>),
}

pub struct Telemetry {
    consent: Mutex<Consent>,
    api_key: String,
    install_id: String,
    session_id: String,
    t0: Instant,
    started: AtomicBool,
    closed: AtomicBool,
    notice_due: AtomicBool,
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
            consent: Mutex::new(consent),
            api_key: key().unwrap_or_default().to_string(),
            install_id,
            session_id: new_uuid(),
            t0: Instant::now(),
            started: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            sink: Mutex::new(Some(sink)),
            tx: Mutex::new(None),
            thread_spawned: AtomicBool::new(false),
        }
    }

    /// The process-wide telemetry of a real start: env + stored choice + this build's key.
    pub fn for_process(stored: Option<&str>) -> Self {
        let consent = resolve_consent(key(), &EnvView::from_env(), stored);
        let id = if consent.block.is_none() {
            install_id_in(&crate::journal::data_dir())
        } else {
            String::new()
        };
        Self::new(consent, id, Box::new(HttpSink::production()))
    }

    #[cfg(test)]
    pub fn with_key(mut self, k: &str) -> Self {
        self.api_key = k.to_string();
        self
    }

    pub fn consent(&self) -> Consent {
        *self.consent.lock().unwrap_or_else(|p| p.into_inner())
    }

    #[cfg(test)]
    pub fn thread_spawned(&self) -> bool {
        self.thread_spawned.load(Ordering::Relaxed)
    }

    /// The person's checkbox. Off during a session cancels that session's `app_closed`.
    pub fn set_choice(&self, on: bool) {
        self.consent.lock().unwrap_or_else(|p| p.into_inner()).choice = on;
    }

    /// The value to store under `telemetry`, or `None` when a block makes the choice moot
    /// (nothing is stored for a build that cannot send).
    pub fn stored_value(&self) -> Option<&'static str> {
        let c = self.consent();
        c.block.is_none().then_some(if c.choice { "on" } else { "off" })
    }

    /// Called every frame; cheap after the first. Sends `app_started` once and returns
    /// `true` exactly once when the first-start notice is due.
    pub fn frame(&self, lang: &str) -> bool {
        if !self.started.swap(true, Ordering::Relaxed) && self.consent().on() {
            let body = event_json(
                &self.api_key,
                "app_started",
                &self.install_id,
                &self.session_id,
                lang,
                None,
            );
            self.enqueue(Msg::Body(body));
        }
        self.notice_due.swap(false, Ordering::Relaxed)
    }

    /// `app_closed` with the session length, then wait at most `budget`. Only once, only if
    /// this session's start was sent and the person did not switch telemetry off.
    pub fn closed(&self, lang: &str, budget: Duration) {
        self.closed_after(self.t0.elapsed(), lang, budget);
    }

    fn closed_after(&self, elapsed: Duration, lang: &str, budget: Duration) {
        if self.closed.swap(true, Ordering::Relaxed)
            || !self.started.load(Ordering::Relaxed)
            || !self.consent().on()
        {
            return;
        }
        let body = event_json(
            &self.api_key,
            "app_closed",
            &self.install_id,
            &self.session_id,
            lang,
            Some(elapsed.as_secs()),
        );
        self.enqueue(Msg::Body(body));
        let (ack_tx, ack_rx) = sync_channel(1);
        if self.enqueue(Msg::Flush(ack_tx)) {
            let _ = ack_rx.recv_timeout(budget);
        }
    }

    /// Never blocks: a full channel drops the message.
    fn enqueue(&self, msg: Msg) -> bool {
        let mut tx = self.tx.lock().unwrap_or_else(|p| p.into_inner());
        if tx.is_none() {
            let Some(sink) = self.sink.lock().unwrap_or_else(|p| p.into_inner()).take() else {
                return false;
            };
            let (t, rx) = sync_channel::<Msg>(4);
            let spawned = std::thread::Builder::new()
                .name("kt-telemetry".into())
                .spawn(move || run(sink, rx))
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

fn run(mut sink: Box<dyn Sink>, rx: Receiver<Msg>) {
    let mut logged = false;
    for msg in rx {
        match msg {
            Msg::Body(b) => {
                if let Err(e) = sink.send(&b)
                    && !logged
                {
                    logged = true;
                    crate::journal::log("INFO", &format!("telemetry: not sent ({e})"));
                }
            }
            Msg::Flush(ack) => {
                let _ = ack.try_send(());
            }
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

/// What is sent and what is NOT sent — one catalog entry per list, shared by the notice and
/// the checkbox hint, so the two cannot disagree (Audacity lesson, F-COL-046).
pub fn lists_text(sep: &str) -> String {
    use crate::i18n::{k, tr};
    format!("{}{sep}{}", tr(k::telemetry::SENT), tr(k::telemetry::NOT_SENT))
}

/// The first-start notice: headline, both lists, and where it is switched off (rule 19).
pub fn notice_text() -> String {
    use crate::i18n::{k, tr};
    format!("{}\n{}\n{}", tr(k::telemetry::HEADLINE), lists_text("\n"), tr(k::telemetry::HOW_OFF))
}

/// The checkbox hint (one status-bar line): why it is disabled, if it is, then both lists.
pub fn hint_text(block: Option<Block>) -> String {
    use crate::i18n::{k, tr};
    let lists = lists_text(" ");
    match block {
        None => lists,
        Some(Block::NoKey) => format!("{} {lists}", tr(k::telemetry::NO_KEY)),
        Some(Block::Env) => format!("{} {lists}", tr(k::telemetry::ENV_OFF)),
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::Sink;
    use std::sync::{Arc, Mutex};

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
    use super::testing::Recording;
    use super::*;

    fn on() -> Consent {
        Consent { choice: true, first_start: false, block: None }
    }

    fn recorded() -> (Telemetry, Recording) {
        let rec = Recording::default();
        let t = Telemetry::new(on(), new_uuid(), Box::new(rec.clone())).with_key("phc_test");
        (t, rec)
    }

    fn wait_for(rec: &Recording, n: usize) {
        let t0 = Instant::now();
        while rec.events().len() < n && t0.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// t836: one run gives exactly the pair, linked by the session id; a dropped session
    /// sends only the start.
    #[test]
    fn t836_start_and_close_pair() {
        let (t, rec) = recorded();
        t.frame("en");
        t.frame("en");
        t.closed_after(Duration::from_millis(125_600), "en", Duration::from_secs(5));
        t.closed_after(Duration::from_secs(200), "en", Duration::from_secs(5));
        wait_for(&rec, 2);
        let ev = rec.events();
        assert_eq!(ev.len(), 2, "{ev:?}");
        assert_eq!(ev[0]["event"], "app_started");
        assert_eq!(ev[1]["event"], "app_closed");
        assert_eq!(ev[0]["properties"]["$session_id"], ev[1]["properties"]["$session_id"]);
        assert_eq!(ev[1]["properties"]["duration_s"], 125);

        let (t, rec) = recorded();
        t.frame("en");
        drop(t);
        std::thread::sleep(Duration::from_millis(50));
        let ev = rec.events();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0]["event"], "app_started");
    }

    /// t837: only the allow-list, nothing from the person's paths or machine.
    #[test]
    fn t837_payload_is_the_allow_list() {
        let _project = std::path::Path::new("/home/someone/secret/cut.kt");
        let host = std::fs::read_to_string("/etc/hostname").unwrap_or_default();
        for (name, d) in [("app_started", None), ("app_closed", Some(42))] {
            let s = event_json("phc_test", name, &new_uuid(), &new_uuid(), "ru", d);
            let v: serde_json::Value = serde_json::from_str(&s).unwrap();
            let mut keys: Vec<&str> =
                v["properties"].as_object().unwrap().keys().map(|k| k.as_str()).collect();
            keys.sort();
            let mut want = vec![
                "$geoip_disable",
                "$process_person_profile",
                "$session_id",
                "app_version",
                "arch",
                "lang",
                "os",
            ];
            if d.is_some() {
                want.push("duration_s");
            }
            want.sort();
            assert_eq!(keys, want);
            let mut top: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
            top.sort();
            assert_eq!(top, ["api_key", "distinct_id", "event", "properties"]);
            assert_eq!(v["properties"]["$process_person_profile"], false);
            assert_eq!(v["properties"]["$geoip_disable"], true);
            assert!(!s.contains("someone") && !s.contains("secret"));
            let host = host.trim();
            if host.len() > 3 {
                assert!(!s.contains(host), "host name in the payload");
            }
        }
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
            t.frame("en");
            t.closed("en", Duration::from_millis(10));
            assert!(!t.thread_spawned(), "{c:?}");
        }
    }

    /// t841: a black-hole server costs at most the budget; a live one gets two POSTs.
    #[test]
    fn t841_never_blocks_and_posts_json() {
        use std::io::{Read, Write};
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
        let t0 = Instant::now();
        t.frame("en");
        assert!(t0.elapsed() < Duration::from_millis(50), "enqueue blocked: {:?}", t0.elapsed());
        let t1 = Instant::now();
        t.closed("en", EXIT_BUDGET);
        assert!(t1.elapsed() < Duration::from_millis(2500), "exit held {:?}", t1.elapsed());

        // a server that answers 200
        let ok = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/i/v0/e/", ok.local_addr().unwrap());
        let got = Arc::new(Mutex::new(Vec::<String>::new()));
        let g = got.clone();
        std::thread::spawn(move || {
            for mut s in ok.incoming().flatten() {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                // read headers, then Content-Length bytes
                loop {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
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
                            g.lock().unwrap().push(text[..i + 4 + len].to_string());
                            break;
                        }
                    }
                }
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                );
            }
        });
        let t = Telemetry::new(on(), new_uuid(), Box::new(HttpSink::new(url))).with_key("phc_test");
        t.frame("en");
        t.closed("en", Duration::from_secs(10));
        let reqs = got.lock().unwrap().clone();
        assert_eq!(reqs.len(), 2, "{reqs:?}");
        for (r, ev) in reqs.iter().zip(["app_started", "app_closed"]) {
            assert!(r.starts_with("POST /i/v0/e/ "), "{r}");
            let body = &r[r.find("\r\n\r\n").unwrap() + 4..];
            let v: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(v["api_key"], "phc_test");
            assert_eq!(v["event"], ev);
            assert!(v["distinct_id"].as_str().is_some_and(is_uuid));
        }
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
        assert!(rb.contains("packaging/posthog-key"), "release-build.sh does not read the key file");
        if cfg!(debug_assertions) {
            assert!(key().is_none(), "a debug build carries the key");
        }
    }
}
