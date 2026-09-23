//! Тесты 247 (docs/porting-notes.md): ВИДЕО замедляется (Р.37 п.2).
//!
//! Просьба пользователя: «slowed&reverb для видео (отдельная галочка по
//! умолчанию — с некоторой простой генерацией фреймов)». Растяжение времени
//! не выражается покадровым фильтром — оно меняет ВЫБОР исходного кадра,
//! то есть одну функцию `clip_material_pos`. От неё же кормятся префетч,
//! превью и экспорт, поэтому проверяем именно их согласие.

use kt_engine::VideoRenderer;
use kt_model::*;
use kt_time::{Tick, TickRange};

const SEC: i64 = 90_000;
const STEP: i64 = SEC / 30; // 3000, 30 fps
/// Quality тестов; ступень декода (Р.46) без нативов источников — 1.
const Q: kt_media::Quality = kt_media::Quality { width: 64, height: 36 };

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(4 * SEC),
        source_in: Tick(SEC),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// pts источника, которые рендер запрашивает на каждом кадре события.
fn pts_track(p: &Project) -> Vec<i64> {
    let vr = VideoRenderer::new(p);
    (0..4 * 30)
        .map(|i| {
            let t = Tick(i * STEP);
            vr.layers_at(t, Q).first().map(|(_, s, _)| s.0).expect("слой есть")
        })
        .collect()
}

/// Тест 247: замедление вдвое — каждый исходный кадр держится два выходных;
/// ускорение вдвое — каждый второй пропускается. Позиция считается по
/// интегралу скорости, а не «на глазок».
#[test]
fn t247_video_follows_the_rate() {
    // 0.5×: за 4 с события съедается 2 с материала
    let slow = pts_track(&project(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false));
    assert_eq!(slow[0], SEC, "первый кадр — точка входа");
    assert_eq!(slow[119], SEC + 2 * SEC - STEP, "к концу события съедено 2 с материала");
    // каждый кадр источника держится ровно два выходных
    for i in (0..118).step_by(2) {
        assert_eq!(slow[i], slow[i + 1], "кадр {i} обязан держаться два выходных");
        assert_eq!(slow[i + 2] - slow[i], STEP, "и меняться ровно на кадр");
    }

    // 2×: пропускаем каждый второй
    let fast = pts_track(&project(vec![FxInstance::slowed_reverb(2.0, 0.0, 85.0)], vec![], false));
    for i in 0..119 {
        assert_eq!(fast[i + 1] - fast[i], 2 * STEP, "на 2× шаг источника вдвое больше");
    }

    // rate = 1 — путь БАЙТ-В-БАЙТ прежний (на этом стоит GOLDEN_VIDEO)
    let plain = pts_track(&project(vec![], vec![], false));
    let one = pts_track(&project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], vec![], false));
    assert_eq!(plain, one, "единичная скорость ничего не меняет");
}

/// Тест 247b: кривая скорости — тот же интеграл, что у аудио. Разгон
/// 1.0 → 2.0 за событие съедает ровно 1.5 длины (парабола, не прямая).
#[test]
fn t247_video_follows_the_rate_curve() {
    let dur = 4 * SEC;
    let curve = vec![vec![ParamCurve {
        param: "rate".into(),

        // ключи — в тиках МАТЕРИАЛА от source_in (домен Р.37: ось линейна)
        keys: AutoKey::linear(vec![AutoKey::at(Tick(SEC), 1.0), AutoKey::at(Tick(SEC + dur), 2.0)]),
        source: Default::default(),
    }]];
    let p = project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], curve, false);
    let track = pts_track(&p);
    let prof: Vec<(f64, f64)> = vec![(0.0, 1.0), (1.0, 2.0)];
    for i in [0usize, 30, 60, 90, 119] {
        let tau = (i as i64 * STEP) as f64 / dur as f64;
        let want = SEC + kt_fx::warp_material_ticks(1.0, &prof, dur, tau);
        // рендер выравнивает по сетке кадров — сравниваем с точностью кадра
        assert!(
            (track[i] - (want / STEP) * STEP).abs() <= STEP,
            "кадр {i}: pts {} против интеграла {want}",
            track[i]
        );
    }
    assert!(
        track[119] - SEC > (dur as f64 * 1.4) as i64,
        "разгон обязан съесть ~1.5 длины события, съедено {}",
        track[119] - SEC
    );
}

