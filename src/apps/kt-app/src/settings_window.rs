//! Окно «Настройки проекта» (Р.62): растр, частота кадров, частота аудио,
//! правило приведения по умолчанию, движок питча, форма ключей.
//!
//! Одно тело с комбо панели превью: обе двери шлют `Command::SetSettings`
//! (правило 17). Применение ставит `settings_explicit` — с этого момента
//! первый клип в пустой проект настройки не трогает (правило 20).
//! Невозможное значение (нечётный растр, 29.97) — отказ с причиной, а не
//! молчание (правило 16).

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{Command, ConformRule, PitchEngine, SegShape};

/// Делители 90 000 в пределах 1…120 — частоты, которые сетка тиков
/// выражает точно. 29.97 сюда не попадает по построению.
pub const FPS_CHOICES: [u32; 12] = [12, 15, 18, 20, 24, 25, 30, 36, 45, 48, 50, 60];

pub struct SettingsWindow {
    /// `Some` — окно правит ЛЕНТУ (рабочий вид внутри неё): только растр и
    /// fps, каждый с галкой «как в проекте» (Р.62).
    pub reel: Option<kt_model::MediaId>,
    pub inherit_size: bool,
    pub inherit_fps: bool,
    pub width: String,
    pub height: String,
    pub fps: String,
    pub conform: ConformRule,
    pub pitch: PitchEngine,
    pub shape: SegShape,
    pub error: Option<String>,
    pub focus: bool,
}

/// Растр: оба числа чётные и в пределах 16…8192.
pub fn parse_size(w: &str, h: &str) -> Result<(u32, u32), String> {
    let one = |s: &str| -> Option<u32> {
        let v: u32 = s.trim().parse().ok()?;
        ((16..=8192).contains(&v) && v.is_multiple_of(2)).then_some(v)
    };
    match (one(w), one(h)) {
        (Some(w), Some(h)) => Ok((w, h)),
        _ => Err(tr(k::settings::BAD_SIZE).to_string()),
    }
}

/// Частота кадров: целый делитель 90 000 в пределах 1…120. «29.97» —
/// не целое, «7» — не делитель; обе — отказ с причиной.
pub fn parse_fps(s: &str) -> Result<u32, String> {
    let v: f64 = s.trim().parse().map_err(|_| tr(k::settings::BAD_FPS).to_string())?;
    let n = v.round();
    if (v - n).abs() > 1e-9 || !(1.0..=120.0).contains(&n) || 90_000 % (n as i64) != 0 {
        return Err(tr(k::settings::BAD_FPS).to_string());
    }
    Ok(n as u32)
}

fn shape_label(s: &SegShape) -> &'static str {
    match s {
        SegShape::Linear => tr(k::fx::SHAPE_LINEAR),
        SegShape::Smooth { .. } => tr(k::fx::SHAPE_SMOOTH),
        SegShape::Hold => tr(k::fx::SHAPE_HOLD),
        SegShape::EaseIn { .. } => tr(k::fx::SHAPE_EASE_IN),
        SegShape::EaseOut { .. } => tr(k::fx::SHAPE_EASE_OUT),
        _ => tr(k::fx::SHAPE_ZIGZAG),
    }
}

/// Имя щупа правила для живых сценариев (`settings.conform.<rule>`).
fn rule_probe(r: ConformRule) -> &'static str {
    match r {
        ConformRule::Fit => "settings.conform.fit",
        ConformRule::Fill => "settings.conform.fill",
        ConformRule::Stretch => "settings.conform.stretch",
        ConformRule::AsIs => "settings.conform.as_is",
    }
}

pub fn rule_label(r: ConformRule) -> &'static str {
    match r {
        ConformRule::Fit => tr(k::fx::CONFORM_FIT),
        ConformRule::Fill => tr(k::fx::CONFORM_FILL),
        ConformRule::Stretch => tr(k::fx::CONFORM_STRETCH),
        ConformRule::AsIs => tr(k::fx::CONFORM_AS_IS),
    }
}

