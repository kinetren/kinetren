//! Тесты 233, 234, 236 (docs/porting-notes.md): резка цели морфер-слоем.
//!
//! Морфер с Р.35 действует ПО ВРЕМЕНИ, поэтому цель, накрытую частично,
//! приходится резать. Здесь проверяется всё, что при этом обязано
//! остаться верным: материал кусков, детерминизм их id, вырожденные
//! случаи и запретные зоны (находка 1 Р.35 — резка ломает не цель, а
//! соседей по дорожке).

use kt_model::*;
use kt_time::{Tick, TickRange};

const LEN: Tick = Tick(90_000);

fn clip(id: u64, kind: ClipKind, lane: u32, start: Tick, dur: Tick) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start,
        duration: dur,
        source_in: Tick(3_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn base() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "synth:ramp".into() });
    p.clips.push(clip(10, ClipKind::Video, 0, Tick(0), LEN));
    p.morph_lanes = 1;
    p
}

/// Морфер-слой на середину клипа: подмена источника + видео-эффект.
fn layer(start: Tick, dur: Tick) -> MorpherEvent {
    MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start,
        duration: dur,
        targets: MorphTargets::Clips { ids: vec![ClipId(10)] },
        patch: MorphPatch {
            media: Some(MediaId(2)),
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }
}

/// Тест 233: клип, накрытый ЧАСТИЧНО, распадается на три куска; материал
/// каждого куска совпадает с материалом оригинала в тех же точках, сумма
/// длительностей равна оригиналу, патч достался ровно среднему.
#[test]
fn t233_partial_cover_slices_into_three() {
    let mut p = base();
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3, "три куска");
    assert_eq!(
        eff.iter().map(|c| c.duration.0).sum::<i64>(),
        LEN.0,
        "сумма длительностей равна оригиналу"
    );
    assert_eq!(eff[0].start, Tick(0));
    assert_eq!(eff[1].start, Tick(30_000));
    assert_eq!(eff[2].start, Tick(60_000));
    // материал: точка входа едет ровно на смещение куска
    assert_eq!(eff[0].source_in, Tick(3_000));
    assert_eq!(eff[1].source_in, Tick(33_000));
    assert_eq!(eff[2].source_in, Tick(63_000));
    // патч — только у среднего
    assert_eq!(eff[0].media, MediaId(1));
    assert_eq!(eff[1].media, MediaId(2), "накрытый кусок — на новом источнике");
    assert_eq!(eff[2].media, MediaId(1));
    assert!(eff[0].fx.is_empty() && eff[2].fx.is_empty());
    assert_eq!(eff[1].fx.len(), 1, "эффект слоя дописан накрытому куску");

    // материал точка-в-точку совпадает с оригиналом на всём теле
    for t in [Tick(0), Tick(15_000), Tick(30_000), Tick(45_000), Tick(87_000)] {
        let c = eff.iter().find(|c| c.start <= t && t < c.start + c.duration).unwrap();
        let want = clip_material_at(Tick(3_000), Tick(0), LEN, false, 3_000, t);
        let got = clip_material_at(c.source_in, c.start, c.duration, false, 3_000, t);
        assert_eq!(got, want, "материал в {t:?}");
    }
}

/// Тест 233b: то же на REVERSED клипе — у него материал зеркальный, и
/// точка входа куска едет с его КОНЦОМ.
#[test]
fn t233_reversed_slices_keep_material() {
    let mut p = base();
    p.clips[0].reversed = true;
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3);
    // точки берутся ПО СЕТКЕ КАДРОВ — на ней рендерится видео, и на ней
    // куски обязаны совпасть с оригиналом (границы садит туда `windows_on`)
    for t in [Tick(0), Tick(21_000), Tick(30_000), Tick(57_000), Tick(87_000)] {
        let c = eff.iter().find(|c| c.start <= t && t < c.start + c.duration).unwrap();
        let want = clip_material_at(Tick(3_000), Tick(0), LEN, true, 3_000, t);
        let got = clip_material_at(c.source_in, c.start, c.duration, true, 3_000, t);
        assert_eq!(got, want, "материал реверса в {t:?}");
    }
    // то же правило, что у Split: левая половина играет ХВОСТ источника
    let mut m = TimelineModel::new({
        let mut q = base();
        q.clips[0].reversed = true;
        q
    });
    m.apply(Command::Split {
        id: ClipId(10),
        at: Tick(30_000),
        new_id: ClipId(11),
        new_group: None,
    })
    .unwrap();
    let left = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(left.source_in, eff[0].source_in, "Split и резка считают source_in одинаково");
}

/// Тест 234: id кусков — чистая функция от (оригинал, номер); кусок №0
/// держит id оригинала, `validate` отвергает настоящий клип в
/// зарезервированном диапазоне, оригинал достаётся из самого id.
#[test]
fn t234_slice_ids_are_deterministic_and_reserved() {
    let mut p = base();
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    let ids: Vec<ClipId> = p.effective_clips().iter().map(|c| c.id).collect();
    assert_eq!(ids[0], ClipId(10), "кусок №0 держит id оригинала");
    assert_eq!(ids[1], slice_id(ClipId(10), 1));
    assert_eq!(ids[2], slice_id(ClipId(10), 2));
    assert_eq!(slice_origin(ids[1]), Some(ClipId(10)), "оригинал достаётся из id");
    assert_eq!(slice_origin(ids[0]), None, "настоящий клип куском не притворяется");
    assert!(same_clip(ids[2], ClipId(10)) && !same_clip(ids[2], ClipId(11)));

    // правка соседнего клипа на другой дорожке id кусков не двигает
    let mut q = p.clone();
    q.clips.push(clip(20, ClipKind::Video, 1, Tick(0), LEN));
    let got: Vec<ClipId> =
        q.effective_clips().iter().map(|c| c.id).filter(|&i| same_clip(i, ClipId(10))).collect();
    assert_eq!(got, ids, "сосед на другой дорожке id кусков не двигает");
    // перенос блока морфера мышью — тоже
    let mut r = p.clone();
    r.morphers[0].start = Tick(40_000);
    assert_eq!(r.effective_clips().iter().map(|c| c.id).collect::<Vec<_>>(), ids, "перенос блока");

    // настоящий клип в зарезервированном диапазоне — ошибка модели
    let mut bad = base();
    bad.clips[0].id = ClipId(MAX_REAL_CLIP_ID);
    assert!(matches!(bad.validate(), Err(ProjectError::ReservedClipId(..))));
}

