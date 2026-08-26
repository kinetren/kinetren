//! Стерео peak-метры + мастер-фейдер (порт AudioLevelMeterWidget/
//! PreviewVolumeFader): лестница цветов, peak-hold caps со спадом,
//! фейдер −60..+24 dB, double-click = 0 dB, кнопка M = mute.

use crate::app::KtApp;
use eframe::egui;

pub const METER_MIN_DB: f32 = -48.0;
pub const METER_MAX_DB: f32 = 6.0;
pub const FADER_MIN_DB: f32 = -60.0;
/// Жёсткость подъезда фейдера (рад/с) и порог схождения.
pub const FADER_OMEGA: f32 = 26.0;
pub const FADER_EPS_DB: f32 = 0.02;
pub const FADER_MAX_DB: f32 = 24.0;
/// Спад бара/капов, dB за тик 25 мс (порт: 2.8 / 0.2).
pub const BAR_FALL_DB: f32 = 2.8;
pub const CAP_FALL_DB: f32 = 0.2;

pub struct MeterState {
    pub bar_db: [f32; 2],
    pub cap_db: [f32; 2],
    /// Целевой уровень: бары спадают К НЕМУ, а не к минимуму. В паузе
    /// держит последний сэмпл — без «пилы» между редкими замерами (п.2).
    pub target_db: [f32; 2],
    pub fader_db: f32,
    /// Куда тянет мышь/колесо: фейдер едет к цели с инерцией, как
    /// физический слайдер (п.2 р.8).
    pub fader_target: f32,
    /// Скорость движения фейдера, dB/с (критическое демпфирование).
    pub fader_vel: f32,
    pub muted: bool,
    last_tick: std::time::Instant,
    last_sample: std::time::Instant,
}

impl MeterState {
    /// Есть ли что анимировать (для repaint-политики, п.1 v1.4): бары/капы
    /// ДВИЖУТСЯ, а не просто горят. Статичный показ уровня на курсоре в
    /// паузе (фича п.2 v1.1) не должен держать 60 fps: раньше условие было
    /// «бар выше минимума» и залипало в true навсегда после первого звука —
    /// приложение вечно рисовало 60 fps в простое и грело всю систему.
    /// Тест 38.
    pub fn is_active(&self) -> bool {
        // фейдер в движении тоже держит перерисовку — но ровно до
        // схождения (снап в fader_tick), иначе вернётся вечный 60 fps
        // в простое, ради которого писался t38
        if (self.fader_db - self.fader_target).abs() > FADER_EPS_DB
            || self.fader_vel.abs() > FADER_EPS_DB
        {
            return true;
        }
        (0..2).any(|ch| {
            let floor = self.target_db[ch].max(METER_MIN_DB);
            (self.bar_db[ch] - floor).abs() > 0.05
                || (self.cap_db[ch] - self.bar_db[ch] > 0.05
                    && self.cap_db[ch] > METER_MIN_DB + 0.05)
        })
    }

    /// Шаг инерции фейдера (критическое демпфирование, dt в секундах).
    /// Чистая функция ради теста 80: обязана СХОДИТЬСЯ и вставать ровно
    /// на цель, а не ползти бесконечно.
    pub fn fader_tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let d = self.fader_target - self.fader_db;
        // x'' = -2ζω x' - ω² x при ζ=1: плавный подъезд без перелёта
        let w = FADER_OMEGA;
        self.fader_vel += (w * w * d - 2.0 * w * self.fader_vel) * dt;
        self.fader_db += self.fader_vel * dt;
        if (self.fader_target - self.fader_db).abs() <= FADER_EPS_DB
            && self.fader_vel.abs() <= FADER_EPS_DB
        {
            self.fader_db = self.fader_target;
            self.fader_vel = 0.0;
        }
    }

    /// Один шаг спада (тик 25 мс, спады v1) — чистая логика для теста 38.
    /// `playing` дополнительно отпускает target (иначе при остановке
    /// подкачки бары зависали бы навсегда).
    pub fn tick(&mut self, ticks: f32, playing: bool) {
        for ch in 0..2 {
            let floor = self.target_db[ch].max(METER_MIN_DB);
            self.bar_db[ch] = (self.bar_db[ch] - BAR_FALL_DB * ticks).max(floor);
            self.cap_db[ch] = (self.cap_db[ch] - CAP_FALL_DB * ticks).max(METER_MIN_DB);
            if playing {
                self.target_db[ch] =
                    (self.target_db[ch] - BAR_FALL_DB * 0.7 * ticks).max(METER_MIN_DB);
            }
        }
        for ch in 0..2 {
            self.bar_db[ch] = self.bar_db[ch].max(self.target_db[ch]);
            self.cap_db[ch] = self.cap_db[ch].max(self.bar_db[ch]);
        }
    }

    /// Подача замера уровней (linear 0..1 на канал).
    pub fn feed(&mut self, pl: f32, pr: f32) {
        self.target_db[0] = db_of(pl);
        self.target_db[1] = db_of(pr);
    }
}

