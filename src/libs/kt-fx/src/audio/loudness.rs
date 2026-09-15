//! Громкость по ITU-R BS.1770-4 / EBU R 128 (Р.49, п.370).
//!
//! Что считается: интегрированная громкость (LUFS), диапазон громкости
//! (LRA, LU) и истинный пик (dBTP).
//!
//! **Почему всё считается в f64 — и почему НЕ по той причине, которая первой
//! приходит в голову.** Коэффициенты K-фильтра выглядят как
//! `a1 = -1.99004745483398`, и соблазнительно сказать «в f32 такой хвост
//! обрезается, а биквад рекурсивен». ЗАМЕРЕНО: обрезание коэффициентов до f32
//! двигает результат не больше чем на 0.002 LU на 20…1000 Гц — при допуске
//! EBU 0.1 LU это не причина.
//!
//! Настоящая причина — СУММЫ. Прямое накопление квадратов в f32 за десять
//! минут тона −20 dBFS даёт −23.38 вместо −23.01, то есть 0.37 LU — втрое
//! больше допуска: аккумулятор вырастает, и очередное слагаемое просто
//! перестаёт в него попадать. Путь офлайновый и не горячий, поэтому в f64
//! держится вся цепочка, а не каждая сумма по отдельности (сторожить
//! «а вот эту сумму не переведи в f32» пришлось бы вечно). Обычный `Biquad`
//! в f32 остаётся у эффектов: там он звучит, а не измеряет.
//!
//! **Гейты, а не среднее.** Абсолютный порог −70 LUFS выбрасывает тишину;
//! относительный −10 LU от предварительного результата выбрасывает паузы
//! между фразами. Без них диалог с паузами измерялся бы тише музыки, сведённой
//! в тот же уровень, и «нормализуй до −23» делало бы речь слишком громкой.
//!
//! **Истинный пик, а не пик сэмплов.** Между сэмплами сигнал бывает выше, чем
//! в них: восстановленная волна проходит НАД точками. Оверсэмплинг ×4
//! (полифазный FIR) ловит этот выброс — иначе «пик −0.1 dBFS» превращается в
//! клиппинг после кодека.

use kt_time::AUDIO_CHANNELS;

