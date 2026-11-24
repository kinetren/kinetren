//! The persistent cache in Preferences (project-cache-folder D3): every cache folder the program made,
//! its projects, size and cap, and the notices of a folder that is full or lies elsewhere.
//!
//! The list is read once, on the media lane, when the section comes on screen and after each change;
//! while the window stands nothing is read or counted (rule 12 of the invariants).

use crate::i18n::{k, tr};
use eframe::egui;
use kt_media::cache_folder::{self, Folder, Notice, registry};
use std::path::{Path, PathBuf};

/// One folder of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct CacheRow {
    pub folder: PathBuf,
    /// The projects seen in it that are still there.
    pub projects: Vec<PathBuf>,
    /// Projects were seen in it, and none is there.
    pub orphan: bool,
    pub bytes: u64,
    pub cap: u64,
    pub full: bool,
    /// Why it lies elsewhere than beside the project, or what else a person should know of its place.
    pub place: Option<Place>,
}

/// What the row says of the folder's place.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    /// The project's folder takes no files: what it answered.
    Elsewhere(String),
    /// Inside a folder a cloud tool uploads whole.
    Synced,
}

#[derive(Default)]
pub struct CachesUi {
    rows: Option<Vec<CacheRow>>,
    reading: Option<kt_time::sched::Answer<Vec<CacheRow>>>,
    /// The section is to be brought on screen (a notice's button).
    pub show: bool,
    /// How often the list was read (t3842: once, then after each change).
    #[cfg(test)]
    pub reads: usize,
}

/// The rows as the registry and the disk have them now; a folder that is no more leaves the registry.
fn read_rows() -> Vec<CacheRow> {
    registry::list()
        .into_iter()
        .filter_map(|r| {
            if !r.folder.is_dir() {
                registry::remove(&r.folder);
                return None;
            }
            let live = Folder::peek(&r.folder);
            let cap = r.cap.unwrap_or(cache_folder::DEFAULT_CAP);
            let bytes = match &live {
                Some(f) => f.known_sizes().values().map(|v| v.0).sum::<u64>().max(if f.is_counted() { 0 } else { r.bytes }),
                None => r.bytes,
            };
            let projects: Vec<PathBuf> = r.projects.iter().filter(|p| p.exists()).cloned().collect();
            let place = match live.as_ref().map(|f| f.place()) {
                Some(cache_folder::Place::NoFiles(why)) => Some(Place::Elsewhere(why.clone())),
                _ => (cache_folder::synced(&r.folder) == Some(cache_folder::CloudTool::OneDrive)).then_some(Place::Synced),
            };
            Some(CacheRow {
                place,
                orphan: projects.is_empty() && !r.projects.is_empty(),
                full: bytes >= cap || live.is_some_and(|f| f.is_full()),
                folder: r.folder,
                projects,
                bytes,
                cap,
            })
        })
        .collect()
}

/// Delete a cache folder: the files of one that is open, the whole of another. Only what carries the
/// cache tag is ever removed.
fn delete(folder: &Path) {
    if let Some(f) = Folder::peek(folder) {
        f.clear(None);
        f.flush();
        return;
    }
    let tag = std::fs::read_to_string(folder.join("CACHEDIR.TAG")).unwrap_or_default();
    if tag.starts_with("Signature: 8a477f597d28d172789f06886806bc55") {
        let _ = std::fs::remove_dir_all(folder);
        registry::remove(folder);
    }
}

fn set_cap(folder: &Path, cap: u64) {
    match Folder::peek(folder) {
        Some(f) => f.set_cap(cap),
        None => registry::set_cap(folder, cap),
    }
}

impl crate::app::KtApp {
    /// What the cache folders had to tell since the last frame: one load when there is nothing.
    pub(crate) fn take_cache_notices(&mut self) {
        for n in cache_folder::take_notices() {
            let text = match &n {
                Notice::Full { folder, bytes } => {
                    let of = folder.parent().unwrap_or(folder);
                    crate::tf!(k::msg::CACHE_FOLDER_FULL, of.display(), crate::render_dialog::size_text(*bytes))
                }
                Notice::Elsewhere { folder, why } => crate::tf!(k::msg::CACHE_FOLDER_ELSEWHERE, folder.display(), why),
            };
            self.warn(text);
            if let Some(last) = self.notices.back_mut() {
                last.action = Some(crate::app::NoticeAction::PersistentCache);
            }
        }
    }