/// Тест 247c: реверс + растяжение — те же кадры в обратном порядке.
/// Композиция «ВАРП, потом ЗЕРКАЛО»: зеркало до варпа гнало бы кривую
/// скорости задом наперёд и рассинхронило бы A/V.
#[test]
fn t247_reverse_and_stretch_compose() {
    let fx = vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)];
    let fwd = pts_track(&project(fx.clone(), vec![], false));
    let rev = pts_track(&project(fx, vec![], true));
    // тот же материал, пройденный в обратную сторону
    assert_eq!(rev[0], fwd[119], "реверс начинается с последнего кадра прямого");
    assert_eq!(rev[119], fwd[0], "и кончается первым");
    assert!(rev.windows(2).all(|w| w[0] >= w[1]), "материал идёт строго навстречу");
    assert!(rev.iter().all(|&s| (SEC..SEC + 2 * SEC).contains(&s)), "материал внутри окна");
    // фаза повторов может отличаться на полкадра: композиция —
    // «ВАРП, потом ЗЕРКАЛО» (как у аудио), а не зеркало выходной оси.
    // На rate = 1 расхождения нет вовсе — это и держит t24 и GOLDEN.
    let one = pts_track(&project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], vec![], true));
    let plain = pts_track(&project(vec![], vec![], true));
    assert_eq!(one, plain, "на единичной скорости реверс байт-в-байт прежний");
}

/// Тест 247d: префетч греет РОВНО то, что читает композит. Расхождение
/// этих двух расчётов = вечный промах кэша и чёрный кадр — тот же
/// инвариант, что у аудио (kt-fx: «единый источник правды»).
#[test]
fn t247_prefetch_matches_what_render_reads() {
    let p = project(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false);
    let vr = VideoRenderer::new(&p);
    // сравниваются ПАРЫ (кадр, качество декода): с Р.46 размер — часть
    // заказа, и разойтись по нему так же смертельно, как по pts (п.318)
    let wanted: std::collections::BTreeSet<(i64, kt_media::Quality)> = vr
        .wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q)
        .into_iter()
        .map(|(_, s, q)| (s.0, q))
        .collect();
    let read: std::collections::BTreeSet<(i64, kt_media::Quality)> = (0..4 * 30)
        .flat_map(|i| vr.layers_at(Tick(i * STEP), Q))
        .map(|(_, s, q)| (s.0, q))
        .collect();
    assert_eq!(read, wanted, "префетч и рендер обязаны просить один и тот же набор кадров");
    // ключи строго возрастают и без дублей — предусловие read_video_span
    let keys: Vec<i64> = wanted.iter().map(|(k, _)| *k).collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "ключи строго возрастают");
    // на 0.5× кадров вдвое меньше, чем выходных — повторы схлопнулись
    assert_eq!(keys.len(), 60, "за 4 с на 0.5× нужно 60 исходных кадров, а не 120");
}

