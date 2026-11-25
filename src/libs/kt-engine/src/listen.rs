//! What an audio-reactive generator hears (gen-audio-reactive, F-TXT-011).
//!
//! The series of files live in a process registry keyed by the media path as the project stores it.
//! The host fills it (kt-app's background job, the render job and kt-cli before an export) and
//! clears it when another project opens; the engine only reads it. `gen_sources` turns a listening
//! generator's track into its FEED ([`feed_for`]): the pieces of series of the file events on that
//! track, in the generator's material time. The feed is part of `GenState`, so a series that arrives
//! or a moved piece retunes the source, and `src_rev` carries the feed's key (the disk keys).
//!
//! What is heard is the FILE at the event's plain material time: a warp, a reverse or the event's
//! effects are not followed (the owner's question Q55; the mix after effects is option (b)).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use kt_model::{ClipEvent, ClipKind, FeedPiece, GenFeed, MediaId, Project};
use kt_time::Tick;
use kt_plugin_api::audio_series::AudioSeries;

/// Series kept at most; the oldest put goes first. A series is about 9 MB per hour of sound.
pub const MAX_SERIES: usize = 64;

#[derive(Default)]
struct Reg {
    by_path: HashMap<String, Arc<AudioSeries>>,
    order: Vec<String>,
}

fn reg() -> &'static RwLock<Reg> {
    static R: OnceLock<RwLock<Reg>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(Reg::default()))
}

/// Keep `series` as what `path` (the project's media path) sounds like.
pub fn put(path: &str, series: Arc<AudioSeries>) {
    let mut r = reg().write().unwrap();
    if r.by_path.insert(path.to_string(), series).is_none() {
        r.order.push(path.to_string());
    }
    while r.order.len() > MAX_SERIES {
        let old = r.order.remove(0);
        r.by_path.remove(&old);
    }
}

/// The series of `path`, if it is known.
pub fn get(path: &str) -> Option<Arc<AudioSeries>> {
    reg().read().unwrap().by_path.get(path).cloned()
}

/// Forget what `path` sounded like: another file lies there now, or it came back (found-defects 976).
pub fn forget(path: &str) {
    let mut r = reg().write().unwrap();
    r.by_path.remove(path);
    r.order.retain(|p| p != path);
}

/// Forget every series (another project opened).
pub fn clear() {
    let mut r = reg().write().unwrap();
    r.by_path.clear();
    r.order.clear();
}

/// Whether a media path names a file (not a generator or a synthetic source).
fn is_file(path: &str) -> bool {
    !kt_model::is_pseudo_path(path)
}

/// The FILE events a listening generator `g` hears among `clips` (its reel's clips): the plain audio
/// events on its track, overlapping it in time.
fn heard<'a, 'p>(clips: &'a [ClipEvent], g: &ClipEvent, media_path: &dyn Fn(MediaId) -> Option<&'p str>) -> Vec<(&'a ClipEvent, &'p str)> {
    let Some(lane) = g.r#gen.as_ref().and_then(|k| k.listen_lane()) else { return Vec::new() };
    let (g0, g1) = (g.start.0, g.start.0 + g.duration.0);
    let mut out: Vec<(&'a ClipEvent, &'p str)> = clips
        .iter()
        .filter(|a| a.kind == ClipKind::Audio && a.lane == lane && a.r#gen.is_none() && !a.reversed)
        .filter(|a| a.start.0 < g1 && g0 < a.start.0 + a.duration.0)
        .filter_map(|a| media_path(a.media).filter(|p| is_file(p)).map(|p| (a, p)))
        .collect();
    out.sort_by_key(|(a, _)| (a.start, a.id));
    out
}

/// The feed of generator `g` among `clips`, with the series the registry knows; pieces whose series
/// is not known yet are silence. Overlapping events: the earlier one is heard.
pub fn feed_for<'p>(clips: &[ClipEvent], g: &ClipEvent, media_path: &dyn Fn(MediaId) -> Option<&'p str>) -> GenFeed {
    let mut pieces: Vec<FeedPiece> = Vec::new();
    let mut taken = i64::MIN;
    let base = g.source_in.0 - g.start.0;
    for (a, path) in heard(clips, g, media_path) {
        let lo = a.start.0.max(g.start.0).max(taken);
        let hi = (a.start.0 + a.duration.0).min(g.start.0 + g.duration.0);
        if lo >= hi {
            continue;
        }
        taken = hi;
        let Some(series) = get(path) else { continue };
        pieces.push(FeedPiece { from: Tick(lo + base), to: Tick(hi + base), offset: (a.source_in.0 - a.start.0) - base, series });
    }
    GenFeed(pieces)
}

/// Every file path a listening generator of `project` hears (the root and every reel), once each:
/// what the host computes series for.
pub fn heard_paths(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let path = |id: MediaId| project.media.iter().find(|m| m.id == id).map(|m| m.path.as_str());
    let mut reels: Vec<&[ClipEvent]> = vec![&project.clips];
    reels.extend(project.sequences.iter().map(|s| s.clips.as_slice()));
    for clips in reels {
        for g in clips.iter().filter(|c| c.r#gen.as_ref().is_some_and(|k| k.listen_lane().is_some())) {
            for (_, p) in heard(clips, g, &path) {
                if !out.iter().any(|o| o == p) {
                    out.push(p.to_string());
                }
            }
        }
    }
    out
}

/// Compute and keep every series `project`'s listening generators hear and the registry lacks — what
/// a render does before its first frame, so its bytes never depend on whether a background job had
/// finished (invariant 11). `file` resolves a media path to the file to open, `open` opens a short-lived
/// source of the caller's own (invariant 5's offline door), `store` keeps and gives back series.
/// A file that does not open is heard as silence.
pub fn ensure(
    project: &Project,
    file: &dyn Fn(&str) -> String,
    open: &dyn Fn(&str) -> Option<Box<dyn kt_media::MediaSource>>,
    mut store: Option<&mut dyn kt_media::listen::SeriesStore>,
    stop: &dyn Fn() -> bool,
) {
    for key in heard_paths(project) {
        if get(&key).is_some() {
            continue;
        }
        let f = file(&key);
        let st = store.as_mut().map(|s| &mut **s as &mut dyn kt_media::listen::SeriesStore);
        if let Some(s) = kt_media::listen::series_for(std::path::Path::new(&f), st, || open(&f), stop) {
            put(&key, Arc::new(s));
        }
    }
}
