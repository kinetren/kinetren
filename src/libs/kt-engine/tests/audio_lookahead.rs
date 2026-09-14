//! Тесты 334-336 (Р.48, пп.337-338): СКВОЗНОЕ УПРЕЖДЕНИЕ цепочки.
//!
//! Смысл раунда одной фразой: эффект с окном анализа читает источник
//! ВПЕРЁД, а выход отдаёт выровненным — для остального движка задержки
//! нет. Три утверждения, по одному на каждый способ это сломать:
//! t334 — выход не сдвинулся; t335 — окно и вправду работает (потолок
//! соблюдён, редукция успевает до пика, событие не звучит дольше);
//! t336 — префетч греет то, что цепочка реально прочитает.

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 2;

/// Линейный режим лимитера: порог и потолок на нуле дБ — множитель
/// тождественно 1.0 на сигнале ниже единицы, и остаётся только сдвиг
/// (если он есть). Ради этого свойства тест и существует.
fn limiter_linear() -> FxInstance {
    FxInstance::Limiter {
        threshold_db: 0.0,
        ceiling_db: 0.0,
        lookahead_ms: 5.0,
        release_ms: 80.0,
    }
}

fn project(fx: Vec<FxInstance>, pattern: &str) -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 64, height: 36, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: pattern.into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick::from_audio_frames(DUR),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn source(pattern: AudioPattern) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
    f
}

/// Рендер клипа одним куском от позиции `from`.
fn render(fx: Vec<FxInstance>, pattern: AudioPattern, from: i64) -> Vec<f32> {
    let p = project(fx, "synth:clicks");
    let r = AudioRenderer::new(&p);
    let mut f = source(pattern);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; (DUR - from) as usize * CH];
    let stats = r.render_frames_with(&mut f, &mut store, from, &mut out);
    assert_eq!(stats.underruns, 0, "источник доступен целиком — промахов быть не должно");
    out
}

/// Тот же рендер, но запрошенный несколькими кусками подряд (проверка,
/// что состояние не зависит от нарезки буферов — класс t325а).
fn render_chunked(fx: Vec<FxInstance>, pattern: AudioPattern, chunks: &[i64]) -> Vec<f32> {
    let p = project(fx, "synth:clicks");
    let r = AudioRenderer::new(&p);
    let mut f = source(pattern);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = Vec::new();
    let mut pos = 0i64;
    for &n in chunks {
        let mut buf = vec![0f32; n as usize * CH];
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        out.extend_from_slice(&buf);
        pos += n;
    }
    out
}

fn peak(s: &[f32]) -> f32 {
    s.iter().fold(0f32, |m, v| m.max(v.abs()))
}

/// Тест 334: УПРЕЖДЕНИЕ НЕ СДВИГАЕТ ВЫХОД.
///
/// Лимитер с окном 5 мс (240 кадров) в линейном режиме обязан отдать
/// ровно то же, что цепочка без него — сэмпл в сэмпл, а не «похоже».
/// Это и есть контракт PDC: цена окна платится чтением источника вперёд,
/// а не задержкой звука. (Red-proof: убрать `+ la` из входной позиции
/// чанка в `produce` — выход уезжает на 240 кадров, тест краснеет.)
#[test]
fn t334_lookahead_does_not_shift_the_output() {
    let dry = render(vec![], AudioPattern::Clicks, 0);
    let lim = render(vec![limiter_linear()], AudioPattern::Clicks, 0);
    assert_eq!(dry.len(), lim.len());
    assert_eq!(
        dry, lim,
        "линейный лимитер обязан быть тождеством: упреждение платится источником, не сдвигом"
    );

    // и то же на варпнутой цепочке: вход ведёт варп, и позиция чанка
    // считается в его домене
    let warped = vec![FxInstance::slowed_reverb(0.8, 0.0, 60.0)];
    let mut with_lim = warped.clone();
    with_lim.push(limiter_linear());
    let a = render(warped, AudioPattern::Clicks, 0);
    let b = render(with_lim, AudioPattern::Clicks, 0);
    assert_eq!(a, b, "линейный лимитер за варпом тоже обязан быть тождеством");

    // нарезка буферов на результат не влияет (упреждение живёт в кольце
    // узла, а не в счётчике вызовов)
    let one = render(vec![FxInstance::limiter()], AudioPattern::Clicks, 0);
    let many = render_chunked(
        vec![FxInstance::limiter()],
        AudioPattern::Clicks,
        &[1000, 17, 48_000, DUR - 49_017],
    );
    assert_eq!(one, many, "рендер куском обязан совпадать с рендером кусками");

    // сила слоя поверх упреждающего узла не двоит материал: сухая
    // сторона задерживается ровно на съеденное узлом окно
    let mixed = render(
        vec![
            FxInstance::limiter(),
            FxInstance::LayerMix { amount: 0.5, from: 0 },
        ],
        AudioPattern::Clicks,
        0,
    );
    let dry_c = render(vec![], AudioPattern::Clicks, 0);
    let wet = render(vec![FxInstance::limiter()], AudioPattern::Clicks, 0);
    for (i, ((m, d), w)) in mixed.iter().zip(&dry_c).zip(&wet).enumerate() {
        let want = 0.5 * d + 0.5 * w;
        assert!(
            (m - want).abs() <= 1e-6,
            "сэмпл {i}: смешение взяло разные моменты материала ({m} вместо {want})"
        );
    }
}

