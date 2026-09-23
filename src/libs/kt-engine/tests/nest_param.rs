//! Тесты 674-679, 686 и 688 (docs/porting-notes.md): переопределения
//! публичных параметров доезжают до КАРТИНКИ и до ЗВУКА (Р.59, стадия 3).
//!
//! Движок про публикацию не знает: ему достаются готовые клипы из
//! `Project::resolve_reel`. Меняется ровно одно — РЕДАКЦИЯ ленты стала
//! функцией пары (лента, набор переопределений экземпляра), и всё, что
//! на редакции стоит (ключи мемо, дерево суб-рендереров, ступень декода,
//! адрес источника генератора), обязано это выдержать.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const FPS: u32 = 30;
const FRAME: i64 = 90_000 / FPS as i64;
const PART: Tick = Tick(FRAME * 30);
const Q: Quality = Quality { width: 96, height: 54 };

const SEQ: MediaId = MediaId(100);
const TITLE: ClipId = ClipId(1);
const PLATE: ClipId = ClipId(2);
const INST_A: ClipId = ClipId(10);
const INST_B: ClipId = ClipId(11);
const HEADLINE: PubId = PubId(1);
const ZOOM: PubId = PubId(2);
const LOGO: PubId = PubId(3);
const LEVEL: PubId = PubId(4);

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: FPS,
        width: 96,
        height: 54,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }
}

fn title(text: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_text();
    if let GeneratorKind::Text { text: t, size, .. } = &mut k {
        *t = text.into();
        *size = 0.4;
    }
    k
}

fn clip(id: u64, kind: ClipKind, lane: u32, start: Tick, dur: Tick, media: u64) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start,
        duration: dur,
        source_in: Tick::ZERO,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: Vec::new(),
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
    }
}

fn solid(r: u8, g: u8, b: u8) -> GeneratorKind {
    let mut k = GeneratorKind::default_for_tag("solid").expect("вид «заливка» есть в реестре");
    k.set_value("color", &ParamValue::Color([r, g, b, 255]));
    k
}

fn base() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef { id: MediaId(1), path: gen_kind_path(&title("")) });
    p.media.push(MediaRef { id: MediaId(2), path: "synth:2".into() });
    // «логотип»: заливка своего цвета. Синтетика для подмены не годится —
    // её кадр есть функция ТИКА, и два разных синтетических медиа в один
    // момент дают один цвет: тест был бы зелёным ни на чём.
    p.media.push(MediaRef { id: MediaId(3), path: gen_kind_path(&solid(220, 20, 20)) });
    p
}

fn file_sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(2),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 }))
            as Box<dyn MediaSource>,
    )]
}

/// Все источники ДОКУМЕНТА: файлы и генераторы по тем адресам, которые
/// назовёт `gen_sources`. Второго перечисления быть не должно.
fn sources(p: &Project) -> std::collections::HashMap<MediaId, Box<dyn MediaSource>> {
    let mut out: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        file_sources().into_iter().collect();
    for (addr, st) in kt_engine::gen_sources(p) {
        out.insert(addr, kt_media::factory::open_gen(&st));
    }
    out
}

/// Композит момента `t`, прогретый РОВНО тем, что заказал `layers_at`.
/// Возвращает (байты, полнота): полнота false значит, что заказ и чтение
/// разошлись — это и есть вечный промах кэша.
fn compose(p: &Project, t: Tick) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs = sources(p);
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

fn render_audio(p: &Project, frames: usize) -> Vec<f32> {
    let mut f = DirectFetcher::new(None);
    for (id, s) in file_sources() {
        f.add_source(id, s);
    }
    for (addr, st) in kt_engine::gen_sources(p) {
        f.add_source(addr, kt_media::factory::open_gen(&st));
    }
    let r = AudioRenderer::new(p);
    let mut out = vec![0f32; frames * CH];
    r.render_frames(&mut f, 0, &mut out);
    out
}