impl Default for MeterState {
    fn default() -> Self {
        MeterState {
            bar_db: [METER_MIN_DB; 2],
            cap_db: [METER_MIN_DB; 2],
            target_db: [METER_MIN_DB; 2],
            fader_db: 0.0,
            fader_target: 0.0,
            fader_vel: 0.0,
            muted: false,
            last_tick: std::time::Instant::now(),
            last_sample: std::time::Instant::now(),
        }
    }
}

fn db_of(linear: f32) -> f32 {
    if linear <= 1e-6 { METER_MIN_DB } else { (20.0 * linear.log10()).max(METER_MIN_DB) }
}

pub(crate) fn zone_color(db: f32) -> egui::Color32 {
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

/// Зонные полосы dB (верх, низ) сверху вниз: смежные, без дыр и
/// перекрытий, покрывают весь диапазон метра (тест 40).
pub(crate) const ZONE_BANDS: [(f32, f32); 6] =
    [(6.0, 0.0), (0.0, -3.0), (-3.0, -9.0), (-9.0, -18.0), (-18.0, -36.0), (-36.0, -48.0)];

/// dB-остановки вертикального градиента свечения: низ метра, границы зон
/// ниже уровня, сам уровень. Отсортированы снизу вверх (тест 40).
pub(crate) fn glow_stops(level_db: f32) -> Vec<f32> {
    let level = level_db.clamp(METER_MIN_DB, METER_MAX_DB);
    let mut v = vec![METER_MIN_DB];
    for &(top, _bot) in ZONE_BANDS.iter().rev() {
        if top > METER_MIN_DB && top < level {
            v.push(top);
        }
    }
    v.push(level);
    v
}

/// Ореол слева от панели метров для оверлея поверх UI. Правая кромка —
/// ровно левый край панели: раньше +8 px наезжали на первый бар у подписи
/// dB (п.10 v1.4, тест 40).
pub(crate) fn overlay_glow_rect(panel: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(panel.left() - 46.0, panel.top()),
        egui::pos2(panel.left(), panel.bottom()),
    )
}

/// Радиальное свечение (порт drawGlowEllipse: QRadialGradient, Plus).
/// 7 вложенных эллипсов с гаусс-профилем альфы, УБЫВАЮЩЕЙ наружу —
/// плавный градиент без жёстких кромок (п.7: у трёх плоских колец
/// альфа росла к центру и кромки читались ступенями).
fn glow_ellipse(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: egui::Vec2,
    col: egui::Color32,
    alpha: u8,
) {
    const RINGS: usize = 4;
    for i in (0..RINGS).rev() {
        let k = (i + 1) as f32 / RINGS as f32; // 1/7 .. 1
        // гаусс: яркость выше к центру, мягкий спад к краю
        let g = (-(k * k) * 2.2f32).exp();
        let a = (alpha as f32 * g * 0.55).min(255.0) as u8;
        if a == 0 {
            continue;
        }
        painter.add(egui::Shape::from(egui::epaint::EllipseShape {
            center,
            radius: radius * k,
            fill: egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), a),
            stroke: egui::Stroke::NONE,
            angle: 0.0,
        }));
    }
}

/// Цвет свечения кэпа (порт capGlowColor: кремово-белая база, тонировка зоной).
fn cap_glow_color(db: f32) -> egui::Color32 {
    if db > 0.0 {
        egui::Color32::from_rgb(255, 120, 95)
    } else if db >= -3.0 {
        egui::Color32::from_rgb(255, 85, 70)
    } else if db >= -9.0 {
        egui::Color32::from_rgb(255, 205, 60)
    } else if db >= -18.0 {
        egui::Color32::from_rgb(245, 242, 230)
    } else {
        zone_color(db)
    }
}

