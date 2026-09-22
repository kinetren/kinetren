//! Тесты 702-704 (Р.61): связь — значение ведёт другое свойство.
//!
//! Решение владельца: адресуется ЛЮБОЕ событие проекта. Выведенное из
//! инвариантов ограничение: связь не пересекает границу ленты — наружу
//! потому, что лента перестала бы быть самодостаточной и сломался бы закон
//! Э, внутрь потому, что для этого уже есть публичные параметры композиции.

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;
const FRAME: i64 = SEC / 30;

fn clip(id: u64, start: i64, dur: i64, src_in: i64) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(src_in),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::Blur { radius: 8, radius_y: 0 }],
        fx_bypass: vec![false],
        fx_auto: vec![Vec::new()],
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
    }
}

fn base() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.video_lanes = 1;
    p
}

/// Кривая-ведущая: линейно от `a` к `b` по материалу.
fn driver(param: &str, a: f64, b: f64, from: i64, to: i64) -> ParamCurve {
    ParamCurve {
        param: param.into(),
        keys: vec![
            AutoKey::shaped(Tick(from), a, SegShape::Linear),
            AutoKey::shaped(Tick(to), b, SegShape::Linear),
        ],
        source: CurveSource::Keys,
    }
}

#[test]
fn t702_a_link_reads_inside_the_event_and_across_events() {
    let mut p = base();
    // ведущее событие: radius идёт 0 → 30 по своему материалу
    let mut lead = clip(1, 0, 3 * SEC, 0);
    lead.fx_auto[0].push(driver("radius", 0.0, 30.0, 0, 3 * SEC));
    // ведомое событие стоит ПОЗЖЕ и с другой точкой входа: общего
    // материального времени у них нет, знаменатель — ТАЙМЛАЙН
    let follow = clip(2, 5 * SEC, 2 * SEC, 7 * SEC);
    p.clips.push(lead.clone());
    p.clips.push(follow.clone());

    let me = LinkCtx::of(&follow, FRAME);
    let path = LinkPath::parse("1/fx0/radius").expect("адрес разбирается");

    // середина ведомого события = 6 c таймлайна; у ведущего это 6 c его
    // материала, то есть ровно за концом (3 c) → крайнее значение
    let at_mid = kt_model::clip_material_at(
        follow.source_in,
        follow.start,
        follow.duration,
        follow.reversed,
        FRAME,
        Tick(6 * SEC),
    );
    let v = read_link(&p, &me, &path, at_mid).expect("связь читается");
    assert!((v - 30.0).abs() < 1e-9, "за телом источника берётся крайнее значение: {v}");

    // а если ведомое стоит ВНУТРИ ведущего, значения совпадают по таймлайну
    let inside = clip(3, SEC, SEC, 0);
    let me2 = LinkCtx::of(&inside, FRAME);
    let mut q = base();
    q.clips.push(lead.clone());
    q.clips.push(inside.clone());
    let at = kt_model::clip_material_at(
        inside.source_in,
        inside.start,
        inside.duration,
        inside.reversed,
        FRAME,
        Tick(3 * SEC / 2),
    );
    let got = read_link(&q, &me2, &path, at).expect("связь читается");
    let want = kt_model::curve_value(&lead.fx_auto[0][0].keys, Tick(3 * SEC / 2));
    assert!((got - want).abs() < 1e-9, "между событиями знаменатель не таймлайн: {got} vs {want}");

    // --- ВНУТРИ одного события перевод не нужен: домен общий ----------
    let mut solo = clip(4, 2 * SEC, 2 * SEC, SEC);
    solo.fx = vec![FxInstance::Blur { radius: 8, radius_y: 0 }, FxInstance::gain(0.0)];
    solo.fx_bypass = vec![false, false];
    solo.fx_auto = vec![vec![driver("radius", 4.0, 12.0, SEC, 3 * SEC)], Vec::new()];
    let mut r = base();
    r.clips.push(solo.clone());
    let me3 = LinkCtx::of(&solo, FRAME);
    let own = LinkPath::parse("self/fx0/radius").expect("свой адрес");
    assert_eq!(own.clip, None, "«self» не требует знать свой номер");
    let at = Tick(2 * SEC);
    let got = read_link(&r, &me3, &own, at).expect("своя связь читается");
    let want = kt_model::curve_value(&solo.fx_auto[0][0].keys, at);
    assert!((got - want).abs() < 1e-9, "внутри события перевод лишний: {got} vs {want}");

    // --- РЕВЕРС и ТРИМ ведущего учтены той же парой функций -----------
    let mut rev = lead.clone();
    rev.reversed = true;
    let mut t = base();
    t.clips.push(rev);
    t.clips.push(inside);
    let mirrored = read_link(&t, &me2, &path, at).expect("читается");
    assert!(mirrored.is_finite(), "зеркальный источник не читается вовсе");

    // адрес круглый: текст → разбор → текст
    for txt in ["1/fx0/radius", "self/gen/size", "12/clip/opacity", "3/fx7/amount"] {
        let pp = LinkPath::parse(txt).unwrap_or_else(|| panic!("{txt} не разобрался"));
        assert_eq!(pp.to_text(), txt, "адрес не пережил круг");
    }
    for bad in ["", "1/fx0", "1/fx0/a/b", "1/wat/x", "x/fx0/a"] {
        assert!(LinkPath::parse(bad).is_none(), "{bad:?} разобрался как адрес");
    }
}

