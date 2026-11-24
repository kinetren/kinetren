//! The persistent cache of the preview, on the interface's side (persistent-cache, rows A10, B1, D1): the
//! switch beside the preview quality, the copy's mark on a bin card, "Use a camera's copy…", and the two
//! rows of the Diagnostics window. The folders themselves are the Preferences' (`cache_prefs`).

use crate::app::KtApp;
use crate::i18n::{k, tr};
use crate::render_dialog::size_text;
use eframe::egui;
use kt_engine::copies::{Card, Config};
use kt_media::cache_folder::{Folder, Tenant};
use kt_media::copy::{self, CopyMaker, CopyMode, Mismatch};
use kt_model::MediaId;
use std::path::PathBuf;
use std::sync::Arc;

const MODES: [CopyMode; 4] = [CopyMode::AsPreview, CopyMode::Half, CopyMode::Quarter, CopyMode::Off];

/// The mode as the start record keeps it.
fn code(m: CopyMode) -> &'static str {
    match m {
        CopyMode::AsPreview => "preview",
        CopyMode::Half => "half",
        CopyMode::Quarter => "quarter",
        CopyMode::Off => "off",
    }
}

fn mode_name(m: CopyMode) -> &'static str {
    match m {
        CopyMode::AsPreview => tr(k::pcache::AS_PREVIEW),
        CopyMode::Half => "1/2",
        CopyMode::Quarter => "1/4",
        CopyMode::Off => tr(k::pcache::OFF),
    }
}

/// What a kind of the store holds and did in this process.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Figures {
    pub bytes: u64,
    pub reads: u64,
    pub writes: u64,
}

enum Bound {
    Yes,
    No(Mismatch),
    Failed(String),
}

#[derive(Default)]
pub struct CopiesUi {
    /// The marks as laid out for the cards' state (`Copies::stamp`).
    marks: std::collections::HashMap<MediaId, (Card, Arc<egui::Galley>)>,
    marks_at: Option<u64>,
    /// A camera's copy being checked: the card it is for.
    binding: Option<(MediaId, kt_time::sched::Answer<Bound>)>,
    /// Frames and copies, as last sampled for the Diagnostics window.
    pub figures: [Figures; 2],
}

impl KtApp {
    /// Which copies the preview plays from: the person's choice, kept between projects.
    pub(crate) fn cache_mode(&self) -> CopyMode {
        let kept = self.start.store.start.persistent_cache.as_deref();
        MODES.into_iter().find(|m| Some(code(*m)) == kept).unwrap_or_default()
    }

