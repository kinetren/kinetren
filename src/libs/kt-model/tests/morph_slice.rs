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
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
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
    m.apply(Command::Split { id: ClipId(10), at: Tick(30_000), new_id: ClipId(11), new_group: None })
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

    // патч с одними видео-эффектами не трогает аудио под собой
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
    assert_eq!(eff.iter().filter(|c| c.kind == ClipKind::Video).count(), 3, "видео разрезано");
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
    });
    p.validate().unwrap();
    let zones = p.forbidden_zones(&p.clips[0]);
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
    let zones = p.forbidden_zones(&p.clips[0]);
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
    assert_eq!(p.morphers[0].windows_on(&p.clips[0], &p, &p.rows()), vec![TickRange::new(Tick(0), LEN)]);

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
        assert_eq!((a.start, a.duration, a.source_in, a.media), (b.start, b.duration, b.source_in, b.media));
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
    assert_eq!(mm.project().morphers[0].targets, MorphTargets::Areas { names: vec!["финал".into()] });

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
    m.targets = MorphTargets::Marks { names: vec!["a".into(), "b".into(), "нет такой".into()] };
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
