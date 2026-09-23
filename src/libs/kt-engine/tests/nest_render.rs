//! Тесты 434-436 (docs/porting-notes.md): ЗАКОН Э — вложение с пустой
//! цепочкой равно своему развёрнутому содержимому.
//!
//! Прецедент — тест 36 («рендер с морфером sample-exact равен рендеру после
//! bake»). Формулировка:
//!
//! > Пусть `N` — вложение с пустой цепочкой, `opacity == 1.0`, `gain_db == 0`,
//! > без реверса и варпа, без переходов на краях, длиннее содержимого с
//! > хвостами, с границами на сетке кадров и на границе аудио-кадра, и его
//! > дети не разорваны чужими событиями в порядке сложения. Тогда рендер `P`
//! > БАЙТ-В-БАЙТ равен рендеру `Decompose(P)` — и по видео, и по аудио.
//!
//! Зачем закон нужен. Вложение с пустой цепочкой семантически тождественно
//! содержимому ПО ПОСТРОЕНИЮ: настройки наследуются (решение владельца
//! Р.50), промежуточного растра не возникает. Значит инлайн в
//! `effective_clips` — не «упрощённая версия» вложения, а ЭТАЛОН-ОРАКУЛ, с
//! которым волна Б будет сверять настоящее поддерево. Без него сверять
//! поддерево было бы не с чем.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const FPS: u32 = 30;
const FRAME: i64 = 90_000 / FPS as i64;
/// Длина куска — целое число кадров И целое число аудио-burst'ов.
const PART: Tick = Tick(FRAME * 30);

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: FPS,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![
        (
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 }))
                as Box<dyn MediaSource>,
        ),
        (MediaId(2), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp))),
        (
            MediaId(3),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 })),
        ),
    ]
}

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for (id, s) in sources() {
        f.add_source(id, s);
    }
    f
}

fn ev(id: u64, kind: ClipKind, lane: u32, start: Tick, dur: Tick, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start,
        duration: dur,
        source_in: Tick(FRAME * 3),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn base() -> Project {
    let mut p = Project::new(settings());
    for id in [1u64, 2, 3] {
        p.media.push(MediaRef { id: MediaId(id), path: format!("synth:{id}") });
    }
    p
}

/// `Decompose(P)`: содержимое стоит прямо на дорожках — ровно там, куда его
/// кладёт `ops::expand_instance`.
fn decomposed(kind: ClipKind) -> Project {
    let mut p = base();
    // «хвост» снаружи слева: он не перекрывает вложение и обязан звучать
    // и там, и там одинаково
    p.clips.push(ev(1, kind, 0, Tick(0), PART, 3));
    // содержимое: два события встык на дорожке экземпляра
    p.clips.push(ev(2, kind, 0, PART, PART, 1));
    p.clips.push(ev(3, kind, 0, PART + PART, PART, 2));
    if kind == ClipKind::Video {
        p.video_lanes = 1;
    } else {
        p.audio_lanes = 1;
    }
    p.validate().expect("Decompose(P) обязан быть законным");
    p
}

/// `P`: то же самое, но середина свёрнута в ленту.
fn nested(kind: ClipKind) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Кусок");
    s.clips.push(ev(2, kind, 0, Tick(0), PART, 1));
    s.clips.push(ev(3, kind, 0, PART, PART, 2));
    p.sequences.push(s);
    p.clips.push(ev(1, kind, 0, Tick(0), PART, 3));
    let mut inst = ev(10, kind, 0, PART, PART + PART, 100);
    inst.source_in = Tick(0);
    p.clips.push(inst);
    p.validate().expect("P обязан быть законным");
    p
}

fn render_audio(p: &Project, frames: usize) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut out = vec![0f32; frames * CH];
    let st = r.render_frames(&mut direct(), 0, &mut out);
    assert_eq!(st.underruns, 0, "underrun при полностью доступном источнике");
    out
}

fn render_video(p: &Project, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, t, q)
}

// ---- 434. закон Э по аудио ----------------------------------------------------

#[test]
fn t434_law_e_audio_is_byte_for_byte() {
    let frames = (PART + PART + PART).to_audio_frames() as usize;
    let a = render_audio(&decomposed(ClipKind::Audio), frames);
    let b = render_audio(&nested(ClipKind::Audio), frames);
    assert_eq!(
        a, b,
        "ЗАКОН Э по аудио: вложение с пустой цепочкой обязано звучать ровно как \
         развёрнутое содержимое. Настройки наследуются, промежуточного ресемпла \
         нет — расхождение тут значит, что инлайн считает НЕ ТО, а вместе с ним \
         и весь оракул волны Б"
    );
    // ...и это не «оба молчат»
    assert!(a.iter().any(|x| x.abs() > 1e-6), "фикстура молчит — закон проверен ни на чём");
}

// ---- 435. закон Э по видео + превью == экспорт --------------------------------

#[test]
fn t435_law_e_video_and_preview_equals_export() {
    let q = Quality { width: 64, height: 36 };
    let (a, b) = (decomposed(ClipKind::Video), nested(ClipKind::Video));
    let mut differs_from_black = false;
    // каждый кадр всех трёх участков: хвост снаружи, оба ребёнка внутри
    for i in 0..90 {
        let t = Tick(i * FRAME);
        let (fa, ca) = render_video(&a, t, q);
        let (fb, cb) = render_video(&b, t, q);
        assert_eq!(ca, cb, "полнота композита разошлась на кадре {i}");
        assert_eq!(
            fa, fb,
            "ЗАКОН Э по видео: кадр {i} у вложения и у развёрнутого содержимого \
             обязан совпадать БАЙТ-В-БАЙТ"
        );
        differs_from_black |= fa.iter().any(|&x| x != 0);
    }
    assert!(differs_from_black, "все кадры чёрные — закон проверен ни на чём");
}

// ---- 436. состав заказа и стоимость -------------------------------------------

#[test]
fn t436_nest_orders_the_same_frames_and_costs_the_same() {
    let q = Quality { width: 64, height: 36 };
    let (a, b) = (decomposed(ClipKind::Video), nested(ClipKind::Video));
    let va = VideoRenderer::new(&a);
    let vb = VideoRenderer::new(&b);
    for i in 0..90 {
        let t = Tick(i * FRAME);
        let la: Vec<_> = va.layers_at(t, q).into_iter().collect();
        let lb: Vec<_> = vb.layers_at(t, q).into_iter().collect();
        assert_eq!(
            la, lb,
            "ЗАКАЗ кадров разошёлся на {i}: префетч и рендер обязаны спрашивать \
             ОДНО И ТО ЖЕ (инвариант 4), иначе это вечный промах кэша"
        );
    }

    // цена раскрытия: лента в 500 событий, поставленная ТРИЖДЫ, не должна
    // превращать выдачу клипов в перебор проекта на каждый кадр
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Длинная");
    for i in 0..500u64 {
        s.clips.push(ev(1000 + i, ClipKind::Video, 0, Tick(i as i64 * PART.0), PART, 1));
    }
    p.sequences.push(s);
    for (k, at) in [(0u64, Tick(0)), (1, Tick(500 * PART.0)), (2, Tick(1000 * PART.0))] {
        let mut inst = ev(10 + k, ClipKind::Video, 0, at, Tick(500 * PART.0), 100);
        inst.source_in = Tick(0);
        p.clips.push(inst);
    }
    p.validate().expect("проект с тремя постановками длинной ленты законен");
    let t0 = std::time::Instant::now();
    let eff = p.effective_clips();
    let dt = t0.elapsed();
    assert_eq!(eff.len(), 1500, "раскрылось не всё содержимое: {}", eff.len());
    // адреса кусков уникальны — иначе FX-состояние и выделение поедут
    let mut ids: Vec<u64> = eff.iter().map(|c| c.id.0).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "куски вложения получили одинаковые адреса");
    // потолок с большим запасом: ловим не микросекунды, а квадратичность
    assert!(
        dt.as_millis() < 500,
        "раскрытие трёх постановок ленты в 500 событий заняло {dt:?} — это уже не \
         линейный проход, а перебор на каждый кадр"
    );
}

// ---- 438-441 (Р.50, волна Б): вложение — ГРАНИЦА ПОРЯДКА ОБРАБОТКИ --------

/// Два ПЕРЕКРЫВАЮЩИХСЯ события на соседних дорожках, верхнее —
/// ПОЛУПРОЗРАЧНОЕ. Оба условия обязательны: пока дети не перекрываются
/// (или верхний непрозрачен и накрывает нижнего целиком), «на вложение» и
/// «на каждого» дают одно и то же, и граница неотличима от её отсутствия.
fn inner_pair() -> Vec<ClipEvent> {
    let a = ev(2, ClipKind::Video, 1, Tick(0), PART, 1);
    let mut b = ev(3, ClipKind::Video, 0, Tick(PART.0 / 2), PART, 2);
    b.opacity = 0.5;
    vec![a, b]
}

/// Наибольшее расхождение по каналу.
fn max_diff(a: &[u8], b: &[u8]) -> i32 {
    assert_eq!(a.len(), b.len(), "кадры разного размера");
    a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0)
}

/// `P`: дети внутри ленты, уровень/цепочка — на ЭКЗЕМПЛЯРЕ.
fn nest_with(outer_opacity: f32, fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Кусок");
    s.clips = inner_pair();
    s.video_lanes = 2;
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), PART + PART, 100);
    inst.source_in = Tick(0);
    inst.opacity = outer_opacity;
    inst.fx = fx;
    p.clips.push(inst);
    p.validate().expect("проект с вложением законен");
    p
}

