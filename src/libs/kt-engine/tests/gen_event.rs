//! Р.59, стадия 2: параметры генератора живут НА СОБЫТИИ.
//!
//! Тесты t658, t659, t663 и t670. Все четыре про одно: значения уехали с
//! медиа на событие, значит у каждого события СВОЙ адрес источника —
//! и всё, что стоит на «заказ == чтение» (инвариант 4), обязано спрашивать
//! этот адрес одинаково: план, префетч, композит, экспорт и полоски.

use kt_engine::{VideoRenderer, render_raw_video};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

const Q: Quality = Quality { width: 96, height: 54 };

fn base() -> Project {
    Project::new(ProjectSettings {
        fps: 30,
        width: 96,
        height: 54,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    })
}

fn title(text: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_text();
    if let GeneratorKind::Text { text: t, size, .. } = &mut k {
        *t = text.into();
        *size = 0.4;
    }
    k
}

fn clip(id: u64, lane: u32, start: i64, dur: i64, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
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

/// Два титра ОДНОГО вида (одно медиа) с разным текстом.
fn two_titles() -> Project {
    let mut p = base();
    p.media.push(MediaRef { id: MediaId(1), path: gen_kind_path(&title("")) });
    let mut a = clip(1, 1, 0, 90_000, 1);
    a.r#gen = Some(title("AAAA"));
    let mut b = clip(2, 0, 0, 90_000, 1);
    b.r#gen = Some(title("BB"));
    // верхний слой сдвинут вниз, чтобы буквы не перекрывали друг друга
    if let Some(GeneratorKind::Text { pos_y, .. }) = &mut b.r#gen {
        *pos_y = 0.8;
    }
    p.clips = vec![a, b];
    p.validate().expect("два титра на одном виде законны");
    p
}

fn gen_srcs(p: &Project) -> std::collections::HashMap<MediaId, Box<dyn MediaSource>> {
    kt_engine::gen_sources(p)
        .into_iter()
        .map(|(addr, st)| (addr, kt_media::factory::open_gen(&st)))
        .collect()
}

/// Композит момента `t`, прогретый РОВНО тем, что заказал `layers_at`.
fn compose(p: &Project, t: Tick) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs = gen_srcs(p);
    for (addr, pts, dq) in video.layers_at(t, Q) {
        let f = srcs
            .get_mut(&addr)
            .unwrap_or_else(|| panic!("заказан адрес {addr:?}, а источника по нему нет"))
            .read_video_frame(pts, Some(dq))
            .unwrap();
        cache.insert_video(
            kt_engine::source_id(addr),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            f.into(),
        );
    }
    video.compose_from_cache(&cache, t, Q)
}

/// Тест 658 (Р.59, стадия 2): ЗАКАЗ == ЧТЕНИЕ для двух титров ОДНОГО вида
/// с разным текстом.
///
/// До этого раунда такой сцены не существовало: текст был частью пути
/// медиа, и два титра с разным текстом были двумя РАЗНЫМИ медиа. Теперь
/// медиа одно, а источников два — по одному на событие, — и вся
/// арифметика кэша обязана это выдержать: адреса различны, композит полон,
/// картинки разные.
#[test]
fn t658_order_equals_read_for_two_titles_of_one_kind() {
    let p = two_titles();
    let video = VideoRenderer::new(&p);
    let order = video.layers_at(Tick(0), Q);
    assert_eq!(order.len(), 2, "заказаны оба слоя");
    assert_ne!(order[0].0, order[1].0, "у двух событий обязаны быть РАЗНЫЕ адреса источника");
    assert_eq!(
        order[0].0,
        kt_engine::gen_source_addr(ClipId(1)),
        "адрес считается от события, а не от медиа (порядок — задний план первым)"
    );

    // ключ заказа содержит ровно то, что умеет отдать источник
    let (frame, complete) = compose(&p, Tick(0));
    assert!(complete, "композит неполон: заказ и чтение разошлись по адресу");
    assert!(frame.iter().any(|&x| x != 0), "кадр пуст — сверять нечего");

    // и тексты РАЗНЫЕ: одно медиа больше не значит одну картинку
    let mut srcs = gen_srcs(&p);
    let ink = |f: &kt_media::VideoFrame| f.rgba.chunks_exact(4).map(|px| px[3] as u64).sum::<u64>();
    let a = srcs
        .get_mut(&kt_engine::gen_source_addr(ClipId(1)))
        .unwrap()
        .read_video_frame(Tick(0), Some(Q))
        .unwrap();
    let b = srcs
        .get_mut(&kt_engine::gen_source_addr(ClipId(2)))
        .unwrap()
        .read_video_frame(Tick(0), Some(Q))
        .unwrap();
    assert_ne!(a.rgba, b.rgba, "два события на одном медиа отдали одну картинку");
    assert!(ink(&a) > ink(&b), "«AAAA» обязана занимать больше краски, чем «BB»");
}