    /// Do `f` with a folder on the media lane, then read the list again.
    fn cache_job(&mut self, f: impl FnOnce() + Send + 'static) {
        let (answer, reply) = kt_time::sched::Answer::new();
        crate::app::media_lane().post(move |_| {
            f();
            reply.send(read_rows());
            crate::wake::now();
        });
        self.start.caches.reading = Some(answer);
        #[cfg(test)]
        {
            self.start.caches.reads += 1;
        }
    }

    /// A folder's cap, set by the person.
    pub(crate) fn set_cache_cap(&mut self, folder: PathBuf, cap: u64) {
        self.cache_job(move || set_cap(&folder, cap));
    }

    /// The window closed: the next showing reads again.
    pub(crate) fn caches_closed(&mut self) {
        self.start.caches.rows = None;
        self.start.caches.reading = None;
    }

    /// The section “Persistent cache”.
    pub(crate) fn draw_cache_section(&mut self, ui: &mut egui::Ui) {
        if let Some(rows) = self.start.caches.reading.as_ref().and_then(|a| a.take_within(std::time::Duration::ZERO)) {
            self.start.caches.reading = None;
            self.start.caches.rows = Some(rows);
        }
        if self.start.caches.rows.is_none() && self.start.caches.reading.is_none() {
            self.cache_job(|| {});
        }
        ui.add(egui::Label::new(tr(k::prefs::CACHE_ABOUT)).wrap());
        let Some(rows) = self.start.caches.rows.clone() else { return };
        if rows.is_empty() {
            ui.weak(tr(k::prefs::CACHE_NONE));
            return;
        }
        let mut deleted = None;
        for (i, row) in rows.iter().enumerate() {
            ui.add_space(4.0);
            let path = row.folder.display().to_string();
            ui.add(egui::Label::new(egui::RichText::new(&path).small()).wrap());
            ui.horizontal_wrapped(|ui| {
                if row.orphan {
                    ui.colored_label(crate::app::HINT_ERROR, tr(k::prefs::CACHE_NO_PROJECT));
                } else if row.projects.is_empty() {
                    ui.weak(tr(k::prefs::CACHE_UNTITLED));
                } else {
                    let names: Vec<String> =
                        row.projects.iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect();
                    ui.weak(names.join(", "));
                }
                let size = crate::render_dialog::size_text(row.bytes);
                ui.label(if row.full { crate::tf!(k::prefs::CACHE_FULL, size) } else { size });
            });
            match &row.place {
                Some(Place::Elsewhere(why)) => {
                    ui.add(egui::Label::new(crate::tf!(k::prefs::CACHE_ELSEWHERE, why)).wrap());
                }
                Some(Place::Synced) => {
                    ui.weak(tr(k::prefs::CACHE_SYNCED));
                }
                None => {}
            }
            ui.horizontal_wrapped(|ui| {
                let mut gb = row.cap as f64 / 1e9;
                let num = crate::widgets::NumRow {
                    label: tr(k::prefs::CACHE_CAP),
                    probe: &format!("prefs.cache.{i}.cap"),
                    unit: kt_model::ParamUnit::None,
                    min: 1.0,
                    max: 1000.0,
                    step: 1.0,
                    slider: None,
                };
                let r = crate::widgets::plain_row(ui, &num, &mut gb);
                let r = self.hint(r, tr(k::prefs::HINT_CACHE_CAP));
                if r.changed()
                    && let Some(m) = self.start.caches.rows.as_mut().and_then(|v| v.get_mut(i))
                {
                    m.cap = (gb * 1e9) as u64;
                }
                // saved when the edit is done, not at every step of a drag
                if r.drag_stopped() || r.lost_focus() {
                    self.set_cache_cap(row.folder.clone(), (gb * 1e9) as u64);
                }
                let r = ui.button(tr(k::prefs::CACHE_DELETE));
                crate::widgets::field_probe::note(&format!("prefs.cache.{i}.delete"), r.rect);
                let r = self.hint(r, tr(k::prefs::HINT_CACHE_DELETE));
                if r.clicked() {
                    deleted = Some(row.folder.clone());
                }
            });
        }
        ui.add_space(4.0);
        let total: u64 = rows.iter().map(|r| r.bytes).sum();
        ui.label(crate::tf!(k::prefs::CACHE_TOTAL, crate::render_dialog::size_text(total)));
        if let Some(folder) = deleted {
            self.cache_job(move || delete(&folder));
        }
    }
}

#[cfg(test)]
impl CachesUi {
    pub fn rows_for_tests(&self) -> Vec<CacheRow> {
        self.rows.clone().unwrap_or_default()
    }
}
