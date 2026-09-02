//! Тест 113 (п.14 р.14): кадры в ОФЛАЙН-рендере (тот же путь, что экспорт)
//! идут строго вперёд, равномерно и без чёрных дыр — включая случай, когда
//! fps проекта не совпадает с fps источника.
//!
//! Пользователь: «подлагивание есть и в отрендеренных видео всегда». Одна
//! причина была в декодере (тест 112 на реальном файле), вторая проверяется
//! здесь: маппинг «тик проекта → кадр источника» и покадровая выдача
//! экспорта. Синтетика включена в режим собственной сетки кадров — цвет
//! кадра кодирует НОМЕР кадра источника, поэтому из готового RGBA видно,
//! какой именно кадр попал в вывод.

use kt_engine::export::render_raw_video;
use kt_media::MediaSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

const DUR: Tick = Tick(90_000 * 3); // 3 с

fn project(fps: u32, reversed: bool) -> Project {
    let mut p = Project::new(ProjectSettings { fps, width: 64, height: 36, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: DUR,
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx_bypass: vec![],
        fx: vec![],
        opacity: 1.0,
    });
    p.validate().unwrap();
    p
}

fn sources(src_fps: u32) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(DUR, AudioPattern::Silence).with_video_fps(src_fps))
            as Box<dyn MediaSource>,
    )]
}

/// Номера кадров источника, попавшие в вывод (цвет кодирует номер).
fn rendered_indices(project_fps: u32, src_fps: u32, reversed: bool) -> Vec<i64> {
    let p = project(project_fps, reversed);
    let q = kt_media::Quality { width: 64, height: 36 };
    let mut out = Vec::new();
    render_raw_video(&p, sources(src_fps), q, |rgba| {
        let idx = rgba[0] as i64 | (rgba[1] as i64) << 8 | (rgba[2] as i64) << 16;
        out.push(idx);
        Ok(())
    })
    .expect("офлайн-рендер");
    out
}

#[test]
fn t113_export_frames_are_monotonic_and_uniform() {
    // --- сетки НЕ совпадают: проект 60, источник 25
    let idx = rendered_indices(60, 25, false);
    assert_eq!(idx.len(), 180, "3 с при 60 fps = 180 кадров, получили {}", idx.len());
    assert!(idx.iter().all(|&i| i >= 0), "чёрный кадр (индекс не прочитан)");
    // 1) НИКОГДА не назад
    for w in idx.windows(2) {
        assert!(w[1] >= w[0], "кадр откатился назад: {} → {}", w[0], w[1]);
    }
    // 2) без пропусков: соседние кадры либо тот же, либо следующий
    for w in idx.windows(2) {
        assert!(w[1] - w[0] <= 1, "пропущен кадр источника: {} → {}", w[0], w[1]);
    }
    // 3) равномерность: длины серий одинаковых кадров отличаются ≤ 1
    let mut runs: Vec<usize> = Vec::new();
    let mut cur = idx[0];
    let mut len = 0usize;
    for &i in &idx {
        if i == cur {
            len += 1;
        } else {
            runs.push(len);
            cur = i;
            len = 1;
        }
    }
    runs.push(len);
    // крайние серии могут быть обрезаны границами диапазона — их не судим
    let mid = &runs[1..runs.len() - 1];
    let lo = mid.iter().min().copied().unwrap();
    let hi = mid.iter().max().copied().unwrap();
    assert!(hi - lo <= 1, "неравномерные дубли: серии от {lo} до {hi} ({runs:?})");

    // --- сетки совпадают: строго 1:1
    let same = rendered_indices(30, 30, false);
    assert_eq!(same.len(), 90);
    for (n, &i) in same.iter().enumerate() {
        assert_eq!(i, n as i64, "при равных сетках кадр {n} обязан быть 1:1, получили {i}");
    }

    // --- реверс: строго назад, тоже без дыр
    let rev = rendered_indices(30, 30, true);
    assert_eq!(rev.len(), 90);
    for w in rev.windows(2) {
        assert!(w[1] <= w[0], "реверс: кадр ушёл вперёд {} → {}", w[0], w[1]);
        assert!(w[0] - w[1] <= 1, "реверс: пропущен кадр {} → {}", w[0], w[1]);
    }
}