/// Тест 236: вырожденные случаи. Клип, накрытый целиком, НЕ режется и
/// сохраняет id (а с ним аудио-FX-состояние, инвариант №3); окно короче
/// минимума не действует; огрызок липнет к краю тела; патч, который
/// этому виду клипа ничего не делает, не режет его вовсе.
#[test]
fn t236_degenerate_windows() {
    // накрыт целиком — один клип со своим id
    let mut p = base();
    p.morphers.push(layer(Tick(-10_000), Tick(200_000)));
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "накрытый целиком не режется");
    assert_eq!(eff[0].id, ClipId(10), "и сохраняет id");
    assert_eq!(eff[0].media, MediaId(2));

    // окно короче минимума не действует вовсе
    let mut p = base();
    p.morphers.push(layer(Tick(30_000), Tick(50)));
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1);
    assert_eq!(eff[0].media, MediaId(1), "микроскопическое окно ничего не делает");

    // огрызок в 50 тиков у начала тела липнет к краю: два куска, не три
    let mut p = base();
    p.morphers.push(layer(Tick(50), Tick(30_000)));
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 2, "огрызок схлопнулся к краю тела");
    assert_eq!(eff[0].start, Tick(0));
    assert_eq!(eff[0].media, MediaId(2), "первый кусок накрыт целиком");
    assert_eq!(eff[0].duration, Tick(30_000), "и конец сел на сетку кадров");

    // Патч с одними видео-эффектами не трогает аудио под собой — и с Р.55
    // (С2) НЕ РЕЖЕТ И ВИДЕО: выключить вклад цепочки можно кривой силы,
    // а значит резать ради неё незачем. Раньше тут было три куска.
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Audio, 0, Tick(0), LEN));
    p.audio_lanes = 1;
    let mut m = layer(Tick(30_000), Tick(30_000));
    m.patch.media = None;
    m.targets = MorphTargets::Clips { ids: vec![ClipId(10), ClipId(11)] };
    p.morphers.push(m);
    let eff = p.effective_clips();
    let audio: Vec<&ClipEvent> = eff.iter().filter(|c| c.kind == ClipKind::Audio).collect();
    assert_eq!(audio.len(), 1, "аудио не разрезано");
    assert_eq!(audio[0].id, ClipId(11));
    let video: Vec<&ClipEvent> = eff.iter().filter(|c| c.kind == ClipKind::Video).collect();
    assert_eq!(video.len(), 1, "видео тоже НЕ разрезано: цепочку гасят, а не вырезают");
    assert_eq!(video[0].id, ClipId(10), "и кусок держит id оригинала (инвариант 3)");
    assert!(
        video[0].fx.iter().any(|f| matches!(f, FxInstance::Mirror { .. })),
        "цепочка слоя обязана быть на теле"
    );
    // ВОРОТА: сила нулевая вне окна блока и полная внутри
    let (i, _) = video[0]
        .fx
        .iter()
        .enumerate()
        .find(|(_, f)| matches!(f, FxInstance::LayerMix { .. }))
        .expect("слот силы обязан появиться: без него слой действовал бы на всё тело");
    let cur = &video[0].fx_auto[i][0];
    let at = |t: i64| {
        let ks = &cur.keys;
        let j = ks.partition_point(|k| k.t.0 <= t).saturating_sub(1);
        ks[j].v
    };
    assert!(at(10_000) < 0.01, "до окна слой обязан молчать, а не действовать");
    assert!(at(45_000) > 0.99, "внутри окна слой обязан действовать в полную силу");
    assert!(at(80_000) < 0.01, "после окна слой обязан молчать");
}

/// Тест 236b: запретные зоны. Граница слоя не встаёт внутрь окна
/// перехода и внутрь наезда соседа — она ЛИПНЕТ к их краю (решение 8
/// Р.35): иначе разрезанный кусок с бо́льшим `start` вылезал бы над
/// соседом (правило Vegas) и менял бы веса кроссфейда.
#[test]
fn t236_forbidden_zones_snap_the_boundary() {
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, LEN, LEN));
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let zones = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert!(zones.iter().any(|z| z.start == Tick(81_000) && z.end == LEN), "окно перехода — зона");

    // граница внутри окна перехода выталкивается к ближнему краю
    p.morphers.push(layer(Tick(30_000), Tick(53_000))); // конец 83_000 — внутри
    let eff = p.effective_clips();
    let first: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(10))).collect();
    assert_eq!(first.len(), 3, "два реза: начало слоя и вытолкнутый конец");
    assert_eq!(first[1].start, Tick(30_000));
    assert_eq!(first[2].start, Tick(81_000), "выталкивание к ближнему краю окна перехода");
    assert_eq!(first[1].media, MediaId(2), "слой достался среднему куску");
    assert_eq!(first[2].media, MediaId(1), "окно перехода слой не накрыл");

    // наезд соседа по дорожке — тоже запретная зона
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, Tick(70_000), LEN));
    let zones = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert!(zones.iter().any(|z| z.start == Tick(70_000) && z.end == LEN), "наезд — зона");
    p.morphers.push(layer(Tick(30_000), Tick(45_000))); // конец 75_000 — внутри наезда
    let eff = p.effective_clips();
    let first: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(10))).collect();
    assert_eq!(first.len(), 3);
    assert_eq!(first[2].start, Tick(70_000), "граница липнет к краю наезда, а не к сетке");
}

/// Тест 236c: `windows_on` — окно, в котором морфер реально действует.
/// Файлы до v4 (`timed: false`) патчат тело целиком независимо от
/// положения блока — на этом держится GOLDEN мигрированных проектов.
#[test]
fn t236_untimed_morpher_ignores_block_position() {
    let mut p = base();
    let mut m = layer(Tick(500_000), Tick(1_000)); // блок ВНЕ клипа
    m.timed = false;
    p.morphers.push(m);
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "старый морфер не режет");
    assert_eq!(eff[0].media, MediaId(2), "и действует независимо от времени блока");
    assert_eq!(
        p.morphers[0].windows_on(&p.clips[0], &p, &p, &p.rows()),
        vec![TickRange::new(Tick(0), LEN)]
    );

    // выключенный морфер не действует и не режет
    p.morphers[0].timed = true;
    p.morphers[0].start = Tick(30_000);
    p.morphers[0].duration = Tick(30_000);
    p.morphers[0].enabled = false;
    assert_eq!(p.effective_clips(), p.clips, "выключенный морфер прозрачен");
}

/// Тест 235 (Р.35 С1, кусок 4): режим СЛОЯ. Морфер без выделения
/// действует на всё, что НИЖЕ его строки; порядок наложения — снизу
/// вверх (нижний слой первым, как adjustment layer в AE); перестановка
/// строк меняет порядок эффектов и откатывается undo.
#[test]
fn t235_below_layer_and_bottom_up_order() {
    let mut p = base();
    p.video_lanes = 2;
    p.audio_lanes = 1;
    p.clips.push(clip(20, ClipKind::Video, 1, Tick(0), LEN));
    p.clips.push(clip(30, ClipKind::Audio, 0, Tick(0), LEN));
    let mut m = layer(Tick(0), LEN);
    m.targets = MorphTargets::Below;
    m.patch = MorphPatch {
        video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        ..Default::default()
    };
    p.morphers.push(m);
    // морф-строка по умолчанию САМАЯ НИЖНЯЯ — «ниже» её пусто
    assert_eq!(p.effective_clips(), p.clips, "снизу списка слой ничего не накрывает");

    // поднимаем морф-строку наверх — накрывает всё видео под собой
    let mut order = p.rows();
    let morph = order.remove(order.iter().position(|&k| k.0 == LaneKind::Morph).unwrap());
    order.insert(0, morph);
    p.set_lane_order(&order);
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3, "слой накрывает целиком — резки нет");
    for c in eff.iter().filter(|c| c.kind == ClipKind::Video) {
        assert_eq!(c.fx.len(), 1, "видео под слоем получило эффект");
    }
    assert!(
        eff.iter().find(|c| c.id == ClipId(30)).unwrap().fx.is_empty(),
        "видео-слой не трогает аудио"
    );

    // слой видит ровно то, что НИЖЕ: между двумя видео-дорожками
    let mut order = p.rows();
    let morph = order.remove(order.iter().position(|&k| k.0 == LaneKind::Morph).unwrap());
    order.insert(1, morph);
    p.set_lane_order(&order);
    let eff = p.effective_clips();
    let above = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    let below = eff.iter().find(|c| c.id == ClipId(20)).unwrap();
    assert!(above.fx.is_empty(), "то, что ВЫШЕ слоя, не трогается");
    assert_eq!(below.fx.len(), 1, "то, что ниже, — трогается");
}

