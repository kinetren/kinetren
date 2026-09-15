//! Тесты 166–168 (docs/porting-notes.md) — регрессия воспроизведения Р.28,
//! жалобы 8.1–8.4 («шумит компьютер», растут underruns, картинка замирает»).
//!
//! Корень: заворот loop-региона рвал клип-локальную позицию FX-клипа, и
//! `ClipProcessor` считал это seek'ом — полный `Freeverb::reset`,
//! пересоздание sinc-ресемплера и пре-ролл ≈1.25 с НА КАЖДОМ КРУГЕ (при
//! регионе короче бёрста — несколько раз за один callback). Пре-ролл при
//! этом читал источник ЗА пределами прогретого окна, и каждый его промах
//! шёл в `underruns`.
//!
//! 166 — FX в loop-регионе: заворот — ШОВ (реверб дозванивает через круг),
//!       промахи прогрева не underrun'ы;
//! 167 — регион короче бёрста: нет ни повторных прогревов, ни лавины чтений;
//! 168 — сложный монтаж (перекрытия, переходы, фейды, FX, реверс): линейный
//!       прогон и луп без единого underrun'а при доступном источнике.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore, RenderStats};
use kt_media::AudioBurst;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const BURST: i64 = AUDIO_BURST_FRAMES as i64;

/// Фетчер поверх DirectFetcher: считает обращения и умеет «не иметь»
/// бёрсты ниже `floor` — модель холодного кэша слева от прогретого окна
/// (именно туда лезет пре-ролл после hard_seek).
struct Probe {
    inner: DirectFetcher,
    floor: i64,
    fetches: u64,
    misses: u64,
}

impl Probe {
    fn new(pattern: AudioPattern, floor: i64) -> Probe {
        let mut inner = DirectFetcher::new(None);
        inner.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
        inner.add_source(
            MediaId(2),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 220.0 })),
        );
        Probe { inner, floor, fetches: 0, misses: 0 }
    }
}

impl AudioFetcher for Probe {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        self.fetches += 1;
        if idx < self.floor {
            self.misses += 1;
            return None;
        }
        self.inner.audio_burst(media, idx)
    }
}

fn base_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "synth:sine".into() });
    p
}

fn clip(id: u64, media: u64, lane: u32, start: i64, dur: i64, src_in: i64) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick::from_audio_frames(start),
        duration: Tick::from_audio_frames(dur),
        source_in: Tick::from_audio_frames(src_in),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn slowed_reverb(rate: f64, mix: f32) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix,
        reverb_delay_ms: 60.0,
        damp: 0.4,
        decay: 0.7,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }
}

/// Один бёрст выхода из позиции `at`, с заворотом по региону.
fn looped_burst(
    r: &AudioRenderer,
    f: &mut dyn AudioFetcher,
    store: &mut FxStateStore,
    region: std::ops::Range<i64>,
    at: i64,
    out: &mut [f32],
) -> RenderStats {
    r.render_looped_with(f, store, region, at, out)
}

// ---- Тест 166: FX в loop-регионе -------------------------------------------