/// Тест 247e: морфер-слой не режет РАСТЯНУТЫЙ клип независимо от вида —
/// и в ссылочных режимах тоже (вето стояло ниже них, дефект до Р.37).
#[test]
fn t247_layer_never_slices_a_stretched_clip() {
    for kind in [ClipKind::Video, ClipKind::Audio] {
        let mut p = project(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false);
        p.clips[0].kind = kind;
        if kind == ClipKind::Audio {
            p.audio_lanes = 1;
        }
        p.markers.push(Marker {
            t: Tick(SEC),
            duration: Tick(SEC),
            name: "врез".into(),
            color: None,
        });
        p.morph_lanes = 1;
        p.morphers.push(MorpherEvent {
            id: MorphId(1),
            lane: 0,
            start: Tick(SEC),
            duration: Tick(SEC),
            targets: MorphTargets::Marks { names: vec!["врез".into()] },
            patch: MorphPatch {
                video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
                audio_fx: vec![FxInstance::gain(3.0)],
                ..Default::default()
            },
            timed: true,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        });
        p.validate().unwrap();
        assert_eq!(
            p.effective_clips().len(),
            1,
            "{kind:?}: растянутый клип не режется даже по именованной ссылке"
        );
    }
}

/// Тест 250 (Р.37 С5): режим кадров «смешивание». Пре-FX кадр слоя —
/// линейная смесь двух соседних исходных, сделанная ТЕМ ЖЕ
/// `dissolve_into`, что кроссфейд. Смешивание живёт в ОБЩЕМ коде плана,
/// поэтому оба исполнителя получают один и тот же кадр (инвариант №7).
#[test]
fn t250_blend_mode_mixes_neighbouring_frames() {
    use kt_media::MediaSource;
    use kt_media::synthetic::{AudioPattern, SyntheticSource};

    let blend_fx = || {
        vec![FxInstance::SlowedReverb {
            rate: 0.5,
            mix: 0.0,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            frames: FrameMode::Blend,
            width: 1.0,
            automation: vec![],
        }]
    };
    let p = project(blend_fx(), vec![], false);
    let mut vr = VideoRenderer::new(&p);
    let q = kt_media::Quality { width: 64, height: 36 };
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    // греем ровно то, что просит план
    for (media, pts, _q) in vr.wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q) {
        let f = synth.read_video_frame(pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video_cache_idx(pts),
            std::sync::Arc::new(f),
        );
    }

    // на 0.5× нечётные выходные кадры стоят ровно посередине между
    // исходными: вес 2048/4096
    let t = Tick(STEP); // второй кадр события
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete, "оба кадра пары прогреты");
    let a = synth.read_video_frame(Tick(SEC), Some(q)).unwrap().rgba.into_vec();
    let b = synth.read_video_frame(Tick(SEC + STEP), Some(q)).unwrap().rgba.into_vec();
    let want: Vec<u8> = a
        .iter()
        .zip(&b)
        .map(|(&x, &y)| ((x as u32 * 2048 + y as u32 * 2048 + 2048) >> 12) as u8)
        .collect();
    assert_eq!(out, want, "смесь считается тем же целочисленным ядром, что кроссфейд");

    // чётные кадры стоят НА сетке — вес 0, пары нет, путь прежний
    let (on_grid, _) = vr.compose_from_cache(&cache, Tick(0), q);
    assert_eq!(on_grid, a, "на сетке смешивать нечего");

    // «ближайший» на тех же данных даёт ЧИСТЫЙ кадр, а не смесь
    let mut near = VideoRenderer::new(&project(
        vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)],
        vec![],
        false,
    ));
    let (n, _) = near.compose_from_cache(&cache, t, q);
    assert_eq!(n, a, "режим «ближайший» держит кадр");
    assert_ne!(n, out, "и отличается от смешивания");
}

/// Тест 250b: префетч греет ОБА кадра пары. Не согреет — второй не
/// запросят никогда, и слой вечно голодает.
#[test]
fn t250_blend_prefetches_both_sides() {
    let p = project(
        vec![FxInstance::SlowedReverb {
            rate: 0.5,
            mix: 0.0,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            frames: FrameMode::Blend,
            width: 1.0,
            automation: vec![],
        }],
        vec![],
        false,
    );
    let vr = VideoRenderer::new(&p);
    let wanted: std::collections::BTreeSet<i64> = vr
        .wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q)
        .into_iter()
        .map(|(_, s, _)| s.0)
        .collect();
    // соседи добавили по кадру справа: 61 вместо 60
    assert_eq!(wanted.len(), 61, "греются обе стороны смеси, а не одна");
    let keys: Vec<i64> = wanted.iter().copied().collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "ключи по-прежнему строго возрастают");

    // смешивающий слой держит ДВА кадра — экспорт обязан это учитывать,
    // иначе чанк вытеснит соседа и байты станут зависеть от кэша
    assert_eq!(vr.max_concurrent_layers(), 2, "смешивающий слой весит два кадра");
}