/// Тест 235b: порядок наложения снизу вверх и его откат undo.
#[test]
fn t235_layer_order_is_bottom_up_and_undoable() {
    let mut p = base();
    p.morph_lanes = 2;
    for (id, lane, fx) in [
        (1u64, 0u32, FxInstance::Mirror { horizontal: true, vertical: false }),
        (2, 1, FxInstance::Mirror { horizontal: false, vertical: true }),
    ] {
        let mut m = layer(Tick(0), LEN);
        m.id = MorphId(id);
        m.lane = lane;
        m.targets = MorphTargets::Below;
        m.patch = MorphPatch { video_fx: vec![fx], ..Default::default() };
        p.morphers.push(m);
    }
    // обе морф-строки — над видео
    let mut order = p.rows();
    order.retain(|k| k.0 != LaneKind::Morph);
    order.insert(0, (LaneKind::Morph, 0));
    order.insert(1, (LaneKind::Morph, 1));
    p.set_lane_order(&order);
    let mut m = TimelineModel::new(p);
    let chain = |m: &TimelineModel| m.project().effective_clips()[0].fx.clone();
    let first = chain(&m);
    assert_eq!(first.len(), 2, "оба слоя действуют");
    assert!(
        matches!(first[0], FxInstance::Mirror { vertical: true, .. }),
        "НИЖНИЙ слой (строка 1) применён первым"
    );

    // меняем строки местами — порядок эффектов обязан перевернуться
    let mut order = m.project().rows();
    order.swap(0, 1);
    m.apply(Command::SetLaneOrder { order }).unwrap();
    let swapped = chain(&m);
    assert!(
        matches!(swapped[0], FxInstance::Mirror { horizontal: true, .. }),
        "перестановка строк перевернула порядок наложения"
    );
    m.undo().unwrap();
    assert_eq!(chain(&m), first, "undo вернул и порядок строк, и порядок эффектов");
}

/// Тест 239 (Р.35 С1, кусок 4): bake РЕЖЕТ честно. Рендер-вид до bake и
/// содержимое клипов после совпадают, куски получают настоящие id,
/// переход перевешивается на кусок со стыком, всё — одним undo-шагом.
#[test]
fn t239_bake_materializes_slices() {
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, LEN, LEN));
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.morphers.push(layer(Tick(30_000), Tick(30_000)));
    p.validate().unwrap();
    let before = p.effective_clips();
    assert_eq!(before.len(), 4, "три куска первого клипа + сосед");

    let mut m = TimelineModel::new(p);
    m.apply(Command::BakeMorpher { id: MorphId(1) }).unwrap();
    assert!(m.project().morphers.is_empty(), "bake убрал событие");
    assert_eq!(m.project().clips.len(), 4, "куски стали настоящими клипами");
    assert!(
        m.project().clips.iter().all(|c| slice_origin(c.id).is_none()),
        "синтетических id в проекте не остаётся"
    );
    // геометрия и материал — те же, что рисовались
    for (a, b) in m.project().clips.iter().zip(&before) {
        assert_eq!(
            (a.start, a.duration, a.source_in, a.media),
            (b.start, b.duration, b.source_in, b.media)
        );
        assert_eq!(a.fx, b.fx);
    }
    assert_eq!(m.project().effective_clips().len(), 4, "рендер не изменился");
    // переход уехал на кусок, которому достался стык
    let t = &m.project().transitions[0];
    let a = m.project().clips.iter().find(|c| Some(c.id) == t.a).unwrap();
    assert_eq!(a.start + a.duration, LEN, "конец перехода — у куска со стыком");
    assert_ne!(t.a, Some(ClipId(10)), "а не у куска №0");
    m.project().validate().unwrap();

    m.undo().unwrap();
    assert_eq!(m.project().clips.len(), 2, "один undo вернул монтаж");
    assert_eq!(m.project().morphers.len(), 1, "и морфер");
    assert_eq!(m.project().transitions[0].a, Some(ClipId(10)));
}

/// Тест 243 (Р.35 С2): патч несёт И подмену источника, И цепочки — это
/// одно событие, а не два взаимоисключающих вида (`MorphKind` удалён).
/// Выключатель морф-слота гасит ТОЛЬКО подмену; гард `RemoveMedia`
/// спрашивает `media_refs`, а не паттерн-матч по виду (компилятор такие
/// места не ловит — на этом и держался прежний дефект).
#[test]
fn t243_patch_carries_source_and_chains() {
    let mut p = base();
    let mut m = layer(Tick(0), LEN);
    m.patch.source_in_delta = Tick(4_500);
    m.patch.video_fx = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
    p.morphers.push(m);
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "накрыт целиком — не режется");
    assert_eq!(eff[0].media, MediaId(2), "подмена источника действует");
    assert_eq!(eff[0].source_in, Tick(7_500), "и сдвиг точки входа тоже");
    assert_eq!(eff[0].fx.len(), 1, "и цепочка — одним и тем же событием");

    // галка морф-слота гасит ТОЛЬКО подмену, эффекты остаются
    p.morphers[0].patch.src_off = true;
    let eff = p.effective_clips();
    assert_eq!(eff[0].media, MediaId(1), "выключенный морф-слот возвращает исходный источник");
    assert_eq!(eff[0].source_in, Tick(3_000), "и исходную точку входа");
    assert_eq!(eff[0].fx.len(), 1, "эффекты слоя к морф-слоту отношения не имеют");
    assert!(!p.morphers[0].patch.is_empty(), "настройки подмены целы");

    // медиа, которое держит ТОЛЬКО морфер, из проекта не выкинуть
    let mut mm = TimelineModel::new(p);
    assert!(
        mm.apply(Command::RemoveMedia { id: MediaId(2) }).is_err(),
        "гард обязан отбить удаление медиа, которое держит морфер"
    );
    assert!(mm.project().media.iter().any(|m| m.id == MediaId(2)));
}

