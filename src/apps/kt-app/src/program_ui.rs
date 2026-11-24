//! The program stage under the meters and "Match loudness…" (loudness-norms): the project's loudness
//! target is in Project settings; here the person matches the mix, or the chosen events, to it.

use crate::analysis::{AnalysisKind, AnalysisOut};
use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{ClipId, ClipKind, Command, LoudnessTarget, ProgramStage, Project};
use std::path::Path;

const AMBER: egui::Color32 = egui::Color32::from_rgb(255, 190, 120);
const LIT: egui::Color32 = egui::Color32::from_rgb(255, 120, 60);
/// The row under the meters.
pub(crate) const PROGRAM_H: f32 = 22.0;
/// The stage's light stays lit this long after the limiter acted.
const LIGHT_MS: u128 = 400;

/// What "Match loudness…" asks.
pub struct MatchWin {
    pub ids: Vec<ClipId>,
    /// 0 — dialogue (the project's target), 1 — music, 2 — effects, 3 — a number.
    pub pick: usize,
    pub number: f64,
}

/// LU under the project's target for music and for effects.
const MUSIC_UNDER: f64 = 8.0;
const EFFECTS_UNDER: f64 = 4.0;

impl MatchWin {
    pub fn target(&self, t: &LoudnessTarget) -> f64 {
        match self.pick {
            0 => t.target_lufs,
            1 => t.target_lufs - MUSIC_UNDER,
            2 => t.target_lufs - EFFECTS_UNDER,
            _ => self.number,
        }
    }
}

/// The sources the sound of `p` needs: no picture is opened. The names of those that did not open.
fn open_heard(p: &Project, base: Option<&Path>) -> (Vec<(kt_model::MediaId, Box<dyn kt_media::MediaSource>)>, Vec<String>) {
    let mut want = std::collections::HashSet::new();
    let mut reels = vec![None];
    let mut seen = std::collections::HashSet::new();
    while let Some(r) = reels.pop() {
        for c in p.resolve_reel(r, &[]).clips.iter().filter(|c| c.kind == ClipKind::Audio) {
            want.insert(c.media);
            if p.sequence(c.media).is_some() && seen.insert(c.media) {
                reels.push(Some(c.media));
            }
        }
    }
    let mut q = Project::new(p.settings.clone());
    q.media = p.media.iter().filter(|m| want.contains(&m.id)).cloned().collect();
    crate::render_dialog::open_all(&q, base)
}

/// How the stage stands against what was measured.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Stand {
    /// No measurement yet.
    Unmeasured,
    Measured,
    /// The sound, the target or the gain is not what was measured.
    Changed,
}

pub(crate) fn stand(g: &ProgramStage, target: &LoudnessTarget, sound: Option<u64>) -> Stand {
    let Some(m) = &g.measured else { return Stand::Unmeasured };
    // the builder has not landed yet: not known is not changed
    let heard = sound.is_none_or(|s| s == m.sound);
    if heard && m.gain_db == g.gain_db && m.target == target.target_lufs && m.ceiling == target.ceiling_dbtp {
        Stand::Measured
    } else {
        Stand::Changed
    }
}

/// How far the limiter holds the program under its target; `None` — it reaches it.
pub(crate) fn held_back(g: &ProgramStage) -> Option<f64> {
    g.measured.map(|m| m.target - m.reached).filter(|d| *d > 0.3)
}

impl KtApp {
    /// What is heard now, as the builder counted it (`None` — not yet, or no stage).
    fn heard_now(&self) -> Option<u64> {
        self.player.built().sound
    }