#[test]
fn t702b_a_link_never_crosses_the_reel_boundary() {
    let mut p = base();
    let mut seq = Sequence::new(MediaId(100), "Лента");
    let mut inner = clip(10, 0, 2 * SEC, 0);
    // событие ВНУТРИ ленты пытается вести себя от события КОРНЯ
    inner.fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "1/fx0/radius".into(),
            expr: None,
            stops: Vec::new(),
        },
    });
    seq.clips.push(inner);
    seq.video_lanes = 1;
    p.sequences.push(seq);
    p.media.push(MediaRef { id: MediaId(2), path: "synth:2".into() });
    let mut root = clip(1, 0, 3 * SEC, 0);
    root.fx_auto[0].push(driver("radius", 0.0, 30.0, 0, 3 * SEC));
    p.clips.push(root);

    let err = p.validate().expect_err("связь через границу ленты обязана быть отказом");
    let text = err.to_string();
    assert!(
        text.contains("outside its reel") || text.contains("self-contained"),
        "отказ не объясняет, почему нельзя: {text}"
    );
    assert!(
        text.contains("composition parameter") || text.contains("publish"),
        "отказ не называет законный способ (публичные параметры): {text}"
    );

    // а связь ВНУТРИ ленты законна
    let mut q = base();
    let mut seq = Sequence::new(MediaId(100), "Лента");
    let mut lead = clip(10, 0, 3 * SEC, 0);
    lead.fx_auto[0].push(driver("radius", 0.0, 30.0, 0, 3 * SEC));
    let mut follow = clip(11, 0, 3 * SEC, 0);
    follow.lane = 0;
    follow.fx_auto[0].push(ParamCurve {
        param: "radius_y".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "10/fx0/radius".into(),
            expr: Some("src / 2".into()),
            stops: Vec::new(),
        },
    });
    seq.clips.push(lead);
    seq.clips.push(follow);
    seq.video_lanes = 1;
    q.sequences.push(seq);
    q.media.push(MediaRef { id: MediaId(2), path: "synth:2".into() });
    q.validate().expect("связь внутри одной ленты законна");
}