/// `Decompose(P)`: те же дети прямо на дорожках, уровень/цепочка — на КАЖДОМ.
fn spread_with(opacity: f32, fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut flat = inner_pair();
    for c in flat.iter_mut() {
        c.opacity *= opacity;
        c.fx = fx.clone();
    }
    p.clips = flat;
    p.video_lanes = 2;
    p.validate().expect("развёрнутый проект законен");
    p
}

#[test]
fn t438_law_g_level_on_the_nest_is_not_level_on_each_child() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20); // внутри перекрытия
    let (nested_half, _) = (render_video(&nest_with(0.5, vec![]), t, q).0, ());
    let spread_half = render_video(&spread_with(0.5, vec![]), t, q).0;
    assert_ne!(
        nested_half, spread_half,
        "ЗАКОН Г: уровень 50 % НА ВЛОЖЕНИИ обязан отличаться от 50 % на каждом \
         ребёнке. Совпали — значит границы порядка обработки нет, и вложение \
         бессмысленно: ровно ради «эффект на РЕЗУЛЬТАТ» оно и заводилось"
    );
    // ...и это не «одно из них чёрное»
    assert!(nested_half.iter().any(|&x| x != 0), "композит вложения чёрный");
    assert!(spread_half.iter().any(|&x| x != 0), "развёрнутый композит чёрный");

    // при уровне 1.0 разницы быть НЕ должно: непрозрачный композит
    // накладывается так же, как его непрозрачные дети (закон Э)
    assert_eq!(
        render_video(&nest_with(1.0, vec![]), t, q).0,
        render_video(&spread_with(1.0, vec![]), t, q).0,
        "без уровня и цепочки вложение обязано быть ТОЖДЕСТВЕННО содержимому"
    );
}

#[test]
fn t439_semi_transparent_subcomposite_is_not_twice_as_dark() {
    // Дефект, найденный разведкой до того, как он появился: аккумулятор
    // композита хранит ПРЕМУЛЬТИПЛИЦИРОВАННЫЙ RGB при ПРЯМОЙ альфе, а
    // `alpha_over` ждёт прямую. Подать суб-композит обратно в композит без
    // конверсии — картинка вдвое темнее РОВНО в полупрозрачных местах.
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 5);

    // фон снизу, полупрозрачный ребёнок внутри ленты сверху
    let mut nested = base();
    let mut s = Sequence::new(MediaId(100), "Кусок");
    let mut child = ev(2, ClipKind::Video, 0, Tick(0), PART, 1);
    child.opacity = 0.5;
    s.clips = vec![child.clone()];
    nested.sequences.push(s);
    nested.video_lanes = 2;
    // ФОН другого цвета: у синтетики цвет — функция pts, поэтому без
    // своей точки входа фон и содержимое совпали бы, и потемнение было бы
    // не на чем увидеть
    let mut bg_clip = ev(1, ClipKind::Video, 1, Tick(0), PART, 3);
    bg_clip.source_in = Tick(FRAME * 500);
    nested.clips.push(bg_clip.clone());
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), PART, 100);
    inst.source_in = Tick(0);
    nested.clips.push(inst);
    nested.validate().unwrap();

    let mut flat = base();
    flat.video_lanes = 2;
    flat.clips.push(bg_clip.clone());
    flat.clips.push(child);
    flat.validate().unwrap();

    let bg = render_video(
        &{
            let mut p = base();
            let mut c = bg_clip;
            c.lane = 0;
            p.clips.push(c);
            p.validate().unwrap();
            p
        },
        t,
        q,
    )
    .0;
    let a = render_video(&nested, t, q).0;
    let b = render_video(&flat, t, q).0;

    // ЗДЕСЬ закон Э честно НЕ выполняется байт-в-байт, и это не чинится:
    // промежуток между уровнями 8-битный, и premultiply→unpremultiply
    // теряет один младший разряд. Предел названный, а не подобранный: ОДНО
    // округление 8-битного промежутка.
    let d = max_diff(&a, &b);
    assert!(
        d <= 1,
        "суб-композит лёг на фон не так, как то же содержимое легло бы прямо \
         (расхождение {d} > 1 LSB). Сигнатура дефекта — примерно ВДВОЕ темнее \
         ровно в полупрозрачных местах: аккумулятор отдаёт премультиплицированный \
         цвет, а наложение ждёт прямой"
    );
    // ...и это утверждение не пустое: без конверсии картинка уезжает К ФОНУ
    // (полупрозрачный слой вносит вдвое меньше), а вовсе не на один разряд
    let to_bg = max_diff(&b, &bg);
    assert!(
        to_bg > 8,
        "фикстура вырождена: содержимое вложения и так почти равно фону ({to_bg}), \
         на ней потемнение было бы не видно"
    );
}

#[test]
fn t440_a_chain_on_the_nest_acts_on_the_assembled_picture() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    // Постеризация НЕЛИНЕЙНА: квантование смеси двух цветов — не смесь их
    // квантований. Линейный эффект (размытие, уровень) на плоских кадрах
    // ничего бы не показал, и тест был бы зелёным на сломанном коде.
    let fx = vec![FxInstance::Posterize { levels: 3 }];
    let on_nest = render_video(&nest_with(1.0, fx.clone()), t, q).0;
    let on_each = render_video(&spread_with(1.0, fx), t, q).0;
    assert_ne!(
        on_nest, on_each,
        "цепочка на вложении посчиталась ПОКУСОЧНО. «Эффект на результат» — \
         единственный способ выразить это там, где эффекты живут на событиях, \
         и ровно он тут не работает"
    );
    assert!(on_nest.iter().any(|&x| x != 0), "композит с цепочкой чёрный");
    // ...а без цепочки те же два проекта совпадают — значит разошлись они
    // ИМЕННО на цепочке, а не на устройстве фикстуры
    assert_eq!(
        render_video(&nest_with(1.0, vec![]), t, q).0,
        render_video(&spread_with(1.0, vec![]), t, q).0,
        "фикстуры расходятся и без цепочки — тест доказывал бы не то"
    );
}

#[test]
fn t441_hunger_and_memoization_cross_the_boundary() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    let p = nest_with(1.0, vec![]);
    let seq = MediaId(100);

    // ГОЛОД ВНУТРИ виден СНАРУЖИ: кадра ребёнка нет — композит неполон.
    // Иначе плеер считал бы кадр готовым и никогда его не перезапросил.
    let mut v = VideoRenderer::new(&p);
    let cold = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let (_, complete) = v.compose_from_cache(&cold, t, q);
    assert!(!complete, "голод ВНУТРИ ленты не дошёл до внешнего композита");

    // Прогрели — и полный.
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in v.layers_at(t, q) {
        assert_ne!(media, seq, "заказ не спустился вглубь: адрес ЛЕНТЫ источника не имеет");
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview, complete) = v.compose_from_cache(&cache, t, q);
    assert!(complete, "прогретый композит с вложением обязан быть полным");

    // Мемоизация суб-композита есть в ПРЕВЬЮ...
    let h = kt_engine::content_hash(&p, seq);
    let inner_t = t; // экземпляр стоит в нуле с source_in = 0
    assert!(
        cache
            .get_postfx_video(kt_engine::nest_source_id(seq), (q.width, q.height), h, inner_t.0)
            .is_some(),
        "суб-композит не мемоизирован в превью — цепочка ленты платилась бы на КАЖДЫЙ кадр"
    );

    // ...и НЕТ в экспорте: полноразмерные суб-композиты вытеснили бы ещё
    // не использованные пре-кадры соседних слоёв из бюджета чанка —
    // дословно регрессия Р.31 п.4 («рендер поломан»).
    let export_cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    for (media, src_pts, dq) in v.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        export_cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let mut out = Vec::new();
    let complete = v.compose_export_into(&export_cache, t, q, &mut out);
    assert!(complete, "экспортный композит с вложением обязан быть полным");
    assert_eq!(out, preview, "превью и экспорт обязаны совпадать БАЙТ-В-БАЙТ");
    assert!(
        export_cache
            .get_postfx_video(kt_engine::nest_source_id(seq), (q.width, q.height), h, inner_t.0)
            .is_none(),
        "экспорт мемоизировал суб-композит: бюджет чанка вытеснит пре-кадры соседей"
    );
}

// ---- 442-443 (Р.50, Б2): ключи мемоизации ------------------------------------

/// Прогреть пре-FX кадры плана и вернуть композит.
fn warm_and_compose(
    v: &mut VideoRenderer,
    cache: &kt_cache::BurstCache,
    t: Tick,
    q: Quality,
) -> Vec<u8> {
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in v.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    v.compose_from_cache(cache, t, q).0
}

#[test]
fn t442_editing_inside_the_reel_invalidates_the_instance() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    // цепочка НА ЭКЗЕМПЛЯРЕ: именно её пост-FX кадр мемоизируется, и
    // именно он стал бы стейлом
    let mut p = nest_with(1.0, vec![FxInstance::Posterize { levels: 3 }]);
    // кэш ОДИН на оба прогона — в программе он и есть один: правка модели
    // пересоздаёт рендерер, но не кэш
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let before = warm_and_compose(&mut VideoRenderer::new(&p), &cache, t, q);

    // правим ВНУТРИ ленты. Ни одно поле клипа-экземпляра не меняется:
    // его `fx_hash` по геометрии и цепочке остался бы прежним, и без
    // `content_hash` кэш отдал бы КАДР ДО ПРАВКИ.
    p.sequences[0].clips[1].source_in = Tick(FRAME * 77);
    p.validate().unwrap();
    let after = warm_and_compose(&mut VideoRenderer::new(&p), &cache, t, q);
    assert_ne!(
        before, after,
        "правка ВНУТРИ ленты не дошла до картинки: обещание «правка модели \
         пересоздаёт рендерер, стейл-кадры не читаются» для вложения держится \
         только на хэше РЕДАКЦИИ — у экземпляра не меняется ни одно поле"
    );
}

