//! Стерео peak-метры + мастер-фейдер (порт AudioLevelMeterWidget/
//! PreviewVolumeFader): лестница цветов, peak-hold caps со спадом,
//! фейдер −60..+24 dB, double-click = 0 dB, кнопка M = mute.

use crate::app::KtApp;
use eframe::egui;

pub const METER_MIN_DB: f32 = -48.0;
pub const METER_MAX_DB: f32 = 6.0;
pub const FADER_MIN_DB: f32 = -60.0;
pub const FADER_MAX_DB: f32 = 24.0;
/// Спад бара/капов, dB за тик 25 мс (порт: 2.8 / 0.2).
pub const BAR_FALL_DB: f32 = 2.8;
pub const CAP_FALL_DB: f32 = 0.2;

pub struct MeterState {
    pub bar_db: [f32; 2],
    pub cap_db: [f32; 2],
    pub fader_db: f32,
    pub muted: bool,
    last_tick: std::time::Instant,
    last_sample: std::time::Instant,
}

impl Default for MeterState {
    fn default() -> Self {
        MeterState {
            bar_db: [METER_MIN_DB; 2],
            cap_db: [METER_MIN_DB; 2],
            fader_db: 0.0,
            muted: false,
            last_tick: std::time::Instant::now(),
            last_sample: std::time::Instant::now(),
        }
    }
}

fn db_of(linear: f32) -> f32 {
    if linear <= 1e-6 { METER_MIN_DB } else { (20.0 * linear.log10()).max(METER_MIN_DB) }
}

fn zone_color(db: f32) -> egui::Color32 {
    if db > 0.0 {
        egui::Color32::from_rgb(255, 60, 40)
    } else if db >= -3.0 {
        egui::Color32::from_rgb(235, 80, 60)
    } else if db >= -9.0 {
        egui::Color32::from_rgb(240, 180, 60)
    } else if db >= -18.0 {
        egui::Color32::from_rgb(120, 230, 90)
    } else if db >= -36.0 {
        egui::Color32::from_rgb(70, 190, 80)
    } else {
        egui::Color32::from_rgb(40, 120, 60)
    }
}

impl KtApp {
    /// Панель метров+фейдера справа от превью. Возвращает высоту не более
    /// доступной; ширина ~64 px.
    pub fn draw_meters(&mut self, ui: &mut egui::Ui) {
        // 1) подкачка уровней
        let playing = self.player.transport.is_playing();
        let (pl, pr) = if playing {
            self.player.meters.take()
        } else if self.meter.last_sample.elapsed().as_millis() >= 120 {
            self.meter.last_sample = std::time::Instant::now();
            self.player.sample_levels_at(self.player.transport.playhead())
        } else {
            (0.0, 0.0)
        };
        // 2) спад и peak-hold (тик 25 мс)
        let ticks = (self.meter.last_tick.elapsed().as_millis() / 25).min(40) as f32;
        if ticks >= 1.0 {
            self.meter.last_tick = std::time::Instant::now();
            for ch in 0..2 {
                self.meter.bar_db[ch] =
                    (self.meter.bar_db[ch] - BAR_FALL_DB * ticks).max(METER_MIN_DB);
                self.meter.cap_db[ch] =
                    (self.meter.cap_db[ch] - CAP_FALL_DB * ticks).max(METER_MIN_DB);
            }
        }
        for (ch, lin) in [(0usize, pl), (1, pr)] {
            let db = db_of(lin);
            self.meter.bar_db[ch] = self.meter.bar_db[ch].max(db);
            self.meter.cap_db[ch] = self.meter.cap_db[ch].max(db);
        }

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(64.0, ui.available_height()),
            egui::Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_gray(16));