#[test]
fn t703_cycles_are_refused_with_the_path() {
    let link = |to: &str| ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link { path: to.into(), expr: None, stops: Vec::new() },
    };

    // --- пара, ведущая друг друга и НИКЕМ не читаемая -----------------
    let mut p = base();
    let mut a = clip(1, 0, SEC, 0);
    let mut b = clip(2, 0, SEC, 0);
    a.fx_auto[0].push(link("2/fx0/radius"));
    b.fx_auto[0].push(link("1/fx0/radius"));
    p.clips.push(a);
    p.clips.push(b);
    let err = p.validate().expect_err("пара в цикле обязана быть отказом");
    let text = err.to_string();
    assert!(text.contains("->"), "путь цикла не напечатан: {text}");
    assert!(text.contains("circle") || text.contains("drive"), "{text}");

    // --- тройка через разные события ----------------------------------
    let mut q = base();
    let mut a = clip(1, 0, SEC, 0);
    let mut b = clip(2, 0, SEC, 0);
    let mut c = clip(3, 0, SEC, 0);
    a.fx_auto[0].push(link("2/fx0/radius"));
    b.fx_auto[0].push(link("3/fx0/radius"));
    c.fx_auto[0].push(link("1/fx0/radius"));
    q.clips.push(a);
    q.clips.push(b);
    q.clips.push(c);
    assert!(q.validate().is_err(), "цикл длиной три прошёл");

    // --- свойство, ведущее САМО СЕБЯ ----------------------------------
    let mut r = base();
    let mut a = clip(1, 0, SEC, 0);
    a.fx_auto[0].push(link("1/fx0/radius"));
    r.clips.push(a);
    assert!(r.validate().is_err(), "самоссылка прошла");
    let mut r2 = base();
    let mut a2 = clip(1, 0, SEC, 0);
    a2.fx_auto[0].push(link("self/fx0/radius"));
    r2.clips.push(a2);
    assert!(r2.validate().is_err(), "самоссылка через «self» прошла");

    // --- ЦЕПОЧКА без цикла законна ------------------------------------
    let mut s = base();
    let mut a = clip(1, 0, SEC, 0);
    let mut b = clip(2, 0, SEC, 0);
    let mut c = clip(3, 0, SEC, 0);
    a.fx_auto[0].push(driver("radius", 0.0, 10.0, 0, SEC));
    b.fx_auto[0].push(link("1/fx0/radius"));
    c.fx_auto[0].push(link("2/fx0/radius"));
    s.clips.push(a);
    s.clips.push(b);
    s.clips.push(c);
    s.validate().expect("цепочка A → B → C без цикла законна");

    // --- проект БЕЗ связей валиден как прежде -------------------------
    let mut plain = base();
    plain.clips.push(clip(1, 0, SEC, 0));
    plain.validate().expect("проект без связей валиден");
}

#[test]
fn t704_non_numeric_targets_follow_thresholds_and_snapshots() {
    let snap = |at: f64, v: ParamValue| LinkStop { at, value: v };
    let stops = vec![
        snap(0.0, ParamValue::Text("низ".into())),
        snap(10.0, ParamValue::Text("середина".into())),
        snap(20.0, ParamValue::Text("верх".into())),
    ];

    // между отметками держится ПРЕДЫДУЩИЙ снимок: половины между
    // вариантами не существует
    let text = |v: f64| match stop_value(&stops, v) {
        Some(ParamValue::Text(s)) => s.clone(),
        other => panic!("не текст: {other:?}"),
    };
    assert_eq!(text(0.0), "низ", "на самой отметке действует ЕЁ снимок");
    assert_eq!(text(9.99), "низ", "между отметками держится предыдущий");
    assert_eq!(text(10.0), "середина");
    assert_eq!(text(19.0), "середина");
    assert_eq!(text(20.0), "верх");
    assert_eq!(text(1e9), "верх", "выше последней — последний");
    assert_eq!(text(-5.0), "низ", "ниже первой — ПЕРВЫЙ снимок, а не пусто");

    // флаг — тот же механизм двумя отметками, без дрожания на равенстве
    let flag = vec![snap(0.0, ParamValue::Flag(false)), snap(0.5, ParamValue::Flag(true))];
    let on = |v: f64| matches!(stop_value(&flag, v), Some(ParamValue::Flag(true)));
    assert!(!on(0.49));
    assert!(on(0.5), "на самом пороге переключение уже состоялось");
    assert!(on(0.51));
    // равенство трактуется В ОДНУ сторону и не дрожит
    for _ in 0..8 {
        assert!(on(0.5), "порог дрожит на равенстве");
    }

    // пустой набор отметок — это «нечего подставлять», а не паника
    assert!(stop_value(&[], 1.0).is_none());

    // отметки едут в файл вместе со связью
    let src = CurveSource::Link {
        path: "1/gen/text".into(),
        expr: None,
        stops: stops.clone(),
    };
    let j = serde_json::to_string(&src).expect("сериализуется");
    let back: CurveSource = serde_json::from_str(&j).expect("читается");
    assert_eq!(back, src, "снимки не пережили круг");
    // а связь БЕЗ снимков их в файл не пишет
    let bare = CurveSource::Link { path: "1/fx0/radius".into(), expr: None, stops: Vec::new() };
    let j = serde_json::to_string(&bare).expect("сериализуется");
    assert!(!j.contains("stops"), "пустые снимки уехали в файл: {j}");
}