#[test]
fn t166_loop_wrap_is_a_seam_not_a_seek() {
    let mut p = base_project();
    let dur = 10 * AUDIO_SAMPLE_RATE as i64;
    let mut c = clip(1, 1, 0, 0, dur, 0);
    c.fx = vec![slowed_reverb(0.8, 0.5)];
    p.clips.push(c);
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);

    // регион ~2 с со сдвигом от границы бёрста (начало не выровнено);
    // длина кратна бёрсту, чтобы голова второго круга приходилась ровно на
    // границу вызова — иначе её нечем сравнить с головой первого
    let region = {
        let s = 3 * AUDIO_SAMPLE_RATE as i64 + 137;
        s..s + 94 * BURST
    };
    // прогрето ровно с бёрста, с которого начинается ЧТЕНИЕ источника в
    // начале региона (rate 0.8 → источник идёт медленнее выхода); всё, что
    // левее, — «холод»: туда ходит только пре-ролл
    let floor = (region.start as f64 * 0.8) as i64 / BURST;
    let mut f = Probe::new(AudioPattern::Sine { hz: 440.0 }, floor);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let mut out = vec![0f32; BURST as usize * CH];
    // первый бёрст: холодный старт — пре-ролл законен и обязан промахнуться
    let first = looped_burst(&r, &mut f, &mut store, region.clone(), region.start, &mut out);
    assert_eq!(first.underruns, 0, "промахи ПРЕ-РОЛЛА не underrun'ы (8.1)");
    assert!(first.warmup_misses > 0, "пре-ролл после холодного старта обязан быть");
    let head_turn1 = out.clone();

    // крутим ~3 круга; wrap не должен порождать НИ ОДНОГО нового прогрева
    let span = region.end - region.start;
    let turns = 3;
    let bursts = (span * turns) / BURST;
    let mut head_turn2: Option<Vec<f32>> = None;
    let mut pos = region.start + BURST;
    let mut total = RenderStats::default();
    for i in 0..bursts {
        let s = looped_burst(&r, &mut f, &mut store, region.clone(), pos, &mut out);
        total.underruns += s.underruns;
        total.warmup_misses += s.warmup_misses;
        // бёрст, который целиком лёг на начало второго круга
        let tl = region.start + (pos - region.start).rem_euclid(span);
        if head_turn2.is_none() && i > 4 && tl == region.start {
            head_turn2 = Some(out.clone());
        }
        pos += BURST;
    }
    assert_eq!(total.underruns, 0, "underrun'ов на завороте быть не должно (8.1)");
    assert_eq!(
        total.warmup_misses, 0,
        "заворот — ШОВ: ни одного нового пре-ролла за {turns} круга (8.2/8.4)"
    );

    // состояние FX пережило заворот: та же позиция на втором круге звучит
    // ИНАЧЕ, чем при холодном старте — реверб дозванивает через шов
    if let Some(h2) = head_turn2 {
        let diff =
            h2.iter().zip(head_turn1.iter()).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(diff > 1e-6, "хвост реверба обязан переживать заворот (шов, а не reset)");
        assert!(h2.iter().all(|s| s.is_finite() && s.abs() <= 4.0), "шов не даёт выбросов");
    } else {
        panic!("за {turns} круга второй круг обязан был начаться");
    }
}

// ---- Тест 167: регион короче бёрста ----------------------------------------

#[test]
fn t167_short_region_does_not_re_seek_every_wrap() {
    let mut p = base_project();
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let mut c = clip(1, 1, 0, 0, dur, 0);
    c.fx = vec![slowed_reverb(1.0, 0.4)]; // без варпа, но с ревербом
    p.clips.push(c);
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);

    // регион 700 кадров — КОРОЧЕ бёрста: раньше на каждый бёрст выхода
    // приходилось по два hard_seek'а с пре-роллом в 60187 кадров
    let region = AUDIO_SAMPLE_RATE as i64..AUDIO_SAMPLE_RATE as i64 + 700;
    let floor = region.start / BURST;
    let mut f = Probe::new(AudioPattern::Sine { hz: 330.0 }, floor);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let mut out = vec![0f32; BURST as usize * CH];
    let bursts = 20i64;
    let mut total = RenderStats::default();
    let mut first_warmup = 0u64;
    let mut energy = 0f64;
    for i in 0..bursts {
        let s = looped_burst(
            &r,
            &mut f,
            &mut store,
            region.clone(),
            region.start + i * BURST,
            &mut out,
        );
        total.underruns += s.underruns;
        // первый бёрст — законный холодный пре-ролл, дальше прогревов нет
        if i == 0 {
            first_warmup = s.warmup_misses;
        } else {
            total.warmup_misses += s.warmup_misses;
        }
        energy += out.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
    }
    assert!(first_warmup > 0, "холодный старт обязан прогреться");
    assert_eq!(total.underruns, 0, "короткий регион не должен давать underrun'ов (8.4)");
    assert_eq!(total.warmup_misses, 0, "повторных прогревов на завороте нет");
    assert!(energy > 1.0, "воспроизведение не должно вырождаться в тишину (8.3)");
    // Лавина чтений: один hard_seek — это ≈59 бёрстов пре-ролла, а на 20
    // бёрстов выхода приходится ~30 заворотов. Реальный расход — единицы
    // чтений на бёрст (регион целиком лежит в двух source-бёрстах).
    assert!(
        f.fetches < 20 * bursts as u64,
        "лавина чтений источника: {} обращений на {bursts} бёрстов",
        f.fetches
    );
}

// ---- Тест 168: сложный монтаж ----------------------------------------------

