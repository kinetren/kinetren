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