/// Тест 335: ОКНО РАБОТАЕТ.
///
/// Потолок соблюдается строго (окно максимума включает сам выходной
/// кадр), редукция начинается ДО пика — за это и платят упреждением, —
/// а событие при этом не начинает звучать дольше: упреждение не входит
/// в хвост.
#[test]
fn t335_limiter_ceiling_and_no_tail_growth() {
    let lim = FxInstance::Limiter {
        threshold_db: -12.0,
        ceiling_db: -6.0,
        lookahead_ms: 5.0,
        release_ms: 200.0,
    };
    let la = (5.0 / 1000.0 * 48_000.0) as usize;
    let out = render(vec![lim.clone()], AudioPattern::Clicks, 0);
    let ceiling = 10f32.powf(-6.0 / 20.0);
    let p = peak(&out);
    println!("t335 peak {p:.4} ceiling {ceiling:.4}");
    assert!(p <= ceiling + 1e-4, "потолок пробит: пик {p}, потолок {ceiling}");

    // Редукция успевает ДО пика — за это и платят упреждением. Меряем
    // по ФОНУ (ClicksBed: щелчки поверх тихого синуса): на нулевом фоне
    // гейн не виден, делить было бы не на что.
    let dry = render(vec![], AudioPattern::ClicksBed, 0);
    let wet = render(vec![lim.clone()], AudioPattern::ClicksBed, 0);
    let click = dry
        .chunks_exact(CH)
        .enumerate()
        .skip(48_000 / 2)
        .find(|(_, fr)| fr[0].abs() > 0.3 || fr[1].abs() > 0.3)
        .map(|(i, _)| i)
        .expect("стимул обязан содержать щелчок");
    // окно [пик − la, пик): гейн здесь обязан быть ниже, чем далеко до
    // пика (там лимитер отпустил и держит makeup)
    // Гейн меряем по ЭНЕРГИИ окна в 64 кадра: фон 200 Гц каждые 120
    // кадров проходит через ноль, и отношение в одной точке там
    // неопределено (первая версия теста ловила именно эти нули).
    let gain_at = |center: usize| -> f64 {
        let (a, b) = (center.saturating_sub(32), (center + 32).min(DUR as usize));
        let en = |s: &[f32]| -> f64 {
            s[a * CH..b * CH].iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>()
        };
        (en(&wet) / en(&dry).max(1e-30)).sqrt()
    };
    let near = gain_at(click - la / 2);
    let far = gain_at(click - la * 4);
    println!("t335 gain near {near:.3} (за {} кадров), far {far:.3}", la / 2);
    assert!(
        near < far * 0.9,
        "лимитер обязан снизить громкость ДО пика: {near:.3} против {far:.3} вдали"
    );

    // ЧУЖОЙ МАТЕРИАЛ ЗА КОНЦОМ СОБЫТИЯ не влияет на его звук: гейт
    // считается во ВХОДНОМ домене, поэтому за концом материала цепочка
    // получает тишину, а не то, что лежит в файле дальше. Иначе щелчок
    // сразу за границей приглушил бы последние кадры события — окно
    // лимитера его «увидело» бы. (Стимул: щелчки каждые 4800 кадров, и
    // DUR кратен 4800 — щелчок стоит ровно на границе.)
    let end_gain = gain_at(DUR as usize - la / 2);
    println!("t335 gain at the very end {end_gain:.3}");
    assert!(
        end_gain > far * 0.9,
        "материал за концом события приглушил его конец: {end_gain:.3} против {far:.3}"
    );

    // хвост: упреждение НЕ продлевает событие — за его концом тишина
    let auto = kt_fx::AutoProfiles::from_curves_rev(&[vec![]], 0, DUR, false);
    assert_eq!(
        kt_fx::chain_tail(std::slice::from_ref(&lim), &auto),
        Tick::ZERO,
        "у лимитера нет хвоста: окно анализа — это не дозванивание"
    );
    assert!(
        kt_fx::chain_lookahead(&[lim], &auto) > 0,
        "но упреждение у него есть — иначе тест ничего не проверяет"
    );
}

