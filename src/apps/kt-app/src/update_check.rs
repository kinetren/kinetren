//! The daily "is a newer release out" check (change `update-check`, capability `distribution`).
//!
//! Once per UTC day, after the first frame of a release build, a background thread asks the
//! GitHub Releases API for the latest normal release of `kinetren/kinetren`. A newer release is
//! said in the bell (one notice) and as a short word after the version in the title — never
//! in a window, never downloaded (owner, 2026-10-27).
//!
//! "Newer" is read from the release's ASSET names, which carry the binary's own commit-date
//! version (`kinetren-<YY.MM.DD>-<hash8>-…`, `release-build.sh`); the tag is only a name and may
//! be dated after its commit (t872). The day's answer lives in `<data_dir>/update-check`,
//! written by the thread itself, so a crash in the first 30 s does not cost a second request
//! (t873). The UI never waits, the exit neither (t874).
//!
//! One `Checker` per process (`install`/`global`), like `telemetry.rs`: File → New/Open
//! rebuild `KtApp` in place, and a per-app checker would ask and say it again.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// The one question this module asks.
pub const API_URL: &str = "https://api.github.com/repos/kinetren/kinetren/releases/latest";
/// Where a person gets the new release (said in the notice and the title hint).
pub const RELEASES_PAGE: &str = "github.com/kinetren/kinetren/releases";
/// The day's answer inside the data directory.
pub const STATE_FILE: &str = "update-check";
/// eframe storage key of the person's choice (`on`/`off`), next to `telemetry`.
pub const STORAGE_KEY: &str = "update_check";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(10);
const BODY_LIMIT: u64 = 1 << 20;

/// A commit-date version: `YY.MM.DD` and the 8-hex hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub date: String,
    pub hash: String,
}

impl Version {
    /// `ktYY.MM.DD-hash8` (the binary's `kt_version::FULL`); `kt00.00.00-nogit` is `None`.
    pub fn parse_full(s: &str) -> Option<Version> {
        let (date, hash) = s.strip_prefix("kt")?.split_once('-')?;
        Self::checked(date, hash)
    }

    /// The version inside a release asset name, `kinetren-<YY.MM.DD>-<hash8>-…`.
    pub fn from_asset(name: &str) -> Option<Version> {
        let rest = name.strip_prefix("kinetren-")?;
        let (date, rest) = (rest.get(..8)?, rest.get(8..)?);
        let hash = rest.strip_prefix('-')?.get(..8)?;
        rest.get(9..)?.starts_with('-').then_some(())?;
        Self::checked(date, hash)
    }

    fn checked(date: &str, hash: &str) -> Option<Version> {
        let parts: Vec<&str> = date.split('.').collect();
        let date_ok = parts.len() == 3
            && parts.iter().all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_digit()))
            && date != "00.00.00";
        let hash_ok = hash.len() == 8 && hash.bytes().all(|b| b.is_ascii_hexdigit());
        (date_ok && hash_ok).then(|| Version { date: date.into(), hash: hash.to_ascii_lowercase() })
    }

    pub fn full(&self) -> String {
        format!("kt{}-{}", self.date, self.hash)
    }

    /// Strictly later date AND another hash (design D2: a same-day release is not "newer").
    pub fn is_newer_than(&self, own: &Version) -> bool {
        // YY.MM.DD is fixed-width: the string order is the date order
        self.date > own.date && self.hash != own.hash
    }
}

/// What the latest-release JSON says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub normal: bool,
    pub version: Option<Version>,
}

/// Parse the API answer; an unparsable body is an error (the check failed, the day stays due).
pub fn parse_release(body: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("not JSON: {e}"))?;
    let assets = v.get("assets").and_then(|a| a.as_array()).ok_or("no assets list")?;
    let flag = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
    let version = assets
        .iter()
        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
        .find_map(Version::from_asset);
    Ok(Release { normal: !flag("prerelease") && !flag("draft"), version })
}

