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
    let mut va = VideoRenderer::new(&a);
    let mut vb = VideoRenderer::new(&b);
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