// ---------------------------------------------------------------------------
// Тесты 272 (Р.39): резка и домен автоматизации на растянутом клипе.
// ---------------------------------------------------------------------------

/// Тест 272: разрезанный проект рендерит ТЕ ЖЕ кадры, что неразрезанный.
///
/// Это и есть тот самый «скачок на стыке», о котором шла речь: до Р.39
/// `clip_slice` считал `source_in` линейно, и последовательность исходных
/// pts расходилась НА КАЖДОМ кадре после разреза.
#[test]
fn t272_split_renders_the_same_frames() {
    for (name, fx, auto, reversed) in [
        ("0.5x", vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false),
        ("2x", vec![FxInstance::slowed_reverb(2.0, 0.0, 85.0)], vec![], false),
        ("0.5x rev", vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], true),
        (
            "кривая",
            vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)],
            vec![vec![ParamCurve {
                param: "rate".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(SEC), 1.0),
                    AutoKey::at(Tick(SEC + 4 * SEC), 2.0),
                ]),
                source: Default::default(),
            }]],
            false,
        ),
    ] {
        let whole = project(fx.clone(), auto.clone(), reversed);
        let before = pts_track(&whole);
        for at in [2 * SEC, SEC * 17 / 10] {
            let mut m = TimelineModel::new(whole.clone());
            m.apply(Command::Split {
                id: ClipId(1),
                at: Tick(at),
                new_id: ClipId(2),
                new_group: None,
            })
            .expect("разрез проходит");
            let after = pts_track(m.project());
            let bad = before
                .iter()
                .zip(&after)
                .enumerate()
                .filter(|(_, (a, b))| (*a - *b).abs() > STEP)
                .count();
            assert_eq!(
                bad,
                0,
                "{name}, разрез на {at}: {bad} кадров из {} разошлись — резка попала не в тот \
                 материал",
                before.len()
            );
        }
    }
}

/// Тест 272б: кривая ВИДЕО-FX сэмплируется в том же домене, что звук и UI.
///
/// До Р.39 `layer_src` брал варп-зависимую позицию кадра, то есть на
/// замедленном клипе картинка анимировалась по своему расписанию. Ось
/// линейна по решению Р.37 («БЕЗ учёта замедления-ускорения»).
#[test]
fn t272b_video_curves_use_the_linear_axis() {
    // Blur с кривой радиуса + замедление вдвое. Ключ ровно на середине
    // события: значение под ним обязано совпасть со значением, которое
    // покажет UI (тот же `clip_material_at`).
    let mut p = project(
        vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0), FxInstance::blur(1)],
        vec![
            vec![],
            vec![ParamCurve {
                param: "radius".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(SEC), 1.0),
                    AutoKey::at(Tick(SEC + 2 * SEC), 48.0),
                ]),
                source: Default::default(),
            }],
        ],
        false,
    );
    p.clips[0].fx_bypass = vec![false, false];
    p.validate().unwrap();

    let mid = Tick(2 * SEC); // середина события
    let want_at = kt_model::clip_material_at(
        p.clips[0].source_in,
        p.clips[0].start,
        p.clips[0].duration,
        false,
        STEP,
        mid,
    );
    // ключи покрывают [SEC, 3·SEC]; линейная ось ставит середину события в
    // материал 3·SEC — то есть конец кривой
    assert_eq!(want_at, Tick(SEC + 2 * SEC), "линейная ось: середина события = середина материала");

    let vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let plan = vr.frame_plan(&cache, mid, kt_media::Quality { width: 64, height: 36 });
    let radius = plan.layers.iter().find_map(|l| match l {
        kt_engine::PlanLayer::One { src, .. } => src.fx.iter().find_map(|f| match f {
            FxInstance::Blur { radius, .. } => Some(*radius),
            _ => None,
        }),
        _ => None,
    });
    assert_eq!(
        radius,
        Some(48),
        "радиус обязан быть взят по ЛИНЕЙНОЙ оси (конец кривой), а не по варп-позиции"
    );
}