/// Биквад двойной точности — для измерений (см. модульный комментарий).
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad64 {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad64 {
    pub fn new(b0: f64, b1: f64, b2: f64, a1: f64, a2: f64) -> Biquad64 {
        Biquad64 { b0, b1, b2, a1, a2, z1: 0.0, z2: 0.0 }
    }

    /// Прямая форма II transposed — та же, что у `Biquad`.
    pub fn step(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// K-фильтр BS.1770: полочный подъём высоких + фильтр высоких частот.
/// Коэффициенты — из спецификации для 48 кГц, ровно как напечатаны.
fn k_filter() -> (Biquad64, Biquad64) {
    let shelf = Biquad64::new(
        1.53512485958697,
        -2.69169618940638,
        1.19839281085285,
        -1.69065929318241,
        0.73248077421585,
    );
    let hp = Biquad64::new(1.0, -2.0, 1.0, -1.99004745483398, 0.99007225036621);
    (shelf, hp)
}

/// Веса каналов BS.1770. Стерео: оба по 1.0 (окружение и тыл весят больше,
/// но их у нас нет — тракт всегда 2 канала, инвариант 1).
const CH_WEIGHT: [f64; 2] = [1.0, 1.0];

/// Блок измерения: 400 мс с перекрытием 75 % (шаг 100 мс).
const BLOCK_MS: usize = 400;
const STEP_MS: usize = 100;
/// Абсолютный гейт.
const ABS_GATE_LUFS: f64 = -70.0;
/// Относительный гейт от предварительной громкости.
const REL_GATE_LU: f64 = -10.0;

/// Итог измерения.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loudness {
    /// Интегрированная громкость, LUFS. `None` — материал целиком ниже
    /// абсолютного гейта (тишина): «−70» тут было бы враньём о числе,
    /// которого нет.
    pub lufs: Option<f64>,
    /// Диапазон громкости, LU (EBU Tech 3342).
    pub lra: f64,
    /// Истинный пик, dBTP.
    pub dbtp: f64,
}

/// Накопитель: кормится кусками аудио в любом порядке нарезки.
///
/// Нарезка на результат НЕ ВЛИЯЕТ — это свойство, а не совпадение: и
/// фильтры, и сетка блоков живут в накопителе, а не в вызове.
pub struct LoudnessMeter {
    filters: [(Biquad64, Biquad64); AUDIO_CHANNELS],
    /// Сумма квадратов текущего шага (100 мс) по каналам.
    step_sum: [f64; AUDIO_CHANNELS],
    step_frames: usize,
    frames_per_step: usize,
    /// Энергии последних шагов — блок это 4 шага подряд.
    steps: std::collections::VecDeque<[f64; AUDIO_CHANNELS]>,
    /// Средние квадраты блоков (для гейтов и LRA).
    blocks: Vec<f64>,
    /// Короткие блоки 3 с для LRA.
    short_steps: std::collections::VecDeque<[f64; AUDIO_CHANNELS]>,
    short_blocks: Vec<f64>,
    peak: f64,
    /// Наибольший модуль СЭМПЛА — им гейтится дорогая интерполяция.
    sample_peak: f32,
    /// Сколько кадров ещё считать интерполяцию после «большого» сэмпла.
    tp_countdown: usize,
    /// Хвост для полифазного оверсэмплинга true-peak.
    tp_tail: [[f32; TP_TAPS]; AUDIO_CHANNELS],
}

/// Длина ядра интерполятора true-peak на фазу.
const TP_TAPS: usize = 8;
/// Оверсэмплинг ×4 — минимум, требуемый BS.1770-4 для 48 кГц.
const TP_OVER: usize = 4;
/// Доля от наибольшего сэмпла, начиная с которой считается интерполяция.
/// 0.7 ≈ −3 dB: межсэмпловый выброс band-limited сигнала не превышает
/// соседний сэмпл больше чем примерно на 1.5 dB, так что запас тройной.
const TP_GATE: f32 = 0.7;

impl Default for LoudnessMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl LoudnessMeter {
    pub fn new() -> LoudnessMeter {
        let sr = kt_time::AUDIO_SAMPLE_RATE as usize;
        LoudnessMeter {
            filters: [k_filter(); AUDIO_CHANNELS],
            step_sum: [0.0; AUDIO_CHANNELS],
            step_frames: 0,
            frames_per_step: sr * STEP_MS / 1000,
            steps: std::collections::VecDeque::new(),
            blocks: Vec::new(),
            short_steps: std::collections::VecDeque::new(),
            short_blocks: Vec::new(),
            peak: 0.0,
            sample_peak: 0.0,
            tp_countdown: 0,
            tp_tail: [[0.0; TP_TAPS]; AUDIO_CHANNELS],
        }
    }

    /// Скормить кусок interleaved-аудио (f32, 48 кГц, 2 канала — инвариант 1).
    pub fn feed(&mut self, data: &[f32]) {
        for frame in data.chunks_exact(AUDIO_CHANNELS) {
            for (ch, &x) in frame.iter().enumerate() {
                let (shelf, hp) = &mut self.filters[ch];
                let y = hp.step(shelf.step(x as f64));
                self.step_sum[ch] += y * y;
                self.true_peak(ch, x);
            }
            self.step_frames += 1;
            if self.step_frames >= self.frames_per_step {
                self.close_step();
            }
        }
    }

    fn close_step(&mut self) {
        let n = self.step_frames.max(1) as f64;
        let mut mean = [0.0f64; AUDIO_CHANNELS];
        for (m, sum) in mean.iter_mut().zip(self.step_sum.iter_mut()) {
            *m = *sum / n;
            *sum = 0.0;
        }
        self.step_frames = 0;

        // блок 400 мс = 4 шага
        self.steps.push_back(mean);
        while self.steps.len() > BLOCK_MS / STEP_MS {
            self.steps.pop_front();
        }
        if self.steps.len() == BLOCK_MS / STEP_MS {
            self.blocks.push(block_power(&self.steps));
        }
        // короткий блок 3 с — для LRA
        self.short_steps.push_back(mean);
        while self.short_steps.len() > 3000 / STEP_MS {
            self.short_steps.pop_front();
        }
        if self.short_steps.len() == 3000 / STEP_MS {
            self.short_blocks.push(block_power(&self.short_steps));
        }
    }

    /// Истинный пик: сигнал между сэмплами бывает выше, чем в них.
    ///
    /// Интерполяция считается НЕ на каждом кадре. Межсэмпловый выброс живёт
    /// рядом с большими сэмплами — это и значит «пик»; в тишине его быть не
    /// может. Поэтому полифазная сумма (восемь тапов на четыре фазы, то есть
    /// 64 умножения на кадр) запускается только вокруг сэмплов, сравнимых с
    /// наибольшим, и держится ещё длину ядра после него, чтобы выброс СПРАВА
    /// от большого сэмпла тоже попал в счёт.
    ///
    /// Без гейта проход по часовому файлу упирался в эту сумму: индексация
    /// материала замедлялась в разы, а она идёт при каждом открытии проекта.
    fn true_peak(&mut self, ch: usize, x: f32) {
        let tail = &mut self.tp_tail[ch];
        tail.rotate_left(1);
        tail[TP_TAPS - 1] = x;
        let a = x.abs();
        if a > self.sample_peak {
            self.sample_peak = a;
        }
        if a >= self.sample_peak * TP_GATE {
            self.tp_countdown = TP_TAPS;
        }
        if self.tp_countdown == 0 {
            return;
        }
        if ch + 1 == AUDIO_CHANNELS {
            self.tp_countdown -= 1;
        }
        let tail = self.tp_tail[ch];
        for phase in 0..TP_OVER {
            let mut acc = 0.0f64;
            for (i, &s) in tail.iter().enumerate() {
                acc += s as f64 * tp_kernel(i, phase);
            }
            self.peak = self.peak.max(acc.abs());
        }
    }

    /// Закрыть измерение. Хвост короче шага дописывается: выбросить его
    /// значило бы не заметить пик в последней десятой доле секунды.
    pub fn finish(mut self) -> Loudness {
        if self.step_frames > 0 {
            self.close_step();
        }
        let dbtp = if self.peak > 0.0 { 20.0 * self.peak.log10() } else { -f64::INFINITY };
        let lufs = gated_lufs(&self.blocks);
        let lra = loudness_range(&self.short_blocks);
        Loudness { lufs, lra, dbtp }
    }
}

fn block_power(steps: &std::collections::VecDeque<[f64; AUDIO_CHANNELS]>) -> f64 {
    let mut sum = 0.0;
    for ch in 0..AUDIO_CHANNELS {
        let mean: f64 = steps.iter().map(|s| s[ch]).sum::<f64>() / steps.len() as f64;
        sum += CH_WEIGHT[ch] * mean;
    }
    sum
}

fn power_to_lufs(p: f64) -> f64 {
    if p <= 0.0 { -f64::INFINITY } else { -0.691 + 10.0 * p.log10() }
}

/// Двухпроходный гейт BS.1770: абсолютный −70 LUFS, потом относительный
/// −10 LU от того, что осталось.
fn gated_lufs(blocks: &[f64]) -> Option<f64> {
    let pass1: Vec<f64> =
        blocks.iter().copied().filter(|p| power_to_lufs(*p) > ABS_GATE_LUFS).collect();
    if pass1.is_empty() {
        return None;
    }
    let prelim = power_to_lufs(pass1.iter().sum::<f64>() / pass1.len() as f64);
    let gate = prelim + REL_GATE_LU;
    let pass2: Vec<f64> = pass1.into_iter().filter(|p| power_to_lufs(*p) > gate).collect();
    if pass2.is_empty() {
        return None;
    }
    Some(power_to_lufs(pass2.iter().sum::<f64>() / pass2.len() as f64))
}

/// LRA (EBU Tech 3342): разница 95-го и 10-го перцентилей коротких блоков,
/// прошедших абсолютный −70 и относительный −20 LU гейты.
fn loudness_range(short: &[f64]) -> f64 {
    let pass1: Vec<f64> =
        short.iter().copied().filter(|p| power_to_lufs(*p) > ABS_GATE_LUFS).collect();
    if pass1.len() < 2 {
        return 0.0;
    }
    let prelim = power_to_lufs(pass1.iter().sum::<f64>() / pass1.len() as f64);
    let gate = prelim - 20.0;
    let mut vals: Vec<f64> = pass1.into_iter().map(power_to_lufs).filter(|l| *l > gate).collect();
    if vals.len() < 2 {
        return 0.0;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| -> f64 {
        let i = ((vals.len() - 1) as f64 * q).round() as usize;
        vals[i]
    };
    (at(0.95) - at(0.10)).max(0.0)
}

/// Полифазное ядро интерполятора: окно Ханна на sinc. Считается один раз на
/// (тап, фазу) — таблица маленькая, а в горячем пути умножений и так восемь.
fn tp_kernel(tap: usize, phase: usize) -> f64 {
    static TABLE: std::sync::OnceLock<[[f64; TP_OVER]; TP_TAPS]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [[0.0f64; TP_OVER]; TP_TAPS];
        let center = (TP_TAPS as f64 - 1.0) / 2.0;
        for (i, row) in t.iter_mut().enumerate() {
            for (p, v) in row.iter_mut().enumerate() {
                let x = i as f64 - center - p as f64 / TP_OVER as f64;
                let s = if x.abs() < 1e-12 {
                    1.0
                } else {
                    (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
                };
                // окно Ханна по длине ядра — без него боковые лепестки
                // добавляют к пику то, чего в сигнале нет
                let w = 0.5
                    - 0.5 * (2.0 * std::f64::consts::PI * (i as f64 + 0.5) / TP_TAPS as f64).cos();
                *v = s * w;
            }
        }
        // нормировка по каждой фазе: постоянный сигнал обязан давать сам себя
        for p in 0..TP_OVER {
            let sum: f64 = (0..TP_TAPS).map(|i| t[i][p]).sum();
            if sum.abs() > 1e-9 {
                for row in t.iter_mut() {
                    row[p] /= sum;
                }
            }
        }
        t
    });
    t[tap][phase]
}

/// Разовое измерение готового буфера.
pub fn measure(data: &[f32]) -> Loudness {
    let mut m = LoudnessMeter::new();
    m.feed(data);
    m.finish()
}