/// Тест 115 (п.14 р.14): при МЕДЛЕННОМ воспроизведении (0.25×) ключ
/// показанного кадра не откатывается назад — интеграционно, через живой
/// `Player` с фейковым аудио-колбэком (звуковое железо не нужно).
#[test]
fn t115_preview_keys_are_monotonic_at_quarter_rate() {
    use kt_engine::Player;
    use kt_engine::player::preview_key_step;

    let p = project(30, false);
    let (player, mut ring) = Player::new(
        p,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 30), AudioPattern::Ramp)) as _,
        )],
    );
    player.seek(Tick(0));
    player.set_playback_rate(0.25);
    player.play();

    let grid = player.video_grid();
    let step = grid.frame_step();
    let mut prev: Option<i64> = None;
    let mut rollbacks = 0u64;
    let start = std::time::Instant::now();
    let mut consumed = 0usize;
    // ~1.5 с реального времени: колбэк по 1024 кадра, между ними
    // несколько «UI-кадров» с чтением позиции показа
    while start.elapsed().as_millis() < 1500 {
        for _ in 0..1024 {
            if ring.pop().is_err() {
                break;
            }
        }
        consumed += 1;
        player
            .shared
            .consumed_frames
            .fetch_add(256, std::sync::atomic::Ordering::AcqRel);
        for _ in 0..4 {
            let t = player.video_playhead();
            let key = grid.align_floor(t).0;
            let (key, _, rolled) = preview_key_step(prev, key, step, 1, false);
            if rolled {
                rollbacks += 1;
            }
            if let Some(pv) = prev {
                assert!(key >= pv, "ключ показа уехал назад: {pv} → {key}");
            }
            prev = Some(key);
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
    }
    assert!(consumed > 10, "колбэк не крутился: {consumed}");
    // ратчет — страховка, а не рабочий режим: при исправных часах он
    // срабатывает единицами раз, при дрожащих — постоянно
    assert!(rollbacks < 20, "часы показа дрожат при 0.25×: {rollbacks} откатов");
}

/// Тест 116 (п.1 р.15): рендер не выдаёт ЧЁРНЫЕ кадры на большом
/// разрешении.
///
/// Регрессия: экспорт декодировал пачками по 120 кадров в кэш на 64 МБ.
/// При 640×360 пачка весит 111 МБ, при 1080p — 995 МБ: LRU выбрасывал
/// кадры ДО композиции, и рендер выходил чёрным с редкими уцелевшими
/// кадрами. На 320×180 пачка влезала — поэтому живая проверка прошлого
/// раунда дефект не увидела.
#[test]
fn t116_export_has_no_black_frames_at_large_quality() {
    use kt_engine::export::{EXPORT_VIDEO_CACHE_BYTES, export_chunk_frames};

    // чистая функция: пачка не должна превышать половину бюджета
    for (w, h) in [(320u32, 180u32), (640, 360), (1280, 720), (1920, 1080)] {
        let n = export_chunk_frames(EXPORT_VIDEO_CACHE_BYTES, w, h);
        assert!(n >= 1, "{w}×{h}: пачка выродилась в 0 кадров");
        let bytes = n as usize * w as usize * h as usize * 4;
        assert!(
            bytes * 2 <= EXPORT_VIDEO_CACHE_BYTES,
            "{w}×{h}: пачка {n} кадров = {} МБ не влезает в бюджет кэша",
            bytes / 1_000_000
        );
    }

    // живой рендер на разрешении, где старый чанк гарантированно не влезал
    // в кэш (640×360 × 120 кадров = 111 МБ). Признак выброшенного кадра —
    // не «чернота» (индекс 0 сам по себе чёрный), а НЕДОСТАЮЩИЙ СЛОЙ и
    // разрыв в последовательности индексов.
    let p = project(30, false);
    let q = kt_media::Quality { width: 640, height: 360 };
    let mut idx: Vec<i64> = Vec::new();
    let stats = kt_engine::export::render_raw_video(&p, sources(25), q, |rgba| {
        idx.push(rgba[0] as i64 | (rgba[1] as i64) << 8 | (rgba[2] as i64) << 16);
        Ok(())
    })
    .expect("рендер");
    assert_eq!(idx.len(), 90, "3 с при 30 fps");
    assert_eq!(
        stats.missing_video_layers, 0,
        "кадры выброшены из кэша ДО композиции: недостающих слоёв {}",
        stats.missing_video_layers
    );
    for w in idx.windows(2) {
        assert!(w[1] >= w[0], "индекс кадра откатился: {} → {}", w[0], w[1]);
        assert!(w[1] - w[0] <= 1, "пропущен кадр источника: {} → {}", w[0], w[1]);
    }
    assert!(*idx.last().unwrap() >= 70, "рендер не дошёл до конца: {:?}", idx.last());
}

