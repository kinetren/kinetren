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
        if dt <= 0.0 {
            return;
        }
        // ТОЧНОЕ решение x'' + 2ωx' + ω²x = 0 при ζ=1: x(t) = (A + Bt)e^{−ωt}.
        // Явный Эйлер здесь РАСХОДИЛСЯ при ω·dt > 2√2−2, то есть уже при
        // dt > 32 мс: а stable_dt после простоя приходит равным 100 мс
        // (кламп) — отклонение умножалось на −10 за кадр, фейдер улетал на
        // сотни dB и это уходило в звук (п.7 р.9). Экспонента не расходится
        // ни при каком dt.
        let w = FADER_OMEGA;
        let x0 = self.fader_db - self.fader_target;
        let b = self.fader_vel + w * x0;
        let e = (-w * dt).exp();
        let xt = (x0 + b * dt) * e;
        self.fader_db = (self.fader_target + xt).clamp(FADER_MIN_DB, FADER_MAX_DB);
        self.fader_vel = (b - w * (x0 + b * dt)) * e;
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

/// Допустимые шаги LED-сетки в ДЕЦИБЕЛАХ (п.8 р.11, уточнено п.1 р.12).
/// Каждый делит НАЦЕЛО все цветовые зоны (6/3/6/9/18/12 dB), поэтому
/// сегмент никогда не бывает двухцветным и всегда горит целиком либо не
/// горит вовсе. Пиксельная нарезка (6 px + 2 px), как в первой версии,
/// этого не гарантировала: границы зон резали сегмент пополам.
pub const SEG_STEPS: [f32; 5] = [0.5, 0.75, 1.0, 1.5, 3.0];
/// Желаемый шаг сетки на экране: v1 рисовал 6 px сегмент + 2 px зазор.
pub const SEG_TARGET_PX: f32 = 7.0;

/// Сколько сегментов на всю шкалу при данном шаге.
pub fn seg_count(seg_db: f32) -> usize {
    ((METER_MAX_DB - METER_MIN_DB) / seg_db).round() as usize
}

/// Шаг сетки под высоту бара: берём тот из `SEG_STEPS`, чей шаг на экране
/// ближе всего к `SEG_TARGET_PX`. Иначе на высоком баре сегменты выходят
/// огромными и редкими (жалоба п.1 р.12: «сегментов очень мало»).
pub fn seg_db_for(bar_h: f32) -> f32 {
    let span = METER_MAX_DB - METER_MIN_DB;
    *SEG_STEPS
        .iter()
        .min_by(|a, b| {
            let px = |s: f32| (bar_h * s / span - SEG_TARGET_PX).abs();
            px(**a).total_cmp(&px(**b))
        })
        .expect("SEG_STEPS не пуст")
}

/// Сколько сегментов зажжено при уровне db (чистая, тест 97).
pub fn led_lit(db: f32, seg_db: f32) -> usize {
    if db <= METER_MIN_DB {
        return 0;
    }
    let n = ((db - METER_MIN_DB) / seg_db).ceil() as i32;
    n.clamp(0, seg_count(seg_db) as i32) as usize
}