#[test]
fn t443_three_keys_separate_reels_instances_and_editions() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 20);
    let p = nest_with(1.0, vec![]);
    let seq = MediaId(100);

    // РЕДАКЦИИ: тот же адрес, другое содержимое — другой ключ
    let h1 = kt_engine::content_hash(&p, seq);
    let mut q2 = p.clone();
    q2.sequences[0].clips[1].source_in = Tick(FRAME * 77);
    let h2 = kt_engine::content_hash(&q2, seq);
    assert_ne!(h1, h2, "правка внутри ленты не изменила хэш редакции");

    // ...и рекурсивно: правка во ВНУКЕ обязана дойти до деда
    let mut deep = base();
    let mut inner = Sequence::new(MediaId(101), "Внутренняя");
    inner.clips = vec![ev(20, ClipKind::Video, 0, Tick(0), PART, 1)];
    let mut outer = Sequence::new(MediaId(100), "Внешняя");
    let mut mid = ev(21, ClipKind::Video, 0, Tick(0), PART, 101);
    mid.source_in = Tick(0);
    outer.clips = vec![mid];
    deep.sequences.push(inner);
    deep.sequences.push(outer);
    let mut top = ev(10, ClipKind::Video, 0, Tick(0), PART, 100);
    top.source_in = Tick(0);
    deep.clips.push(top);
    deep.validate().unwrap();
    let d1 = kt_engine::content_hash(&deep, MediaId(100));
    deep.sequences[0].clips[0].source_in = Tick(FRAME * 9);
    let d2 = kt_engine::content_hash(&deep, MediaId(100));
    assert_ne!(d1, d2, "правка во ВНУКЕ не дошла до хэша деда — мемо отдаст стейл");

    // ЛЕНТЫ: адрес суб-композита в кэше — не адрес файла
    assert_ne!(
        kt_engine::nest_source_id(seq),
        kt_engine::source_id(MediaId(seq.0)),
        "лента и файл с тем же номером делят адрес в кэше"
    );
    assert!(
        kt_engine::nest_source_id(seq).0 >= kt_model::NEST_TAG,
        "адрес ленты обязан лежать выше диапазона файлов"
    );

    // ЭКЗЕМПЛЯРЫ: две постановки ОДНОЙ ленты с разными цепочками не
    // схлопываются в один пост-FX кадр, а суб-композит у них ОБЩИЙ —
    // в этом и выигрыш
    let mut two = nest_with(1.0, vec![FxInstance::Posterize { levels: 3 }]);
    let mut twin = two.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * PART.0);
    twin.fx = vec![FxInstance::Posterize { levels: 5 }];
    two.clips.push(twin);
    two.validate().unwrap();
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut v = VideoRenderer::new(&two);
    let a = warm_and_compose(&mut v, &cache, t, q);
    let b = warm_and_compose(&mut v, &cache, Tick(10 * PART.0 + t.0), q);
    assert_ne!(
        a, b,
        "две постановки одной ленты с РАЗНЫМИ цепочками отдали один кадр — \
         пост-FX кэш ключуется не экземпляром"
    );
    // суб-композит ленты лежит в кэше ОДИН — по адресу ленты и редакции
    assert!(
        cache
            .get_postfx_video(
                kt_engine::nest_source_id(seq),
                (q.width, q.height),
                kt_engine::content_hash(&two, seq),
                t.0
            )
            .is_some(),
        "суб-композит не мемоизирован по адресу ленты"
    );
}

// ---- 447-449 (Р.50, Б4): аудио — суб-микс --------------------------------

/// Реверб без растяжения времени: варп на вложении пока запрещён, а
/// хвост — ровно то, что должно тянуться ЧЕРЕЗ шов между репликами.
fn reverb() -> FxInstance {
    FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.6,
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

fn audio_nest(outer_fx: Vec<FxInstance>, inner_fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Реплики");
    // две реплики встык — то самое «десять реплик под одной цепочкой»
    let mut a = ev(2, ClipKind::Audio, 0, Tick(0), PART, 1);
    let mut b = ev(3, ClipKind::Audio, 0, PART, PART, 2);
    a.fx = inner_fx.clone();
    b.fx = inner_fx;
    s.clips = vec![a, b];
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Audio, 0, Tick(0), PART + PART, 100);
    inst.source_in = Tick(0);
    inst.fx = outer_fx;
    p.clips.push(inst);
    p.validate().expect("проект со звуковым вложением законен");
    p
}

fn spread_audio(fx: Vec<FxInstance>) -> Project {
    let mut p = base();
    let mut a = ev(2, ClipKind::Audio, 0, Tick(0), PART, 1);
    let mut b = ev(3, ClipKind::Audio, 0, PART, PART, 2);
    a.fx = fx.clone();
    b.fx = fx;
    p.clips = vec![a, b];
    p.validate().expect("развёрнутый проект законен");
    p
}

#[test]
fn t447_audio_law_e_through_the_submix() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let a = render_audio(&spread_audio(vec![]), frames);
    let b = render_audio(&audio_nest(vec![], vec![]), frames);
    assert_eq!(
        a, b,
        "ЗАКОН Э по аудио через СУБ-МИКС: вложение с пустой цепочкой обязано \
         звучать БАЙТ-В-БАЙТ как развёрнутое содержимое. Вложение — виртуальное \
         медиа под фетчером, а не ветка в миксере, именно чтобы порядок сложения \
         f32 не поехал"
    );
    assert!(a.iter().any(|x| x.abs() > 1e-6), "фикстура молчит");

    // ЗАКАЗ БЁРСТОВ уходит вглубь: у ленты источника нет, и адрес ленты в
    // заказе означал бы, что префетч греет несуществующее, а суб-микс
    // читает настоящее — вечный промах (п.13)
    let r = AudioRenderer::new(&audio_nest(vec![], vec![]));
    let want = r.wanted_audio_bursts(0..frames as i64);
    assert!(!want.is_empty(), "заказ пуст — проверять нечего");
    assert!(
        want.iter().all(|(m, _)| *m != MediaId(100)),
        "в заказе бёрстов стоит АДРЕС ЛЕНТЫ: {want:?}"
    );
    assert!(
        want.iter().any(|(m, _)| *m == MediaId(1)) && want.iter().any(|(m, _)| *m == MediaId(2)),
        "заказ не покрыл содержимое ленты: {want:?}"
    );
}

#[test]
fn t449_one_reverb_on_ten_replicas_is_not_a_chain_on_each() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let rev = vec![reverb()];
    // цепочка НА ВЛОЖЕНИИ: один хвост на обе реплики, и он тянется ЧЕРЕЗ шов
    let on_nest = render_audio(&audio_nest(rev.clone(), vec![]), frames);
    // цепочка на КАЖДОМ ребёнке: два независимых хвоста, обрывающихся на швах
    let on_each = render_audio(&spread_audio(rev.clone()), frames);
    assert_ne!(
        on_nest, on_each,
        "реверб на вложении посчитался покусочно. «Одна цепочка на группу \
         реплик» — то, ради чего звуковое вложение и нужно"
    );
    assert!(on_nest.iter().any(|x| x.abs() > 1e-6), "вложение молчит");

    // ...и цепочка на вложении ВООБЩЕ действует: без неё звучало бы сухо
    let dry = render_audio(&audio_nest(vec![], vec![]), frames);
    assert_ne!(
        on_nest, dry,
        "цепочка на вложении молча пропала — до Б4 её роняло раскрытие, и \
         это был бы ровно тот случай «тихо посчитали другое»"
    );

    // Цепочка ВНУТРИ ленты тоже обязана звучать: её процессоры живут в
    // своём сторе, и без рекурсии `sync_fx` их бы просто не завели.
    let inside = render_audio(&audio_nest(vec![], rev), frames);
    // Ключевое утверждение — НЕ «отличается от сухого»: без синхронизации
    // процессор внутренней цепочки не заводится, `render_fx_clip` выходит
    // на первой строке, и событие просто ПРОПАДАЕТ. Отличие от сухого при
    // этом тоже есть, и тест был бы зелёным на сломанном коде.
    let energy = |v: &[f32]| v.iter().map(|x| x.abs() as f64).sum::<f64>();
    assert!(
        energy(&inside) > energy(&dry) * 0.3,
        "звук внутри ленты ПРОПАЛ ({:.1} против {:.1} у сухого): состояние \
         вложенных цепочек не синхронизировано с моделью, и `render_fx_clip` \
         молча вышел, не найдя процессора",
        energy(&inside),
        energy(&dry)
    );
    assert_ne!(inside, dry, "цепочка внутри ленты не изменила звук");
    assert_ne!(inside, on_nest, "цепочка внутри и снаружи дали одно и то же");
}