/// Тест 705 (Р.61): разрешение производных источников стоит НОЛЬ, когда их
/// нет, и не облагает соседей, когда они есть.
///
/// Требование владельца: «если проект простой без выкрутасов — то и лагать
/// он не должен вовсе, ведь плагины/кривые не применяются». У звука это
/// правило исполнено с самого начала; здесь оно проверяется тестом, а не
/// обещанием.
#[test]
fn t705_derived_sources_cost_nothing_when_unused() {
    // --- НЕТ производных источников — лента ЗАИМСТВУЕТСЯ --------------
    let mut plain = Vec::new();
    for i in 1..=8 {
        let mut c = clip(i, (i as i64 - 1) * SEC, SEC, 0);
        c.fx_auto[0].push(driver("radius", 1.0, 9.0, 0, SEC));
        plain.push(c);
    }
    let got = resolve_derived(&plain, FRAME);
    assert!(
        matches!(got, std::borrow::Cow::Borrowed(_)),
        "лента без связей и формул скопирована — графа не должно быть вовсе"
    );
    assert_eq!(&got[..], &plain[..], "заимствованная лента изменилась");

    // --- ОДНА связь среди многих: соседи побайтово прежние ------------
    let mut mixed = plain.clone();
    mixed[7].fx_auto[0].push(ParamCurve {
        param: "radius_y".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "1/fx0/radius".into(),
            expr: Some("src * 2".into()),
            stops: Vec::new(),
        },
    });
    let out = resolve_derived(&mixed, FRAME);
    assert!(matches!(out, std::borrow::Cow::Owned(_)), "связь не разрешилась вовсе");
    for i in 0..7 {
        assert_eq!(
            out[i], plain[i],
            "событие {i} изменилось от связи на СОСЕДНЕМ — цена поехала по монтажу"
        );
    }

    // --- связь разрешилась в обычные ключи ---------------------------
    let done = &out[7].fx_auto[0][1];
    assert!(done.source.is_keys(), "источник остался сырым: {:?}", done.source);
    assert!(!done.keys.is_empty(), "ключей после разрешения нет");
    assert!(
        done.keys.iter().all(|k| k.shape == SegShape::Linear),
        "производная кривая обязана быть кусочно-линейной: она сэмплирована"
    );
    // и значения — ровно удвоенный ведущий
    let lead = &plain[0].fx_auto[0][0];
    for k in &done.keys {
        let tl = kt_model::clip_timeline_at(
            plain[7].source_in,
            plain[7].start,
            plain[7].duration,
            false,
            FRAME,
            k.t,
        );
        let tl = Tick(tl.0.clamp(plain[0].start.0, plain[0].start.0 + plain[0].duration.0));
        let mat = kt_model::clip_material_at(
            plain[0].source_in,
            plain[0].start,
            plain[0].duration,
            false,
            FRAME,
            tl,
        );
        let want = kt_model::curve_value(&lead.keys, mat) * 2.0;
        assert!((k.v - want).abs() < 1e-9, "выражение пересчёта не применилось: {} vs {want}", k.v);
    }

    // --- ЦЕПОЧКА A → B → C разрешается целиком -----------------------
    let mut chain = vec![clip(1, 0, SEC, 0), clip(2, 0, SEC, 0), clip(3, 0, SEC, 0)];
    chain[0].fx_auto[0].push(driver("radius", 2.0, 2.0, 0, SEC));
    chain[1].fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "1/fx0/radius".into(),
            expr: Some("src + 1".into()),
            stops: Vec::new(),
        },
    });
    chain[2].fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "2/fx0/radius".into(),
            expr: Some("src + 1".into()),
            stops: Vec::new(),
        },
    });
    let out = resolve_derived(&chain, FRAME);
    for (i, want) in [(1usize, 3.0), (2usize, 4.0)] {
        let c = &out[i].fx_auto[0][0];
        assert!(c.source.is_keys(), "звено {i} цепочки осталось сырым");
        let v = c.keys[0].v;
        assert!(
            (v - want).abs() < 1e-9,
            "звено {i} разрешилось до ведущего: {v}, ждали {want} (порядок не топологический)"
        );
    }
}