impl KtApp {
    /// Панель метров+фейдера справа от превью. Возвращает высоту не более
    /// доступной; ширина ~64 px.
    pub fn draw_meters(&mut self, ui: &mut egui::Ui) {
        // 1) подкачка уровней: играем — каждый кадр; пауза — замер на
        // курсоре раз в 120 мс (v1), target ДЕРЖИТ последний замер
        let playing = self.player.transport.is_playing();
        let sample = if playing {
            Some(self.player.meters.take())
        } else if self.meter.last_sample.elapsed().as_millis() >= 120 {
            self.meter.last_sample = std::time::Instant::now();
            Some(self.player.sample_levels_at(self.player.transport.playhead()))
        } else {
            None // между замерами target не трогаем (раньше сюда шли нули)
        };
        if let Some((pl, pr)) = sample {
            self.meter.feed(pl, pr);
        }
        // 2) спад к target и peak-hold (тик 25 мс, спады v1) — чистая
        // логика в MeterState::tick (тест 38)
        let ticks = (self.meter.last_tick.elapsed().as_millis() / 25).min(40) as f32;
        if ticks >= 1.0 {
            self.meter.last_tick = std::time::Instant::now();
            self.meter.tick(ticks, playing);
        } else {
            // клампы к target — и между тиками (подкачка выше спада)
            self.meter.tick(0.0, false);
        }

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(64.0, ui.available_height()),
            egui::Sense::click_and_drag(),
        );
        {
            // ореол для оверлея поверх UI (п.8)
            let loud = self.meter.bar_db[0].max(self.meter.bar_db[1]);
            let k = ((loud - METER_MIN_DB) / (METER_MAX_DB - METER_MIN_DB)).clamp(0.0, 1.0);
            // (панель, интенсивность, уровень dB) — оверлей красит по зонам
            self.meter_glow = (k > 0.05).then_some((rect, k, loud));
        }
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

        let loud_db = self.meter.bar_db[0].max(self.meter.bar_db[1]);