/// Тест 247 (Р.35 С4, дословно: «можно area-selection сделать сохраняемой
/// как marked_region в виде marked_area, и тоже на неё ссылаться… если
/// юзер передумал он может в виджете морфера поменять/удалить/добавить
/// регион/area на другие»). Ссылка ПО ИМЕНИ: имена уникальны в одном
/// пространстве с метками-диапазонами, переименование правит ссылки,
/// удаление оставляет ссылку мёртвой, а не молча её выкидывает.
#[test]
fn t247_named_areas_and_references() {
    // `unique_name` — одна функция на проверку и на вставку
    let taken = vec!["финал".to_string(), "финал 2".to_string()];
    assert_eq!(unique_name(&taken, "финал", "область"), "финал 3");
    assert_eq!(unique_name(&taken, "начало", "область"), "начало");
    assert_eq!(unique_name(&taken, "  ", "область"), "область");

    let mut p = base();
    p.video_lanes = 2;
    p.clips.push(clip(20, ClipKind::Video, 1, Tick(0), LEN));
    p.areas.push(MarkedArea {
        name: "финал".into(),
        start: Tick(30_000),
        duration: Tick(30_000),
        lanes: vec![(LaneKind::Video, 0)],
        color: None,
    });
    let mut m = layer(Tick(0), Tick(1)); // блок КРОШЕЧНЫЙ и не там
    m.targets = MorphTargets::Areas { names: vec!["финал".into()] };
    p.morphers.push(m);
    p.validate().unwrap();

    // окно взято из ОБЛАСТИ, а не из блока
    let eff = p.effective_clips();
    let first: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(10))).collect();
    assert_eq!(first.len(), 3, "клип на дорожке области разрезан по её границам");
    assert_eq!(first[1].start, Tick(30_000));
    assert_eq!(first[1].media, MediaId(2), "внутри области патч действует");
    assert_eq!(first[0].media, MediaId(1), "снаружи — нет");
    // область назвала ДОРОЖКИ: соседняя не тронута
    let other: Vec<&ClipEvent> = eff.iter().filter(|c| same_clip(c.id, ClipId(20))).collect();
    assert_eq!(other.len(), 1, "дорожка вне области не режется");
    assert_eq!(other[0].media, MediaId(1));

    // одно пространство имён: метка-диапазон с тем же именем незаконна
    let mut bad = p.clone();
    bad.markers.push(Marker {
        t: Tick(0),
        duration: Tick(1_000),
        name: "финал".into(),
        color: None,
    });
    assert!(matches!(bad.validate(), Err(ProjectError::DuplicateName(..))));

    let mut mm = TimelineModel::new(p);
    // команда тоже отбивает занятое имя, и своё пустое
    assert!(matches!(
        mm.apply(Command::AddArea(MarkedArea {
            name: "финал".into(),
            start: Tick(0),
            duration: Tick(1_000),
            lanes: vec![],
            color: None,
        })),
        Err(CommandError::NameTaken(..))
    ));

    // переименование правит ССЫЛКИ и откатывается ОДНИМ undo
    mm.apply(Command::RenameArea { from: "финал".into(), to: "конец".into() }).unwrap();
    assert_eq!(mm.project().areas[0].name, "конец");
    assert_eq!(
        mm.project().morphers[0].targets,
        MorphTargets::Areas { names: vec!["конец".into()] },
        "ссылка морфера переехала вместе с именем"
    );
    assert_eq!(mm.project().effective_clips().len(), 4, "и продолжает работать");
    mm.undo().unwrap();
    assert_eq!(mm.project().areas[0].name, "финал");
    assert_eq!(
        mm.project().morphers[0].targets,
        MorphTargets::Areas { names: vec!["финал".into()] }
    );

    // удаление области оставляет ссылку МЁРТВОЙ: пользователь вернёт
    // область тем же именем, и всё оживёт (правило 20 — ничего не
    // отрубается само)
    mm.apply(Command::RemoveArea { name: "финал".into() }).unwrap();
    assert!(mm.project().areas.is_empty());
    assert_eq!(
        mm.project().morphers[0].targets,
        MorphTargets::Areas { names: vec!["финал".into()] },
        "ссылка осталась в модели"
    );
    assert_eq!(mm.project().effective_clips(), mm.project().clips, "но никуда не ведёт");
    mm.project().validate().unwrap();
}

/// Тест 247b: ссылка на метку-ДИАПАЗОН даёт то же окно, что область на
/// всех дорожках; несколько пересекающихся ссылок сливаются в одну.
#[test]
fn t247_marks_give_windows_and_merge() {
    let mut p = base();
    p.markers.push(Marker {
        t: Tick(30_000),
        duration: Tick(20_000),
        name: "a".into(),
        color: None,
    });
    p.markers.push(Marker {
        t: Tick(45_000),
        duration: Tick(15_000),
        name: "b".into(),
        color: None,
    });
    let mut m = layer(Tick(0), Tick(1));
    m.targets =
        MorphTargets::Marks { names: vec!["a".into(), "b".into(), "нет такой".into()] };
    p.morphers.push(m);
    p.validate().unwrap();

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 3, "две пересекающиеся ссылки слились в ОДНО окно");
    assert_eq!(eff[1].start, Tick(30_000));
    assert_eq!(eff[1].duration, Tick(30_000), "[30000,60000) — объединение");
    assert_eq!(eff[1].media, MediaId(2));
    // точечная метка окна не задаёт
    let mut q = base();
    q.markers.push(Marker { t: Tick(30_000), duration: Tick::ZERO, name: "t".into(), color: None });
    let mut m = layer(Tick(0), Tick(1));
    m.targets = MorphTargets::Marks { names: vec!["t".into()] };
    q.morphers.push(m);
    assert_eq!(q.effective_clips(), q.clips, "точечная метка не диапазон");
}