// ---- Р.57: знаковая скорость -------------------------------------------------

/// Тест 604a (Р.57): видео идёт по материалу НАЗАД, когда скорость
/// отрицательна, и разворачивается там, где кривая меняет знак.
///
/// Претензия владельца дословно: «реверсивное проигрывание в slowed-эффекте
/// (отрицательный rate)».
#[test]
fn t604a_video_follows_a_signed_curve() {
    let dur = 4 * SEC;
    // 1 → −1: позиция поднимается к середине события и возвращается
    let curve = vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(SEC), 1.0),
            AutoKey::at(Tick(SEC + dur), -1.0),
        ]),
        source: Default::default(),
    }]];
    let track = pts_track(&project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], curve, false));
    let top = track.iter().enumerate().max_by_key(|(_, v)| **v).unwrap().0;
    assert!(
        (55..65).contains(&top),
        "вершина не посередине события: кадр {top}, дорожка {:?}",
        &track[..5]
    );
    assert!(track[top] > track[0] + SEC / 2, "материал не продвинулся вперёд вовсе");
    // и ВЕРНУЛСЯ туда, откуда вышел: ∫rate по всему событию равен нулю
    assert!(
        (track[119] - track[0]).abs() <= 2 * STEP,
        "кривая не вернулась в старт: {} против {}",
        track[119],
        track[0]
    );
    // вторая половина строго убывает — это и есть ход назад
    for i in (top + 2)..118 {
        assert!(track[i] >= track[i + 1], "кадр {i}: материал не идёт назад");
    }
}

/// Тест 604b (Р.57): нулевая скорость — СТОП-КАДР, а не «очень медленно».
#[test]
fn t604b_zero_rate_is_a_freeze() {
    let track = pts_track(&project(vec![FxInstance::slowed_reverb(0.0, 0.0, 85.0)], vec![], false));
    assert!(track.iter().all(|&t| t == track[0]), "стоп-кадр поехал: {:?}", &track[..6]);
    assert_eq!(track[0], SEC, "стоп держит кадр точки входа");
    // и заказ на такой клип — ОДИН ключ на всё событие
    let p = project(vec![FxInstance::slowed_reverb(0.0, 0.0, 85.0)], vec![], false);
    let vr = VideoRenderer::new(&p);
    let keys = vr.wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q);
    assert_eq!(keys.len(), 1, "стоп-кадр заказал больше одного кадра: {keys:?}");
}

/// Тест 604c (Р.57): у начала материала кадр ДЕРЖИТСЯ, а не уходит в минус.
#[test]
fn t604c_edge_holds_at_the_material_start() {
    // source_in = 1 c, скорость −1: через секунду упрёмся в ноль файла
    let track =
        pts_track(&project(vec![FxInstance::slowed_reverb(-1.0, 0.0, 85.0)], vec![], false));
    assert!(track.iter().all(|&t| t >= 0), "кадр ушёл за начало файла: {:?}", &track[28..34]);
    assert_eq!(track[0], SEC);
    assert_eq!(track[119], 0, "к концу события мы у начала файла и держимся");
    assert!(track[35] == 0 && track[60] == 0, "край не держится");
}