#[test]
fn t168_complex_montage_plays_without_underruns() {
    let sr = AUDIO_SAMPLE_RATE as i64;
    let mut p = base_project();
    p.overlap_layers = true;
    // 12 клипов: две дорожки, перекрытия, FX, реверс, разные gain'ы
    for i in 0..6i64 {
        let mut c = clip(1 + i as u64, 1 + (i % 2) as u64, 0, i * sr - i * 900, sr + 400, i * 1000);
        c.gain_db = -3.0 * (i % 3) as f32;
        if i % 2 == 0 {
            c.fx = vec![slowed_reverb(if i == 0 { 0.75 } else { 1.0 }, 0.35)];
        }
        c.reversed = i == 3;
        p.clips.push(c);
    }
    for i in 0..6i64 {
        let mut c =
            clip(10 + i as u64, 2 - (i % 2) as u64, 1, i * sr + 300, sr - 200, i * 700 + 50);
        c.gain_db = -1.5;
        p.clips.push(c);
    }
    // кроссфейд на стыке пары клипов второй дорожки + фейд у первого клипа
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: None,
        b: Some(ClipId(10)),
        in_offset: Tick::ZERO,
        out_offset: Tick::from_audio_frames(2000),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
    });
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);

    let mut f = Probe::new(AudioPattern::Ramp, i64::MIN);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; BURST as usize * CH];

    // 1) линейный прогон всего монтажа
    let end = p.clips.iter().map(|c| (c.start + c.duration).to_audio_frames()).max().unwrap();
    let mut total = RenderStats::default();
    let mut pos = 0i64;
    while pos < end {
        let s = r.render_frames_with(&mut f, &mut store, pos, &mut out);
        total.underruns += s.underruns;
        pos += BURST;
    }
    assert_eq!(total.underruns, 0, "полный монтаж при доступном источнике — без underrun'ов");

    // 2) луп через стык с переходом и перекрытиями, 3 круга
    let region = sr + 12_000..3 * sr + 5_000;
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut looped = RenderStats::default();
    let turns = 3;
    let bursts = ((region.end - region.start) * turns) / BURST;
    for i in 0..bursts {
        let s = looped_burst(
            &r,
            &mut f,
            &mut store,
            region.clone(),
            region.start + i * BURST,
            &mut out,
        );
        looped.underruns += s.underruns;
        if i > 0 {
            looped.warmup_misses += s.warmup_misses;
        }
    }
    assert_eq!(looped.underruns, 0, "луп по сложному монтажу — без underrun'ов (8.2)");
}

// ---- Окно упреждения префетча (п.167) --------------------------------------

#[test]
fn t167b_prefetch_lookahead_stays_inside_loop_region() {
    use kt_engine::lookahead_windows;
    // курсор вне региона — линейное окно
    assert_eq!(lookahead_windows(10, 100, Some((500, 900))), vec![10..110]);
    assert_eq!(lookahead_windows(10, 100, None), vec![10..110]);
    // внутри региона — до конца и заворот в голову
    assert_eq!(lookahead_windows(800, 300, Some((500, 900))), vec![800..900, 500..700]);
    // окно короче остатка региона — один диапазон
    assert_eq!(lookahead_windows(600, 100, Some((500, 900))), vec![600..700]);
    // окно длиннее круга — не больше круга (иначе десятки заказов одного)
    assert_eq!(lookahead_windows(500, 10_000, Some((500, 900))), vec![500..900]);
    // грань конца региона инклюзивна (семантика транспорта, п.132):
    // курсор на конце — это уже голова следующего круга
    assert_eq!(lookahead_windows(900, 100, Some((500, 900))), vec![500..600]);
}

// ---- Тест 175: «чёрный ящик» воспроизведения (п.10) ------------------------