/// Шаблон: титр над подложкой, две постановки встык.
fn template() -> Project {
    let mut p = base();
    let mut s = Sequence::new(SEQ, "Выпуск");
    let mut t = clip(TITLE.0, ClipKind::Video, 0, Tick::ZERO, PART, 1);
    t.r#gen = Some(title("ПЕРВЫЙ"));
    s.clips.push(t);
    s.clips.push(clip(PLATE.0, ClipKind::Video, 1, Tick::ZERO, PART, 2));
    s.video_lanes = 2;
    s.published = vec![
        PublishedParam {
            id: HEADLINE,
            name: "Заголовок".into(),
            target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
            ty: ParamType::Text { multiline: false },
            range: PubRange::default(),
        },
        PublishedParam {
            id: LOGO,
            name: "Подложка".into(),
            target: ParamAddr::new(PLATE, ParamOwner::ClipAttr, "media"),
            ty: ParamType::Media,
            range: PubRange::default(),
        },
    ];
    p.sequences.push(s);
    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("шаблон законен");
    p
}

fn set_override(p: &mut Project, id: ClipId, ov: Override) {
    let c = p.clips.iter_mut().find(|c| c.id == id).expect("экземпляр на месте");
    c.overrides.retain(|x| x.pub_id != ov.pub_id);
    c.overrides.push(ov);
    p.validate().expect("проект с переопределением законен");
}

// ---- 675. два экземпляра, разные кадры ----------------------------------------

/// Тест 675 (Р.59, стадия 3): два экземпляра одной ленты с разным
/// заголовком дают РАЗНЫЕ кадры.
///
/// Это и есть постановка владельца целиком: «десять выпусков одной
/// заставки». До Р.59 текст был частью адреса медиа, то есть общим на все
/// места; с Р.59 он на событии, а «событие внутри ленты» перестало
/// однозначно определять значения — их задаёт ЭКЗЕМПЛЯР.
#[test]
fn t675_two_instances_with_different_headlines_render_differently() {
    let mut p = template();
    set_override(&mut p, INST_B, Override::value(HEADLINE, ParamValue::Text("ВТОРОЙ".into())));

    let mid = Tick(FRAME * 10);
    let (a, ca) = compose(&p, mid);
    let (b, cb) = compose(&p, Tick(PART.0 + mid.0));
    assert!(ca && cb, "композит неполон — заказ разошёлся с чтением");
    assert_ne!(
        a, b,
        "два экземпляра сыграли ОДИН заголовок: значения экземпляра не доехали \
         до кадра. Именно это владелец и назвал «не будут же люди каждый раз \
         лезть в композицию»"
    );
    assert!(a.iter().any(|&x| x != 0), "кадр чёрный — тест проверен ни на чём");

    // ...и это не «любые два кадра разные»: без переопределения они равны
    let plain = template();
    let (a0, _) = compose(&plain, mid);
    let (b0, _) = compose(&plain, Tick(PART.0 + mid.0));
    assert_eq!(a0, b0, "без переопределений экземпляры обязаны быть неотличимы");
    assert_eq!(a, a0, "переопределение второго экземпляра изменило ПЕРВЫЙ");
}

// ---- 676. ноль цены -----------------------------------------------------------

/// Тест 676 (Р.59, стадия 3): без переопределений — ОДИН суб-рендерер и
/// прежние байты.
///
/// Требование владельца: «если проект простой без выкрутасов — то и лагать
/// он не должен вовсе». Ключ дерева стал парой (лента, редакция), и это
/// сторож того, что пара с пустым набором — та же самая запись.
#[test]
fn t676_without_overrides_there_is_one_subrenderer_and_the_old_bytes() {
    let plain = template();
    let v = VideoRenderer::new(&plain);
    assert_eq!(
        v.nest_count(),
        1,
        "две постановки одной ленты без переопределений завели ДВА суб-рендерера — \
         ровно тот выигрыш, ради которого дерево и строилось (t442/t443)"
    );

    // лента без единой публикации — эталон «как было до раунда»
    let mut bare = template();
    bare.sequence_mut(SEQ).unwrap().published.clear();
    assert_eq!(
        kt_engine::content_hash(&bare, SEQ),
        kt_engine::instance_content(&bare, SEQ, &[]),
        "пустой набор обязан давать РОВНО content_hash: иначе сдвинулись бы все \
         ключи мемо и sidecar-превью у проектов, которые публикацией не пользуются"
    );
    for i in 0..20 {
        let t = Tick(i * FRAME * 3);
        assert_eq!(
            compose(&bare, t).0,
            compose(&plain, t).0,
            "объявление ручки изменило кадр на {i}: публикация обязана быть бесплатной"
        );
    }

    // ...а как только набор появился — записей становится две
    let mut two = template();
    set_override(&mut two, INST_B, Override::value(HEADLINE, ParamValue::Text("ВТОРОЙ".into())));
    assert_eq!(
        VideoRenderer::new(&two).nest_count(),
        2,
        "экземпляр с переопределением обязан получить СВОЙ суб-рендерер: иначе \
         два выпуска делят один суб-композит и один заголовок"
    );
}

