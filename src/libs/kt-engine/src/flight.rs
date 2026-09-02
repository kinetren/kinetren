//! «Чёрный ящик» воспроизведения (Р.28, п.10).
//!
//! Просьба пользователя дословно: «нужно продумать хитрую систему
//! логирования, которая поможет будущим агентам с плавающими багами
//! воспроизведения, нужно компактно это делать, т.к. я могу взять и
//! попрыгать по нескольким супер-мелким loop-регионам что могло бы
//! породить тонны логов».
//!
//! Отсюда три правила:
//! 1. **Кольцо фиксированного размера.** Сколько ни прыгай — памяти ровно
//!    `CAP` записей, самые старые вытесняются. Ни файла на диске во время
//!    игры, ни роста RAM.
//! 2. **Дедупликация подряд идущих.** Одинаковое событие (тот же вид и те
//!    же поля) не плодит записи, а наращивает счётчик `n` и время
//!    последнего повтора: сто заворотов одного и того же лупа — ОДНА
//!    строка `WRAP … ×100`.
//! 3. **Снимок счётчиков раз в секунду, и только если что-то изменилось.**
//!    В тишине журнал не растёт вообще.
//!
//! Пишет ТОЛЬКО render/prefetch-поток. В cpal-колбэк рекордер не заходит
//! (инвариант №6: ни аллокаций, ни локов) — счётчики колбэка снимаются из
//! атомиков при очередном снимке.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

/// Ёмкость кольца. 512 записей — это десятки секунд самой бурной работы
/// (прыжки + снимки), в дампе ~40 КБ текста.
pub const CAP: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ev {
    Play,
    Pause,
    /// Прыжок транспорта (generation): a = кадр назначения.
    Seek,
    /// Смена loop-региона: a..b в кадрах (a<0 — регион снят).
    Region,
    /// Заворот региона: a = кадр, с которого начался круг.
    Wrap,
    /// Смена направления: a = знак.
    Dir,
    /// Мини-seek без бампа generation (флип armed/дизарм, смена знака).
    Reset,
    /// Снимок счётчиков (см. `Snap`).
    Snap,
    /// Замирание: playhead не двигался, хотя игра идёт. a = мс простоя.
    Stall,
}

impl Ev {
    fn tag(self) -> &'static str {
        match self {
            Ev::Play => "PLAY",
            Ev::Pause => "PAUSE",
            Ev::Seek => "SEEK",
            Ev::Region => "REGION",
            Ev::Wrap => "WRAP",
            Ev::Dir => "DIR",
            Ev::Reset => "RESET",
            Ev::Snap => "SNAP",
            Ev::Stall => "STALL",
        }
    }
}

/// Счётчики снимка — дельты за секунду, а не абсолютные значения: именно
/// дельта показывает «левое число подрастает каждый повтор».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Snap {
    pub render_underruns: u64,
    pub callback_underruns: u64,
    pub warmup_misses: u64,
    pub head_frames: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rec {
    /// Мс от старта записи (монотонно).
    pub at_ms: u64,
    pub kind: Ev,
    pub a: i64,
    pub b: i64,
    pub snap: Snap,
    /// Сколько раз подряд повторилось (дедуп).
    pub n: u32,
}

pub struct FlightRecorder {
    ring: Mutex<VecDeque<Rec>>,
    t0: Instant,
}

impl Default for FlightRecorder {
    fn default() -> Self {
        FlightRecorder::new()
    }
}

impl FlightRecorder {
    pub fn new() -> FlightRecorder {
        FlightRecorder { ring: Mutex::new(VecDeque::with_capacity(CAP)), t0: Instant::now() }
    }

    pub fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    /// Событие без счётчиков.
    pub fn push(&self, kind: Ev, a: i64, b: i64) {
        self.push_at(self.now_ms(), kind, a, b, Snap::default());
    }

    /// Снимок счётчиков. Пустой (все дельты нулевые и голова не двигалась)
    /// не пишется — в тишине журнал не растёт.
    pub fn push_snap(&self, s: Snap) {
        if s.render_underruns == 0 && s.callback_underruns == 0 && s.warmup_misses == 0 {
            return;
        }
        self.push_at(self.now_ms(), Ev::Snap, 0, 0, s);
    }

    pub fn push_at(&self, at_ms: u64, kind: Ev, a: i64, b: i64, snap: Snap) {
        let mut ring = self.ring.lock().unwrap();
        // дедуп подряд идущих: «×N» вместо N строк (правило 2)
        if let Some(last) = ring.back_mut()
            && last.kind == kind
            && last.a == a
            && last.b == b
            && last.snap == snap
        {
            last.n += 1;
            last.at_ms = at_ms;
            return;
        }
        if ring.len() == CAP {
            ring.pop_front();
        }
        ring.push_back(Rec { at_ms, kind, a, b, snap, n: 1 });
    }

    pub fn records(&self) -> Vec<Rec> {
        self.ring.lock().unwrap().iter().copied().collect()
    }

    pub fn clear(&self) {
        self.ring.lock().unwrap().clear();
    }

    /// Текстовый дамп: одна строка на запись, время в секундах от старта,
    /// кадры — в секундах (48 кГц), чтобы читалось без калькулятора.
    pub fn dump(&self) -> String {
        let sec = |f: i64| f as f64 / kt_time::AUDIO_SAMPLE_RATE as f64;
        let mut out = String::new();
        for r in self.records() {
            out.push_str(&format!("{:8.3} {:6}", r.at_ms as f64 / 1000.0, r.kind.tag()));
            match r.kind {
                Ev::Snap => out.push_str(&format!(
                    " u={}/{} w={} head={:.3}",
                    r.snap.render_underruns,
                    r.snap.callback_underruns,
                    r.snap.warmup_misses,
                    sec(r.snap.head_frames)
                )),
                Ev::Region => {
                    if r.a < 0 {
                        out.push_str(" off");
                    } else {
                        out.push_str(&format!(" {:.3}..{:.3}", sec(r.a), sec(r.b)));
                    }
                }
                Ev::Stall => out.push_str(&format!(" {} ms head={:.3}", r.a, sec(r.b))),
                Ev::Play | Ev::Pause | Ev::Seek | Ev::Wrap | Ev::Reset => {
                    out.push_str(&format!(" head={:.3}", sec(r.a)))
                }
                Ev::Dir => out.push_str(if r.a < 0 { " reverse" } else { " forward" }),
            }
            if r.n > 1 {
                out.push_str(&format!(" ×{}", r.n));
            }
            out.push('\n');
        }
        out
    }
}