/// Тест 249 (Р.35 С5, дословно: «При выделении на таймлайне всего подряд
/// и ctrl+c ctrl+v я хочу чтобы у меня проставлялись связи и для морферов
/// тоже… и marked_area-ы тоже при выделении и копировании копируются, и
/// marked_region тоже вместе с теми что внутри него. Имена — уникальные,
/// и можно попросить пользователя ввести префикс»).
#[test]
fn t249_clipboard_carries_morphers_areas_and_marks() {
    use kt_model::ops::*;
    let mut p = base();
    p.clips.push(clip(11, ClipKind::Video, 0, LEN, LEN));
    p.areas.push(MarkedArea {
        name: "финал".into(),
        start: Tick(0),
        duration: Tick(45_000),
        lanes: vec![(LaneKind::Video, 0)],
        color: None,
    });
    p.markers.push(Marker {
        t: Tick(10_000),
        duration: Tick(20_000),
        name: "врез".into(),
        color: None,
    });
    // морфер на ПЕРВЫЙ клип и морфер-ссылка на область
    p.morphers.push(layer(Tick(0), Tick(45_000)));
    let mut byref = layer(Tick(0), Tick(45_000));
    byref.id = MorphId(2);
    byref.targets = MorphTargets::Areas { names: vec!["финал".into(), "чужая".into()] };
    p.morphers.push(byref);
    // морфер, чьи цели ВНЕ блока — его вставка обязана пропустить
    let mut orphan = layer(Tick(0), Tick(45_000));
    orphan.id = MorphId(3);
    orphan.targets = MorphTargets::Clips { ids: vec![ClipId(11)] };
    p.morphers.push(orphan);
    p.validate().unwrap();

    // копируем ПЕРВЫЙ клип: с ним едут его морфер, область и метка
    let block = copy_clips(&p, &[ClipId(10)]);
    assert_eq!(block.clips.len(), 1);
    assert_eq!(block.areas.len(), 1, "область в окне блока копируется");
    assert_eq!(block.markers.len(), 1, "метка-диапазон тоже");
    assert_eq!(block.morphers.len(), 2, "морфер, чьи цели ВНЕ блока, не копируется вовсе");
    // ...а если блок пришёл со стороны (системный буфер — это JSON) и
    // ссылается на клип, которого в нём нет, вставка ПРОПУСКАЕТ такой
    // морфер и считает его: батч атомарен, и один плохой элемент уронил
    // бы всю вставку, а выглядело бы это как «копипаст не работает»
    let mut block = block;
    let mut foreign = block.morphers[0].clone();
    foreign.id = MorphId(9);
    foreign.targets = MorphTargets::Clips { ids: vec![ClipId(777)] };
    block.morphers.push(foreign);

    let mut m = TimelineModel::new(p);
    let (mut nc, mut ng, mut nt, mut nm) = (100u64, 100u64, 100u64, 100u64);
    let mut ncam = 100u64;
    let names =
        PasteNames { affix: "копия".into(), before: false, taken: m.project().taken_names() };
    let (cmds, _end, skipped) = paste_block(
        &block,
        Tick(300_000),
        PasteIds {
            clip: &mut nc,
            group: &mut ng,
            transition: &mut nt,
            morph: &mut nm,
            camera: &mut ncam,
        },
        &names,
    );
    assert_eq!(skipped.morphers, 1, "морфер без уцелевших целей пропущен");
    m.apply_batch(cmds).expect("батч обязан пройти, а не упасть из-за пропущенного");

    // имена уникальны и с приставкой; умолчание — СУФФИКС (Р.38 п.19:
    // «надо по дефолту проставлять не префикс а суффикс»)
    let a = m.project().areas.iter().find(|a| a.name != "финал").expect("копия области");
    assert_eq!(a.name, "финал копия");
    assert_eq!(a.start, Tick(300_000), "область ребейзнута к точке вставки");
    assert!(
        m.project().markers.iter().any(|k| k.name == "врез копия" && k.t == Tick(310_000)),
        "метка тоже переименована и переехала"
    );

    // цели скопированного морфера указывают ТОЛЬКО на новые id
    let pasted: Vec<&MorpherEvent> =
        m.project().morphers.iter().filter(|x| x.id.0 >= 100).collect();
    assert_eq!(pasted.len(), 2, "два морфера доехали, третий пропущен");
    let frozen = pasted.iter().find(|x| !x.targets.clip_ids().is_empty()).unwrap();
    assert_eq!(frozen.targets.clip_ids(), &[ClipId(100)], "цель — новый клип, а не старый");
    // ссылка на СКОПИРОВАННОЕ имя переехала, на нескопированное — нет
    let byref = pasted.iter().find(|x| x.targets.clip_ids().is_empty()).unwrap();
    assert_eq!(
        byref.targets,
        MorphTargets::Areas { names: vec!["финал копия".into(), "чужая".into()] }
    );
    m.project().validate().unwrap();

    // ВТОРАЯ вставка тем же словом даёт « 2» — имена остаются адресами
    let names =
        PasteNames { affix: "копия".into(), before: false, taken: m.project().taken_names() };
    let (cmds, _end, _) = paste_block(
        &block,
        Tick(600_000),
        PasteIds {
            clip: &mut nc,
            group: &mut ng,
            transition: &mut nt,
            morph: &mut nm,
            camera: &mut ncam,
        },
        &names,
    );
    m.apply_batch(cmds).unwrap();
    assert!(
        m.project().areas.iter().any(|a| a.name == "финал копия 2"),
        "вторая вставка обязана дать уникальное имя: {:?}",
        m.project().areas.iter().map(|a| &a.name).collect::<Vec<_>>()
    );
}

/// Тест 266 (Р.38 п.13, дословно: «у морфера я не могу задавать
/// автоматизацию эффектов»). Кривые слоя живут в его патче в домене
/// БЛОКА (у морфера нет материала), а `apply_to` переводит их в материал
/// цели — тем же отображением, что и всё остальное.
#[test]
fn t266_morph_layer_curves_rebase_to_target_material() {
    use kt_model::*;
    use kt_time::Tick;

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(90_000),
        duration: Tick(90_000 * 4),
        // точка входа НЕ нулевая: иначе «материал» и «таймлайн» совпали бы
        // и тест не отличил бы правильный перевод от его отсутствия
        source_in: Tick(90_000 * 10),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.morph_lanes = 1;
    p.lane_order = vec![(LaneKind::Morph, 0), (LaneKind::Video, 0), (LaneKind::Audio, 0)];
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        // блок начинается ПОЗЖЕ клипа — сдвиг обязан войти в перевод
        start: Tick(90_000 * 2),
        duration: Tick(90_000 * 2),
        targets: MorphTargets::Below,
        patch: MorphPatch {
            video_fx: vec![FxInstance::blur(4)],
            video_auto: vec![vec![ParamCurve {
                param: "radius".into(),
                ease: false,
                keys: vec![AutoKey { t: Tick(0), v: 2.0 }, AutoKey { t: Tick(90_000 * 2), v: 9.0 }],
            }]],
            ..Default::default()
        },
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
    });
    p.validate().unwrap();

    let eff = p.effective_clips();
    let patched: Vec<&ClipEvent> = eff.iter().filter(|c| !c.fx.is_empty()).collect();
    assert_eq!(patched.len(), 1, "накрыт ровно один кусок: {:?}", eff.len());
    let c = patched[0];
    assert_eq!(c.fx_auto.len(), c.fx.len(), "кривые параллельны цепочке");
    let keys = &c.fx_auto[0][0].keys;
    assert_eq!(keys.len(), 2);
    // блок 2..4 c таймлайна = материал 11..13 c (source_in = 10 c,
    // клип начинается в 1 c)
    assert_eq!(keys[0].t, Tick(90_000 * 11), "начало блока → материал цели");
    assert_eq!(keys[1].t, Tick(90_000 * 13), "конец блока → материал цели");
    assert_eq!(keys[0].v, 2.0);
    assert_eq!(keys[1].v, 9.0);

    // выключенный слот кривую с собой не тащит: `auto_for` идёт тем же
    // порядком, что `chain_for`, иначе кривая уехала бы на чужой эффект
    let mut p2 = p.clone();
    p2.morphers[0].patch.video_bypass = vec![true];
    let eff2 = p2.effective_clips();
    assert!(
        eff2.iter().all(|c| c.fx.is_empty() && c.fx_auto.iter().all(|a| a.is_empty())),
        "выключенный слот не приносит ни эффекта, ни кривой"
    );

    // и патч с одними кривыми не считается пустым: «пустой» не пишется в файл
    let mut only = MorphPatch::default();
    assert!(only.is_empty());
    only.video_auto = vec![vec![ParamCurve {
        param: "radius".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 1.0 }],
    }]];
    assert!(!only.is_empty(), "патч с кривыми обязан сериализоваться");
}