impl KtApp {
    /// Открыть окно на текущих настройках ДОКУМЕНТА (настройки — свойство
    /// документа, а не раскрытой ленты).
    pub fn open_project_settings(&mut self) {
        let s = self.model.document().settings.clone();
        // внутри ленты та же дверь правит ЛЕНТУ: у неё свои растр и fps
        let reel = self.model.editing();
        let seq = reel.and_then(|id| self.model.document().sequence(id).cloned());
        let (inherit_size, inherit_fps, w, h, fps) = match seq {
            Some(seq) => (
                seq.raster.is_none(),
                seq.fps.is_none(),
                seq.raster.map(|r| r.0).unwrap_or(s.width),
                seq.raster.map(|r| r.1).unwrap_or(s.height),
                seq.fps.unwrap_or(s.fps),
            ),
            None => (false, false, s.width, s.height, s.fps),
        };
        self.settings_win = Some(SettingsWindow {
            reel,
            inherit_size,
            inherit_fps,
            width: w.to_string(),
            height: h.to_string(),
            fps: fps.to_string(),
            conform: s.conform,
            pitch: s.pitch_engine,
            shape: s.default_shape,
            error: None,
            focus: true,
        });
    }

    pub(crate) fn draw_settings_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.settings_win.take() else { return };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let cur = self.model.document().settings.clone();
        let reel_name =
            d.reel.and_then(|id| self.model.document().sequence(id).map(|s| s.name.clone()));
        let title = match &reel_name {
            Some(n) => crate::tf!(k::settings::REEL_TITLE, n),
            None => tr(k::settings::TITLE).to_string(),
        };
        let is_reel = d.reel.is_some();
        egui::Window::new(title)
            .id(egui::Id::new("project_settings"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                crate::widgets::window_body(ui, |ui| {
                // ГРУППЫ по образцу «Трансформации» (Р.66): кадр, звук,
                // кривые. Свёртка — состояние окна, не проекта.
                crate::widgets::group(ui, "settings.frame", tr(k::grp::FRAME), |ui| {
                egui::Grid::new("project_settings_grid").num_columns(2).show(ui, |ui| {
                    ui.label(tr(k::settings::SIZE));
                    ui.horizontal(|ui| {
                        if is_reel {
                            ui.checkbox(&mut d.inherit_size, tr(k::settings::INHERIT));
                        }
                        let rw = ui.add_enabled(
                            !(is_reel && d.inherit_size),
                            egui::TextEdit::singleline(&mut d.width)
                                .id(egui::Id::new("settings_w"))
                                .desired_width(56.0),
                        );
                        crate::widgets::field_probe::note("settings.width", rw.rect);
                        if d.focus {
                            rw.request_focus();
                            d.focus = false;
                        }
                        ui.label("×");
                        let rh = ui.add_enabled(
                            !(is_reel && d.inherit_size),
                            egui::TextEdit::singleline(&mut d.height)
                                .id(egui::Id::new("settings_h"))
                                .desired_width(56.0),
                        );
                        crate::widgets::field_probe::note("settings.height", rh.rect);
                    });
                    ui.end_row();
                    ui.label(tr(k::settings::FPS));
                    ui.horizontal(|ui| {
                        if is_reel {
                            ui.checkbox(&mut d.inherit_fps, tr(k::settings::INHERIT));
                        }
                        let rf = ui.add_enabled(
                            !(is_reel && d.inherit_fps),
                            egui::TextEdit::singleline(&mut d.fps)
                                .id(egui::Id::new("settings_fps"))
                                .desired_width(48.0),
                        );
                        crate::widgets::field_probe::note("settings.fps", rf.rect);
                        for f in FPS_CHOICES {
                            if ui
                                .selectable_label(d.fps.trim() == f.to_string(), f.to_string())
                                .clicked()
                            {
                                d.fps = f.to_string();
                            }
                        }
                    });
                    ui.end_row();
                    if is_reel {
                        return;
                    }
                    ui.label(tr(k::settings::CONFORM));
                    ui.horizontal(|ui| {
                        for r in [
                            ConformRule::Fit,
                            ConformRule::Fill,
                            ConformRule::Stretch,
                            ConformRule::AsIs,
                        ] {
                            let resp = ui.selectable_value(&mut d.conform, r, rule_label(r));
                            crate::widgets::field_probe::note(rule_probe(r), resp.rect);
                        }
                    });
                    ui.end_row();
                    ui.end_row();
                });
                });
                crate::widgets::group(ui, "settings.audio", tr(k::grp::AUDIO), |ui| {
                egui::Grid::new("project_settings_audio").num_columns(2).show(ui, |ui| {
                    ui.label(tr(k::settings::SAMPLE_RATE));
                    ui.label(format!("{} Hz", cur.sample_rate));
                    ui.end_row();
                    ui.label(tr(k::settings::PITCH));
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut d.pitch,
                            PitchEngine::Wsola,
                            tr(k::fx::PITCH_WSOLA),
                        );
                        ui.selectable_value(
                            &mut d.pitch,
                            PitchEngine::Vocoder,
                            tr(k::fx::PITCH_VOCODER),
                        );
                    });
                    ui.end_row();
                    ui.end_row();
                });
                });
                crate::widgets::group(ui, "settings.curves", tr(k::grp::CURVES), |ui| {
                egui::Grid::new("project_settings_curves").num_columns(2).show(ui, |ui| {
                    ui.label(tr(k::settings::SHAPE));
                    ui.horizontal(|ui| {
                        for s in [
                            SegShape::Linear,
                            SegShape::DEFAULT,
                            SegShape::Hold,
                            SegShape::EaseIn { tension: 1.0 },
                            SegShape::EaseOut { tension: 1.0 },
                        ] {
                            let same =
                                std::mem::discriminant(&d.shape) == std::mem::discriminant(&s);
                            if ui.selectable_label(same, shape_label(&s)).clicked() {
                                d.shape = s;
                            }
                        }
                    });
                    ui.end_row();
                });
                });
                if let Some(e) = &d.error {
                    ui.colored_label(crate::app::HINT_ERROR, e);
                }
                ui.label(
                    egui::RichText::new(if is_reel {
                        tr(k::settings::REEL_NOTE)
                    } else if cur.settings_explicit {
                        tr(k::settings::EXPLICIT_NOTE)
                    } else {
                        tr(k::settings::ADOPT_NOTE)
                    })
                    .weak()
                    .small(),
                );
                });
                crate::widgets::window_actions(ui, |ui| {
                ui.horizontal(|ui| {
                    let ra = ui.button(tr(k::settings::APPLY));
                    crate::widgets::field_probe::note("settings.apply", ra.rect);
                    if ra.clicked() {
                        apply = true;
                    }
                    if ui.button(tr(k::settings::CANCEL)).clicked() {
                        cancel = true;
                    }
                });
                });
            });
        if apply && let Some(id) = d.reel {
            let raster =
                if d.inherit_size { Ok(None) } else { parse_size(&d.width, &d.height).map(Some) };
            let fps = if d.inherit_fps { Ok(None) } else { parse_fps(&d.fps).map(Some) };
            match (raster, fps) {
                (Ok(raster), Ok(fps)) => {
                    self.apply(Command::SetSequenceSettings { id, raster, fps });
                    open = false;
                }
                (r, f) => {
                    let e = r.err().or(f.err()).unwrap_or_default();
                    self.refuse(e.clone());
                    d.error = Some(e);
                }
            }
        } else if apply {
            match (parse_size(&d.width, &d.height), parse_fps(&d.fps)) {
                (Ok((w, h)), Ok(fps)) => {
                    let mut st = cur.clone();
                    st.width = w;
                    st.height = h;
                    st.fps = fps;
                    st.conform = d.conform;
                    st.pitch_engine = d.pitch;
                    st.default_shape = d.shape;
                    st.settings_explicit = true;
                    if st != cur {
                        // доля превью сохраняется, а не пиксели (п.20 р.7)
                        let pq = self.player.preview_quality();
                        let frac = pq.width as f32 / cur.width.max(1) as f32;
                        self.apply(Command::SetSettings { settings: st });
                        self.player
                            .set_preview_quality(crate::app::preview_quality_for(w, h, frac));
                    } else if !cur.settings_explicit {
                        let mut st = cur.clone();
                        st.settings_explicit = true;
                        self.apply(Command::SetSettings { settings: st });
                    }
                    open = false;
                }
                (size, fps) => {
                    let e = size.err().or(fps.err()).unwrap_or_default();
                    self.refuse(e.clone());
                    d.error = Some(e);
                }
            }
        }
        if cancel {
            open = false;
        }
        if open {
            self.settings_win = Some(d);
        } else {
            // всё, что берёт фокус, обязано его ОТДАВАТЬ (правило 14)
            for id in ["settings_w", "settings_h", "settings_fps"] {
                ctx.memory_mut(|m| m.surrender_focus(egui::Id::new(id)));
            }
        }
    }
}