        // фоны ОБОИХ баров ДО свечения (п.9 v1.4: раньше glow рисовался
        // внутри цикла под ch==0 и непрозрачный фон второго бара его
        // закрашивал — эффект был виден только на первом)
        for &x0 in &bar_x {
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0 - 1.0, bars.top() - 1.0),
                    egui::pos2(x0 + bar_w + 1.0, bars.bottom() + 1.0),
                ),
                2.0,
                egui::Color32::from_rgb(8, 9, 11),
            );
        }

        // --- свечение (пп.8-9 v1.4): цвет по ЗОНАМ высоты, один меш ------
        // вертикальный градиент с остановками на границах зон (glow_stops,
        // тест 40) и треугольным горизонтальным затуханием — заменяет
        // ~250 тессилируемых эллипсов шлейфа и зон прежней версии
        if loud_db > METER_MIN_DB + 1.0 {
            let cx = bars.center().x;
            let half_w = bars.width() * 1.15;
            let level_y = db_to_y(loud_db);
            let mut mesh = egui::Mesh::default();
            let stops = glow_stops(loud_db);
            let push_row = |mesh: &mut egui::Mesh, y: f32, col: egui::Color32, a: u8| {
                for (x, alpha) in
                    [(cx - half_w, 0u8), (cx, a), (cx + half_w, 0u8)]
                {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(x, y),
                        uv: egui::epaint::WHITE_UV,
                        color: egui::Color32::from_rgba_unmultiplied(
                            col.r(),
                            col.g(),
                            col.b(),
                            alpha,
                        ),
                    });
                }
            };
            for &db in &stops {
                push_row(&mut mesh, db_to_y(db), zone_color(db), 46);
            }
            // мягкий хвост над уровнем — затухание в ноль
            push_row(&mut mesh, (level_y - 22.0).max(bars.top()), zone_color(loud_db), 0);
            let rows = stops.len() + 1;
            for r in 0..rows - 1 {
                let b = (r * 3) as u32;
                // два квада между соседними рядами (3 колонки)
                for c in 0..2u32 {
                    mesh.indices.extend_from_slice(&[
                        b + c,
                        b + c + 1,
                        b + c + 4,
                        b + c,
                        b + c + 4,
                        b + c + 3,
                    ]);
                }
            }
            painter.add(egui::Shape::mesh(mesh));
            // подсветка низа (v1)
            glow_ellipse(
                &painter,
                egui::pos2(cx, bars.bottom()),
                egui::vec2(half_w * 0.85, 28.0),
                zone_color(METER_MIN_DB + 6.0),
                20,
            );
        }

        for (ch, &x0) in bar_x.iter().enumerate() {
            // LED-сегменты v1: 6 px + 2 px зазор, светлая верхняя кромка
            let mut y = bars.bottom();
            while y > bars.top() {
                let seg_top = (y - 6.0).max(bars.top());
                let db_here = METER_MIN_DB
                    + (bars.bottom() - y) / bars.height() * (METER_MAX_DB - METER_MIN_DB);
                let lit = db_here <= self.meter.bar_db[ch];
                let col =
                    if lit { zone_color(db_here) } else { egui::Color32::from_rgb(14, 16, 19) };
                let seg = egui::Rect::from_min_max(
                    egui::pos2(x0, seg_top),
                    egui::pos2(x0 + bar_w, y - 1.0),
                );
                painter.rect_filled(seg.shrink(0.5), 1.0, col);
                if lit {
                    painter.line_segment(
                        [
                            egui::pos2(seg.left() + 1.0, seg.top() + 0.5),
                            egui::pos2(seg.right() - 1.0, seg.top() + 0.5),
                        ],
                        egui::Stroke::new(1.0, col.gamma_multiply(1.25)),
                    );
                }
                y -= 8.0;
            }
            // линия 0 dB поперёк бара (порт: 95,105,118,140)
            let zero_y = db_to_y(0.0);
            painter.line_segment(
                [egui::pos2(x0, zero_y), egui::pos2(x0 + bar_w, zero_y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(95, 105, 118, 140)),
            );
        }

        // головной блик ПОВЕРХ LED обоих баров (bloom симметричен, п.9);
        // цвет — зоны уровня: локально у головы, ниже доминируют зонные
        // цвета меша (п.8)
        if loud_db > METER_MIN_DB + 1.0 {
            let cx = bars.center().x;
            let wide = bars.width() * 1.15;
            let level_y = db_to_y(loud_db);
            glow_ellipse(
                &painter,
                egui::pos2(cx, level_y),
                egui::vec2(wide * 1.08, 38.0),
                zone_color(loud_db),
                42,
            );
            glow_ellipse(
                &painter,
                egui::pos2(cx, level_y - 14.0),
                egui::vec2(wide * 0.95, 34.0),
                zone_color(loud_db),
                28,
            );
        }

        for (ch, &x0) in bar_x.iter().enumerate() {
            // peak-hold cap: кремово-белый сегмент со свечением (v1)
            let cap_db = self.meter.cap_db[ch];
            if cap_db > self.meter.bar_db[ch] + 0.4 && cap_db > METER_MIN_DB + 1.0 {
                let cap_y = db_to_y(cap_db);
                let cap_col = cap_glow_color(cap_db);
                glow_ellipse(
                    &painter,
                    egui::pos2(x0 + bar_w / 2.0, cap_y),
                    egui::vec2(bar_w * 2.0, 13.0),
                    cap_col,
                    22,
                );
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0, cap_y - 2.5),
                        egui::pos2(x0 + bar_w, cap_y),
                    ),
                    1.0,
                    cap_col,
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
        // Ручка v1 (PreviewVolumeFader): вертикальный градиент, насечки,
        // центральная канавка, тень снизу, обводка тёмным.
        let handle = egui::Rect::from_center_size(
            egui::pos2(bars.center().x, hy),
            egui::vec2(bars.width() + 6.0, 14.0),
        );
        let (g_top, g_mid, g_bot) = if self.meter.muted {
            (
                egui::Color32::from_rgb(210, 90, 90),
                egui::Color32::from_rgb(172, 52, 52),
                egui::Color32::from_rgb(110, 32, 32),
            )
        } else {
            (
                egui::Color32::from_rgb(128, 132, 142),
                egui::Color32::from_rgb(78, 82, 90),
                egui::Color32::from_rgb(48, 50, 56),
            )
        };
        // тень под ручкой
        painter.rect_filled(
            handle.translate(egui::vec2(0.0, 2.0)),
            3.0,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70),
        );
        // 3-стоповый градиент двумя мешами (стоп 0.45 как в v1)
        let mid_y = handle.top() + handle.height() * 0.45;
        crate::toolbar::gradient_rect(
            &painter,
            egui::Rect::from_min_max(handle.left_top(), egui::pos2(handle.right(), mid_y)),
            g_top,
            g_mid,
        );
        crate::toolbar::gradient_rect(
            &painter,
            egui::Rect::from_min_max(egui::pos2(handle.left(), mid_y), handle.right_bottom()),
            g_mid,
            g_bot,
        );
        painter.rect(
            handle,
            3.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(12, 14, 18)),
            egui::StrokeKind::Outside,
        );
        // насечки каждые 4 px (белые, альфа 40)
        let mut nx = handle.left() + 7.0;
        while nx <= handle.right() - 7.0 {
            painter.line_segment(
                [egui::pos2(nx, handle.top() + 3.0), egui::pos2(nx, handle.bottom() - 3.0)],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
            );
            nx += 4.0;
        }
        // центральная канавка
        painter.line_segment(
            [
                egui::pos2(handle.left() + 3.0, handle.center().y),
                egui::pos2(handle.right() - 3.0, handle.center().y),
            ],
            egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 45)),
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

        // Интеракции фейдера (п.2 р.8). Хит-зона ШИРЕ виджета (рисование
        // не меняется): раньше приходилось целиться в узкую полоску, а
        // любой промах телепортировал громкость под курсор.
        let hit = egui::Rect::from_x_y_ranges(
            rect.x_range(),
            egui::Rangef::new(bars.top() - 10.0, bars.bottom() + 10.0),
        );
        let fader = ui.interact(hit, ui.id().with("fader"), egui::Sense::click_and_drag());
        let db_at = |y: f32| {
            let t = ((bars.bottom() - y) / bars.height()).clamp(0.0, 1.0);
            FADER_MIN_DB + t * (FADER_MAX_DB - FADER_MIN_DB)
        };
        if response.clicked() && response.interact_pointer_pos().is_some_and(|p| mute_rect.contains(p))
        {
            self.meter.muted = !self.meter.muted;
        } else if fader.double_clicked() {
            self.meter.fader_target = 0.0;
        } else if let Some(pos) = fader.interact_pointer_pos() {
            if fader.drag_started() {
                // нажали рядом с ручкой — тянем ОТНОСИТЕЛЬНО (без рывка);
                // нажали по треку — едем к точке
                self.fader_grab =
                    if (pos.y - hy).abs() <= 12.0 { Some(hy - pos.y) } else { None };
                if self.fader_grab.is_none() {
                    self.meter.fader_target = db_at(pos.y);
                }
            }
            if fader.dragged() {
                let y = pos.y + self.fader_grab.unwrap_or(0.0);
                self.meter.fader_target = db_at(y);
            }
            if fader.clicked() && !mute_rect.contains(pos) {
                self.meter.fader_target = db_at(pos.y);
            }
        }
        if fader.drag_stopped() {
            self.fader_grab = None;
        }
        // колесо над метром: шаг 1 dB, с Shift — точный 0.2 dB
        if fader.contains_pointer() {
            let (scroll, shift) =
                ui.input(|i| (i.smooth_scroll_delta.y, i.modifiers.shift));
            if scroll != 0.0 {
                let step = if shift { 0.2 } else { 1.0 };
                self.meter.fader_target = (self.meter.fader_target
                    + scroll.signum() * step)
                    .clamp(FADER_MIN_DB, FADER_MAX_DB);
            }
        }
        // инерция: фейдер догоняет цель между кадрами
        let dt = ui.input(|i| i.stable_dt);
        self.meter.fader_tick(dt);
        let gain = if self.meter.muted || self.meter.fader_db <= FADER_MIN_DB + 0.01 {
            0.0
        } else {
            10f32.powf(self.meter.fader_db / 20.0)
        };
        self.player.set_master_gain(gain);
        // подсказка зависит от зоны под курсором (п.1 р.9)
        let over_mute = fader.hover_pos().is_some_and(|p| mute_rect.contains(p));
        let text = if over_mute {
            "Мьют мастера превью"
        } else {
            "Мастер-громкость превью: тяни или колесо (Shift — точно), double-click — 0 dB"
        };
        self.hint(fader, text);
        self.hint(response, "Пиковый уровень мастера, dBFS");
    }
}