/// Тест 117 (п.2 р.15): на ВТОРОМ круге loop'а видео не зависает.
///
/// Регрессия: анти-откатный ратчет ключа показа разрешал уехать назад
/// только при смене generation, а loop-wrap его намеренно не бампает
/// (звук и FX-хвосты продолжаются) — картинка замерзала на конце региона,
/// пока звук играл дальше.
#[test]
fn t117_loop_wrap_unfreezes_video() {
    use kt_engine::player::preview_key_step;
    use kt_engine::transport::Transport;

    // 1) транспорт считает перескоки, но НЕ трогает generation
    let tr = Transport::new();
    let (s, e) = (48_000i64, 96_000i64);
    tr.seek_frames(s);
    let gen0 = tr.generation();
    assert_eq!(tr.wraps(), 0);
    tr.advance_frames_wrapped(40_000, Some((s, e)));
    assert_eq!(tr.wraps(), 0, "внутри региона перескока нет");
    tr.advance_frames_wrapped(40_000, Some((s, e)));
    assert_eq!(tr.wraps(), 1, "перескок через конец региона не посчитан");
    assert_eq!(tr.generation(), gen0, "wrap не должен бампать generation");
    assert!(tr.playhead_frames() >= s && tr.playhead_frames() < e);
    // реверс через начало региона — тоже перескок
    tr.advance_frames_wrapped(-40_000, Some((s, e)));
    assert_eq!(tr.wraps(), 2, "перескок назад не посчитан");

    // 2) ратчет: без переанкора ключ замерзает (это и была регрессия),
    //    с переанкором по wrap'у — уезжает в начало региона
    let step = 3000;
    let (frozen, _, rolled) = preview_key_step(Some(90_000), 48_000, step, 1, false);
    assert_eq!((frozen, rolled), (90_000, true), "без переанкора ключ обязан замереть");
    let (jumped, _, rolled) = preview_key_step(Some(90_000), 48_000, step, 1, true);
    assert_eq!((jumped, rolled), (48_000, false), "по wrap'у ключ обязан прыгнуть в регион");

    // 3) ЖИВОЙ плеер: два круга loop'а — ключ показа обязан вернуться к
    //    началу региона, а не замереть на его конце (звук-то идёт)
    use kt_engine::Player;
    let proj = project(30, false);
    let (player, mut ring) = Player::new(
        proj,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(DUR, AudioPattern::Ramp).with_video_fps(30)) as _,
        )],
    );
    let r0 = Tick(90_000 / 2); // регион 0.5 c … 1.0 c
    let r1 = Tick(90_000);
    player.set_loop_region(Some(kt_time::TickRange::new(r0, r1)));
    player.seek(r0);
    player.play();

    let grid = player.video_grid();
    let step = grid.frame_step();
    let mut prev: Option<i64> = None;
    let mut anchor = (player.transport.generation(), player.transport.wraps());
    let mut keys: Vec<i64> = Vec::new();
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < 2500 && player.transport.wraps() < 2 {
        for _ in 0..1024 {
            if ring.pop().is_err() {
                break;
            }
        }
        player.shared.consumed_frames.fetch_add(1024, std::sync::atomic::Ordering::AcqRel);
        let now = (player.transport.generation(), player.transport.wraps());
        let reanchor = now != anchor;
        anchor = now;
        let key = grid.align_floor(player.video_playhead()).0;
        let (key, _, _) = preview_key_step(prev, key, step, 1, reanchor);
        prev = Some(key);
        keys.push(key);
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    assert!(player.transport.wraps() >= 1, "loop не перескочил ни разу: {keys:?}");
    // после перескока ключ обязан оказаться СНОВА в начале региона
    let wrap_at = keys
        .windows(2)
        .position(|w| w[1] < w[0])
        .expect("ключ показа ни разу не вернулся к началу региона — видео зависло");
    assert!(
        keys[wrap_at + 1] < r0.0 + 6 * step,
        "после перескока ключ не у начала региона: {}",
        keys[wrap_at + 1]
    );
    // и дальше он снова движется вперёд. Сравниваем с МАКСИМУМОМ, а не с
    // последним: за 2.5 с окна успевает случиться и второй заворот, и если
    // он попадает на последнюю выборку, `last` законно меньше `first`
    // (ловилось как редкий флак под нагрузкой).
    let after: Vec<i64> = keys[wrap_at + 1..].to_vec();
    assert!(
        after.iter().max().unwrap() > after.first().unwrap(),
        "после перескока видео не поехало дальше: {after:?}"
    );
}