#[test]
fn t448_submix_is_the_same_whole_and_in_chunks() {
    // Потоковая цепочка внутри ленты обязана переживать нарезку выхода: у
    // плеера кольцо заполняется кусками, и шов между ними не должен быть
    // слышен (родня t338 «куском == кусками»).
    let p = audio_nest(vec![reverb()], vec![]);
    let frames = (PART + PART).to_audio_frames() as usize;
    let whole = render_audio(&p, frames);

    let r = AudioRenderer::new(&p);
    let mut store = kt_engine::fxstore::FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = direct();
    let mut pieces = vec![0f32; frames * CH];
    let chunk = 1024usize;
    let mut done = 0usize;
    while done < frames {
        let n = chunk.min(frames - done);
        let (_, tail) = pieces.split_at_mut(done * CH);
        let (dst, _) = tail.split_at_mut(n * CH);
        r.render_frames_with(&mut f, &mut store, done as i64, dst);
        done += n;
    }
    assert_eq!(
        whole, pieces,
        "суб-микс, посчитанный КУСКАМИ, разошёлся с ним же целиком: внутренняя \
         цепочка теряет непрерывность на швах выхода"
    );

    // seek уходит ВГЛУБЬ: после reset хвост из другого места не звучит
    let mut store2 = kt_engine::fxstore::FxStateStore::new();
    r.sync_fx(&mut store2);
    let mut far = vec![0f32; 4096 * CH];
    r.render_frames_with(&mut f, &mut store2, 200_000, &mut far);
    store2.reset_all();
    let mut after = vec![0f32; 4096 * CH];
    r.render_frames_with(&mut f, &mut store2, 0, &mut after);
    let mut fresh_store = kt_engine::fxstore::FxStateStore::new();
    r.sync_fx(&mut fresh_store);
    let mut fresh = vec![0f32; 4096 * CH];
    r.render_frames_with(&mut f, &mut fresh_store, 0, &mut fresh);
    assert_eq!(
        after, fresh,
        "после seek внутри ленты остался хвост из ДРУГОГО места: `reset_all` \
         не дошёл до цепочек вложения"
    );
}

// ---- 450-452 (Р.50, Б5-Б6): ступень, бюджеты, прогретость -----------------

/// Вложение, у экземпляра которого цепочка ЗУМИТ собранную картинку.
fn zooming_nest(zoom: f64) -> Project {
    let mut p = base();
    let mut s = Sequence::new(MediaId(100), "Кусок");
    s.clips = vec![ev(2, ClipKind::Video, 0, Tick(0), PART, 1)];
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), PART, 100);
    inst.source_in = Tick(0);
    inst.fx = vec![FxInstance::Pan { mode: Default::default(), pan_x: 0.0, pan_y: 0.0, zoom }];
    p.clips.push(inst);
    p.validate().expect("зумящее вложение законно");
    p
}

#[test]
fn t450_nest_decode_stage_is_a_property_of_the_reel() {
    let q = Quality { width: 64, height: 36 };
    let t = Tick(FRAME * 5);

    // без зума ступень 1 — путь прежний
    let flat = VideoRenderer::new(&zooming_nest(1.0));
    let plain: Vec<Quality> = flat.layers_at(t, q).into_iter().map(|(_, _, dq)| dq).collect();
    assert!(plain.iter().all(|dq| *dq == q), "без зума заказ обязан идти в Quality: {plain:?}");

    // с зумом суб-план собирается КРУПНЕЕ, и заказ содержимого — тоже:
    // разойдись они — префетч грел бы один размер, а композит читал другой
    let z = VideoRenderer::new(&zooming_nest(3.0));
    let big: Vec<Quality> = z.layers_at(t, q).into_iter().map(|(_, _, dq)| dq).collect();
    assert!(!big.is_empty(), "заказ пуст");
    assert!(
        big.iter().all(|dq| dq.width > q.width),
        "заказ содержимого зумящего вложения остался в Quality — суб-композит \
         вышел бы мягким: {big:?}"
    );
    assert!(
        z.max_decode_quality(q).width >= big[0].width,
        "бюджеты считают меньше, чем заказывает префетч"
    );

    // ступень — свойство ЛЕНТЫ: второй экземпляр БЕЗ зума обязан получить
    // ТУ ЖЕ ступень, иначе у внутреннего клипа два ключа кэша и два заказа
    let mut two = zooming_nest(3.0);
    let mut twin = two.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * PART.0);
    twin.fx = vec![];
    two.clips.push(twin);
    two.validate().unwrap();
    let v = VideoRenderer::new(&two);
    let a: Vec<Quality> = v.layers_at(t, q).into_iter().map(|(_, _, dq)| dq).collect();
    let b: Vec<Quality> =
        v.layers_at(Tick(10 * PART.0 + t.0), q).into_iter().map(|(_, _, dq)| dq).collect();
    assert_eq!(a, b, "два экземпляра одной ленты заказали РАЗНЫЕ размеры: {a:?} против {b:?}");
}

#[test]
fn t451_nest_costs_its_subcomposite_in_the_chunk_budget() {
    let p = nest_with(1.0, vec![]);
    let v = VideoRenderer::new(&p);
    // внутри ленты два перекрывающихся события, снаружи один экземпляр:
    // бюджет обязан увидеть 1 (буфер суб-композита) + 2 (дети), а не 1
    assert_eq!(
        v.max_concurrent_layers(),
        3,
        "бюджет чанка не увидел содержимого вложения. Недосчёт вытесняет соседа \
         до композиции, срабатывает ветка голода, и БАЙТЫ ЭКСПОРТА начинают \
         зависеть от тайминга кэша"
    );
    // ...а плоский проект считается как раньше
    assert_eq!(VideoRenderer::new(&spread_with(1.0, vec![])).max_concurrent_layers(), 2);
}

#[test]
fn t452_nest_warmth_is_the_intersection_of_its_children() {
    let q = Quality { width: 64, height: 36 };
    let p = nest_with(1.0, vec![]);
    let v = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());

    // Пусто — холодно ВЕЗДЕ, ГДЕ ЕСТЬ ЧТО ДЕКОДИРОВАТЬ. Хвост ленты,
    // где детей нет вовсе, остаётся тёплым, и это правда: декодировать
    // там нечего, прозрачность приходит мгновенно.
    let material = 0..(PART.0 + PART.0 / 2); // дети живут здесь
    let cold = v.nest_coverage(&cache, q);
    assert_eq!(cold.len(), 1, "полоска вложения не построена");
    assert!(
        cold[0].1.iter().all(|r| r.end <= material.start || r.start >= material.end),
        "пустой кэш показан прогретым там, где лежат дети: {:?}",
        cold[0].1
    );

    // прогрели ОДИН момент: тёплой обязана стать только окрестность, и
    // только там, где пришли ВСЕ накрывающие дети
    let t = Tick(FRAME * 20);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in v.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let warm = v.nest_coverage(&cache, q);
    let runs = &warm[0].1;
    assert!(!runs.is_empty(), "прогретый момент не виден в полоске");
    assert!(
        runs.iter().any(|r| r.start <= t.0 && t.0 < r.end),
        "прогретый момент {} не попал ни в один прогон {runs:?}",
        t.0
    );
    // ...но НЕ весь пролёт: остальное не грели
    let span: i64 =
        runs.iter().map(|r| (r.end.min(material.end) - r.start.max(material.start)).max(0)).sum();
    assert!(
        span < material.end - material.start,
        "прогрет один кадр, а полоска показывает всю ленту тёплой ({span}) — \
         это объединение вместо пересечения, и оно врёт в ХОРОШУЮ сторону"
    );
}

/// Тест 463b (п.399, Р.51): переход МЕЖДУ репликами внутри ленты слышен.
/// Найденный попутно дефект Р.50: аудио вешало окна переходов из
/// `project.transitions` (корень), и переход внутри вложенной секвенции
/// молча не кроссфейдил звук — видео при этом брало переходы своей ленты.
#[test]
fn t463b_transition_inside_a_reel_is_audible() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let add_tr = |v: &mut Vec<TransitionEvent>| {
        v.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(2)),
            b: Some(ClipId(3)),
            in_offset: Tick(9_000),
            out_offset: Tick(9_000),
            kind: TransitionKind::Crossfade,
            audio: AudioCurve::Linear,
            auto: vec![],
            invert: false,
        });
    };
    let mut nested = audio_nest(vec![], vec![]);
    add_tr(&mut nested.sequences[0].transitions);
    nested.validate().expect("переход внутри ленты законен");
    let mut spread = spread_audio(vec![]);
    add_tr(&mut spread.transitions);
    spread.validate().unwrap();

    let inside = render_audio(&nested, frames);
    // закон Э: переход внутри ленты звучит байт-в-байт как развёрнутый
    assert_eq!(
        inside,
        render_audio(&spread, frames),
        "переход внутри ленты обязан звучать как тот же переход снаружи"
    );
    // ...и он вообще действует (до фикса тут была тишина про переход)
    assert_ne!(
        inside,
        render_audio(&audio_nest(vec![], vec![]), frames),
        "переход внутри ленты молча пропал"
    );
}

// ---- Р.55 (С4): реверс и растяжение НА вложении ---------------------------

/// Проект, где содержимое ленты стоит ПРЯМО на дорожках и целиком
/// зеркально: два клипа встык, порядок переставлен, у каждого `reversed`.
/// Это и есть «развёрнутое содержимое, проигранное задом наперёд» — тот
/// эталон, с которым обязан совпасть реверс ВЛОЖЕНИЯ на ленте без
/// хвостатых эффектов.
fn spread_audio_mirrored() -> Project {
    let mut p = base();
    // зеркало всего куска: бывший второй клип встаёт первым
    let mut a = ev(2, ClipKind::Audio, 0, PART, PART, 1);
    let mut b = ev(3, ClipKind::Audio, 0, Tick(0), PART, 2);
    a.reversed = true;
    b.reversed = true;
    p.clips = vec![a, b];
    p.validate().expect("зеркальный развёрнутый проект законен");
    p
}