// ---- 674. кривая переопределения ---------------------------------------------

/// Тест 674 (Р.59, стадия 3): кривая переопределения живёт в материальном
/// времени ЭКЗЕМПЛЯРА (оно же время ленты) и приземляется на тот же кадр
/// материала цели — под тримом и реверсом экземпляра.
#[test]
fn t674_an_override_curve_lands_on_the_same_material_frame() {
    let mut p = base();
    let mut s = Sequence::new(SEQ, "Лента");
    // цель СДВИНУТА и по таймлайну ленты, и по материалу: на нулях
    // перевод домена вырождался бы в тождество и ничего не проверял
    let mut plate = clip(PLATE.0, ClipKind::Video, 0, Tick(FRAME * 5), PART, 2);
    plate.source_in = Tick(FRAME * 40);
    plate.fx =
        vec![FxInstance::Pan { mode: Default::default(), pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }];
    s.clips.push(plate);
    s.published = vec![PublishedParam {
        id: ZOOM,
        name: "Наезд".into(),
        target: ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "zoom"),
        ty: ParamType::Num,
        range: PubRange::default(),
    }];
    p.sequences.push(s);

    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick(FRAME * 5);
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick(FRAME * 5);
    b.reversed = true;
    p.clips.push(b);

    // ключи — в тиках ЛЕНТЫ
    let key = |t: i64, v: f64| AutoKey::at(Tick(t), v);
    let ov = Override {
        pub_id: ZOOM,
        value: ParamValue::Num(1.0),
        curves: vec![ParamCurve {
            param: "zoom".into(),

            keys: AutoKey::linear(vec![key(FRAME * 5, 1.0), key(FRAME * 25, 3.0)]),
            source: CurveSource::Keys,
        }],
    };
    for id in [INST_A, INST_B] {
        p.clips.iter_mut().find(|c| c.id == id).unwrap().overrides = vec![ov.clone()];
    }
    p.validate().expect("проект с кривой переопределения законен");

    // перевод домена: тик ленты τ → материал цели source_in + (τ − start)
    let want: Vec<i64> = vec![FRAME * 40, FRAME * 60];
    for id in [INST_A, INST_B] {
        let inst = p.clips.iter().find(|c| c.id == id).unwrap();
        let r = p.resolve_reel(Some(SEQ), &inst.overrides);
        let target = r.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
        let got: Vec<i64> = target.fx_auto[0]
            .iter()
            .find(|c| c.param == "zoom")
            .expect("кривая не доехала до цели")
            .keys
            .iter()
            .map(|k| k.t.0)
            .collect();
        assert_eq!(
            got, want,
            "ключи переопределения у экземпляра {id:?} приземлились не на тот кадр \
             материала: кривая живёт в ВРЕМЕНИ ЛЕНТЫ, и трим с реверсом экземпляра \
             её не двигают"
        );
    }

    // и это видно в кадре: тот же тик ленты у прямого и у зеркального
    // экземпляра даёт ОДИН кадр
    let reel_tick = FRAME * 20;
    let at_a = Tick(reel_tick - FRAME * 5);
    // зеркальный играет ленту навстречу: последний кадр показывает source_in
    let at_b = Tick(PART.0 + (PART.0 - FRAME - (reel_tick - FRAME * 5)));
    let (fa, ca) = compose(&p, at_a);
    let (fb, cb) = compose(&p, at_b);
    assert!(ca && cb, "композит неполон");
    assert_eq!(
        fa, fb,
        "один тик ленты у прямого и у зеркального экземпляра дал РАЗНЫЕ кадры — \
         значит кривая переопределения приклеена не к ленте, а к экрану"
    );
    // и кривая действительно работает: начало и конец различаются
    let (f0, _) = compose(&p, Tick(0));
    assert_ne!(f0, fa, "кривая переопределения не двигает байты — тест ни о чём");
}