    /// The row under the meters: the stage's gain, its light, a mark when it was measured on another
    /// sound. The document's: inside a composition there is none.
    pub(crate) fn draw_program_row(&mut self, ui: &mut egui::Ui) {
        if self.model.editing().is_some() {
            return;
        }
        let (stage, target) = (self.model.doc().program, self.model.doc().loudness);
        let cut = self.player.shared.program_cut.swap(0, std::sync::atomic::Ordering::Relaxed);
        if cut > 5 {
            self.program_lit = Some(std::time::Instant::now());
        }
        let lit = self.program_lit.is_some_and(|t| t.elapsed().as_millis() < LIGHT_MS);
        if lit {
            // the light goes out after a pause too, once
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(LIGHT_MS as u64));
        }
        let changed = stage.is_some_and(|g| stand(&g, &target, self.heard_now()) == Stand::Changed);
        let text = match &stage {
            None => tr(k::loudness::ROW_OFF).to_string(),
            Some(g) => format!("{:+.1} dB", g.gain_db),
        };
        let text = egui::RichText::new(text).small().color(if lit {
            LIT
        } else if changed {
            AMBER
        } else {
            egui::Color32::from_gray(if stage.is_some() { 210 } else { 130 })
        });
        let r = ui.add_sized(ui.available_size(), egui::Button::new(text));
        crate::widgets::field_probe::note("program.row", r.rect);
        if changed {
            crate::widgets::field_probe::note("program.changed", r.rect);
        }
        let hint = if changed { tr(k::loudness::CHANGED) } else { tr(k::loudness::ROW_HINT) };
        let r = self.hint(r, hint);
        if r.clicked() {
            self.program_win = !self.program_win;
        }
    }

    pub(crate) fn draw_program_window(&mut self, ctx: &egui::Context) {
        if !self.program_win || self.model.editing().is_some() {
            self.program_win = false;
            return;
        }
        let (stage, target) = (self.model.doc().program, self.model.doc().loudness);
        let mut on = stage.is_some();
        let mut g = stage.unwrap_or_default();
        let (mut edit, mut run, mut settings, mut close) = (false, false, false, false);
        let stand = stand(&g, &target, self.heard_now());
        let busy = self.analysis.current() == Some(AnalysisKind::Program) || self.analysis.waits(AnalysisKind::Program);
        let win = crate::window_shell::Shell::new(
            egui::Id::new("kt_program"),
            tr(k::loudness::PROGRAM_TITLE),
            crate::window_shell::Kind::Tool,
        )
        .width(360.0, [300.0, 200.0])
        .show(ctx, |ui| {
            crate::widgets::window_body(ui, |ui| {
                let r = ui.checkbox(&mut on, tr(k::loudness::STAGE_ON));
                crate::widgets::field_probe::note("program.on", r.rect);
                ui.horizontal(|ui| {
                    ui.label(crate::tf!(
                        k::loudness::TARGET_IS,
                        format!("{:.1}", target.target_lufs),
                        format!("{:.1}", target.ceiling_dbtp)
                    ));
                    let r = ui.button(tr(k::loudness::TO_SETTINGS));
                    crate::widgets::field_probe::note("program.settings", r.rect);
                    settings = r.clicked();
                });
                if on {
                    ui.horizontal(|ui| {
                        let row = crate::widgets::NumRow {
                            label: tr(k::loudness::GAIN),
                            probe: "program.gain",
                            unit: kt_model::ParamUnit::Db,
                            min: *kt_model::PROGRAM_GAIN_DB.start(),
                            max: *kt_model::PROGRAM_GAIN_DB.end(),
                            step: 0.1,
                            slider: None,
                        };
                        edit = crate::widgets::plain_row(ui, &row, &mut g.gain_db).changed();
                    });
                    if let Some(m) = &g.measured {
                        ui.label(crate::tf!(
                            k::loudness::MEASURED,
                            format!("{:.1}", m.lufs),
                            format!("{:.1}", m.dbtp),
                            format!("{:.1}", m.reached)
                        ));
                    }
                    match stand {
                        Stand::Changed => {
                            let r = ui.colored_label(AMBER, tr(k::loudness::CHANGED));
                            crate::widgets::field_probe::note("program.win_changed", r.rect);
                        }
                        Stand::Measured => {
                            if let Some(d) = held_back(&g) {
                                ui.colored_label(AMBER, crate::tf!(k::loudness::HELD_BACK, format!("{d:.1}")));
                            }
                        }
                        Stand::Unmeasured => {}
                    }
                }
                ui.label(egui::RichText::new(tr(k::loudness::PROGRAM_NOTE)).weak().small());
            });
            crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    let r = ui.add_enabled(!busy, egui::Button::new(tr(k::loudness::MATCH_PROGRAM)));
                    crate::widgets::field_probe::note("program.match", r.rect);
                    run = r.clicked();
                    close = ui.button(tr(k::track::CLOSE)).clicked();
                });
            });
        });
        if on != stage.is_some() {
            self.apply(Command::SetProgram { stage: on.then_some(g) });
        } else if edit {
            self.apply_coalesced(Command::SetProgram { stage: Some(g) }, self.gesture_counter);
        }
        if run {
            self.match_program();
        }
        if settings {
            self.open_project_settings();
        }
        if close || win.closed() {
            self.program_win = false;
        }
    }

    /// "Match the program": the whole edit is measured in the background; the gain lands when it answers.
    pub(crate) fn match_program(&mut self) {
        let p = self.model.doc_arc();
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        self.analysis.request_with(AnalysisKind::Program, self.model_rev, true, move |ctl, send| {
            let mut missing = Vec::new();
            let mut open = |q: &Project| {
                let (s, m) = open_heard(q, base.as_deref());
                missing = m;
                s
            };
            let cancel = ctl.cancel_flag().clone();
            let r = kt_engine::program::match_program(&p, &mut open, &cancel, &mut |done, all| {
                // a play holds the job here, chunk by chunk (one-scheduler)
                let _ = ctl.should_stop();
                send.send(done as f32 / all.max(1) as f32);
            });
            if ctl.should_stop() {
                return Err("cancelled".into());
            }
            if !missing.is_empty() {
                return Err(missing.join(", "));
            }
            r.map(AnalysisOut::Program).map_err(|e| e.to_string())
        });
    }

    /// "Match loudness…" on events of the reel at hand.
    pub(crate) fn open_match(&mut self, ids: Vec<ClipId>) {
        let ids: Vec<ClipId> = {
            let doc = self.model.project();
            ids.into_iter().filter(|id| doc.clips.iter().any(|c| c.id == *id && c.kind == ClipKind::Audio)).collect()
        };
        if ids.is_empty() {
            self.refuse(tr(k::loudness::NO_EVENTS));
            return;
        }
        let (pick, number) = self.start.store.start.match_pick.unwrap_or((0, -14.0));
        self.match_win = Some(MatchWin { ids, pick: (pick as usize).min(3), number });
    }

    /// The audio events of a track, for its menu.
    pub(crate) fn audio_events_of(&self, lane: u32) -> Vec<ClipId> {
        let p = self.model.project();
        p.clips.iter().filter(|c| c.kind == ClipKind::Audio && c.lane == lane).map(|c| c.id).collect()
    }

    pub(crate) fn draw_match_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.match_win.take() else { return };
        let target = self.model.doc().loudness;
        let (mut go, mut cancel) = (false, false);
        let win = crate::window_shell::Shell::new(
            egui::Id::new("kt_match_loudness"),
            tr(k::loudness::MATCH_TITLE),
            crate::window_shell::Kind::Dialog,
        )
        .width(360.0, [300.0, 160.0])
        .show(ctx, |ui| {
            crate::widgets::window_body(ui, |ui| {
                ui.label(crate::tf!(k::loudness::MATCH_N, w.ids.len()));
                let rows = [
                    (k::loudness::MATCH_DIALOGUE, target.target_lufs, "match.dialogue"),
                    (k::loudness::MATCH_MUSIC, target.target_lufs - MUSIC_UNDER, "match.music"),
                    (k::loudness::MATCH_EFFECTS, target.target_lufs - EFFECTS_UNDER, "match.effects"),
                ];
                for (i, (label, lufs, probe)) in rows.into_iter().enumerate() {
                    let r = ui.radio_value(&mut w.pick, i, crate::tf!(label, format!("{lufs:.1}")));
                    crate::widgets::field_probe::note(probe, r.rect);
                }
                ui.horizontal(|ui| {
                    let r = ui.radio_value(&mut w.pick, 3, tr(k::loudness::MATCH_NUMBER));
                    crate::widgets::field_probe::note("match.number", r.rect);
                    let row = crate::widgets::NumRow {
                        label: "",
                        probe: "match.number_value",
                        unit: kt_model::ParamUnit::None,
                        min: *kt_model::TARGET_LUFS.start(),
                        max: *kt_model::TARGET_LUFS.end(),
                        step: 0.5,
                        slider: None,
                    };
                    if crate::widgets::plain_row(ui, &row, &mut w.number).changed() {
                        w.pick = 3;
                    }
                    ui.label("LUFS");
                });
                ui.label(egui::RichText::new(tr(k::loudness::MATCH_NOTE)).weak().small());
            });
            crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    let r = ui.button(tr(k::loudness::MATCH_GO));
                    crate::widgets::field_probe::note("match.go", r.rect);
                    go = r.clicked();
                    cancel = ui.button(tr(k::settings::CANCEL)).clicked();
                });
            });
        });
        if go {
            self.start.store.set_match_pick(w.pick as u8, w.number);
            self.match_events(&w.ids, w.target(&target));
        } else if !cancel && !win.closed() {
            self.match_win = Some(w);
        }
    }

    /// The chosen events are measured in the background; their levels move when it answers, in one step.
    pub(crate) fn match_events(&mut self, ids: &[ClipId], target: f64) {
        let p = match self.model.editing() {
            Some(_) => std::sync::Arc::new(self.model.reel_document()),
            None => self.model.doc_arc(),
        };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let ids = ids.to_vec();
        let sound = kt_engine::program::sound(&p);
        self.analysis.request_with(AnalysisKind::Clips, self.model_rev, true, move |ctl, send| {
            let mut missing = Vec::new();
            let mut open = |q: &Project| {
                let (s, m) = open_heard(q, base.as_deref());
                missing.extend(m);
                s
            };
            let cancel = ctl.cancel_flag().clone();
            let r = kt_engine::program::match_clips(&p, &ids, target, &mut open, &cancel, &mut |done, all| {
                let _ = ctl.should_stop();
                send.send(done as f32 / all.max(1) as f32);
            });
            if ctl.should_stop() {
                return Err("cancelled".into());
            }
            if !missing.is_empty() {
                missing.dedup();
                return Err(missing.join(", "));
            }
            r.map(|m| AnalysisOut::Clips { matched: m, sound }).map_err(|e| e.to_string())
        });
    }

    /// The program was measured: the stage lands if the sound is still the one measured.
    pub(crate) fn on_program_matched(&mut self, found: Option<ProgramStage>) {
        let Some(g) = found else {
            self.warn(tr(k::loudness::SILENT_PROGRAM));
            return;
        };
        let doc = self.model.doc_arc();
        let target = doc.loudness;
        let m = g.measured.expect("a match is measured");
        if m.sound != kt_engine::program::sound(&doc) || m.target != target.target_lufs || m.ceiling != target.ceiling_dbtp {
            self.warn(tr(k::loudness::SOUND_CHANGED));
            return;
        }
        if self.model.editing().is_some() {
            // the stage is the document's: a command given inside a composition would land there too
            self.warn(tr(k::loudness::SOUND_CHANGED));
            return;
        }
        self.apply(Command::SetProgram { stage: Some(g) });
        self.note(crate::tf!(
            k::loudness::PROGRAM_DONE,
            format!("{:+.1}", g.gain_db),
            format!("{:.1}", m.lufs),
            format!("{:.1}", m.reached)
        ));
    }

    /// The events were measured: their levels move in one undo step.
    pub(crate) fn on_clips_matched(&mut self, m: kt_engine::program::Matched, sound: u64) {
        let now = match self.model.editing() {
            Some(_) => kt_engine::program::sound(&self.model.reel_document()),
            None => kt_engine::program::sound(self.model.doc()),
        };
        if now != sound {
            self.warn(tr(k::loudness::SOUND_CHANGED));
            return;
        }
        let name = |app: &KtApp, id: &ClipId| {
            let p = app.model.project();
            let media = p.clips.iter().find(|c| c.id == *id).map(|c| c.media);
            let path = media.and_then(|m| p.media.iter().find(|x| x.id == m)).map(|m| m.path.clone());
            format!("{} #{}", path.map(|p| crate::util::media_label(&p)).unwrap_or_default(), id.0)
        };
        let list = |app: &KtApp, ids: &[ClipId]| ids.iter().map(|id| name(app, id)).collect::<Vec<_>>().join(", ");
        let (silent, held) = (list(self, &m.silent), list(self, &m.held));
        let n = m.gains.len();
        let cmds: Vec<Command> = m.gains.into_iter().map(|(id, gain_db)| Command::SetGain { id, gain_db }).collect();
        if n > 0 && !self.apply_batch(cmds) {
            return;
        }
        if !m.silent.is_empty() {
            self.warn(crate::tf!(k::loudness::MATCH_SILENT, silent));
        }
        if !m.held.is_empty() {
            self.warn(crate::tf!(k::loudness::MATCH_HELD, held));
        }
        if n > 0 {
            self.note(crate::tf!(k::loudness::MATCH_DONE, n));
        }
    }
}