        // dB-надпись сверху
        let label = if self.meter.muted {
            "MUTE".to_string()
        } else if self.meter.fader_db.abs() < 10.0 {
            format!("{:+.1} dB", self.meter.fader_db)
        } else {
            format!("{:+.0} dB", self.meter.fader_db)
        };
        painter.text(
            egui::pos2(rect.center().x, rect.top() + 4.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(11.0),
            if self.meter.muted {
                egui::Color32::from_rgb(255, 90, 90)
            } else {
                egui::Color32::from_gray(210)
            },
        );

        let bars = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 5.0, rect.top() + 22.0),
            egui::pos2(rect.right() - 16.0, rect.bottom() - 30.0),
        );
        let db_to_y = |db: f32| -> f32 {
            let t = (db - METER_MIN_DB) / (METER_MAX_DB - METER_MIN_DB);
            bars.bottom() - t.clamp(0.0, 1.0) * bars.height()
        };
        // два бара с зазором по центру — фейдер живёт в зазоре (порт старого)
        let gap = 10.0;
        let bar_w = (bars.width() - gap) / 2.0;
        let bar_x = [bars.left(), bars.left() + bar_w + gap];

        // мягкое свечение фона от громкого канала (имитация additive glow)
        let loud_db = self.meter.bar_db[0].max(self.meter.bar_db[1]);
        if loud_db > METER_MIN_DB + 6.0 {
            let k = ((loud_db - METER_MIN_DB) / (METER_MAX_DB - METER_MIN_DB)).clamp(0.0, 1.0);
            let glow_col = zone_color(loud_db);
            painter.rect_filled(
                bars.expand(3.0),
                6.0,
                egui::Color32::from_rgba_unmultiplied(
                    glow_col.r(),
                    glow_col.g(),
                    glow_col.b(),
                    (k * 26.0) as u8,
                ),
            );
        }

        for ch in 0..2 {
            let x0 = bar_x[ch];
            let mut y = bars.bottom();
            while y > bars.top() {
                let seg_top = (y - 5.0).max(bars.top());
                let db_here = METER_MIN_DB
                    + (bars.bottom() - y) / bars.height() * (METER_MAX_DB - METER_MIN_DB);
                let lit = db_here <= self.meter.bar_db[ch];
                let col = if lit { zone_color(db_here) } else { egui::Color32::from_gray(34) };
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0, seg_top),
                        egui::pos2(x0 + bar_w, y - 1.0),
                    ),
                    1.0,
                    col,
                );
                y -= 7.0;
            }
            // peak-hold cap со свечением (два слоя ореола)
            let cap_db = self.meter.cap_db[ch];
            if cap_db > METER_MIN_DB + 0.5 {
                let cap_y = db_to_y(cap_db);
                let cap_col = zone_color(cap_db);
                for (expand, alpha) in [(4.0f32, 26u8), (2.0, 60)] {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0 - expand, cap_y - 2.0 - expand),
                            egui::pos2(x0 + bar_w + expand, cap_y + expand),
                        ),
                        3.0,
                        egui::Color32::from_rgba_unmultiplied(
                            cap_col.r(),
                            cap_col.g(),
                            cap_col.b(),
                            alpha,
                        ),
                    );
                }
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0, cap_y - 2.0),
                        egui::pos2(x0 + bar_w, cap_y),
                    ),
                    0.0,
                    cap_col.gamma_multiply(1.25),
                );
            }
        }
        // шкала 0/-12/-24/-48
        for mark in [0.0f32, -12.0, -24.0, -48.0] {
            let y = db_to_y(mark);
            painter.text(
                egui::pos2(rect.right() - 2.0, y),
                egui::Align2::RIGHT_CENTER,
                format!("{mark:.0}"),
                egui::FontId::proportional(8.5),
                egui::Color32::from_gray(120),
            );
        }

        // --- фейдер МЕЖДУ барами (порт PreviewVolumeFader как центральной
        //     накладки): трек в зазоре + широкая ручка через оба бара
        let track = egui::Rect::from_min_max(
            egui::pos2(bars.left() + bar_w + 2.5, bars.top()),
            egui::pos2(bars.left() + bar_w + gap - 2.5, bars.bottom()),
        );
        painter.rect_filled(track, 2.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120));
        let ft = (self.meter.fader_db - FADER_MIN_DB) / (FADER_MAX_DB - FADER_MIN_DB);
        let hy = bars.bottom() - ft.clamp(0.0, 1.0) * bars.height();
        let handle = egui::Rect::from_center_size(
            egui::pos2(bars.center().x, hy),
            egui::vec2(bars.width() + 6.0, 9.0),
        );
        let handle_col = if self.meter.muted {
            egui::Color32::from_rgb(220, 70, 70)
        } else {
            egui::Color32::from_gray(235)
        };
        // ореол ручки
        painter.rect_filled(
            handle.expand(2.5),
            4.0,
            egui::Color32::from_rgba_unmultiplied(
                handle_col.r(),
                handle_col.g(),
                handle_col.b(),
                40,
            ),
        );
        painter.rect_filled(handle, 3.0, handle_col.gamma_multiply(0.95));
        painter.line_segment(
            [
                egui::pos2(handle.left() + 4.0, handle.center().y),
                egui::pos2(handle.right() - 4.0, handle.center().y),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120)),
        );

        // кнопка M (mute) внизу
        let mute_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 6.0, rect.bottom() - 26.0),
            egui::pos2(rect.right() - 6.0, rect.bottom() - 4.0),
        );
        painter.rect_filled(
            mute_rect,
            3.0,
            if self.meter.muted {
                egui::Color32::from_rgb(160, 40, 40)
            } else {
                egui::Color32::from_gray(45)
            },
        );
        painter.text(
            mute_rect.center(),
            egui::Align2::CENTER_CENTER,
            "M",
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );

        // интеракции: драг по всей области баров двигает фейдер
        if let Some(pos) = response.interact_pointer_pos() {
            if response.double_clicked() && !mute_rect.contains(pos) {
                self.meter.fader_db = 0.0;
            } else if response.clicked() && mute_rect.contains(pos) {
                self.meter.muted = !self.meter.muted;
            } else if (response.dragged() || response.drag_started())
                && pos.y >= bars.top() - 6.0
                && pos.y <= bars.bottom() + 6.0
            {
                let t = ((bars.bottom() - pos.y) / bars.height()).clamp(0.0, 1.0);
                self.meter.fader_db = FADER_MIN_DB + t * (FADER_MAX_DB - FADER_MIN_DB);
            }
        }
        let gain = if self.meter.muted || self.meter.fader_db <= FADER_MIN_DB + 0.01 {
            0.0
        } else {
            10f32.powf(self.meter.fader_db / 20.0)
        };
        self.player.set_master_gain(gain);
        response.on_hover_text("Мастер-громкость превью. Double-click: 0 dB. M: mute.");
    }
}