/// Тест 561 (Р.55, С4): РЕВЕРС ВЛОЖЕНИЯ == реверс развёрнутого содержимого.
///
/// **Претензия владельца дословно:** «на композицию не работает реверсы».
/// Она и не работала: `validate` отвергал зеркальный экземпляр, потому что
/// суб-микс читался строго по порядку.
///
/// Это определение, а не приближение: «реверс собранного куска» — это
/// «отрендерить содержимое ВПЕРЁД и проиграть задом наперёд». На ленте без
/// хвостатых эффектов равенство обязано быть БАЙТ-В-БАЙТ — именно потому,
/// что суб-микс рендерится вперёд, а зеркало применяется к его сэмплам,
/// как к сэмплам файла.
#[test]
fn t561_reversing_a_nest_equals_reversing_its_content() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let mut p = audio_nest(vec![], vec![]);
    p.clips[0].reversed = true;
    p.validate().expect("зеркальное вложение обязано быть законным");

    let nested = render_audio(&p, frames);
    let spread = render_audio(&spread_audio_mirrored(), frames);
    assert_eq!(
        nested, spread,
        "реверс ВЛОЖЕНИЯ разошёлся с реверсом развёрнутого содержимого. Зеркало \
         действует на СЭМПЛЫ собранного микса — так же, как на сэмплы файла; \
         рендерить ленту назад значило бы посчитать третий, ничей результат"
    );
    assert!(nested.iter().any(|x| x.abs() > 1e-6), "зеркальное вложение молчит");
}

/// Тест 562 (Р.55, С4): ЗАМЕДЛЕНИЕ ВЛОЖЕНИЯ ≠ замедление каждого куска.
///
/// Реверс-аналог уже есть (t428 в модели), а это — аналог ЗАКОНА Г для
/// скорости: снаружи ресемплируется УЖЕ СОБРАННЫЙ микс со сшитыми
/// хвостами, внутри — каждый кусок со своим оборванным. Ровно ради этого
/// «смонтируй, потом замедли целиком» и заводится вложение.
#[test]
fn t562_slowing_a_nest_is_not_slowing_each_piece() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let slow = vec![FxInstance::slowed_reverb(0.5, 0.6, 60.0)];
    let on_nest = render_audio(&audio_nest(slow.clone(), vec![]), frames);
    let on_each = render_audio(&spread_audio(slow.clone()), frames);
    assert_ne!(
        on_nest, on_each,
        "замедление вложения посчиталось покусочно — «смонтируй, потом замедли \
         целиком» перестало отличаться от «замедли каждое»"
    );
    assert!(on_nest.iter().any(|x| x.abs() > 1e-6), "замедленное вложение молчит");
}

/// Тест 563 (Р.55, С4): суб-микс КУСКОМ == КУСКАМИ и при реверсе.
///
/// Родня t448/t338. Продюсер суб-микса читает ленту не по порядку, и без
/// канонической сетки блоков значение бёрста зависело бы от того, каким
/// куском его спросили, — то есть от размера выходного чанка. В аудио
/// такого не бывает (инвариант 11).
#[test]
fn t563_reversed_submix_is_the_same_whole_and_in_chunks() {
    let frames = (PART + PART).to_audio_frames() as usize;
    let mut p = audio_nest(vec![reverb()], vec![reverb()]);
    p.clips[0].reversed = true;
    p.validate().expect("зеркальное вложение с ревербами законно");

    let whole = render_audio(&p, frames);
    for chunk in [777usize, 1024, 4096] {
        let r = AudioRenderer::new(&p);
        let mut store = kt_engine::fxstore::FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; frames * CH];
        let mut f = direct();
        let mut done = 0usize;
        while done < frames {
            let n = chunk.min(frames - done);
            r.render_frames_with(
                &mut f,
                &mut store,
                done as i64,
                &mut out[done * CH..(done + n) * CH],
            );
            done += n;
        }
        assert_eq!(
            out, whole,
            "суб-микс реверснутого вложения зависит от НАРЕЗКИ выхода \
             (чанк {chunk}) — байты поехали от размера запроса"
        );
    }
}

/// Тест 564 (Р.55, С4): два экземпляра одной ленты — ДВА ПОТОКА.
///
/// Состояние суб-микса ключевалось адресом ЛЕНТЫ, при том что комментарий
/// рядом сам утверждал обратное («лента в пяти местах — ПЯТЬ разных
/// потоков»). Два одновременно звучащих экземпляра делили и внутренние
/// цепочки, и однобёрстовый кэш: под реверсом и растяжением они ещё и
/// дерутся за курсор продюсера.
#[test]
fn t564_two_instances_of_one_reel_do_not_share_a_reverb() {
    let frames = (PART + PART).to_audio_frames() as usize;
    // одна и та же лента в ДВУХ местах, со сдвигом, звучат одновременно
    let mut p = audio_nest(vec![], vec![reverb()]);
    let mut twin = p.clips[0].clone();
    twin.id = ClipId(11);
    twin.lane = 1;
    twin.start = Tick(PART.0 / 2);
    p.clips.push(twin);
    p.audio_lanes = 2;
    p.validate().expect("лента в двух местах законна");
    let both = render_audio(&p, frames);

    // каждый экземпляр ОТДЕЛЬНО — и сумма обязана совпасть с общим миксом
    let mut only_a = p.clone();
    only_a.clips.retain(|c| c.id == ClipId(10));
    let mut only_b = p.clone();
    only_b.clips.retain(|c| c.id == ClipId(11));
    let a = render_audio(&only_a, frames);
    let b = render_audio(&only_b, frames);

    let worst = both
        .iter()
        .zip(a.iter().zip(b.iter()))
        .map(|(m, (x, y))| (m - (x + y)).abs())
        .fold(0f32, f32::max);
    assert!(
        worst < 1e-5,
        "два экземпляра одной ленты дерутся за один поток суб-микса: худшее \
         расхождение с суммой независимых {worst:.6}. Ключ состояния обязан быть \
         адресом ЭКЗЕМПЛЯРА, а не ленты"
    );
    assert!(both.iter().any(|x| x.abs() > 1e-6), "сцена молчит");
}

/// Тест 565 (Р.55, С4): СЫРОЙ реверс-транспорт над вложением ЗВУЧИТ.
///
/// Дефект, найденный по ходу и живший с Р.50: `render_frames_raw` не
/// оборачивал фетчер, и `audio_burst(адрес ленты)` уходил в обычный
/// фетчер, где такого источника нет, — тишина плюс underrun'ы по кадрам.
/// При этом `wanted_audio_bursts_raw` честно рекурсировал внутрь: заказ
/// уходил, а чтение до него не доходило. Нарушение «заказ == чтение»
/// (инвариант 4) прямо в дереве.
#[test]
fn t565_raw_reverse_transport_over_a_nest_is_not_silence() {
    let frames = PART.to_audio_frames() as usize;
    let p = audio_nest(vec![], vec![]);
    let r = AudioRenderer::new(&p);
    let mut out = vec![0f32; frames * CH];
    let st = r.render_frames_raw(&mut direct(), 0, &mut out);
    assert!(
        out.iter().any(|x| x.abs() > 1e-6),
        "сырой путь над вложением отдал ТИШИНУ — адрес ленты ушёл в фетчер, \
         который про ленты не знает"
    );
    assert_eq!(st.underruns, 0, "сырой путь над вложением копит underrun'ы: {st:?}");
    // и ЗАКАЗ покрывает то, что он читает
    let want = r.wanted_audio_bursts_raw(0..frames as i64);
    assert!(
        want.iter().all(|(m, _)| *m != MediaId(100)),
        "в сыром заказе стоит адрес ЛЕНТЫ: {want:?}"
    );
    assert!(!want.is_empty(), "сырой заказ пуст");
}

// ---- Р.56 (п.448): НЕПРЕРЫВНОЕ внутреннее время ленты ---------------------
//
// Претензия владельца дословно: «slowed-reverb на композиции → текст и
// анимация внутри дёргается покадрово». Причина была одна и точная:
// внутреннее время вложения ФЛОРИЛОСЬ к сетке кадров, хотя декодированных
// кадров у ленты нет. При rate 0.5 два соседних выходных кадра получали
// ОДИН И ТОТ ЖЕ `inner_t`, мемо суб-композита отдавало ту же запись, и всё
// процедурное внутри (титры, автоматизация, камеры, зерно) шло ступеньками.
//
// Разделение, на котором стоят все тесты ниже: **пол по сетке ставит ЛИСТ, и
// только ФАЙЛОВЫЙ**. Футаж прибит к кадрам файла (просить между ними
// бессмысленно), процедурный источник и лента — нет.

/// Адреса процедурных медиа в фикстурах ниже.
const GEN: u64 = 4; // ЖИВОЙ вид: кадр — функция тика
const FLAT: u64 = 5; // СТАТИЧЕСКИЙ вид: кадр не зависит от времени вовсе

fn gen_kind() -> GeneratorKind {
    // шум counter-based: зерно = f(тик материала), поэтому ЛЮБОЕ смещение
    // тика меняет весь кадр — «дёргается или нет» видно без порогов
    GeneratorKind::NoiseField { amount: 1.0, mono: true }
}

/// Ровная заливка: кадр НЕ зависит от времени, поэтому единственным
/// источником движения в фикстуре остаётся автоматизация. Без этого мерить
/// «монотонно ли едет анимация» не на чем: цвет синтетики скачет с pts.
fn flat_kind() -> GeneratorKind {
    GeneratorKind::Solid { color_r: 20, color_g: 20, color_b: 20 }
}

/// Р.59, стадия 2: источники генераторов стоят по адресам СОБЫТИЙ, и
/// список у них один на всех — `gen_sources`.
fn sources_gen(p: &Project) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    let mut v = sources();
    for (addr, state) in kt_engine::gen_sources(p) {
        v.push((addr, kt_media::factory::open_gen(&state)));
    }
    v
}

/// Факты открытых источников — ровно так, как их собирают плеер, экспорт и
/// kt-cli: спросив у ОТКРЫТОГО источника его паспорт.
fn facts(p: &Project) -> kt_engine::SourceFacts {
    let mut f = kt_engine::SourceFacts::default();
    for (id, s) in sources_gen(p) {
        f.observe(id, s.info());
    }
    f
}