/// The decision of the spec table (t872). The thread takes the same steps one by one
/// (`parse_release` → `is_newer_than`), because it also records a release that is not newer.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The running build has no git version: no request at all.
    NoCheck,
    NotNewer,
    Newer(String),
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn decide(own_full: &str, body: &str) -> Decision {
    let Some(own) = Version::parse_full(own_full) else {
        return Decision::NoCheck;
    };
    match parse_release(body) {
        Ok(Release { normal: true, version: Some(v) }) if v.is_newer_than(&own) => Decision::Newer(v.full()),
        _ => Decision::NotNewer,
    }
}

/// The UTC calendar day of a moment, `YYYY-MM-DD` (design D3: no time-zone database).
pub fn utc_day(t: std::time::SystemTime) -> String {
    let days = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // days → civil date (H. Hinnant, "chrono-compatible low-level date algorithms")
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Is a check due on `today`, or what did today's check find (t873).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Due {
    Yes,
    /// Checked today; the latest normal release version then (`None`: none recognised).
    No(Option<String>),
}

pub fn due_in(dir: &Path, today: &str) -> Due {
    let Ok(text) = std::fs::read_to_string(dir.join(STATE_FILE)) else {
        return Due::Yes;
    };
    let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)).map(str::trim);
    match (field("day="), field("latest=")) {
        (Some(day), Some("")) if day == today => Due::No(None),
        (Some(day), Some(v)) if day == today && Version::parse_full(v).is_some() => Due::No(Some(v.into())),
        // another day, or a file we did not write: ask again and rewrite it
        _ => Due::Yes,
    }
}

/// Record a SUCCESSFUL check (tmp + rename: a kill mid-write leaves the old file).
pub fn record_in(dir: &Path, today: &str, latest: Option<&str>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{STATE_FILE}.tmp"));
    std::fs::write(&tmp, format!("day={today}\nlatest={}\n", latest.unwrap_or("")))?;
    std::fs::rename(&tmp, dir.join(STATE_FILE))
}

/// Why the check cannot run, whatever the person chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    /// A debug build without `KT_UPDATE_URL` (every `cargo run`, test and live scenario).
    Build,
    /// `KINETREN_UPDATE_CHECK=0|off|false`.
    Env,
}

/// The environment variables that decide consent, read once.
#[derive(Debug, Clone, Default)]
pub struct EnvView {
    pub url: Option<String>,
    pub switch: Option<String>,
}