/// Середина сегмента i (0 — самый нижний) в dB: по ней берётся цвет зоны.
pub fn seg_mid_db(i: usize, seg_db: f32) -> f32 {
    METER_MIN_DB + (i as f32 + 0.5) * seg_db
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
        // KT_METER_DEMO=1 — подать метру фиктивные уровни: вид метра
        // (свечение, зоны, пик) иначе не проверить без звукового железа
        if std::env::var_os("KT_METER_DEMO").is_some() {
            let t = self.meter.last_sample.elapsed().as_secs_f32();
            self.meter.feed(0.75, 0.42 + 0.2 * (t * 3.0).sin().abs());
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

        // п.3 р.9: занимаем ВСЮ ширину панели — жёсткие 64 px при панели
        // 70 оставляли мёртвый зазор у правого края окна (и он рос, если
        // панель расширяли). Внутренняя геометрия ниже относительная.
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ui.available_height()),
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
        // шаг LED-сетки под фактическую высоту бара (п.1 р.12)
        let seg_db = seg_db_for(bars.height());
        let segs = seg_count(seg_db);
        let seg_h = bars.height() / segs as f32;
        // два бара с зазором по центру — фейдер живёт в зазоре (порт старого)
        let gap = 10.0;
        let bar_w = (bars.width() - gap) / 2.0;
        let bar_x = [bars.left(), bars.left() + bar_w + gap];

        let loud_db = self.meter.bar_db[0].max(self.meter.bar_db[1]);

        // Порядок слоёв — как в «красивой» версии до р.11 (п.1 р.13):
        // подложка → свечение → LED → блик и пик ПОВЕРХ. В р.11 подложка
        // уехала поверх свечения и закрасила его целиком, а блик ушёл под
        // непрозрачные сегменты — метр стал плоским и «фейковым».
        let lit_n: [usize; 2] = [
            led_lit(self.meter.bar_db[0], seg_db),
            led_lit(self.meter.bar_db[1], seg_db),
        ];
        let cap_seg: [Option<usize>; 2] = std::array::from_fn(|ch| {
            let cap = self.meter.cap_db[ch];
            (cap > self.meter.bar_db[ch] + 0.4 && cap > METER_MIN_DB + 1.0)
                .then(|| led_lit(cap, seg_db).saturating_sub(1))
        });
        let back = egui::Color32::from_rgb(8, 9, 11);
        // 1) дно баров
        for &x0 in &bar_x {
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0 - 1.0, bars.top() - 1.0),
                    egui::pos2(x0 + bar_w + 1.0, bars.bottom() + 1.0),
                ),
                2.0,
                back,
            );
        }

        // 2) зонное свечение по дну бара (зазоры между LED им и светятся)
        if loud_db > METER_MIN_DB + 1.0 {
            let cx = bars.center().x;
            let half_w = bars.width() * 1.15;
            let level_y = db_to_y(loud_db);
            let mut mesh = egui::Mesh::default();
            let stops = glow_stops(loud_db);
            let push_row = |mesh: &mut egui::Mesh, y: f32, col: egui::Color32, a: u8| {
                for (x, alpha) in [(cx - half_w, 0u8), (cx, a), (cx + half_w, 0u8)] {
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
            // хвост КОРОТКИЙ (8 px): прежние 22 px и делали погашенную
            // часть «полузажжённой» — вот это и была настоящая причина
            // жалобы п.8 р.11, а вовсе не частичная заливка сегментов
            push_row(&mut mesh, (level_y - 8.0).max(bars.top()), zone_color(loud_db), 0);
            let rows = stops.len() + 1;
            for r in 0..rows - 1 {
                let b = (r * 3) as u32;
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
            glow_ellipse(
                &painter,
                egui::pos2(cx, bars.bottom()),
                egui::vec2(half_w * 0.85, 28.0),
                zone_color(METER_MIN_DB + 6.0),
                20,
            );
        }

        // 3) выше уровня КАЖДОГО канала бар глухой: свечение соседнего
        // (громкого) канала не должно подсвечивать погашенную шкалу
        for (ch, &x0) in bar_x.iter().enumerate() {
            let top_lit = db_to_y(METER_MIN_DB + lit_n[ch] as f32 * seg_db);
            if top_lit > bars.top() {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0 - 1.0, bars.top() - 1.0),
                        egui::pos2(x0 + bar_w + 1.0, top_lit),
                    ),
                    0.0,
                    back,
                );
            }
        }

        // 4) LED-сегменты: скважность ~55 %, как у v1 (там было 4 px
        // нарисованных на 8 px шага) — в р.11-12 доходило до 82 %, отчего
        // шкала выглядела сплошной «колбасой со швами»
        for (ch, &x0) in bar_x.iter().enumerate() {
            let gap_y = (seg_h * 0.38).clamp(1.0, 2.5);
            for i in 0..segs {
                let y_bot = db_to_y(METER_MIN_DB + i as f32 * seg_db);
                let y_top = db_to_y(METER_MIN_DB + (i + 1) as f32 * seg_db);
                let seg = egui::Rect::from_min_max(
                    egui::pos2(x0, y_top),
                    egui::pos2(x0 + bar_w, y_bot - gap_y),
                )
                .shrink2(egui::vec2(0.5, 0.0));
                let lit = i < lit_n[ch];
                let col = if lit {
                    zone_color(seg_mid_db(i, seg_db))
                } else {
                    egui::Color32::from_rgb(13, 15, 18)
                };
                painter.rect_filled(seg, 1.0, col);
                if lit {
                    painter.line_segment(
                        [
                            egui::pos2(seg.left() + 1.0, seg.top() + 0.5),
                            egui::pos2(seg.right() - 1.0, seg.top() + 0.5),
                        ],
                        egui::Stroke::new(1.0, col.gamma_multiply(1.12)),
                    );
                }
            }
            // линия 0 dB поперёк бара (порт: 95,105,118,140)
            let zero_y = db_to_y(0.0);
            painter.line_segment(
                [egui::pos2(x0, zero_y), egui::pos2(x0 + bar_w, zero_y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(95, 105, 118, 140)),
            );
        }

        // 5) головной блик ПОВЕРХ горящих LED (тот самый bloom из v1)
        if loud_db > METER_MIN_DB + 1.0 {
            let cx = bars.center().x;
            let wide = bars.width() * 1.15;
            let level_y = db_to_y(loud_db);
            glow_ellipse(
                &painter,
                egui::pos2(cx, level_y),
                egui::vec2(wide * 1.08, 34.0),
                zone_color(loud_db),
                38,
            );
            glow_ellipse(
                &painter,
                egui::pos2(cx, level_y - 12.0),
                egui::vec2(wide * 0.95, 30.0),
                zone_color(loud_db),
                24,
            );
        }

        // 6) peak-hold: кремовая ЛЕНТА по верхней кромке своего сегмента
        // (сетка LED не нарушается) + ореол — поверх сегментов
        for (ch, &x0) in bar_x.iter().enumerate() {
            let Some(seg_i) = cap_seg[ch] else { continue };
            let cap_db = self.meter.cap_db[ch];
            let col = cap_glow_color(cap_db);
            let cap_y = db_to_y(METER_MIN_DB + (seg_i + 1) as f32 * seg_db);
            glow_ellipse(
                &painter,
                egui::pos2(x0 + bar_w / 2.0, cap_y),
                egui::vec2(bar_w * 2.0, 13.0),
                col,
                22,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0, cap_y),
                    egui::pos2(x0 + bar_w, cap_y + 2.5),
                ),
                1.0,
                col,
            );
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
            (FADER_MIN_DB + t * (FADER_MAX_DB - FADER_MIN_DB))
                .clamp(FADER_MIN_DB, FADER_MAX_DB)
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

    /// Тест 84 (п.7 р.9): фейдер обязан выдержать ЛЮБОЙ dt из клампа.
    /// Прежняя схема (явный Эйлер) расходилась уже при 30 fps, а после
    /// простоя dt приходит равным 0.1 — фейдер улетал за 400 dB и это
    /// превращалось в усиление 10^20 на выходе движка.
    #[test]
    fn t84_fader_never_diverges_on_bad_dt() {
        let series: Vec<Vec<f32>> = vec![
            vec![0.1; 60],
            vec![1.0 / 30.0; 300],
            vec![0.05; 200],
            // чередование — худший случай для явных схем
            (0..300).map(|i| if i % 2 == 0 { 0.1 } else { 1.0 / 240.0 }).collect(),
        ];
        for dts in series {
            let mut m = MeterState { fader_target: -20.0, ..Default::default() };
            for dt in &dts {
                m.fader_tick(*dt);
                assert!(m.fader_db.is_finite() && m.fader_vel.is_finite(), "NaN/inf при dt={dt}");
                assert!(
                    (FADER_MIN_DB..=FADER_MAX_DB).contains(&m.fader_db),
                    "фейдер улетел: {} dB при dt={dt}",
                    m.fader_db
                );
            }
            assert!((m.fader_db + 20.0).abs() < 0.2, "не сошёлся: {}", m.fader_db);
        }
    }

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