// ---------------------------------------------------------------------------
// Тесты 270 (Р.39): резка РАСТЯНУТОГО клипа попадает в тот материал.
// ---------------------------------------------------------------------------

fn warped(rate: f64, keys: Vec<AutoKey>, reversed: bool) -> ClipEvent {
    ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(4 * 90_000),
        source_in: Tick(90_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx: vec![FxInstance::slowed_reverb(rate, 0.0, 85.0)],
        fx_bypass: vec![false],
        fx_auto: if keys.is_empty() {
            vec![vec![]]
        } else {
            vec![vec![ParamCurve { param: "rate".into(), ease: false, keys }]]
        },
        opacity: 1.0,
        fx_off: false,
    }
}

/// Тест 270: половинная скорость — половина материала.
#[test]
fn t270_split_half_speed_in_the_middle() {
    let c = warped(0.5, vec![], false);
    let half = TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000));
    let right = clip_slice(&c, half, ClipId(2));
    assert_eq!(
        right.source_in,
        Tick(90_000 + 90_000),
        "за 2 с выхода на 0.5× съедена 1 с материала, а не 2 (до Р.39 было 270_000)"
    );
    // левый кусок начинается там же, где родитель
    let left = clip_slice(&c, TickRange::new(Tick(0), Tick(2 * 90_000)), ClipId(1));
    assert_eq!(left.source_in, c.source_in, "разрез в начале материал не двигает");
}

/// Тест 270б: то же зеркально. «ВАРП, ПОТОМ ЗЕРКАЛО» — якорь считается от
/// фактически потреблённого материала, а не от длины события.
#[test]
fn t270b_split_reversed_half_speed() {
    let c = warped(0.5, vec![], true);
    // всего съедается 2 с материала
    let first = clip_slice(&c, TickRange::new(Tick(0), Tick(2 * 90_000)), ClipId(1));
    assert_eq!(
        first.source_in,
        Tick(90_000 + 90_000),
        "зеркальный клип начинает с ХВОСТА потреблённого: 1 с, а не 2"
    );
    let last = clip_slice(&c, TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000)), ClipId(2));
    assert_eq!(last.source_in, c.source_in, "последний кусок играет начало источника");
}

/// Тест 270в: КРИВАЯ скорости — интеграл, а не умножение. Плюс главное:
/// ключи едут вместе с якорем, иначе за ним уезжает вся кривая.
#[test]
fn t270c_split_on_a_rate_curve_moves_keys_with_the_anchor() {
    // рампа 1.0 → 2.0 по всему событию; ключи в тиках материала от source_in
    let c = warped(
        1.0,
        vec![AutoKey { t: Tick(90_000), v: 1.0 }, AutoKey { t: Tick(90_000 + 4 * 90_000), v: 2.0 }],
        false,
    );
    let at = Tick(2 * 90_000);
    let right = clip_slice(&c, TickRange::new(at, Tick(4 * 90_000)), ClipId(2));

    // S(0.5) для рампы 1→2 = 0.5·1 + 0.5²·0.5/… — трапеция: (1 + 1.5)/2 · 0.5 = 0.625
    let want = Tick(90_000 + (4.0f64 * 90_000.0 * 0.625).round() as i64);
    assert_eq!(right.source_in, want, "якорь — интеграл скорости, а не 2 с");

    // ключи сдвинулись на ТУ ЖЕ дельту: разность (k.t − source_in) цела
    let d = (want - c.source_in).0 - (at - c.start).0;
    assert_ne!(d, 0, "сцена должна быть варпнутой, иначе тест ничего не проверяет");
    for (before, after) in c.fx_auto[0][0].keys.iter().zip(&right.fx_auto[0][0].keys) {
        assert_eq!(
            after.t.0 - right.source_in.0,
            before.t.0 - c.source_in.0 - (at - c.start).0,
            "ось автоматизации куска обязана остаться линейной (Р.37), \
             а сдвинуться обязан только материал"
        );
    }
}

/// Тест 270г: шов. Конец первого куска и начало второго — один и тот же
/// материал; сумма потреблённого равна родительской.
#[test]
fn t270d_slice_seam_has_no_gap() {
    for (rate, rev) in [(0.5, false), (2.0, false), (0.5, true), (2.0, true)] {
        let c = warped(rate, vec![], rev);
        let at = Tick(90_000 * 3 / 2); // не на круглой границе
        let l = clip_slice(&c, TickRange::new(Tick(0), at), ClipId(1));
        let r = clip_slice(&c, TickRange::new(at, Tick(4 * 90_000)), ClipId(2));
        let used = |x: &ClipEvent| kt_model::clip_warp(x).map(|w| w.used()).unwrap_or(x.duration.0);
        let (near, far) = if rev { (&r, &l) } else { (&l, &r) };
        let seam = near.source_in.0 + used(near);
        assert!(
            (seam - far.source_in.0).abs() <= 1,
            "rate {rate} rev {rev}: шов расходится на {} тиков",
            seam - far.source_in.0
        );
        assert!(
            (used(&l) + used(&r) - used(&c)).abs() <= 2,
            "rate {rate} rev {rev}: суммарно съедено не столько, сколько родителем"
        );
    }
}

/// Тест 270д: на клипе БЕЗ варпа правило прежнее байт-в-байт — на этом
/// стоят GOLDEN'ы и все старые тесты.
#[test]
fn t270e_unwarped_slice_is_unchanged() {
    for reversed in [false, true] {
        for fx in [vec![], vec![FxInstance::blur(4)]] {
            let mut c = warped(1.0, vec![], reversed);
            c.fx = fx.clone();
            c.fx_bypass = vec![false; fx.len()];
            c.fx_auto = vec![vec![]; fx.len()];
            for w in [(0, 2), (1, 3), (2, 4)] {
                let win = TickRange::new(Tick(w.0 * 90_000), Tick(w.1 * 90_000));
                let want = if reversed {
                    c.source_in + ((c.start + c.duration) - win.end)
                } else {
                    c.source_in + (win.start - c.start)
                };
                assert_eq!(
                    clip_slice(&c, win, ClipId(9)).source_in,
                    want,
                    "без варпа — прежняя линейная формула, reversed={reversed}"
                );
            }
        }
    }
    // и выключенный слот варпа тоже не варп
    let mut off = warped(0.5, vec![], false);
    off.fx_bypass = vec![true];
    assert_eq!(
        clip_slice(&off, TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000)), ClipId(9)).source_in,
        Tick(90_000 + 2 * 90_000),
        "выключенный слот варпа резку не искривляет"
    );
    let mut master = warped(0.5, vec![], false);
    master.fx_off = true;
    assert_eq!(
        clip_slice(&master, TickRange::new(Tick(2 * 90_000), Tick(4 * 90_000)), ClipId(9))
            .source_in,
        Tick(90_000 + 2 * 90_000),
        "мастер-выключатель цепочки — тоже"
    );
}