/// Пользователь: «нужно продумать хитрую систему логирования, которая
/// поможет будущим агентам с плавающими багами воспроизведения, нужно
/// компактно это делать, т.к. я могу взять и попрыгать по нескольким
/// супер-мелким loop-регионам что могло бы породить тонны логов».
/// Проверяем ровно это: тысяча событий не рождает тысячу строк.
#[test]
fn t175_flight_recorder_stays_compact() {
    use kt_engine::flight::{CAP, Ev, FlightRecorder, Snap};
    let f = FlightRecorder::new();

    // 1000 заворотов одного лупа — ОДНА строка со счётчиком
    for _ in 0..1000 {
        f.push_at(0, Ev::Wrap, 48_000, 0, Snap::default());
    }
    let recs = f.records();
    assert_eq!(recs.len(), 1, "подряд идущие одинаковые события дедуплицируются");
    assert_eq!(recs[0].n, 1000);
    assert!(f.dump().contains("×1000"), "повторы видны числом: {}", f.dump());

    // прыжки по РАЗНЫМ мелким регионам всё равно ограничены кольцом
    for i in 0..(CAP as i64 * 4) {
        f.push_at(i as u64, Ev::Region, i * 100, i * 100 + 700, Snap::default());
    }
    assert_eq!(f.records().len(), CAP, "кольцо не растёт: {} записей", f.records().len());

    // пустой снимок не пишется вообще (в тишине журнала нет)
    f.clear();
    f.push_snap(Snap { head_frames: 48_000, ..Default::default() });
    assert!(f.records().is_empty(), "снимок без единого промаха не пишется");
    f.push_snap(Snap { render_underruns: 3, head_frames: 48_000, ..Default::default() });
    assert_eq!(f.records().len(), 1, "снимок с промахами пишется");

    // дамп читается человеком и содержит суть
    f.clear();
    f.push_at(0, Ev::Play, 0, 0, Snap::default());
    f.push_at(100, Ev::Region, 48_000, 96_000, Snap::default());
    f.push_at(200, Ev::Stall, 1200, 48_000, Snap::default());
    let d = f.dump();
    assert!(d.contains("PLAY"), "{d}");
    assert!(d.contains("REGION") && d.contains("1.000..2.000"), "регион в секундах: {d}");
    assert!(d.contains("STALL") && d.contains("1200 ms"), "залипание с длительностью: {d}");
}

// ---- Тест 280: единица underrun'ов одна на оба пути (Р.43, п.289) ----------

/// Претензия пользователя: «конкретно в этом месте underruns очень давно
/// сломался, мы не будем bisect делать».
///
/// Bisect и не нужен: это не регресс, а РАЗНЫЕ ЕДИНИЦЫ с самого начала.
/// `render_plain_clip` (клип без FX) считал промах КАДРАМИ тела, а
/// `fill_source` (клип с FX) — по единице за промах БЁРСТА. Одно и то же
/// голодание давало числа, различающиеся в `AUDIO_BURST_FRAMES` = 1024 раза,
/// и сравнивать их было нельзя ни между событиями, ни во времени.
///
/// Тест утверждает СЛЕДСТВИЕ: два одинаковых события на полностью холодном
/// источнике дают сопоставимые числа. Точного равенства не требуем — путь с
/// FX тянет источник через варп и режет чтения иначе, — но разница обязана
/// быть в разы, а не в тысячу раз.
#[test]
fn t280_underruns_are_counted_in_frames_on_both_paths() {
    let sr = AUDIO_SAMPLE_RATE as i64;
    let span = sr / 2; // полсекунды тела

    let mut plain = base_project();
    plain.clips.push(clip(1, 1, 0, 0, span, 0));
    plain.validate().unwrap();

    let mut fxp = base_project();
    let mut c = clip(1, 1, 0, 0, span, 0);
    c.fx = vec![slowed_reverb(1.0, 0.0)];
    fxp.clips.push(c);
    fxp.validate().unwrap();

    // источник недоступен ЦЕЛИКОМ: floor выше любого бёрста
    let run = |p: &Project| -> u64 {
        let r = AudioRenderer::new(p);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut f = Probe::new(AudioPattern::Ramp, i64::MAX);
        let mut out = vec![0f32; BURST as usize * CH];
        let mut total = 0u64;
        let mut pos = 0i64;
        while pos < span {
            total += r.render_frames_with(&mut f, &mut store, pos, &mut out).underruns;
            pos += BURST;
        }
        total
    };

    let a = run(&plain);
    let b = run(&fxp);
    assert!(a > 0 && b > 0, "полностью холодный источник обязан дать underrun'ы: {a} и {b}");
    // оба — в кадрах, поэтому оба одного порядка с длиной тела
    let (lo, hi) = (a.min(b), a.max(b));
    assert!(
        hi <= lo * 4,
        "единица счётчика одна на оба пути: без FX {a}, с FX {b} \
         (до Р.43 было ~{span} против ~{}, разница в 1024 раза)",
        span / BURST
    );
    assert!(
        lo >= span as u64 / 4,
        "счётчик в КАДРАХ: на {span} кадрах тела ожидаем сотни, а не единицы — {a}/{b}"
    );
}
