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

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool) -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 64, height: 36, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
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
            vr.layers_at(t).first().map(|(_, s)| s.0).expect("слой есть")
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
        ease: false,
        // ключи — в тиках МАТЕРИАЛА от source_in (домен Р.37: ось линейна)
        keys: vec![
            AutoKey { t: Tick(SEC), v: 1.0 },
            AutoKey { t: Tick(SEC + dur), v: 2.0 },
        ],
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
    let wanted: std::collections::BTreeSet<i64> = vr
        .wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)))
        .into_iter()
        .map(|(_, s)| s.0)
        .collect();
    let read: std::collections::BTreeSet<i64> =
        pts_track(&p).into_iter().collect();
    assert_eq!(read, wanted, "префетч и рендер обязаны просить один и тот же набор кадров");
    // ключи строго возрастают и без дублей — предусловие read_video_span
    let keys: Vec<i64> = wanted.iter().copied().collect();
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
    for (media, pts) in vr.wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC))) {
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
        .wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)))
        .into_iter()
        .map(|(_, s)| s.0)
        .collect();
    // соседи добавили по кадру справа: 61 вместо 60
    assert_eq!(wanted.len(), 61, "греются обе стороны смеси, а не одна");
    let keys: Vec<i64> = wanted.iter().copied().collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "ключи по-прежнему строго возрастают");

    // смешивающий слой держит ДВА кадра — экспорт обязан это учитывать,
    // иначе чанк вытеснит соседа и байты станут зависеть от кэша
    assert_eq!(vr.max_concurrent_layers(), 2, "смешивающий слой весит два кадра");
}