/// Тест 288 (Р.43, п.298): вето «варпнутый клип не резать» не отменяет
/// ВРЕМЯ блока.
///
/// Претензия пользователя на реальном проекте: «в problems1 … звук от
/// другого события откуда-то взялся», «в rendereg на самом верхнем
/// событии кривые автоматизации не работают», «регион зацикленность_5
/// какой-то поломанный», — и решающая подсказка: «есть немного ранняя
/// версия проекта где как минимум rendereg работает правильно». В ней НЕ
/// БЫЛО НИ ОДНОГО МОРФЕРА.
///
/// Причина: `windows_on` проверял вето ПЕРВЫМ и возвращал всё тело клипа,
/// ни разу не спросив, дотягивается ли блок до этого клипа по времени.
/// «Целиком» — это про ГРАНИЦЫ куска, а не про ДОСЯГАЕМОСТЬ. В проекте
/// пользователя морф-блок длиной 0.6 с на 5020-й секунде подменял
/// источник у событий на 148-й («Салют» вместо «далеко»), а блок со
/// slowed&reverb на 169-й секунде раздавал ВАРП ВРЕМЕНИ всем событиям
/// проекта ниже своей строки — включая звук, у которого его в файле нет.
#[test]
fn t288_layer_does_not_reach_a_warped_clip_outside_its_window() {
    let warp = FxInstance::SlowedReverb {
        rate: 0.5,
        mix: 0.0,
        reverb_delay_ms: 60.0,
        damp: 0.4,
        decay: 0.7,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    };

    // строка слоя обязана быть ВЫШЕ дорожки клипа, иначе `Below` не
    // накрывает его и тест зеленел бы по совсем другой причине
    let above = |p: &mut Project| {
        p.lane_order = vec![(LaneKind::Morph, 0), (LaneKind::Video, 0), (LaneKind::Audio, 0)];
    };

    // --- 1. варп у САМОГО КЛИПА: блок далеко, трогать нечего -------------
    let mut p = base();
    above(&mut p);
    p.clips[0].fx = vec![warp.clone()];
    // цель — `Below`, то есть «всё, что ниже строки слоя»; блок стоит
    // ДАЛЕКО от клипа по времени
    let mut m = layer(Tick(10 * 90_000), Tick(90_000));
    m.targets = MorphTargets::Below;
    p.morphers.push(m);
    p.validate().unwrap();

    let rows = p.rows();
    assert!(
        p.morphers[0].covers(&p.clips[0], &rows),
        "слой обязан накрывать клип ПО ПРОСТРАНСТВУ — иначе проверка про ВРЕМЯ          выполнялась бы сама собой"
    );
    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &rows);
    assert!(
        wins.is_empty(),
        "блок на 10-й секунде не имеет отношения к клипу 0..1 с, даже если тот \
         растянут: {wins:?}"
    );
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "клип не режется");
    assert_eq!(eff[0].media, MediaId(1), "источник обязан остаться своим");
    assert_eq!(eff[0].fx.len(), 1, "чужих эффектов у клипа не появилось");

    // --- 2. варп в ПАТЧЕ слоя: тот же запрет ------------------------------
    let mut p = base();
    above(&mut p);
    let mut m = layer(Tick(10 * 90_000), Tick(90_000));
    m.targets = MorphTargets::Below;
    m.patch = MorphPatch { audio_fx: vec![warp.clone()], ..Default::default() };
    p.morphers.push(m);
    p.clips.push(clip(11, ClipKind::Audio, 0, Tick(0), LEN));
    p.audio_lanes = 1;
    p.validate().unwrap();

    let eff = p.effective_clips();
    for c in &eff {
        assert!(
            c.fx.is_empty(),
            "слой со slowed&reverb раздал варп событию {:?} за 10 секунд от себя: {:?}",
            c.id,
            c.fx
        );
    }

    // --- 3. а ВНУТРИ окна варпнутый клип по-прежнему патчится ЦЕЛИКОМ -----
    let mut p = base();
    above(&mut p);
    p.clips[0].fx = vec![warp];
    let mut m = layer(Tick(30_000), Tick(30_000)); // середина клипа
    m.targets = MorphTargets::Below;
    p.morphers.push(m);
    p.validate().unwrap();

    let rows = p.rows();
    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &rows);
    assert_eq!(
        wins,
        vec![TickRange::new(Tick(0), LEN)],
        "накрытый растянутый клип патчится целиком и НЕ режется (Р.37)"
    );
    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "растянутый клип резать нельзя");
    assert_eq!(eff[0].media, MediaId(2), "и патч на нём действует");
}

/// Тест 291 (Р.43, п.300): подмена источника слоем НЕ теряет собственную
/// анимацию события.
///
/// Претензия дословно: «в rendereg почему-то на самое верхнее событие не
/// применяется pan3d пока я в морфере не отключу замену сурса, что
/// странно».
///
/// Ключ автоматизации приклеен к МАТЕРИАЛУ (t212) — и это верно, пока
/// материал тот же: обрезка прячет и открывает ключи, разрез делит кривую.
/// Но подмена источника слоем меняет саму систему координат: у события
/// пользователя `source_in` уезжал с 47.0 с на 99.0 с, а ключи оставались
/// на 47.0..59.6 с — то есть ЦЕЛИКОМ левее нового окна. Кривая зажималась
/// последним ключом, и pan3d становился постоянным: rot_y = 0, rot_z = 0.
/// Со стороны это ровно «pan3d не применяется».
///
/// Правило то же, что у `clip_slice`: якорь и ключи едут ОДНОЙ дельтой.
#[test]
fn t291_source_replacement_carries_the_clips_own_curves() {
    const STEP: i64 = 90_000 / 30;
    let peak = Tick(3_000 + LEN.0 / 2);

    let make = |replace: bool| -> ClipEvent {
        let mut p = base();
        p.clips[0].fx = vec![FxInstance::Pan3d {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
            scale: 1.0,
            camera: None,
        }];
        p.clips[0].fx_auto = vec![vec![ParamCurve {
            param: "rot_y".into(),
            ease: false,
            keys: vec![
                AutoKey { t: Tick(3_000), v: 0.0 },
                AutoKey { t: peak, v: 30.0 },
                AutoKey { t: Tick(3_000 + LEN.0), v: 0.0 },
            ],
        }]];
        let mut m = layer(Tick(0), LEN);
        m.patch = MorphPatch {
            media: replace.then_some(MediaId(2)),
            source_in_delta: if replace { Tick(50 * 90_000) } else { Tick::ZERO },
            ..Default::default()
        };
        // слой, который вообще ничего не делает, до цели не доходит —
        // при `replace == false` патч пустой, и это законно
        if replace {
            p.morphers.push(m);
        }
        p.validate().unwrap();
        let eff = p.effective_clips();
        assert_eq!(eff.len(), 1, "событие не режется");
        eff.into_iter().next().unwrap()
    };

    let plain = make(false);
    let patched = make(true);
    assert_eq!(patched.media, MediaId(2), "источник обязан быть подменён");
    assert_ne!(patched.source_in, plain.source_in, "точка входа обязана уехать");

    // значение кривой в СЕРЕДИНЕ события — там, где пользователь поставил
    // пик, — обязано остаться тем же: анимация приклеена к событию, а не
    // к номеру кадра чужого файла
    let at = |c: &ClipEvent| {
        let mid = Tick(c.start.0 + c.duration.0 / 2);
        let mat =
            kt_model::clip_material_at(c.source_in, c.start, c.duration, c.reversed, STEP, mid);
        kt_model::curve_value(&c.fx_auto[0][0].keys, false, mat)
    };
    let (a, b) = (at(&plain), at(&patched));
    assert!((a - 30.0).abs() < 1e-6, "без подмены в середине события стоит пик кривой, а не {a}");
    assert!(
        (b - a).abs() < 1e-6,
        "после подмены источника кривая обязана давать то же самое: было {a}, стало {b} \
         (до Р.43 ключи оставались в СТАРОМ домене материала и кривая зажималась \
          последним ключом — «pan3d не применяется»)"
    );
}