#[cfg(test)]
mod repaint_tests {
    use super::*;

    /// Тест 80 (п.2 р.8): инерция фейдера — сходится к цели за разумное
    /// время и ОСТАНАВЛИВАЕТСЯ (иначе вечный repaint, ради которого
    /// писался t38).
    #[test]
    fn t80_fader_inertia_settles() {
        let mut m = MeterState { fader_target: -20.0, ..Default::default() };
        assert!(m.is_active(), "поехали — перерисовка нужна");
        let mut steps = 0;
        while m.is_active() && steps < 400 {
            m.fader_tick(1.0 / 60.0);
            steps += 1;
        }
        assert!(steps < 120, "фейдер едет слишком долго: {steps} кадров");
        assert_eq!(m.fader_db, -20.0, "не встал ровно на цель");
        assert_eq!(m.fader_vel, 0.0);
        assert!(!m.is_active(), "после схождения перерисовка отпущена");
        // без цели ничего не происходит
        m.fader_tick(1.0 / 60.0);
        assert_eq!(m.fader_db, -20.0);
        // движение к цели монотонно (без перелёта): критическое демпфирование
        m.fader_target = 0.0;
        let mut prev = m.fader_db;
        for _ in 0..200 {
            m.fader_tick(1.0 / 60.0);
            assert!(m.fader_db >= prev - 1e-3, "перелёт вниз");
            assert!(m.fader_db <= 0.0 + 1e-3, "перелёт вверх");
            prev = m.fader_db;
        }
    }