    /// Keep the preview's frames and the copies of heavy sources in the project's cache folder, as the
    /// switch says. "Off" reads and writes neither; a copy that plays gives its place to the original.
    pub(crate) fn set_persistent_cache(&self) {
        let mode = self.cache_mode();
        let root = self.cache_folder();
        let frames = root.clone().filter(|_| mode != CopyMode::Off);
        self.player.set_render_cache(frames.map(|root| kt_engine::render_cache::Config {
            codec: Arc::new(kt_media::segment::ffmpeg::FfmpegSegments::from_env()),
            root,
            made: kt_engine::render_cache::Made::of(kt_media::ffmpeg::backend_info().map_or(0, |b| b.avcodec_version.0)),
            cap: kt_engine::render_cache::cap_from_env(),
        }));
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(PathBuf::from));
        self.player.render_cache().set_notify(crate::wake::soon);
        let copies = self.player.copies();
        copies.set_mode(mode);
        copies.set_notify(crate::wake::soon);
        copies.set_config(root.map(|root| Config {
            maker: Arc::new(copy::ffmpeg::FfmpegCopies::from_env()),
            root,
            resolve: Arc::new(move |p| {
                let p = PathBuf::from(crate::util::resolve_media_path(base.as_deref(), p));
                p.is_file().then_some(p)
            }),
        }));
    }

    /// The switch beside the preview quality.
    pub(crate) fn draw_cache_switch(&mut self, ui: &mut egui::Ui) {
        let was = self.cache_mode();
        let mut mode = was;
        // the disk and the value, at every width of the window (t790); the hint and the list say the name
        let text = format!("{} {}", crate::icons::DISK, mode_name(mode));
        let shown = egui::ComboBox::from_id_salt("persistent_cache").selected_text(text).show_ui(ui, |ui| {
            ui.weak(tr(k::pcache::TITLE));
            for m in MODES {
                let r = ui.selectable_value(&mut mode, m, mode_name(m));
                crate::widgets::field_probe::note(
                    match m {
                        CopyMode::AsPreview => "pcache.preview",
                        CopyMode::Half => "pcache.half",
                        CopyMode::Quarter => "pcache.quarter",
                        CopyMode::Off => "pcache.off",
                    },
                    r.rect,
                );
            }
        });
        crate::widgets::field_probe::note("pcache.switch", shown.response.rect);
        self.hint(shown.response, tr(k::pcache::HINT));
        if mode != was {
            self.start.store.set_persistent_cache(code(mode));
            crate::tele_journal::record(crate::tele_journal::Kind::Act, &format!("persistent cache: {}", code(mode)));
            // the harness sets a store of its own: its "Off" drops it
            if cfg!(test) {
                self.player.copies().set_mode(mode);
                if mode == CopyMode::Off {
                    self.player.set_render_cache(None);
                }
            } else {
                self.set_persistent_cache();
            }
        }
    }

    /// The mark of a card's copy, in its corner; the text a hover adds.
    pub(crate) fn draw_copy_mark(&mut self, p: &egui::Painter, thumb: egui::Rect, id: MediaId) {
        let copies = self.player.copies();
        let at = copies.stamp();
        if self.copies_ui.marks_at != Some(at) {
            self.copies_ui.marks_at = Some(at);
            self.copies_ui.marks.clear();
            for m in self.model.doc().media.iter() {
                let Some(c) = copies.card(m.id) else { continue };
                let text = match &c {
                    Card::Making(a, b) => format!("{} {} %", crate::icons::DISK, a * 100 / (*b).max(1)),
                    Card::Copy(_, h) => format!("{} {h}", crate::icons::DISK),
                    Card::Camera => format!("{} {}", crate::icons::DISK, crate::icons::CAMERA),
                    Card::Failed(_) => format!("{} {}", crate::icons::DISK, crate::icons::WARN),
                };
                let g = p.layout_no_wrap(text, egui::FontId::proportional(11.0), egui::Color32::from_gray(230));
                self.copies_ui.marks.insert(m.id, (c, g));
            }
        }
        let Some((_, g)) = self.copies_ui.marks.get(&id) else { return };
        let at = thumb.right_top() + egui::vec2(-4.0 - g.size().x, 2.0);
        let back = egui::Rect::from_min_size(at, g.size()).expand2(egui::vec2(3.0, 1.0));
        p.rect_filled(back, 2.0, egui::Color32::from_black_alpha(170));
        p.galley(at, g.clone(), egui::Color32::from_gray(230));
        crate::widgets::field_probe::note("bin.copy", back);
    }

    /// What the card's hint says of its copy.
    pub(crate) fn copy_hint(&self, id: MediaId) -> Option<String> {
        Some(match &self.copies_ui.marks.get(&id)?.0 {
            Card::Making(a, b) => crate::tf!(k::pcache::MAKING, a, b),
            Card::Copy(w, h) => crate::tf!(k::pcache::COPY, w, h),
            Card::Camera => tr(k::pcache::CAMERA).to_string(),
            Card::Failed(why) => crate::tf!(k::pcache::FAILED, why),
        })
    }

    /// The card's items: a camera's own copy in place of one of ours, and back.
    pub(crate) fn copy_menu(&mut self, ui: &mut egui::Ui, id: MediaId) {
        let camera = matches!(self.copies_ui.marks.get(&id), Some((Card::Camera, _)));
        let r = ui.button(tr(k::pcache::USE_CAMERA));
        crate::widgets::field_probe::note("bin.copy.camera", r.rect);
        if self.hint(r, tr(k::pcache::USE_CAMERA_HINT)).clicked() {
            ui.close();
            if let Some(p) = crate::app::pick_file(tr(k::pcache::USE_CAMERA), "Video", &["mp4", "mov", "lrv", "lrf", "mxf", "mkv"]) {
                self.bind_camera(id, Some(p));
            }
        }
        if camera && ui.button(tr(k::pcache::DROP_CAMERA)).clicked() {
            ui.close();
            self.bind_camera(id, None);
        }
    }

    /// Check and bind a camera's copy on the media lane (`None` — unbind): both files are read whole.
    pub(crate) fn bind_camera(&mut self, id: MediaId, camera: Option<PathBuf>) {
        let Some(root) = self.cache_folder() else { return };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(PathBuf::from));
        let Some(path) = self.model.doc().media.iter().find(|m| m.id == id).map(|m| m.path.clone()) else { return };
        let path = PathBuf::from(crate::util::resolve_media_path(base.as_deref(), &path));
        let (answer, reply) = kt_time::sched::Answer::new();
        crate::app::media_lane().post(move |_| {
            let maker = copy::ffmpeg::FfmpegCopies::from_env();
            let done = (|| {
                let folder = Folder::open(&root).map_err(|e| Bound::Failed(e.to_string()))?;
                let f = maker.facts(&folder, &path).ok_or_else(|| Bound::Failed(path.display().to_string()))?;
                match &camera {
                    Some(c) => {
                        maker.check_camera(&path, c).map_err(Bound::No)?;
                        copy::bind_camera(&folder, f.id, c).map_err(|e| Bound::Failed(e.to_string()))
                    }
                    None => {
                        copy::unbind_camera(&folder, f.id);
                        Ok(())
                    }
                }
            })();
            reply.send(done.err().unwrap_or(Bound::Yes));
            crate::wake::now();
        });
        self.copies_ui.binding = Some((id, answer));
    }

    /// A camera's copy that was checked: it plays, or the card says what differs.
    pub(crate) fn take_camera(&mut self) {
        let grace = std::time::Duration::ZERO;
        let Some(done) = self.copies_ui.binding.as_ref().and_then(|(_, a)| a.take_within(grace)) else { return };
        self.copies_ui.binding = None;
        let secs = |v: f64| format!("{v:.2}");
        match done {
            Bound::Yes => self.player.copies().look_again(),
            Bound::No(Mismatch::Duration(a, b)) => self.refuse(crate::tf!(k::pcache::DIFFERS_DURATION, secs(a), secs(b))),
            Bound::No(Mismatch::Frames(a, b)) => self.refuse(crate::tf!(k::pcache::DIFFERS_FRAMES, a, b)),
            Bound::No(Mismatch::Audio(a, b)) => self.refuse(crate::tf!(k::pcache::DIFFERS_SOUND, secs(a), secs(b))),
            Bound::No(Mismatch::Unreadable(why)) | Bound::Failed(why) => self.refuse(crate::tf!(k::pcache::UNREADABLE, why)),
        }
    }

    /// The store's two kinds, sampled with the other figures of the Diagnostics window: what was last
    /// counted, the disk is not touched.
    pub(crate) fn sample_copies(&mut self) {
        let folder = self.cache_folder().and_then(|r| Folder::peek(&r));
        let sizes = folder.as_ref().map(|f| f.known_sizes()).unwrap_or_default();
        let bytes = |t| sizes.get(&t).map_or(0, |v| v.0);
        let (writes, reads) = self.player.render_cache().counts();
        self.copies_ui.figures = [
            Figures { bytes: bytes(Tenant::Frames), reads, writes },
            Figures {
                bytes: bytes(Tenant::Copy),
                reads: folder.as_ref().map_or(0, |f| f.counts(Tenant::Copy).0),
                writes: self.player.copies().made(),
            },
        ];
    }

    /// The persistent cache in the Diagnostics window: the two kinds the store has, in one row, with a
    /// store to speak of; under it, why it writes nothing when it does not.
    pub(crate) fn draw_cache_figures(&mut self, ui: &mut egui::Ui) {
        let status = self.player.render_cache().status();
        let [frames, copies] = self.copies_ui.figures;
        if status.is_none() && self.copies_ui.figures == [Figures::default(); 2] {
            return;
        }
        let mut shown = Vec::with_capacity(2);
        ui.columns(2, |cols| {
            for (c, (key, f, probe)) in [(k::pcache::FRAMES, frames, "diag.pcache.frames"), (k::pcache::COPIES, copies, "diag.pcache.copies")].into_iter().enumerate() {
                cols[c].style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                let r = cols[c].weak(crate::tf!(key, size_text(f.bytes), f.reads, f.writes));
                crate::widgets::field_probe::note(probe, r.rect);
                shown.push(r);
            }
        });
        let on = match &status {
            Some(Ok(enc)) if !enc.is_empty() => format!(" {}", crate::tf!(k::perf::RENDER_CACHE_ON, enc)),
            _ => String::new(),
        };
        for r in shown {
            self.hint(r, &format!("{}: {}.{on}", tr(k::pcache::TITLE), mode_name(self.cache_mode())));
        }
        if let Some(Err(why)) = status {
            ui.weak(crate::tf!(k::perf::RENDER_CACHE_OFF, why));
        }
    }
}