/// Значения генератора живут на СОБЫТИИ (Р.59, стадия 2): фикстура
/// раздаёт их всем событиям на процедурных медиа.
fn fix_gen(p: &mut Project) {
    let put = |cs: &mut Vec<kt_model::ClipEvent>| {
        for c in cs.iter_mut() {
            if c.media == MediaId(GEN) {
                c.r#gen = Some(gen_kind());
            } else if c.media == MediaId(FLAT) {
                c.r#gen = Some(flat_kind());
            }
        }
    };
    put(&mut p.clips);
    for i in 0..p.sequences.len() {
        let mut cs = std::mem::take(&mut p.sequences[i].clips);
        put(&mut cs);
        p.sequences[i].clips = cs;
    }
}

fn base_gen() -> Project {
    let mut p = base();
    p.media.push(MediaRef { id: MediaId(GEN), path: gen_kind_path(&gen_kind()) });
    p.media.push(MediaRef { id: MediaId(FLAT), path: gen_kind_path(&flat_kind()) });
    p
}

/// Варп-эффект на ВИДЕО: значима только скорость и режим кадров.
fn warp_fx(rate: f64, frames: FrameMode) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        frames,
        width: 1.0,
        automation: vec![],
    }
}

fn grade() -> FxInstance {
    FxInstance::ColorGrade { brightness: 0.0, contrast: 0.0, saturation: 1.0, hue: 0.0 }
}

/// Кривая яркости на ВЕСЬ материал ребёнка: линейная и монотонная —
/// «полушаг» виден как значение МЕЖДУ соседними целыми шагами.
fn grade_curve() -> Vec<Vec<ParamCurve>> {
    vec![vec![ParamCurve {
        param: "brightness".into(),

        // 0.9 на PART материала — это 3.8 уровня яркости на ПОЛУКАДР:
        // видно в байтах и не упирается в потолок на заливке 20
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(PART, 0.9)]),
        source: Default::default(),
    }]]
}

/// Лента с ОДНИМ ребёнком (`media`), экземпляр — со скоростью `rate` и
/// режимом кадров `mode`. `rate == 1.0` без варпа — эталон целых шагов.
fn one_child_nest(
    media: u64,
    warp: Option<(f64, FrameMode)>,
    child_fx: Vec<FxInstance>,
    child_auto: Vec<Vec<ParamCurve>>,
) -> Project {
    let mut p = base_gen();
    let mut s = Sequence::new(MediaId(100), "Кусок");
    let mut child = ev(2, ClipKind::Video, 0, Tick(0), Tick(PART.0 * 2), media);
    child.source_in = Tick(0);
    child.fx = child_fx;
    child.fx_auto = child_auto;
    s.clips = vec![child];
    p.sequences.push(s);
    let mut inst = ev(10, ClipKind::Video, 0, Tick(0), Tick(PART.0 * 2), 100);
    inst.source_in = Tick(0);
    inst.fx = warp.map(|(r, m)| vec![warp_fx(r, m)]).unwrap_or_default();
    p.clips.push(inst);
    fix_gen(&mut p);
    p.validate().expect("замедленное вложение законно");
    p
}

/// Композит момента `t`, прогретый РОВНО тем, что заказал `layers_at`
/// (заказ == чтение — инвариант 4). Рендерер знает факты источников.
fn render_with_facts(p: &Project, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new_with_facts(p, &facts(p));
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources_gen(p).into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, t, q)
}

fn mean_level(f: &[u8]) -> f64 {
    let n = (f.len() / 4).max(1);
    f.chunks_exact(4).map(|p| (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0).sum::<f64>()
        / n as f64
}

const QV: Quality = Quality { width: 64, height: 36 };

/// Тест 579 (Р.56, п.448): замедленная лента анимируется ПОЛУШАГАМИ.
///
/// Красный на старом коде: при поле по сетке `inner_t` у кадров 2i и 2i+1
/// совпадал, суб-композит брался из мемо, и пара была БАЙТ-В-БАЙТ равна.
#[test]
fn t579_a_slowed_reel_animates_in_half_steps() {
    let slow = one_child_nest(FLAT, Some((0.5, FrameMode::Nearest)), vec![grade()], grade_curve());
    let plain = one_child_nest(FLAT, None, vec![grade()], grade_curve());

    let mut frames: Vec<Vec<u8>> = Vec::new();
    for i in 0..40i64 {
        let (f, complete) = render_with_facts(&slow, Tick(i * FRAME), QV);
        assert!(complete, "кадр {i} неполон при полностью прогретом заказе");
        // ЧЁТНЫЙ кадр стоит на целом шаге материала — он обязан совпасть с
        // кадром той же ленты при rate = 1 БАЙТ-В-БАЙТ (закон Э на месте)
        if i % 2 == 0 {
            let (g, _) = render_with_facts(&plain, Tick(i / 2 * FRAME), QV);
            assert_eq!(f, g, "чётный кадр замедленной ленты разошёлся с кадром rate = 1 ({i})");
        }
        frames.push(f);
    }
    for i in 0..19usize {
        assert_ne!(
            frames[2 * i],
            frames[2 * i + 1],
            "кадры {} и {} равны БАЙТ-В-БАЙТ: внутреннее время ленты округляется к \
             сетке, и всё процедурное внутри идёт ступеньками по два кадра — ровно \
             то, что владелец увидел как «текст дёргается»",
            2 * i,
            2 * i + 1
        );
    }
    // ...и это ПОЛУШАГ, а не произвольное другое значение: кадр 2i+1 лежит
    // строго между своими целыми соседями, и яркость едет монотонно
    let means: Vec<f64> = frames.iter().map(|f| mean_level(f)).collect();
    for w in means.windows(2) {
        assert!(w[0] + 1e-9 < w[1], "яркость не монотонна: {:?} -> {:?}", w[0], w[1]);
    }
    assert!(means[0] + 10.0 < means[means.len() - 1], "кривая яркости не доехала до картинки");
    // и ровно то же показывает САМА ЛЕНТА на своём полукадре — независимая
    // формула: «замедление в два раза» это «то же место, шагами вдвое мельче»
    for i in 0..19i64 {
        let (half, _) = render_with_facts(&plain, Tick(i * FRAME + FRAME / 2), QV);
        assert_eq!(
            frames[(2 * i + 1) as usize],
            half,
            "кадр {} замедленной ленты не равен кадру ленты на её полукадре",
            2 * i + 1
        );
    }
}

/// Тест 580 (Р.56, п.448): футаж ПРИБИТ к кадрам файла, процедурное — нет.
///
/// Два утверждения в одном тесте намеренно. Первое краснеет, если
/// «непрерывное время» размазать и на файловый лист: просить кадр между
/// кадрами файла бессмысленно, декодер отдаст тот же, а ключей станет вдвое
/// больше. Второе краснеет на СТАРОМ коде.
#[test]
fn t580_footage_is_pinned_and_the_order_does_not_move() {
    let plain = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    let v = VideoRenderer::new_with_facts(&plain, &facts(&plain));
    for i in 0..10i64 {
        let (a, _) = render_with_facts(&plain, Tick(2 * i * FRAME), QV);
        let (b, _) = render_with_facts(&plain, Tick((2 * i + 1) * FRAME), QV);
        assert_eq!(
            a, b,
            "футаж внутри замедленной ленты поехал на полукадре: у ФАЙЛА кадра \
             «между кадрами» не существует, и пол по сетке ставит именно он"
        );
        assert_eq!(
            v.layers_at(Tick(2 * i * FRAME), QV),
            v.layers_at(Tick((2 * i + 1) * FRAME), QV),
            "заказ на полукадре разошёлся с заказом на кадре"
        );
    }

    // с автоматизацией внутри — кадры РАЗНЫЕ, а заказ тот же: двигается
    // процедурное, футаж стоит
    let live = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![grade()], grade_curve());
    let vl = VideoRenderer::new_with_facts(&live, &facts(&live));
    for i in 0..10i64 {
        let (a, _) = render_with_facts(&live, Tick(2 * i * FRAME), QV);
        let (b, _) = render_with_facts(&live, Tick((2 * i + 1) * FRAME), QV);
        assert_ne!(a, b, "автоматизация внутри ленты стоит на месте полкадра ({i})");
        assert_eq!(
            vl.layers_at(Tick(2 * i * FRAME), QV),
            vl.layers_at(Tick((2 * i + 1) * FRAME), QV),
            "кривая сдвинула ЗАКАЗ кадров — она не имеет права этого делать"
        );
    }
}