/// Тест 659 (Р.59, стадия 2): кривая на параметре генератора меняет
/// БАЙТЫ и делает это детерминированно.
///
/// Кривые разрешает ИСТОЧНИК по своему `pts` — план про них не знает
/// вовсе. Поэтому и проверяется здесь пара «кадр как функция тика» и
/// «второй прогон равен первому до бита» (инвариант 11).
#[test]
fn t659_a_curve_on_a_generator_parameter_moves_the_bytes() {
    let mut p = base();
    p.media.push(MediaRef { id: MediaId(1), path: gen_kind_path(&title("")) });
    let mut c = clip(1, 0, 0, 180_000, 1);
    let mut k = title("ANIMATED");
    // пресет «печатная машинка» — те же формулы, что у миграции
    text_anim_values(&mut k, TextAnim::Typewriter);
    c.r#gen = Some(k);
    c.gen_auto = text_anim_curves(TextAnim::Typewriter, 1.0, 2.0);
    p.clips = vec![c];
    p.validate().expect("кривая генератора законна");

    let shot = |t: Tick| compose(&p, t).0;
    let ink = |f: &[u8]| f.chunks_exact(4).map(|px| px[3] as u64).sum::<u64>();
    let t0 = shot(Tick(0));
    let t_mid = shot(Tick(45_000));
    let t_end = shot(Tick(90_000));
    assert!(ink(&t_mid) > ink(&t0), "к середине кривая обязана раскрыть часть текста");
    assert!(ink(&t_end) > ink(&t_mid), "к концу — весь");

    // детерминизм: тот же тик — те же байты, и это не зависит от того,
    // сколько кадров посчитали до него
    assert_eq!(t_mid, shot(Tick(45_000)), "кадр с кривой обязан быть функцией ТИКА");

    // без кривых картинка от времени не зависит вовсе (ноль цены)
    let mut flat = p.clone();
    flat.clips[0].gen_auto.clear();
    let a = compose(&flat, Tick(0)).0;
    let b = compose(&flat, Tick(45_000)).0;
    assert_eq!(a, b, "у неанимированного титра кадр от тика зависеть не смеет");
}

/// Тест 663 (Р.59, стадия 2): ЛЕНТА не отдаёт устаревший кадр после
/// перенастройки генератора ВНУТРИ неё.
///
/// Скрытый дефект, который здесь закрывается: правка значений идёт по
/// retune-полосе и модель не двигает, а ключ мемоизации суб-композита —
/// это `content_hash` ЛЕНТЫ, посчитанный от модели. Значит он не меняется,
/// и лента продолжает отдавать картинку со старым текстом до первой правки
/// плана. Ответ — инвалидация предков тем же кодом (`reel_chain_of_clip`).
#[test]
fn t663_a_reel_does_not_serve_a_stale_frame_after_a_retune() {
    use kt_engine::Player;

    let mut p = base();
    let dark = GeneratorKind::Solid { color_r: 16, color_g: 16, color_b: 16 };
    let bright = GeneratorKind::Solid { color_r: 220, color_g: 220, color_b: 220 };
    p.media.push(MediaRef { id: MediaId(1), path: gen_kind_path(&dark) });
    let mut s = Sequence::new(MediaId(100), "лента");
    let mut inner = clip(1, 0, 0, 90_000, 1);
    inner.r#gen = Some(dark);
    s.clips = vec![inner];
    p.sequences.push(s);
    p.clips = vec![clip(10, 0, 0, 90_000, 100)];
    p.validate().expect("генератор внутри ленты законен");

    let (player, _consumer) = Player::new(p, vec![]);
    player.set_preview_quality(Q);

    // греем: композит ленты уезжает в мемо по её редакции
    let mut buf = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let _ = player.compose_preview_into(Tick(0), &mut buf);
        if buf.first().is_some_and(|&x| x == 16) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "тёмный кадр ленты не приехал");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(player.update_gen(ClipId(1), bright, Vec::new()), "перенастройка обязана применяться");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let _ = player.compose_preview_into(Tick(0), &mut buf);
        if buf.first().is_some_and(|&x| x == 220) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "ЛЕНТА отдаёт стейл: суб-композит мемоизирован по редакции модели, \
             а её правка значений не двигает — предков обязаны инвалидировать явно"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Тест 670 (Р.59, стадия 2, родня t11/t479): экспорт открывает
/// генераторы САМ, и превью равно экспорту байт-в-байт (инвариант 7).
///
/// Адрес источника производен от события — вызывающему его взять неоткуда,
/// поэтому список источников считает `gen_sources`, и считает его ОДИН
/// код для плеера, экспорта и kt-cli.
#[test]
fn t670_export_opens_its_own_generators_and_equals_preview() {
    let p = two_titles();
    let target = 7usize;
    let run = || {
        let mut captured: Option<Vec<u8>> = None;
        let mut all = Vec::new();
        let mut n = 0usize;
        // НИ ОДНОГО источника снаружи: генераторы экспорт поднимает сам
        let sources: Sources = vec![];
        render_raw_video(&p, sources, Q, |rgba| {
            if n == target {
                captured = Some(rgba.to_vec());
            }
            all.extend_from_slice(rgba);
            n += 1;
            Ok(())
        })
        .unwrap();
        (captured.expect("кадр захвачен"), all)
    };
    let (frame_a, all_a) = run();
    let (_, all_b) = run();
    assert_eq!(all_a, all_b, "экспорт с генераторами на событиях обязан быть детерминирован");
    assert!(frame_a.iter().any(|&x| x != 0), "экспорт отдал чёрное — сверять нечего");

    let (preview, complete) = compose(&p, Tick(target as i64 * 3000));
    assert!(complete, "композит превью неполон");
    assert_eq!(preview, frame_a, "превью и экспорт обязаны совпадать байт-в-байт");
}