// ---- 677. подмена логотипа ----------------------------------------------------

/// Тест 677 (Р.59, стадия 3): подмена МЕДИА у экземпляра — «логотип».
/// Заказ уходит вглубь по НОВОМУ адресу, чтение идёт по нему же.
#[test]
fn t677_a_media_override_swaps_the_logo_and_order_equals_read() {
    let mut p = template();
    set_override(&mut p, INST_B, Override::value(LOGO, ParamValue::Media(MediaId(3))));

    // СНАЧАЛА — сама подмена в содержимом. Без этой строки тест был
    // зелёным при выключенном `c.media = *m`: подогнанные под новое медиа
    // значения генератора (`fit_gen_to_media`) сами уводили событие на
    // адрес генератора, заказ менялся, кадр менялся — и «переопределение
    // работает» оказывалось совпадением. Красный тест обязан быть красным
    // по ТОЙ ЖЕ причине, по которой зелёный зелёный.
    {
        let b = p.clips.iter().find(|c| c.id == INST_B).unwrap();
        let r = p.resolve_reel(Some(SEQ), &b.overrides);
        let plate = r.clips.iter().find(|c| same_clip(c.id, PLATE)).expect("подложка на месте");
        assert_eq!(
            plate.media,
            MediaId(3),
            "переопределение не переписало `media` цели — логотип остался прежним"
        );
        let a = p.clips.iter().find(|c| c.id == INST_A).unwrap();
        let r0 = p.resolve_reel(Some(SEQ), &a.overrides);
        let plate0 = r0.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
        assert_eq!(plate0.media, MediaId(2), "подмена у второго экземпляра задела первый");
    }

    let v = VideoRenderer::new(&p);
    let mid = Tick(FRAME * 10);
    let ord_a: Vec<MediaId> = v.layers_at(mid, Q).into_iter().map(|(m, _, _)| m).collect();
    let ord_b: Vec<MediaId> =
        v.layers_at(Tick(PART.0 + mid.0), Q).into_iter().map(|(m, _, _)| m).collect();
    assert!(ord_a.contains(&MediaId(2)), "первый экземпляр обязан заказывать ПОДЛОЖКУ ленты");
    assert!(
        !ord_b.contains(&MediaId(2)),
        "подмена медиа не доехала до ЗАКАЗА: префетч грел бы не то, что читает композит"
    );
    assert_ne!(ord_a, ord_b, "заказы двух экземпляров совпали — подмены не было");

    let (a, ca) = compose(&p, mid);
    let (b, cb) = compose(&p, Tick(PART.0 + mid.0));
    assert!(ca && cb, "композит неполон — заказ разошёлся с чтением");
    assert_ne!(a, b, "подмена логотипа не изменила кадр");
}

// ---- 678. звук ----------------------------------------------------------------