/// Тест 581 (Р.56, п.448): «смешивание» на вложении смешивает ЛИСТЬЯ.
///
/// Два суб-композита на кадр вдвое дороже на обоих бэкендах и ничего не
/// добавляют: время ленты и так непрерывно. Поэтому режим НАСЛЕДУЕТСЯ вниз,
/// и пару собирает лист — тем же целочисленным ядром, что и t250.
#[test]
fn t581_blend_on_a_nest_mixes_footage_inside() {
    use kt_media::synthetic::{AudioPattern, SyntheticSource};

    let blend = one_child_nest(1, Some((0.5, FrameMode::Blend)), vec![], vec![]);
    let near = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    let t = Tick(FRAME); // первый ПОЛУКАДР: внутреннее время = FRAME/2

    let (out, complete) = render_with_facts(&blend, t, QV);
    assert!(complete, "обе стороны пары обязаны быть в заказе, значит и в кэше");

    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 });
    let a = synth.read_video_frame(Tick(0), Some(QV)).unwrap().rgba.into_vec();
    let b = synth.read_video_frame(Tick(FRAME), Some(QV)).unwrap().rgba.into_vec();
    let want: Vec<u8> = a
        .iter()
        .zip(&b)
        .map(|(&x, &y)| ((x as u32 * 2048 + y as u32 * 2048 + 2048) >> 12) as u8)
        .collect();
    assert_eq!(
        out, want,
        "полукадр смешанной ленты не равен смеси соседних кадров ИСТОЧНИКА — \
         смешивание на вложении либо не считается, либо считается не тем ядром"
    );

    // «ближайший» на тех же данных отдаёт ЧИСТЫЙ кадр
    let (n, _) = render_with_facts(&near, t, QV);
    assert_eq!(n, a, "режим «ближайший» держит кадр");
    assert_ne!(n, out, "смешивание не отличается от ближайшего — режима нет");

    // кадр НА СЕТКЕ не изменился: вес пары там 0
    assert_eq!(
        render_with_facts(&blend, Tick(0), QV).0,
        render_with_facts(&near, Tick(0), QV).0,
        "на целом шаге смешивать нечего, а байты разошлись"
    );

    // ЗАКАЗ несёт обе стороны: не согрей вторую — слой голодал бы вечно
    let v = VideoRenderer::new_with_facts(&blend, &facts(&blend));
    let keys: Vec<(MediaId, Tick, Quality)> = v.layers_at(t, QV);
    assert!(
        keys.contains(&(MediaId(1), Tick(0), QV)) && keys.contains(&(MediaId(1), Tick(FRAME), QV)),
        "в заказе нет соседа пары: {keys:?}"
    );
    let vn = VideoRenderer::new_with_facts(&near, &facts(&near));
    assert_eq!(vn.layers_at(t, QV).len(), 1, "«ближайший» заказывает ОДИН кадр");
}

/// Тест 582 (Р.56, п.448): ключ мемоизации суб-композита несёт РЕЖИМ.
///
/// Тот же `inner_t`, та же редакция ленты, разные байты. Без флага в ключе
/// композит «ближайшего» после композита «смешивания» вернул бы смешанную
/// картинку из мемо — и ошибка была бы невидимой (кадр правдоподобный).
#[test]
fn t582_the_memo_key_carries_the_frame_mode() {
    use kt_media::synthetic::{AudioPattern, SyntheticSource};

    let blend = one_child_nest(1, Some((0.5, FrameMode::Blend)), vec![], vec![]);
    let near = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    let t = Tick(FRAME);
    let seq = MediaId(100);
    // кэш ОДИН на оба прогона — в программе он и есть один
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());

    let mut vb = VideoRenderer::new_with_facts(&blend, &facts(&blend));
    let mixed = warm_and_compose(&mut vb, &cache, t, QV);
    let mut vn = VideoRenderer::new_with_facts(&near, &facts(&near));
    let clean = warm_and_compose(&mut vn, &cache, t, QV);

    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 });
    let a = synth.read_video_frame(Tick(0), Some(QV)).unwrap().rgba.into_vec();
    assert_ne!(mixed, clean, "два режима отдали один кадр");
    assert_eq!(
        clean, a,
        "«ближайший» получил СМЕШАННЫЙ суб-композит из мемо: ключ не различает режим"
    );

    // записи лежат по РАЗНЫМ ключам, и «без смешивания» ключ — сам хэш
    let h = kt_engine::content_hash(&blend, seq);
    assert_eq!(kt_engine::nest_memo_key(h, false), h, "прежние записи обязаны остаться на месте");
    assert_ne!(kt_engine::nest_memo_key(h, true), h);
    let inner = Tick(FRAME / 2);
    for (mode, key) in [(true, kt_engine::nest_memo_key(h, true)), (false, h)] {
        assert!(
            cache
                .get_postfx_video(
                    kt_engine::nest_source_id(seq),
                    (QV.width, QV.height),
                    key,
                    inner.0
                )
                .is_some(),
            "суб-композит режима {mode} не мемоизирован"
        );
    }
}

/// Тест 583 (Р.56, п.448): бюджет чанка видит ПАРУ ВНУТРИ ленты.
///
/// Недосчёт вытесняет соседа до композиции, срабатывает ветка голода, и
/// БАЙТЫ ЭКСПОРТА начинают зависеть от тайминга кэша — дословно регрессия
/// Р.37 С5, только этажом ниже.
#[test]
fn t583_the_chunk_budget_counts_the_pair_inside_the_reel() {
    let blend = one_child_nest(1, Some((0.5, FrameMode::Blend)), vec![], vec![]);
    let near = one_child_nest(1, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    assert_eq!(
        VideoRenderer::new_with_facts(&near, &facts(&near)).max_concurrent_layers(),
        2,
        "буфер суб-композита плюс один ребёнок"
    );
    assert_eq!(
        VideoRenderer::new_with_facts(&blend, &facts(&blend)).max_concurrent_layers(),
        3,
        "смешивающий ЛИСТ внутри ленты держит два кадра — бюджет обязан их увидеть"
    );
}

/// Тест 584 (Р.56, п.448): реверс И растяжение вместе.
///
/// Зеркало считается ПОСЛЕ варпа и якорится фактически потреблённым `used`;
/// на непрерывном времени это ещё и кламп у края материала. Проверяем, что
/// последовательность идёт НАЗАД полушагами и не падает на клампе.
#[test]
fn t584_reversed_and_slowed_nest_runs_backwards_in_half_steps() {
    let mut p = one_child_nest(FLAT, Some((0.5, FrameMode::Nearest)), vec![grade()], grade_curve());
    p.clips[0].reversed = true;
    p.validate().expect("зеркальное замедленное вложение законно");

    let mut frames: Vec<Vec<u8>> = Vec::new();
    for i in 0..40i64 {
        let (f, complete) = render_with_facts(&p, Tick(i * FRAME), QV);
        assert!(complete, "кадр {i} неполон");
        frames.push(f);
    }
    // КАЖДАЯ соседняя пара, а не только (2i, 2i+1): у зеркала фаза пола
    // сдвинута, и «проверю чётные с нечётными» прошло бы мимо дефекта
    for i in 0..frames.len() - 1 {
        assert_ne!(
            frames[i],
            frames[i + 1],
            "зеркальная замедленная лента стоит полкадра на месте ({i})"
        );
    }
    let means: Vec<f64> = frames.iter().map(|f| mean_level(f)).collect();
    for w in means.windows(2) {
        assert!(w[0] + 1e-9 >= w[1], "зеркало идёт не назад: {:?} -> {:?}", w[0], w[1]);
    }
    assert!(means[0] > means[means.len() - 1] + 1.0, "кривая не доехала до картинки");
}

/// Тест 585 (Р.56, п.448): ПРОЦЕДУРНЫЙ источник считается на непрерывном
/// тике — и в ленте, и В КОРНЕ.
///
/// Красный на старом коде дважды: замедленный генератор в корне отдавал
/// пару одинаковых кадров (пол по сетке), и он же внутри ленты — тем более
/// (два пола подряд).
#[test]
fn t585_procedural_sources_run_on_the_continuous_tick() {
    // (а) В КОРНЕ: замедленный генератор прямо на дорожке
    let mut root = base_gen();
    let mut c = ev(1, ClipKind::Video, 0, Tick(0), Tick(PART.0 * 2), GEN);
    c.source_in = Tick(0);
    c.fx = vec![warp_fx(0.5, FrameMode::Nearest)];
    root.clips.push(c);
    fix_gen(&mut root);
    root.validate().expect("замедленный генератор в корне законен");
    for i in 0..8i64 {
        let (a, ca) = render_with_facts(&root, Tick(2 * i * FRAME), QV);
        let (b, cb) = render_with_facts(&root, Tick((2 * i + 1) * FRAME), QV);
        assert!(ca && cb, "заказ не покрыл чтение: кадр {i} неполон");
        assert_ne!(
            a, b,
            "замедленный ГЕНЕРАТОР в корне отдал два одинаковых кадра: у него нет \
             своей сетки кадров, и прижимать его к чужой незачем"
        );
        assert!(a.iter().any(|&x| x != 0), "кадр чёрный — проверять нечего");
    }

    // (б) ВНУТРИ ленты: тот же генератор ребёнком замедленного вложения
    let nested = one_child_nest(GEN, Some((0.5, FrameMode::Nearest)), vec![], vec![]);
    for i in 0..8i64 {
        let (a, ca) = render_with_facts(&nested, Tick(2 * i * FRAME), QV);
        let (b, cb) = render_with_facts(&nested, Tick((2 * i + 1) * FRAME), QV);
        assert!(ca && cb, "кадр {i} внутри ленты неполон");
        assert_ne!(a, b, "замедленный генератор ВНУТРИ ленты дёргается по два кадра");
    }

    // (в) ПОЛОСКА ПРОГРЕТОСТИ честна: пустой кэш — холодно, прогретое
    // ЦЕЛИКОМ окно — тепло. Растянутый ребёнок считается «целиком или
    // никак» (Р.55, С4), и врать в хорошую сторону она не имеет права.
    let v = VideoRenderer::new_with_facts(&nested, &facts(&nested));
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let body = 0..PART.0;
    let cold = v.nest_coverage(&cache, QV);
    assert_eq!(cold.len(), 1, "полоска вложения не построена");
    assert!(
        cold[0].1.iter().all(|r| r.end <= body.start || r.start >= body.end),
        "пустой кэш показан прогретым: {:?}",
        cold[0].1
    );
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources_gen(&nested).into_iter().collect();
    for (media, src_pts, dq) in
        v.wanted_frames(kt_time::TickRange::new(Tick(0), Tick(PART.0 * 2)), QV)
    {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        // ТАК ЖЕ, КАК ВОРКЕР: кадр закрывает полосу в один шаг сетки
        // (`RequestKind::video_on_grid`). Ключ у процедурного источника
        // внесеточный, и полоски честны ровно потому, что соседние заказы
        // ближе шага — иначе между ними осталась бы видимая дырка.
        cache.insert_video_span(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            FRAME,
            frame.into(),
        );
    }
    let warm = v.nest_coverage(&cache, QV);
    assert!(
        warm[0].1.iter().any(|r| r.start <= body.start && r.end >= body.end),
        "прогретое ЦЕЛИКОМ окно показано холодным: {:?}",
        warm[0].1
    );
}

// ---- 631-633 (Р.58, п.459): ступень ЛЕНТЫ ≠ вход цепочки экземпляра -------
//
// Ступень ленты одна на всех её экземпляров (t450: у внутреннего листа обязан
// быть ОДИН ключ кэша). Входом цепочки её отдавать каждому экземпляру нельзя:
// пустая цепочка, одна сила слоя, ведущий стоп (морфер-слой) и цепочка без
// ресемпла ступени не потребляют. До Р.58 такой экземпляр получал суб-композит
// в S×Quality — debug падал, release МОЛЧА выбрасывал вложение из кадра.

/// Фактура, на которой вписывание ВИДНО: у `SyntheticSource` заливка плоская,
/// и разница между «вписали крупный» и «собрали мелкий» на ней неразличима.
fn checker_sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    (1u64..=3)
        .map(|id| {
            (
                MediaId(id),
                // натив ЧУТЬ крупнее Quality: на 512 фактура усредняется в
                // ровную серость на обоих путях, и сторож слепнет
                Box::new(kt_media::synthetic::CheckerSource::new(Tick(90_000 * 60), 128, 72))
                    as Box<dyn MediaSource>,
            )
        })
        .collect()
}