/// Тест 707 (Р.61): формула — источник значения, кривая без ключей.
///
/// Владелец: «возможность задания кривой автоматизации через ввод формулы
/// (без прокликивания мышки)». Разрешается тем же кодом, что связь: разница
/// только в том, что адреса у неё нет, а есть время.
#[test]
fn t707_a_formula_is_a_curve_without_keys() {
    let formula = |expr: &str| ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Formula { expr: expr.into() },
    };

    // --- формула ОТ ВРЕМЕНИ ------------------------------------------
    let mut c = clip(1, 0, 2 * SEC, 0);
    c.fx_auto[0].push(formula("t * 10"));
    let only = vec![c.clone()];
    let out = resolve_derived(&only, FRAME);
    let got = &out[0].fx_auto[0][0];
    assert!(got.source.is_keys(), "формула не разрешилась: {:?}", got.source);
    assert!(got.keys.len() > 30, "сетка кадров дала слишком мало точек: {}", got.keys.len());
    // независимый оракул: t в СЕКУНДАХ от начала материала события
    for k in &got.keys {
        let t_s = (k.t.0 - c.source_in.0) as f64 / 90_000.0;
        assert!(
            (k.v - t_s * 10.0).abs() < 1e-9,
            "формула посчиталась не так: в {:?} дала {}, ждали {}",
            k.t,
            k.v,
            t_s * 10.0
        );
    }

    // --- формула, ЧИТАЮЩАЯ другое свойство, идёт тем же адресом ------
    let mut lead = clip(1, 0, 2 * SEC, 0);
    lead.fx_auto[0].push(driver("radius", 4.0, 4.0, 0, 2 * SEC));
    let mut follow = clip(2, 0, 2 * SEC, 0);
    follow.fx_auto[0].push(ParamCurve {
        param: "radius_y".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "1/fx0/radius".into(),
            expr: Some("src * src".into()),
            stops: Vec::new(),
        },
    });
    let pair = vec![lead, follow];
    let out = resolve_derived(&pair, FRAME);
    let sq = &out[1].fx_auto[0][0];
    assert!(sq.source.is_keys(), "связь с выражением не разрешилась");
    assert!((sq.keys[0].v - 16.0).abs() < 1e-9, "выражение над ведущим не применилось: {}", sq.keys[0].v);

    // --- у формулы на СКОРОСТИ то же правило, что у не-прямой формы --
    let mut p = base();
    let mut rate = clip(1, 0, SEC, 0);
    rate.fx = vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)];
    rate.fx_bypass = vec![false];
    rate.fx_auto = vec![vec![formula("1 + t")]];
    rate.fx_auto[0][0].param = "rate".into();
    p.clips.push(rate);
    // формула законна сама по себе; после разрешения ключи ПРЯМЫЕ, и
    // значит правило скорости не нарушено — сэмплированная кривая по
    // построению кусочно-линейна
    let out = resolve_derived(&p.clips, FRAME);
    assert!(
        out[0].fx_auto[0][0].keys.iter().all(|k| k.shape == SegShape::Linear),
        "разрешённая формула на скорости обязана быть кусочно-линейной"
    );
    assert!(
        kt_model::check_fx_auto(ClipId(1), &out[0].fx_auto).is_ok(),
        "разрешённая формула на скорости отвергнута"
    );
}