/// Обёртка-логгер: пишет каждый прочитанный burst.
struct Recording {
    inner: DirectFetcher,
    log: Vec<(MediaId, i64)>,
}

impl AudioFetcher for Recording {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<kt_media::AudioBurst>> {
        self.log.push((media, idx));
        self.inner.audio_burst(media, idx)
    }
}

/// Тест 336: ПРЕФЕТЧ ГРЕЕТ ТО, ЧТО ЦЕПОЧКА ПРОЧИТАЕТ.
///
/// Окно источника расширяется вперёд на упреждение в ОДНОЙ функции —
/// той, которую спрашивают и рендер, и префетч. Если расширить только у
/// рендера, кэш будет греть не те бёрсты, и разъезд не даст ни ошибки,
/// ни underrun'а: за окном источник отдаёт тишину молча. Поэтому тест
/// проверяет включение прочитанного в wanted, а не только звук.
/// (Red-proof: убрать `+ chain_lookahead` из `chain_source_window_ext`.)
#[test]
fn t336_prefetch_window_covers_the_lookahead() {
    let p = project(vec![FxInstance::limiter()], "synth:clicks");
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    for &pos in &[0i64, 4096, DUR / 2, DUR - 48_000] {
        store.reset_all();
        let frames = 48_000usize.min((DUR - pos) as usize);
        let mut fetcher = Recording { inner: source(AudioPattern::Clicks), log: Vec::new() };
        let mut out = vec![0f32; frames * CH];
        r.render_frames_with(&mut fetcher, &mut store, pos, &mut out);

        let wanted = r.wanted_audio_bursts(pos..pos + frames as i64);
        for &(media, idx) in &fetcher.log {
            let covered = wanted.iter().any(|(m, range)| *m == media && range.contains(&idx));
            assert!(
                covered,
                "burst {idx} прочитан цепочкой с упреждением, но не входит в wanted({pos}..{})",
                pos + frames as i64
            );
        }
    }

    // и наоборот: прогрев строго по wanted даёт рендер без промахов и
    // байт-в-байт равный холодному
    let pos = DUR / 3;
    store.reset_all();
    let mut cold = vec![0f32; 48_000 * CH];
    r.render_frames_with(&mut source(AudioPattern::Clicks), &mut store, pos, &mut cold);

    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let mut warm = DirectFetcher::new(Some(cache.clone()));
    warm.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)),
    );
    for (media, range) in r.wanted_audio_bursts(pos..pos + 48_000) {
        for idx in range {
            warm.audio_burst(media, idx);
        }
    }
    let mut only_cache = kt_engine::CacheFetcher::new(cache);
    store.reset_all();
    let mut hot = vec![0f32; 48_000 * CH];
    let stats = r.render_frames_with(&mut only_cache, &mut store, pos, &mut hot);
    assert_eq!(stats.underruns, 0, "прогрев по wanted обязан покрывать чтение цепочки");
    assert_eq!(hot, cold, "тот же кусок из прогретого кэша обязан быть байт-в-байт");
}