fn render_checker(p: &Project, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        checker_sources().into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, t, q)
}

/// Лента с ДВУМЯ экземплярами: зумящий в нуле и близнец со своей цепочкой в
/// `10·PART` — ровно конструкция t450, но её ещё и рендерят.
fn twin_reel(zoom: f64, twin_fx: Vec<FxInstance>) -> Project {
    let mut p = zooming_nest(zoom);
    let mut twin = p.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * PART.0);
    twin.fx = twin_fx;
    p.clips.push(twin);
    p.validate().expect("лента с двумя экземплярами законна");
    p
}

/// Одинокий экземпляр ленты со своей цепочкой — тот же кадр, но ступень
/// ленты некому поднимать.
fn lone_reel(fx: Vec<FxInstance>) -> Project {
    let mut p = zooming_nest(1.0);
    p.clips[0].id = ClipId(11);
    p.clips[0].start = Tick(10 * PART.0);
    p.clips[0].fx = fx;
    p.validate().expect("одинокий экземпляр законен");
    p
}

fn pan(zoom: f64) -> FxInstance {
    FxInstance::Pan { mode: Default::default(), pan_x: 0.0, pan_y: 0.0, zoom }
}

const T_TWIN: Tick = Tick(10 * PART.0 + FRAME * 5);

#[test]
fn t631_every_instance_of_a_reel_renders() {
    let q = Quality { width: 64, height: 36 };
    let t_zoom = Tick(FRAME * 5);
    // пустая цепочка; только барьер без ресемпла; ведущая сила слоя —
    // ровно то, что кладёт морфер-слой на экземпляр с пустой цепочкой
    let chains: Vec<(&str, Vec<FxInstance>)> = vec![
        ("empty", vec![]),
        ("blur", vec![FxInstance::Blur { radius: 2, radius_y: 2 }]),
        ("leading mix", vec![pan(2.0), FxInstance::LayerMix { amount: 0.5, from: 0 }]),
    ];
    for (name, fx) in chains {
        let p = twin_reel(3.0, fx);
        let (twin, complete) = render_checker(&p, T_TWIN, q);
        assert!(complete, "{name}: кадр объявлен неполным");
        assert!(
            twin.chunks(4).any(|px| px[3] > 0 && px[0] > 0),
            "{name}: вложение ВЫПАЛО из кадра — суб-композит собран крупнее, \
             чем цепочка экземпляра умеет потребить, и слой молча выброшен"
        );
        // соседний экземпляр той же ленты не пострадал
        let (zooming, complete_z) = render_checker(&p, t_zoom, q);
        assert!(complete_z, "{name}: зумящий экземпляр объявлен неполным");
        assert!(zooming.chunks(4).any(|px| px[3] > 0), "{name}: зумящий экземпляр пуст");
        // и заказ у обоих ОДИН (t450 держится)
        let v = VideoRenderer::new(&p);
        let a: Vec<Quality> = v.layers_at(t_zoom, q).into_iter().map(|(_, _, dq)| dq).collect();
        let b: Vec<Quality> = v.layers_at(T_TWIN, q).into_iter().map(|(_, _, dq)| dq).collect();
        assert_eq!(a, b, "{name}: экземпляры одной ленты заказали разные размеры");
    }
}

#[test]
fn t631b_chain_input_is_the_instances_own() {
    let q = Quality { width: 64, height: 36 };
    let t_zoom = Tick(FRAME * 5);

    // (а) ВХОД РАВЕН СТУПЕНИ — путь прежний, байт-в-байт. Цепочка близнеца
    // просит столько же, сколько лента даёт: вписывания нет.
    let both = render_checker(&twin_reel(3.0, vec![pan(3.0)]), T_TWIN, q).0;
    let lone = render_checker(&lone_reel(vec![pan(3.0)]), T_TWIN, q).0;
    assert_eq!(
        both, lone,
        "экземпляр, который сам просит ступень ленты, обязан считаться ТЕМ ЖЕ \
         путём, что и в одиночку: лишний ресемпл на ровном месте"
    );
    // и сам зумящий экземпляр не сдвинулся от появления соседа
    assert_eq!(
        render_checker(&twin_reel(3.0, vec![]), t_zoom, q).0,
        render_checker(&zooming_nest(3.0), t_zoom, q).0,
        "зумящий экземпляр поехал от того, что рядом появился близнец"
    );

    // (б) ВХОД НИЖЕ СТУПЕНИ — кадр равен ВПИСАННОМУ суб-композиту. Оракул
    // считается независимо: содержимое ленты рендерится корнем в 3×Quality
    // и вписывается той же билинейкой.
    let fitted = render_checker(&twin_reel(3.0, vec![]), T_TWIN, q).0;
    let big = Quality { width: q.width * 3, height: q.height * 3 };
    let mut content = base();
    content.clips = vec![ev(2, ClipKind::Video, 0, Tick(0), PART, 1)];
    content.video_lanes = 1;
    content.validate().unwrap();
    let sub = render_checker(&content, Tick(FRAME * 5), big).0;
    let src = kt_fx::video::Frame { w: big.width, h: big.height, rgba: sub, has_alpha: false };
    let mut oracle = vec![0u8; (q.width * q.height * 4) as usize];
    kt_fx::video::fit_into(&src, q.width, q.height, &mut oracle);
    assert_eq!(
        fitted, oracle,
        "вписанный суб-композит разошёлся с независимой формулой: вписывание \
         обязано быть ТОЙ ЖЕ билинейкой, что у сухой стороны смешивания"
    );
    // сторож самой фикстуры: на ровной заливке равенство выше ничего не
    // значило бы — фактура обязана быть В КАДРЕ
    assert!(
        fitted.chunks(4).any(|px| px[0] < 64) && fitted.chunks(4).any(|px| px[0] > 192),
        "в кадре нет фактуры — оракул сверяет две одинаковые заливки"
    );
    // ВПИСЫВАНИЕ вообще случилось: вход цепочки у близнеца ниже ступени
    assert_eq!(
        VideoRenderer::new(&twin_reel(3.0, vec![])).layers_at(T_TWIN, q)[0].2,
        Quality { width: q.width * 3, height: q.height * 3 },
        "содержимое ленты заказано НЕ в ступени — вписывать было бы нечего"
    );
}

#[test]
fn t631c_reel_stage_rides_in_the_postfx_key() {
    let q = Quality { width: 64, height: 36 };
    // ОДИН кэш на оба рендера: сначала лента без зумящего близнеца (ступень
    // 1), потом с ним (ступень 3). Вход цепочки у близнеца в обоих случаях
    // 1×, `fx_hash` тот же — различить кадры обязан ключ.
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        checker_sources().into_iter().collect();
    let mut warm = |p: &Project, v: &VideoRenderer, t: Tick| {
        for (media, src_pts, dq) in v.layers_at(t, q) {
            let _ = p;
            let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
            cache.insert_video(
                kt_engine::source_id(media),
                Some(dq),
                kt_engine::video::video_cache_idx(src_pts),
                frame.into(),
            );
        }
    };
    let fx = vec![FxInstance::Blur { radius: 2, radius_y: 2 }];
    let flat = lone_reel(fx.clone());
    let mut v1 = VideoRenderer::new(&flat);
    warm(&flat, &v1, T_TWIN);
    let _ = v1.compose_from_cache(&cache, T_TWIN, q);

    let staged = twin_reel(3.0, fx);
    let mut v2 = VideoRenderer::new(&staged);
    warm(&staged, &v2, T_TWIN);
    let (dirty, _) = v2.compose_from_cache(&cache, T_TWIN, q);

    let fresh = render_checker(&staged, T_TWIN, q).0;
    assert_eq!(
        dirty, fresh,
        "пост-FX кэш отдал кадр ПРОШЛОЙ ступени ленты: вход цепочки и хэш \
         цепочки те же, а байты разные — ступень обязана быть в ключе"
    );
}