/// Тест 678 (Р.59, стадия 3): переопределение слышно.
#[test]
fn t678_an_override_reaches_the_sound() {
    let mut p = base();
    let mut s = Sequence::new(SEQ, "Реплика");
    s.clips.push(clip(PLATE.0, ClipKind::Audio, 0, Tick::ZERO, PART, 2));
    s.published = vec![PublishedParam {
        id: LEVEL,
        name: "Громкость реплики".into(),
        target: ParamAddr::new(PLATE, ParamOwner::ClipAttr, "gain_db"),
        ty: ParamType::Num,
        range: PubRange::default(),
    }];
    p.sequences.push(s);
    let mut a = clip(INST_A.0, ClipKind::Audio, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    p.validate().expect("аудио-шаблон законен");

    let frames = PART.to_audio_frames() as usize;
    let before = render_audio(&p, frames);
    assert!(before.iter().any(|x| x.abs() > 1e-6), "фикстура молчит");
    assert_eq!(AudioRenderer::new(&p).nest_count(), 1, "один экземпляр — один суб-микс");

    let mut q = p.clone();
    set_override(&mut q, INST_A, Override::value(LEVEL, ParamValue::Num(-12.0)));
    let after = render_audio(&q, frames);
    assert_ne!(before, after, "переопределение громкости внутри ленты не слышно");
    let peak = |v: &[f32]| v.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(
        peak(&after) < peak(&before) * 0.5,
        "−12 дБ обязаны быть слышны как падение уровня: было {}, стало {}",
        peak(&before),
        peak(&after)
    );
}

// ---- 679. опубликованный зум и ступень ленты ---------------------------------

/// Тест 679 (Р.59, стадия 3): ступень ленты считается ПО ДЕРЕВУ
/// ЭКЗЕМПЛЯРОВ и остаётся ОДНОЙ на ленту (t450).
///
/// До раунда `nest_scales` перебирал ленты плоско, со ШАБЛОННЫМИ
/// значениями: экземпляр, у которого опубликованный наезд выкручен
/// наружу, получил бы мягкую картинку — вход цепочки взяли бы в Quality,
/// хотя она зумит в восемь раз. Ступень при этом обязана остаться общей:
/// разные ступени дали бы внутреннему листу два ключа кэша, две полоски
/// прогретости и два заказа (инвариант 4).
#[test]
fn t679_a_published_zoom_raises_the_reel_stage_and_keeps_it_single() {
    // внутренняя лента — та, чью ступень мы меряем
    let mut p = base();
    let mut inner = Sequence::new(MediaId(101), "Кадр");
    inner.clips.push(clip(PLATE.0, ClipKind::Video, 0, Tick::ZERO, PART, 2));
    p.sequences.push(inner);

    // внешняя: экземпляр внутренней с цепочкой НАЕЗДА, зум объявлен наружу
    let mut outer = Sequence::new(SEQ, "Выпуск");
    let mut mid = clip(20, ClipKind::Video, 0, Tick::ZERO, PART, 101);
    mid.source_in = Tick::ZERO;
    mid.fx = vec![FxInstance::Pan { mode: Default::default(), pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }];
    outer.clips.push(mid);
    outer.published = vec![PublishedParam {
        id: ZOOM,
        name: "Наезд".into(),
        target: ParamAddr::new(ClipId(20), ParamOwner::FxSlot { slot: 0 }, "zoom"),
        ty: ParamType::Num,
        range: PubRange::default(),
    }];
    p.sequences.push(outer);

    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("проект с опубликованным зумом законен");

    // качество, в котором заказывается САМЫЙ ГЛУБОКИЙ лист
    let leaf_q = |p: &Project, t: Tick| -> Quality {
        let ord = VideoRenderer::new(p).layers_at(t, Q);
        assert_eq!(ord.len(), 1, "внутри ленты ровно один лист");
        ord[0].2
    };
    let mid_t = Tick(FRAME * 10);
    let flat = leaf_q(&p, mid_t);

    let mut zoomed = p.clone();
    set_override(&mut zoomed, INST_B, Override::value(ZOOM, ParamValue::Num(8.0)));
    let qa = leaf_q(&zoomed, mid_t);
    let qb = leaf_q(&zoomed, Tick(PART.0 + mid_t.0));
    assert!(
        qb.width > flat.width,
        "опубликованный наезд не поднял ступень ленты: {flat:?} → {qb:?}. \
         `nest_scales` обязан обходить ДЕРЕВО экземпляров с их \
         переопределениями, а не перебирать ленты плоско со шаблонными \
         значениями"
    );
    assert_eq!(
        qa, qb,
        "ступень разошлась между экземплярами: она свойство ЛЕНТЫ (t450), иначе у \
         внутреннего листа два ключа кэша, две полоски прогретости и два заказа"
    );
    // заказ == чтение на обоих
    assert!(
        compose(&zoomed, mid_t).1 && compose(&zoomed, Tick(PART.0 + mid_t.0)).1,
        "композит неполон — заказ разошёлся с чтением"
    );
}

// ---- 686. шаблон в шаблоне ----------------------------------------------------

/// Тест 686 (Р.59, стадия 3): `Nested` даёт шаблон в шаблоне бесплатно.
///
/// Внешняя лента объявляет наружу ручку ВНУТРЕННЕГО экземпляра; её
/// переопределение патчит его набор, а дальше он разбирается сам — тем же
/// кодом этажом ниже.
#[test]
fn t686_a_template_inside_a_template_resolves_recursively() {
    let mut p = base();
    // внутренняя лента с титром
    let mut inner = Sequence::new(MediaId(101), "Титр");
    let mut t = clip(TITLE.0, ClipKind::Video, 0, Tick::ZERO, PART, 1);
    t.r#gen = Some(title("ПЕРВЫЙ"));
    inner.clips.push(t);
    inner.published = vec![PublishedParam {
        id: HEADLINE,
        name: "Заголовок".into(),
        target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
        ty: ParamType::Text { multiline: false },
        range: PubRange::default(),
    }];
    p.sequences.push(inner);

    // внешняя: экземпляр внутренней плюс подложка, наружу торчит ЧУЖАЯ ручка
    let mut outer = Sequence::new(SEQ, "Выпуск");
    let mut mid = clip(20, ClipKind::Video, 0, Tick::ZERO, PART, 101);
    mid.source_in = Tick::ZERO;
    outer.clips.push(mid);
    outer.clips.push(clip(PLATE.0, ClipKind::Video, 1, Tick::ZERO, PART, 2));
    outer.video_lanes = 2;
    outer.published = vec![PublishedParam {
        id: PubId(50),
        name: "Заголовок выпуска".into(),
        target: ParamAddr::new(ClipId(20), ParamOwner::Nested { pub_id: HEADLINE }, "text"),
        ty: ParamType::Text { multiline: false },
        range: PubRange::default(),
    }];
    p.sequences.push(outer);

    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("шаблон в шаблоне законен");

    // значение шаблона внешней ручки читается ЧЕРЕЗ два этажа
    assert_eq!(
        p.published_value(SEQ, PubId(50)),
        Some(ParamValue::Text("ПЕРВЫЙ".into())),
        "значение шаблона через `Nested` не прочиталось"
    );

    let mut q = p.clone();
    set_override(&mut q, INST_B, Override::value(PubId(50), ParamValue::Text("ВТОРОЙ".into())));
    // внешняя ручка доехала до НАБОРА внутреннего экземпляра
    let r = q.resolve_reel(Some(SEQ), &q.clips.iter().find(|c| c.id == INST_B).unwrap().overrides);
    let inner_inst = r.clips.iter().find(|c| same_clip(c.id, ClipId(20))).unwrap();
    assert_eq!(
        q.instance_value(inner_inst, HEADLINE),
        Some(ParamValue::Text("ВТОРОЙ".into())),
        "переопределение внешней ручки не доехало до набора внутреннего экземпляра"
    );

    let mid_t = Tick(FRAME * 10);
    let (fa, ca) = compose(&q, mid_t);
    let (fb, cb) = compose(&q, Tick(PART.0 + mid_t.0));
    assert!(ca && cb, "композит неполон");
    assert_ne!(fa, fb, "шаблон в шаблоне не дал разных кадров");
}

// ---- 688. закон Э с впечёнными переопределениями ------------------------------

/// Тест 688 (Р.59, стадия 3): ЗАКОН Э жив.
///
/// Вложение с пустой цепочкой равно своему развёрнутому содержимому — и
/// это обязано оставаться правдой ПОД ПЕРЕОПРЕДЕЛЕНИЯМИ: раскрытие
/// (`effective_clips`, оракул закона и то, что печатает `kt-cli probe`)
/// показывает значения ЭКЗЕМПЛЯРА, а не шаблона.
#[test]
fn t688_law_e_holds_with_overrides_baked_in() {
    let mut p = template();
    // одна постановка: закон Э про тождество вложения содержимому
    p.clips.retain(|c| c.id != INST_B);
    set_override(&mut p, INST_A, Override::value(HEADLINE, ParamValue::Text("ВТОРОЙ".into())));

    // ОРАКУЛ: раскрытие обязано нести значения экземпляра
    let flat = p.effective_clips();
    let baked = flat
        .iter()
        .find_map(|c| c.r#gen.as_ref().and_then(|g| g.get_value("text")))
        .expect("в раскрытии нет титра");
    assert_eq!(
        baked,
        ParamValue::Text("ВТОРОЙ".into()),
        "раскрытие показало ШАБЛОН: оракул закона Э сверял бы поддерево не с тем, \
         а `kt-cli probe` отвечал бы на жалобу «тут другой заголовок» чужим текстом"
    );

    // ...и развёрнутый проект рисует то же, что поддерево
    let mut n = ops::NestCounters::of(&p);
    let (cmds, rep) = ops::expand_instance(&p, INST_A, n.ids());
    assert_eq!(rep.overrides_baked, 1, "разворачивание не впекло переопределение");
    let mut m = TimelineModel::new(p.clone());
    m.apply_batch(cmds).expect("разворачивание законно");
    let spread = m.project().clone();

    for i in 0..20 {
        let t = Tick(i * FRAME);
        let (a, ca) = compose(&p, t);
        let (b, cb) = compose(&spread, t);
        assert_eq!(ca, cb, "полнота композита разошлась на кадре {i}");
        assert_eq!(
            a, b,
            "ЗАКОН Э под переопределениями: кадр {i} у вложения и у развёрнутого \
             содержимого обязан совпадать БАЙТ-В-БАЙТ"
        );
    }
}

/// Тест 675b (Р.59, стадия 3): у экземпляра с переопределением генератора
/// СВОЙ источник — и он попадает в список `gen_sources`.
///
/// От этого зависят два разных механизма, и оба молчаливы при поломке:
/// «заказ == чтение» (адрес, который заказал план, обязан существовать) и
/// закон П — перенастройка значений едет полосой `retune` ПО АДРЕСАМ, и
/// ретюн одного адреса оставил бы второй выпуск со старым кеглем до
/// первой пересборки плана (тот же стейл, против которого стоит t663).
#[test]
fn t675b_an_overridden_instance_gets_its_own_generator_source() {
    let plain = template();
    let addrs = |p: &Project| -> Vec<MediaId> {
        let mut v: Vec<MediaId> = kt_engine::gen_sources(p).into_iter().map(|(a, _)| a).collect();
        v.sort();
        v.dedup();
        v
    };
    let before = addrs(&plain);

    let mut p = template();
    set_override(&mut p, INST_B, Override::value(HEADLINE, ParamValue::Text("ВТОРОЙ".into())));
    let after = addrs(&p);
    assert_eq!(
        after.len(),
        before.len() + 1,
        "у экземпляра с переопределением обязан быть СВОЙ адрес источника: \
         иначе два выпуска читают один кадр, и «второй» молча показывает первый"
    );
    // ...и состояния по этим адресам ДЕЙСТВИТЕЛЬНО разные
    let texts: std::collections::BTreeSet<String> = kt_engine::gen_sources(&p)
        .into_iter()
        .filter_map(|(_, st)| match st.kind.get_value("text") {
            Some(ParamValue::Text(s)) => Some(s),
            _ => None,
        })
        .collect();
    assert!(
        texts.contains("ПЕРВЫЙ") && texts.contains("ВТОРОЙ"),
        "список источников не несёт обоих заголовков: {texts:?}"
    );

    // без переопределений список адресов НЕ РАСТЁТ ни на один: соль равна
    // нулю, и проект без публикаций платит ноль
    let mut bare = template();
    bare.sequence_mut(SEQ).unwrap().published.clear();
    assert_eq!(addrs(&bare), before, "публикация сама по себе завела лишние источники");
}
