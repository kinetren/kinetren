//! The audio-reactive generators' series of a file (gen-audio-reactive, F-TXT-011): computed from a
//! short-lived source of the caller's own and kept behind [`SeriesStore`].
//!
//! The pass opens nothing itself: the caller hands it a source it does not share, the pass promises
//! no tempo and stops when told — invariant 5's offline door. The store is a seam: today the sidecar
//! keeps the series beside the file's peaks and loudness (the purge of a changed file clears it), and
//! the caches' move into the project folder (kkt-26.11.20 A14) moves it by implementing the trait.

use std::path::Path;

use kt_plugin_api::audio_series::{Analyzer, AudioSeries, REC, VERSION};

use crate::MediaSource;

/// Where series are kept, by the file they were computed from. A miss is `None`; a store that fails
/// is a miss (a cache, never an error).
pub trait SeriesStore {
    fn get(&mut self, file: &Path) -> Option<AudioSeries>;
    fn put(&mut self, file: &Path, series: &AudioSeries);
}

impl SeriesStore for crate::index::MediaIndexDb {
    fn get(&mut self, file: &Path) -> Option<AudioSeries> {
        let (id, _) = self.ensure_media(file).ok()?;
        AudioSeries::from_bytes(self.audio_series(id, VERSION).ok()??)
    }
    fn put(&mut self, file: &Path, series: &AudioSeries) {
        if let Ok((id, _)) = self.ensure_media(file) {
            let _ = self.put_audio_series(id, VERSION, series.bytes());
        }
    }
}

/// The series of a source's whole audio, asking `stop` before every burst (`None` — stopped). A
/// source with no sound or no end (a still, a placeholder) has an empty series.
pub fn analyze_until(source: &mut dyn MediaSource, stop: &dyn Fn() -> bool) -> Option<AudioSeries> {
    let info = source.info();
    if !info.has_audio || info.duration >= crate::placeholder::NO_END {
        return AudioSeries::from_bytes(Vec::new());
    }
    let frames = info.duration.to_audio_frames().max(0);
    let per = kt_time::AUDIO_BURST_FRAMES as i64;
    let bursts = (frames + per - 1) / per;
    let mut a = Analyzer::new();
    let mut out = Vec::with_capacity(bursts as usize * REC);
    for b in 0..bursts {
        if stop() {
            return None;
        }
        let Ok(burst) = source.read_audio_burst(b) else { break };
        out.extend_from_slice(&a.burst(&burst.data[..]));
    }
    AudioSeries::from_bytes(out)
}

/// The series of `file`: the store's, or computed from the source `open` gives and stored. `None` —
/// stopped, or the file does not open.
pub fn series_for(
    file: &Path,
    store: Option<&mut dyn SeriesStore>,
    open: impl FnOnce() -> Option<Box<dyn MediaSource>>,
    stop: &dyn Fn() -> bool,
) -> Option<AudioSeries> {
    let mut store = store;
    if let Some(s) = store.as_mut().and_then(|st| st.get(file)) {
        return Some(s);
    }
    let mut src = open()?;
    let s = analyze_until(src.as_mut(), stop)?;
    if let Some(st) = store {
        st.put(file, &s);
    }
    Some(s)
}