    /// Тест 40 (пп.8-10 v1.4): раскладка glow. Зонные полосы смежны, без
    /// дыр и перекрытий, покрывают весь диапазон метра; остановки градиента
    /// отсортированы и включают все границы зон ниже уровня; ореол оверлея
    /// не заходит на панель (раньше +8 px наезжали на первый бар).
    #[test]
    fn t40_glow_layout() {
        // зоны: сверху вниз, верх первой = MAX, низ последней = MIN, стык в стык
        assert_eq!(ZONE_BANDS[0].0, METER_MAX_DB);
        assert_eq!(ZONE_BANDS[ZONE_BANDS.len() - 1].1, METER_MIN_DB);
        for w in ZONE_BANDS.windows(2) {
            assert_eq!(w[0].1, w[1].0, "полосы стык в стык: {w:?}");
        }
        for (top, bot) in ZONE_BANDS {
            assert!(top > bot, "полоса невырожденная: ({top}, {bot})");
        }
        // остановки градиента: низ, границы зон ниже уровня, уровень
        for level in [-40.0f32, -20.0, -10.0, -5.0, -1.0, 3.0, 99.0] {
            let stops = glow_stops(level);
            assert_eq!(stops[0], METER_MIN_DB);
            let clamped = level.clamp(METER_MIN_DB, METER_MAX_DB);
            assert_eq!(*stops.last().unwrap(), clamped);
            assert!(stops.windows(2).all(|w| w[0] < w[1]), "строго вверх: {stops:?}");
            for &(top, _bot) in &ZONE_BANDS {
                if top > METER_MIN_DB && top < clamped {
                    assert!(stops.contains(&top), "граница зоны {top} в остановках {stops:?}");
                }
            }
        }
        // ореол оверлея: правая кромка = левый край панели (не дальше)
        let panel = egui::Rect::from_min_max(egui::pos2(100.0, 0.0), egui::pos2(164.0, 400.0));
        let g = overlay_glow_rect(panel);
        assert!(g.right() <= panel.left(), "ореол не заходит на панель");
        assert!(g.left() < panel.left(), "и реально светит влево");
    }

    /// Тест 38 (п.1 v1.4): метры не держат 60 fps вечно. После остановки
    /// подкачки (пауза, статичный target) бары/капы сходятся и
    /// is_active() гаснет; во время движения — горит. Раньше условие
    /// «бар выше минимума» залипало в true навсегда после первого звука.
    #[test]
    fn t38_meters_settle_and_release_repaint() {
        let mut m = MeterState::default();
        assert!(!m.is_active(), "тишина с нуля — неактивно");

        // громкий замер при игре: движение началось
        m.feed(0.9, 0.9);
        m.tick(1.0, true);
        assert!(m.is_active(), "бар едет к target — активно");

        // пауза: target держит уровень на курсоре (фича п.2 v1.1) — бары
        // сходятся к нему, капы спадают до баров, движение заканчивается
        m.feed(0.5, 0.5);
        for _ in 0..2000 {
            m.tick(1.0, false);
        }
        assert!(
            !m.is_active(),
            "статичный показ уровня в паузе не должен требовать анимации: \
             bar={:?} cap={:?} target={:?}",
            m.bar_db,
            m.cap_db,
            m.target_db
        );
        // при этом уровень ПОКАЗЫВАЕТСЯ (не затух к минимуму — фича)
        assert!(m.bar_db[0] > METER_MIN_DB + 1.0, "уровень на курсоре виден");

        // игра без сигнала: target отпускается, всё затухает до минимума
        m.feed(0.0, 0.0);
        for _ in 0..2000 {
            m.tick(1.0, true);
        }
        assert!(!m.is_active());
        assert!((m.bar_db[0] - METER_MIN_DB).abs() < 0.1, "после игры затухли в пол");
    }
}