impl EnvView {
    pub fn from_env() -> Self {
        EnvView {
            url: std::env::var("KT_UPDATE_URL").ok().filter(|s| !s.trim().is_empty()),
            switch: std::env::var("KINETREN_UPDATE_CHECK").ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consent {
    /// The person's checkbox (default on).
    pub choice: bool,
    pub block: Option<Block>,
    pub url: String,
    /// The debug door: the live scenario's server is asked at every start.
    pub ignore_day: bool,
}

impl Consent {
    pub fn on(&self) -> bool {
        self.choice && self.block.is_none()
    }
}

/// Consent from the build, the environment and the stored choice (t877).
pub fn resolve_consent(debug: bool, env: &EnvView, stored: Option<&str>) -> Consent {
    let forbids = env
        .switch
        .as_deref()
        .is_some_and(|s| ["0", "off", "false", "no"].contains(&s.trim().to_ascii_lowercase().as_str()));
    // the debug door: the live scenario's local server; a release build ignores it
    let door = env.url.clone().filter(|_| debug);
    let block = if debug && door.is_none() {
        Some(Block::Build)
    } else if forbids {
        Some(Block::Env)
    } else {
        None
    };
    Consent {
        choice: stored != Some("off"),
        block,
        ignore_day: door.is_some(),
        url: door.unwrap_or_else(|| API_URL.to_string()),
    }
}

/// Where the request goes. HTTP in production, a canned answer in tests.
pub trait Fetch: Send {
    fn fetch(&mut self, url: &str) -> Result<String, String>;
}

pub struct HttpFetch;

impl Fetch for HttpFetch {
    fn fetch(&mut self, url: &str) -> Result<String, String> {
        let mut b = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(TOTAL_TIMEOUT));
        // the environment's proxy is never right for the machine itself (as in telemetry.rs)
        if ["http://127.0.0.1", "http://localhost", "http://[::1]"].iter().any(|p| url.starts_with(p)) {
            b = b.proxy(None);
        }
        let agent: ureq::Agent = b.build().into();
        // GitHub refuses a request without a User-Agent; ours names only the program version
        agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", format!("kinetren/{}", kt_version::FULL))
            .call()
            .map_err(|e| e.to_string())?
            .into_body()
            .with_config()
            .limit(BODY_LIMIT)
            .read_to_string()
            .map_err(|e| e.to_string())
    }
}

pub struct Checker {
    consent: Mutex<Consent>,
    own: String,
    dir: PathBuf,
    started: AtomicBool,
    thread_spawned: AtomicBool,
    /// Taken by the thread on the first (and only) start.
    fetch: Mutex<Option<Box<dyn Fetch>>>,
    shared: Arc<Shared>,
}

/// What the thread hands back.
#[derive(Default)]
struct Shared {
    /// The latest normal release version known today (from the file or the request).
    latest: Mutex<Option<String>>,
    /// `latest` is newer than the running build — read every frame by the title, so an atomic.
    newer: AtomicBool,
    /// The thread found a newer release: the one notice is due.
    notice_due: AtomicBool,
}

impl Checker {
    pub fn new(consent: Consent, own: &str, dir: PathBuf, fetch: Box<dyn Fetch>) -> Self {
        Checker {
            consent: Mutex::new(consent),
            own: own.to_string(),
            dir,
            started: AtomicBool::new(false),
            thread_spawned: AtomicBool::new(false),
            fetch: Mutex::new(Some(fetch)),
            shared: Arc::new(Shared::default()),
        }
    }

    /// The process-wide checker of a real start.
    pub fn for_process(stored: Option<&str>) -> Self {
        let consent = resolve_consent(cfg!(debug_assertions), &EnvView::from_env(), stored);
        Self::new(consent, kt_version::FULL, crate::journal::data_dir(), Box::new(HttpFetch))
    }

    pub fn consent(&self) -> Consent {
        self.consent.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// The person's checkbox: off hides the title word at once; on asks at the next start.
    pub fn set_choice(&self, on: bool) {
        self.consent.lock().unwrap_or_else(|p| p.into_inner()).choice = on;
    }

    /// The value to store under `update_check`; `None` when a block makes the choice moot.
    pub fn stored_value(&self) -> Option<&'static str> {
        let c = self.consent();
        c.block.is_none().then_some(if c.choice { "on" } else { "off" })
    }

    /// The running build's full version (the notice names it).
    pub fn own(&self) -> &str {
        &self.own
    }

    /// The first frame has started it (the UI skips building `today` after that).
    pub fn has_started(&self) -> bool {
        self.started.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub fn thread_spawned(&self) -> bool {
        self.thread_spawned.load(Ordering::Relaxed)
    }

    /// Called every frame; does something only on the first. `repaint` is called once by the
    /// thread when its answer is in.
    pub fn start(&self, today: &str, repaint: Box<dyn Fn() + Send>) {
        if self.started.swap(true, Ordering::Relaxed) {
            return;
        }
        let c = self.consent();
        let Some(own) = Version::parse_full(&self.own).filter(|_| c.on()) else {
            return;
        };
        if !c.ignore_day
            && let Due::No(latest) = due_in(&self.dir, today)
        {
            // checked today already: the title still says it, the bell does not again
            self.shared.set_latest(latest.as_deref().and_then(Version::parse_full), &own);
            return;
        }
        let Some(mut fetch) = self.fetch.lock().unwrap_or_else(|p| p.into_inner()).take() else {
            return;
        };
        let (shared, dir, today, url) = (self.shared.clone(), self.dir.clone(), today.to_string(), c.url);
        let spawned = std::thread::Builder::new()
            .name("kt-update-check".into())
            .spawn(move || match fetch.fetch(&url).and_then(|b| parse_release(&b)) {
                Ok(rel) => {
                    let latest = rel.version.filter(|_| rel.normal);
                    if let Err(e) = record_in(&dir, &today, latest.as_ref().map(Version::full).as_deref()) {
                        crate::journal::log("INFO", &format!("update check: not recorded ({e})"));
                    }
                    if shared.set_latest(latest, &own) {
                        shared.notice_due.store(true, Ordering::Relaxed);
                    }
                    repaint();
                }
                // silent: the next start asks again (spec: a failure does not mark the day)
                Err(e) => crate::journal::log("INFO", &format!("update check: not answered ({e})")),
            })
            .is_ok();
        self.thread_spawned.store(spawned, Ordering::Relaxed);
    }

    /// `Some(new version)` exactly once, after the request of this start found a newer release.
    pub fn take_notice(&self) -> Option<String> {
        if self.shared.notice_due.swap(false, Ordering::Relaxed) { self.outdated() } else { None }
    }

    /// A newer release is out and the check is on — the title word. Cheap: read every frame.
    pub fn is_outdated(&self) -> bool {
        self.shared.newer.load(Ordering::Relaxed) && self.consent().on()
    }

    /// The newer release, while the check is on (the notice and the title hint).
    pub fn outdated(&self) -> Option<String> {
        if !self.is_outdated() {
            return None;
        }
        self.shared.latest.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

impl Shared {
    /// Publish the latest release; `true` when it is newer than the running build.
    fn set_latest(&self, latest: Option<Version>, own: &Version) -> bool {
        let newer = latest.as_ref().is_some_and(|v| v.is_newer_than(own));
        *self.latest.lock().unwrap_or_else(|p| p.into_inner()) = latest.map(|v| v.full());
        self.newer.store(newer, Ordering::Relaxed);
        newer
    }
}

static GLOBAL: OnceLock<Arc<Checker>> = OnceLock::new();

/// Install the process checker (once, at start in `run`).
pub fn install(c: Checker) -> Arc<Checker> {
    GLOBAL.get_or_init(|| Arc::new(c)).clone()
}

/// The process checker, if `run` installed one (the harness does not).
pub fn global() -> Option<Arc<Checker>> {
    GLOBAL.get().cloned()
}

/// `ktYY.MM.DD` of a full version: the notice names days, the hash would be noise.
pub fn day_form(full: &str) -> &str {
    full.split('-').next().unwrap_or(full)
}

/// The one notice: the new version, ours, where to get it, and where the check is switched
/// off (rule 19) — by the checkbox's own catalog name inside `how_off`.
pub fn notice_text(new: &str, own: &str) -> String {
    use crate::i18n::{k, tr};
    let said = crate::tf!(k::update::NOTICE, day_form(new), day_form(own), RELEASES_PAGE);
    format!("{said}\n{}", tr(k::update::HOW_OFF))
}

/// The status-bar hint over the title word.
pub fn title_hint(new: &str) -> String {
    use crate::i18n::k;
    crate::tf!(k::update::TITLE_HINT, day_form(new), RELEASES_PAGE)
}

/// The checkbox hint: why it is disabled, if it is, then what is asked and sent.
pub fn hint_text(block: Option<Block>) -> String {
    use crate::i18n::{k, tr};
    let what = tr(k::update::CHECKBOX_HINT);
    match block {
        None => what.to_string(),
        Some(Block::Build) => format!("{} {what}", tr(k::update::DEV_BUILD)),
        Some(Block::Env) => format!("{} {what}", tr(k::update::ENV_OFF)),
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::Fetch;
    use std::sync::{Arc, Mutex};

    /// Test fetch: answers the canned body and counts the requests.
    #[derive(Clone)]
    pub struct Canned {
        pub body: Result<String, String>,
        pub calls: Arc<Mutex<Vec<String>>>,
    }

    impl Canned {
        pub fn ok(body: &str) -> Self {
            Canned { body: Ok(body.into()), calls: Arc::default() }
        }
        pub fn calls(&self) -> usize {
            self.calls.lock().unwrap_or_else(|p| p.into_inner()).len()
        }
    }

    impl Fetch for Canned {
        fn fetch(&mut self, url: &str) -> Result<String, String> {
            self.calls.lock().unwrap_or_else(|p| p.into_inner()).push(url.to_string());
            self.body.clone()
        }
    }

    /// A latest-release answer with the tag `v<tag_day>`, flags and asset names. (The day
    /// without its "v": a literal shaped like `v26.11.01` reads as a probe name to the
    /// live-coverage scanner.)
    pub fn release_json(tag_day: &str, prerelease: bool, draft: bool, assets: &[&str]) -> String {
        let tag = format!("v{tag_day}");
        let assets: Vec<serde_json::Value> =
            assets.iter().map(|a| serde_json::json!({ "name": a, "size": 1 })).collect();
        serde_json::json!({
            "tag_name": tag,
            "name": tag,
            "prerelease": prerelease,
            "draft": draft,
            "html_url": "https://github.com/kinetren/kinetren/releases/tag/x",
            "assets": assets,
        })
        .to_string()
    }

    /// Wait for the thread's answer.
    pub fn wait(c: &super::Checker, done: impl Fn(&super::Checker) -> bool) {
        let t0 = std::time::Instant::now();
        while !done(c) && t0.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// A fresh temp data directory for one test.
    pub fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kt-update-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use std::time::Instant;

    const OWN: &str = "kt26.10.20-aaaaaaaa";

    fn on_at(url: &str) -> Consent {
        Consent { choice: true, block: None, url: url.into(), ignore_day: false }
    }

    /// t872: "newer" is decided from asset names, conservatively; the asset form is the one
    /// `release-build.sh` makes.
    #[test]
    fn t872_newer_from_asset_names() {
        let a = |n: &str| format!("kinetren-{n}-linux-x86_64.tar.gz");
        let newer = a("26.11.01-bbbbbbbb");
        let cases = [
            (release_json("26.11.01", false, false, &[&newer, &format!("{newer}.sha256")]), true),
            (release_json("26.11.01", true, false, &[&newer]), false),
            (release_json("26.11.01", false, true, &[&newer]), false),
            (release_json("26.10.20", false, false, &[&a("26.10.20-bbbbbbbb")]), false),
            (release_json("26.11.01", false, false, &[&a("26.11.01-aaaaaaaa")]), false),
            (release_json("26.10.01", false, false, &[&a("26.10.01-cccccccc")]), false),
            (release_json("26.12.01", false, false, &["README"]), false),
            ("<html>rate limited</html>".to_string(), false),
        ];
        for (i, (body, want)) in cases.iter().enumerate() {
            let got = decide(OWN, body);
            if *want {
                assert_eq!(got, Decision::Newer("kt26.11.01-bbbbbbbb".into()), "case {i}");
            } else {
                assert_eq!(got, Decision::NotNewer, "case {i}: {body}");
            }
        }
        assert_eq!(decide("kt00.00.00-nogit", &cases[0].0), Decision::NoCheck);
        // the Windows zip carries the same version form
        assert_eq!(
            Version::from_asset("kinetren-26.11.01-bbbbbbbb-windows-x86_64.zip"),
            Some(Version { date: "26.11.01".into(), hash: "bbbbbbbb".into() })
        );
        // the running build's own version parses (or no check is made at all)
        if kt_version::FULL != "kt00.00.00-nogit" {
            assert!(Version::parse_full(kt_version::FULL).is_some(), "{}", kt_version::FULL);
        }
        // the form the decision parses is the form release-build.sh writes
        let rb = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/release-build.sh"
        ))
        .unwrap();
        assert!(
            rb.contains("kinetren-$VERSION-$TARGET-$ARCH") && rb.contains("VERSION=\"${KT_VERSION_FULL#kt}\""),
            "release-build.sh no longer names assets kinetren-<YY.MM.DD-hash8>-…: every release \
             would silently be \"not newer\""
        );
    }

    /// t873: once a day, the day's answer is reused, a failure does not count.
    #[test]
    fn t873_once_a_day() {
        let dir = temp_dir("day");
        assert_eq!(due_in(&dir, "2026-10-27"), Due::Yes);
        record_in(&dir, "2026-10-27", Some("kt26.11.01-bbbbbbbb")).unwrap();
        assert_eq!(due_in(&dir, "2026-10-27"), Due::No(Some("kt26.11.01-bbbbbbbb".into())));
        assert_eq!(due_in(&dir, "2026-10-28"), Due::Yes);
        std::fs::write(dir.join(STATE_FILE), "garbage").unwrap();
        assert_eq!(due_in(&dir, "2026-10-27"), Due::Yes);
        let _ = std::fs::remove_dir_all(&dir);

        // a failed check: the day stays due, no file
        let dir = temp_dir("fail");
        let fetch = Canned { body: Err("offline".into()), calls: Default::default() };
        let c = Checker::new(on_at(API_URL), OWN, dir.clone(), Box::new(fetch.clone()));
        c.start("2026-10-27", Box::new(|| {}));
        wait(&c, |_| fetch.calls() == 1);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(fetch.calls(), 1);
        assert_eq!(due_in(&dir, "2026-10-27"), Due::Yes);
        assert!(!dir.join(STATE_FILE).exists());
        assert!(c.take_notice().is_none() && c.outdated().is_none());
        let _ = std::fs::remove_dir_all(&dir);

        // the UTC day of known moments
        let at = |s: u64| std::time::UNIX_EPOCH + Duration::from_secs(s);
        assert_eq!(utc_day(at(0)), "1970-01-01");
        assert_eq!(utc_day(at(1_793_059_199)), "2026-10-26");
        assert_eq!(utc_day(at(1_793_059_200)), "2026-10-27");
        assert_eq!(utc_day(at(951_782_400)), "2000-02-29");
    }

    /// t874: the request is anonymous, and a dead server costs the UI nothing.
    #[test]
    fn t874_anonymous_and_never_waited_for() {
        use std::io::{Read, Write};
        // black hole: accepts, never answers
        let hole = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/repos/kinetren/kinetren/releases/latest", hole.local_addr().unwrap());
        std::thread::spawn(move || {
            let mut keep = Vec::new();
            for s in hole.incoming().flatten() {
                keep.push(s);
            }
        });
        let dir = temp_dir("hole");
        let c = Checker::new(on_at(&url), OWN, dir.clone(), Box::new(HttpFetch));
        let t0 = Instant::now();
        c.start("2026-10-27", Box::new(|| {}));
        assert!(t0.elapsed() < Duration::from_millis(50), "start blocked: {:?}", t0.elapsed());
        assert!(c.thread_spawned());
        std::thread::sleep(Duration::from_millis(100));
        assert!(c.take_notice().is_none() && c.outdated().is_none());
        assert!(!dir.join(STATE_FILE).exists(), "the day is not recorded while unanswered");
        // what `on_exit` leaves behind: dropping the checker does not join its thread
        let t1 = Instant::now();
        drop(c);
        assert!(t1.elapsed() < Duration::from_millis(50), "exit held {:?}", t1.elapsed());

        // a server that answers 200 with a newer release
        let ok = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/repos/kinetren/kinetren/releases/latest", ok.local_addr().unwrap());
        let got = Arc::new(Mutex::new(Vec::<String>::new()));
        let g = got.clone();
        let body = release_json("26.11.01", false, false, &["kinetren-26.11.01-bbbbbbbb-linux-x86_64.tar.gz"]);
        std::thread::spawn(move || {
            for mut s in ok.incoming().flatten() {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                while !String::from_utf8_lossy(&buf).contains("\r\n\r\n") {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                g.lock().unwrap().push(String::from_utf8_lossy(&buf).to_string());
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            }
        });
        let dir = temp_dir("ok");
        let c = Checker::new(on_at(&url), OWN, dir.clone(), Box::new(HttpFetch));
        c.start("2026-10-27", Box::new(|| {}));
        wait(&c, |c| c.outdated().is_some());
        assert_eq!(c.outdated().as_deref(), Some("kt26.11.01-bbbbbbbb"));
        assert_eq!(c.take_notice().as_deref(), Some("kt26.11.01-bbbbbbbb"));
        assert!(c.take_notice().is_none(), "the notice is due once");
        assert_eq!(due_in(&dir, "2026-10-27"), Due::No(Some("kt26.11.01-bbbbbbbb".into())));
        let reqs = got.lock().unwrap().clone();
        assert_eq!(reqs.len(), 1, "{reqs:?}");
        let req = &reqs[0];
        assert!(req.starts_with("GET /repos/kinetren/kinetren/releases/latest HTTP/1.1\r\n"), "{req}");
        let mut names = Vec::new();
        for line in req.lines().skip(1).filter(|l| !l.is_empty()) {
            let (name, value) = line.split_once(':').expect("header line");
            let name = name.trim().to_ascii_lowercase();
            if name == "user-agent" {
                assert_eq!(value.trim(), format!("kinetren/{OWN_UA}", OWN_UA = kt_version::FULL));
            }
            if name == "accept" {
                assert_eq!(value.trim(), "application/vnd.github+json");
            }
            names.push(name);
        }
        for want in ["accept", "user-agent"] {
            assert!(names.iter().any(|n| n == want), "{want} missing: {names:?}");
        }
        for n in &names {
            assert!(
                ["host", "accept", "user-agent", "accept-encoding", "connection"].contains(&n.as_str()),
                "header outside the allow-list: {n}"
            );
        }
        let user = std::env::var("USER").unwrap_or_default();
        let host = std::fs::read_to_string("/etc/hostname").unwrap_or_default();
        let data = crate::journal::data_dir().to_string_lossy().to_string();
        for secret in [user.as_str(), host.trim(), data.as_str()] {
            if secret.len() > 3 {
                assert!(!req.contains(secret), "the request names “{secret}”");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(temp_dir("hole"));
    }

    /// t877: consent resolution; a checker resolved off never spawns its thread.
    #[test]
    fn t877_consent_resolution() {
        let none = EnvView::default();
        let env = |u: Option<&str>, s: Option<&str>| EnvView { url: u.map(Into::into), switch: s.map(Into::into) };
        let local = "http://127.0.0.1:9/latest";

        let c = resolve_consent(true, &none, None);
        assert!(!c.on() && c.block == Some(Block::Build));
        let c = resolve_consent(true, &env(Some(local), None), None);
        assert!(c.on() && c.url == local && c.ignore_day);
        let c = resolve_consent(false, &none, None);
        assert!(c.on() && c.url == API_URL && !c.ignore_day);
        let c = resolve_consent(false, &env(None, Some("0")), None);
        assert!(!c.on() && c.block == Some(Block::Env));
        for w in ["off", "false", "OFF"] {
            assert!(!resolve_consent(false, &env(None, Some(w)), None).on(), "{w}");
        }
        let c = resolve_consent(false, &none, Some("off"));
        assert!(!c.on() && c.block.is_none());
        let c = resolve_consent(false, &env(None, Some("1")), Some("on"));
        assert!(c.on());
        // a release build cannot be pointed elsewhere
        let c = resolve_consent(false, &env(Some(local), None), None);
        assert!(c.on() && c.url == API_URL && !c.ignore_day);
        // the environment cannot re-enable what the person switched off
        assert!(!resolve_consent(false, &env(None, Some("1")), Some("off")).on());

        for c in [resolve_consent(true, &none, None), resolve_consent(false, &none, Some("off"))] {
            let dir = temp_dir("off");
            let fetch = Canned::ok("{}");
            let k = Checker::new(c.clone(), OWN, dir, Box::new(fetch.clone()));
            k.start("2026-10-27", Box::new(|| {}));
            assert!(!k.thread_spawned(), "{c:?}");
            assert_eq!(fetch.calls(), 0);
        }
        // a build without a git version never asks either
        let fetch = Canned::ok("{}");
        let k = Checker::new(on_at(API_URL), "kt00.00.00-nogit", temp_dir("nogit"), Box::new(fetch.clone()));
        k.start("2026-10-27", Box::new(|| {}));
        assert!(!k.thread_spawned());
    }
}