/// Клип с фейдом на ОБОИХ краях: два перехода с одним живым концом.
/// `in/out_offset` считаются от СТЫКА (`joint`), поэтому у фейда на входе
/// живёт `out_offset`, у фейда на выходе — `in_offset`.
fn with_fades_both_ends(p: &mut Project, id: ClipId, len: Tick, fade: Tick) {
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: None,
        b: Some(id),
        in_offset: Tick::ZERO,
        out_offset: fade,
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.transitions.push(TransitionEvent {
        id: TransitionId(2),
        a: Some(id),
        b: None,
        in_offset: fade,
        out_offset: Tick::ZERO,
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    let _ = len;
}

/// Тест 555 (Р.55, С2): «по времени» на событии с фейдами означает ровно
/// то, что написано.
///
/// **Претензия владельца дословно:** «еще на событиях с переходом(типо
/// fade) морфер ломается и накладывает правки на всё событие вместо
/// "by time"».
///
/// Причина была не в морфере, а в РЕЗКЕ. Резать кусок внутри окна
/// перехода нельзя — это переслаивает соседей и меняет веса кроссфейда, —
/// поэтому граница выталкивалась из запретной зоны к БЛИЖАЙШЕМУ краю, в
/// том числе НАРУЖУ. На сцене ниже (фейды по 20 000 тиков с обеих сторон,
/// окно слоя 5 000 … 85 000) старый код давал `s = 0`, `e = 90 000` —
/// ровно «на всё событие».
///
/// Лечение — не чинить выталкивание, а не резать вовсе: вклад цепочки
/// гасится кривой силы, и тогда граница точна ДО ТИКА, а переходы и
/// соседи не тронуты совсем.
#[test]
fn t555_time_window_is_exact_on_an_event_with_fades() {
    let mut p = base();
    with_fades_both_ends(&mut p, ClipId(10), LEN, Tick(20_000));
    p.validate().expect("сцена с двумя фейдами законна");
    // зоны, в которые не встаёт граница РЕЗА
    let zones = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert!(zones.iter().any(|z| z.start == Tick(0) && z.end == Tick(20_000)), "фейд на входе");
    assert!(zones.iter().any(|z| z.start == Tick(70_000) && z.end == LEN), "фейд на выходе");

    // слой из ОДНИХ ЭФФЕКТОВ: окно 5 000 … 85 000, оба конца в зонах
    let mut m = layer(Tick(5_000), Tick(80_000));
    m.patch.media = None;
    p.morphers.push(m);

    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &p.rows());
    assert_eq!(
        wins,
        vec![TickRange::new(Tick(5_000), Tick(85_000))],
        "окно обязано быть ТЕМ, что просили: ни выталкивания наружу, ни посадки на сетку"
    );

    let eff = p.effective_clips();
    assert_eq!(eff.len(), 1, "резов не появилось: гасить можно там, где резать нельзя");
    assert_eq!(eff[0].id, ClipId(10), "id оригинала цел — с ним и FX-состояние (инвариант 3)");
    assert_eq!(eff[0].duration, LEN, "тело не тронуто");

    // ВОРОТА силы: молчит вне окна, полная сила внутри
    let (i, _) = eff[0]
        .fx
        .iter()
        .enumerate()
        .find(|(_, f)| matches!(f, FxInstance::LayerMix { .. }))
        .expect("слот силы обязан быть: без него слой бьёт по всему телу");
    let cur = &eff[0].fx_auto[i][0];
    let at = |t: i64| {
        let ks = &cur.keys;
        let j = ks.partition_point(|k| k.t.0 <= t).saturating_sub(1);
        ks[j].v
    };
    // домен ключей — МАТЕРИАЛ цели (`source_in` = 3 000)
    let mat = |tl: i64| tl + 3_000;
    assert!(at(mat(1_000)) < 0.01, "до окна слой обязан молчать");
    assert!(at(mat(45_000)) > 0.99, "внутри окна — полная сила");
    assert!(at(mat(88_000)) < 0.01, "после окна слой обязан молчать");
    // и ГРАНИЦА ТОЧНА: за тик до окна ещё ноль, на самом краю уже сила
    assert!(at(mat(4_999)) < 0.01, "граница уехала влево");
    assert!(at(mat(5_000)) > 0.99, "граница уехала вправо");

    // ПЕРЕХОДЫ ЦЕЛЫ: их окна не поехали, концы не потерялись
    let after = kt_model::forbidden_zones(&p, &p.clips[0]);
    assert_eq!(after, zones, "слой тронул окна переходов — соседи переслоились бы");
}

/// Тест 556 (Р.55, С2): патч с ПОДМЕНОЙ ИСТОЧНИКА по-прежнему режет, а
/// граница уходит ТОЛЬКО ВНУТРЬ.
///
/// «Взять кадр из другого файла» кривой силы не выражается, поэтому здесь
/// резка остаётся. Но выталкивание перестало быть «к ближайшему краю»:
/// окно теперь только СОКРАЩАЕТСЯ. На той же сцене старый код давал всё
/// тело, новый — середину между фейдами.
#[test]
fn t556_source_replacing_layer_still_cuts_but_only_inward() {
    let mut p = base();
    with_fades_both_ends(&mut p, ClipId(10), LEN, Tick(20_000));
    p.morphers.push(layer(Tick(5_000), Tick(80_000))); // patch.media = Some(2)
    p.validate().expect("сцена законна");

    let wins = p.morphers[0].windows_on(&p.clips[0], &p, &p, &p.rows());
    assert_eq!(
        wins,
        vec![TickRange::new(Tick(20_000), Tick(70_000))],
        "границы обязаны уйти ВНУТРЬ, к дальним краям зон, а не наружу"
    );

    let all = p.effective_clips();
    let eff: Vec<&ClipEvent> = all.iter().filter(|c| c.kind == ClipKind::Video).collect();
    assert_eq!(eff.len(), 3, "три куска: до фейда, накрытый, после");
    assert_eq!(eff[0].start, Tick(0));
    assert_eq!(eff[1].start, Tick(20_000), "накрытый кусок начинается ЗА окном фейда");
    assert_eq!(eff[2].start, Tick(70_000), "и кончается ПЕРЕД окном второго фейда");
    assert_eq!(eff[1].media, MediaId(2), "подмена досталась среднему куску");
    assert_eq!(eff[0].media, MediaId(1), "фейд на входе слой не накрыл");
    assert_eq!(eff[2].media, MediaId(1), "и фейд на выходе тоже");
}