/// Тест 604d (Р.57): под отрицательной скоростью пара «смешивания» берётся
/// с ТОЙ ЖЕ стороны, что у положительной, — дробная часть меряется вперёд.
#[test]
fn t604d_blend_pair_under_negative_rate() {
    let mut fx = FxInstance::slowed_reverb(-0.5, 0.0, 85.0);
    if let FxInstance::SlowedReverb { frames, .. } = &mut fx {
        *frames = FrameMode::Blend;
    }
    let p = project(vec![fx], vec![], false);
    let vr = VideoRenderer::new(&p);
    // на полукадре пара обязана быть (a, a+STEP) и вес ровно половина
    for i in [1i64, 3, 5, 7] {
        let keys = vr.layers_at(Tick(i * STEP), Q);
        assert_eq!(keys.len(), 2, "кадр {i}: пары нет — {keys:?}");
        let (a, b) = (keys[0].1.0, keys[1].1.0);
        assert_eq!(b - a, STEP, "кадр {i}: сосед взят не вперёд: {a} и {b}");
    }
}

/// Тест 604e (Р.57): заказ == чтение под сменой знака (класс t247d).
///
/// Окно по двум концам пропустило бы экскурсию: кривая уходит вперёд и
/// возвращается, и максимум лежит ВНУТРИ окна, а не на его краю.
#[test]
fn t604e_prefetch_matches_reads_under_a_sign_change() {
    let dur = 4 * SEC;
    let curve = vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(SEC), 2.0),
            AutoKey::at(Tick(SEC + dur), -2.0),
        ]),
        source: Default::default(),
    }]];
    let p = project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], curve, false);
    let vr = VideoRenderer::new(&p);
    let wanted: std::collections::BTreeSet<i64> = vr
        .wanted_frames(TickRange::new(Tick(0), Tick(dur)), Q)
        .into_iter()
        .map(|(_, s, _)| s.0)
        .collect();
    let read: std::collections::BTreeSet<i64> = (0..4 * 30)
        .flat_map(|i| vr.layers_at(Tick(i * STEP), Q).into_iter().map(|(_, s, _)| s.0))
        .collect();
    assert!(!read.is_empty());
    assert!(
        read.is_subset(&wanted),
        "читается то, чего не заказывали: {:?}",
        read.difference(&wanted).collect::<Vec<_>>()
    );
    // СТОРОЖ СЦЕНЫ: экскурсия обязана быть — иначе тест ничего не ловит
    let first = vr.layers_at(Tick(0), Q)[0].1.0;
    let last = vr.layers_at(Tick(119 * STEP), Q)[0].1.0;
    let top = *read.iter().max().unwrap();
    assert!(top > first.max(last) + STEP, "сцена выродилась: экскурсии нет");
}

/// Тест 604g (Р.57): «Реверс» поверх отрицательной скорости даёт ход
/// ВПЕРЁД — зеркало берётся вокруг пролёта, а не вокруг съеденного.
#[test]
fn t604g_reverse_over_negative_rate_goes_forward() {
    let rev = pts_track(&project(vec![FxInstance::slowed_reverb(-0.5, 0.0, 85.0)], vec![], true));
    // без зеркала −0.5 идёт назад; с зеркалом — вперёд, от нижнего края
    // пролёта к верхнему
    assert!(rev[0] < rev[119], "зеркало не развернуло ход: {} → {}", rev[0], rev[119]);
    for i in 0..118 {
        assert!(rev[i] <= rev[i + 1] + STEP, "кадр {i}: ход не монотонен");
    }
    // пролёт −0.5 на 4 c — это 2 c назад: зеркало кладёт нас в −1 c от
    // точки входа и ведёт к ней
    assert_eq!(rev[0], 0, "зеркало начинает с нижнего края пролёта");
    // допуск в два кадра: зеркало вычитает шаг сетки (первый выходной
    // кадр зеркального клипа — ПОСЛЕДНИЙ настоящий), и последний кадр
    // события лежит ещё на шаг раньше конца
    assert!((rev[119] - SEC).abs() <= 2 * STEP, "зеркало не доходит до точки входа: {}", rev[119]);
}
