//! Тест 190: автоматизация параметров эффектов.
//! curve_value (кламп краёв, линейность, ease), set/get_param по ВСЕМ
//! спецификациям каждого варианта, команды SetFxAuto/SetFx (подрезка),
//! валидация битых кривых, байт-стабильность файлов без автоматизации.
//! С Р.33 ключи стоят в ТИКАХ МАТЕРИАЛА, а не в долях длины события.

use kt_model::*;
use kt_time::Tick;

fn key(t: i64, v: f64) -> AutoKey {
    AutoKey { t: Tick(t), v }
}

#[test]
fn t190_curve_value_semantics() {
    let keys = vec![key(20_000, 10.0), key(80_000, 40.0)];
    // кламп краёв: до первой точки и после последней — крайние значения
    assert_eq!(curve_value(&keys, false, Tick(0)), 10.0);
    assert_eq!(curve_value(&keys, false, Tick(20_000)), 10.0);
    assert_eq!(curve_value(&keys, false, Tick(100_000)), 40.0);
    assert_eq!(
        curve_value(&keys, false, Tick(5_000_000)),
        40.0,
        "далеко за последним ключом — его значение"
    );
    // домен НЕ клампится сверху: позиция за телом клипа законна (ручка
    // перехода, пре-ролл) и продолжает кривую крайним значением
    assert_eq!(curve_value(&keys, false, Tick(-1)), 10.0);
    // линейная середина сегмента
    assert!((curve_value(&keys, false, Tick(50_000)) - 25.0).abs() < 1e-9);
    // ease: середина совпадает (smoothstep(0.5)=0.5), четверть — нет
    assert!((curve_value(&keys, true, Tick(50_000)) - 25.0).abs() < 1e-9);
    let quarter_lin = curve_value(&keys, false, Tick(35_000));
    let quarter_ease = curve_value(&keys, true, Tick(35_000));
    assert!(quarter_ease < quarter_lin, "ease поджимает начало сегмента");
    // одна точка — константа
    assert_eq!(curve_value(&[key(50_000, 7.0)], true, Tick(0)), 7.0);
    assert_eq!(curve_value(&[key(50_000, 7.0)], true, Tick(90_000)), 7.0);
    // пустая кривая — ноль (не срабатывает через apply_fx_auto)
    assert_eq!(curve_value(&[], false, Tick(50_000)), 0.0);
}

/// Ключи приклеены к МАТЕРИАЛУ (Р.33, решение пользователя): обрезка их
/// не двигает, разрез отдаёт каждой половине её кусок, реверс зеркалит
/// анимацию вместе с материалом. Раньше домен был «доля длины события»:
/// обрезка растягивала анимацию, а Split копировал кривую в обе половины
/// целиком — обе играли её от начала до конца.
#[test]
fn t212_keys_stick_to_material_through_edits() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![key(0, 1.0), key(45_000, 40.0), key(90_000, 10.0)],
            source: Default::default(),
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    let keys_of = |m: &TimelineModel, id: ClipId| -> Vec<(i64, f64)> {
        m.clip(id).unwrap().fx_auto[0][0].keys.iter().map(|k| (k.t.0, k.v)).collect()
    };
    let before = keys_of(&m, ClipId(1));

    // обрезка конца: ключи НЕ двигаются, просто последний уходит за край
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(60_000) }).unwrap();
    assert_eq!(keys_of(&m, ClipId(1)), before, "трим не смеет двигать ключи");
    m.undo().unwrap();

    // обрезка начала: тоже не двигает — материал под ключами тот же
    m.apply(Command::TrimStart { id: ClipId(1), new_start: Tick(30_000) }).unwrap();
    assert_eq!(keys_of(&m, ClipId(1)), before, "трим начала не смеет двигать ключи");
    m.undo().unwrap();

    // разрез: обе половины держат ОДНУ кривую в координатах материала,
    // поэтому каждая играет свой кусок (у левой виден ключ 0..45k, у
    // правой — 45k..90k) — дублирования анимации больше нет
    m.apply(Command::Split { id: ClipId(1), at: Tick(45_000), new_id: ClipId(2), new_group: None })
        .unwrap();
    let (l, r) = (m.clip(ClipId(1)).unwrap().clone(), m.clip(ClipId(2)).unwrap().clone());
    assert_eq!(keys_of(&m, ClipId(1)), before);
    assert_eq!(keys_of(&m, ClipId(2)), before);
    // значение кривой на стыке одинаково с обеих сторон — анимация
    // непрерывна через разрез
    let left_end =
        kt_model::clip_material_at(l.source_in, l.start, l.duration, false, 3000, Tick(45_000));
    let right_start =
        kt_model::clip_material_at(r.source_in, r.start, r.duration, false, 3000, Tick(45_000));
    assert_eq!(left_end, right_start, "стык разреза — одна материальная точка");
    assert_eq!(
        curve_value(&l.fx_auto[0][0].keys, false, left_end),
        curve_value(&r.fx_auto[0][0].keys, false, right_start),
        "кривая через разрез непрерывна"
    );
}

/// Каждый параметр из param_specs обязан устанавливаться и читаться;
/// значения клампятся диапазоном; чужое имя — отказ (кривая-сирота).
#[test]
fn t190_set_get_param_all_specs() {
    // Р.47 (п.326): реестр вместо ручного списка — вид, не попавший в
    // FxInstance::all(), не пройдёт замок полноты ниже.
    let variants: Vec<FxInstance> = FxInstance::all().to_vec();
    let tags: std::collections::BTreeSet<&str> = variants.iter().map(|f| f.tag()).collect();
    assert_eq!(tags.len(), variants.len(), "теги реестра уникальны");
    let mut specs_total = 0;
    for mut f in variants {
        for spec in f.param_specs() {
            specs_total += 1;
            let (name, lo, hi) = (spec.name.clone(), spec.min, spec.max);
            // дефолт спеки обязан лежать в её же диапазоне
            assert!(
                (lo..=hi).contains(&spec.default),
                "{name}: дефолт {} вне [{lo}, {hi}]",
                spec.default
            );
            assert!(spec.step > 0.0, "{name}: шаг обязан быть положительным");
            // середина диапазона ставится и читается (с точностью до
            // округления целочисленных полей)
            let mid = (lo + hi) / 2.0;
            assert!(f.set_param(&name, mid), "{name} не установился");
            let got = f.get_param(&name).unwrap();
            assert!((got - mid).abs() <= 0.5, "{name}: {got} vs {mid}");
            // выход за диапазон клампится
            // f32-поля возвращают ближайший representable к границе
            // (0.99f32 = 0.99000001) — допуск честного f32-эпсилона
            let eps = 1e-6 * hi.abs().max(1.0);
            assert!(f.set_param(&name, hi + 1e6));
            assert!(f.get_param(&name).unwrap() <= hi + eps, "{name} не клампится");
            assert!(f.set_param(&name, lo - 1e6));
            assert!(f.get_param(&name).unwrap() >= lo - eps);
        }
        // чужое имя — отказ, значение не тронуто
        assert!(!f.set_param("no_such_param", 1.0));
        assert_eq!(f.get_param("no_such_param"), None);
    }
    assert!(specs_total >= 20, "спеки покрывают все видео-варианты: {specs_total}");
    // СКОРОСТЬ СО ЗНАКОМ (Р.57, решение владельца): минус — ход по
    // материалу назад, ноль — стоп-кадр. Обобщённая проверка выше клампы
    // уже щупает; здесь названы САМИ ГРАНИЦЫ, потому что именно они были
    // претензией («не превращая диапазон -0.25...0...0.25 в 0»).
    let mut slowed = FxInstance::slowed_reverb(1.0, 0.0, 85.0);
    let rate = slowed.param_specs().iter().find(|s| s.name == "rate").expect("спека скорости");
    assert_eq!((rate.min, rate.max), (-32.0, 32.0), "диапазон скорости");
    for v in [-32.0, -1.0, -0.03, 0.0, 0.05, 1.0, 32.0] {
        assert!(slowed.set_param("rate", v), "скорость {v} не установилась");
        assert_eq!(slowed.get_param("rate").unwrap(), v, "скорость {v} поехала");
    }
    // у зеркала числовых параметров нет вовсе
    assert!(FxInstance::Mirror { horizontal: true, vertical: false }.param_specs().is_empty());
    // Р.33: аудио получило спеки — автоматизируется тем же механизмом
    let gain = FxInstance::gain(0.0);
    assert_eq!(
        gain.auto_params().count(),
        2,
        "громкость и баланс обязаны автоматизироваться (Р.33 С5)"
    );
    let slowed = FxInstance::slowed_reverb(1.0, 0.3, 60.0);
    assert!(
        slowed.auto_params().any(|s| s.name == "rate"),
        "скорость slowed&reverb обязана быть автоматизируемой"
    );
    assert_eq!(
        slowed.param_spec("reverb_delay_ms").map(|s| s.kind),
        Some(ParamKind::Structural),
        "длина линий реверба — структурный параметр, кривой не подлежит"
    );
    assert!(
        !slowed.auto_params().any(|s| s.name == "reverb_delay_ms"),
        "структурный параметр не предлагается для автоматизации"
    );
}

#[test]
fn t190_apply_fx_auto() {
    let mut f = FxInstance::blur(8);
    let curves = vec![
        ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![key(0, 1.0), key(90_000, 47.0)],
            source: Default::default(),
        },
        ParamCurve {
            param: "orphan".into(),
            ease: false,
            keys: vec![key(0, 5.0)],
            source: Default::default(),
        },
        ParamCurve { param: "empty".into(), ease: false, keys: vec![], source: Default::default() },
    ];
    assert_eq!(apply_fx_auto(&mut f, &curves, Tick(45_000)), 1, "сироты и пустые не считаются");
    assert_eq!(f.get_param("radius"), Some(24.0), "лерп 1..47 в середине");
}

fn clip_with_fx() -> ClipEvent {
    ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::blur(8), FxInstance::glow(0.5)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn model_with_clip() -> TimelineModel {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(clip_with_fx());
    p.validate().unwrap();
    TimelineModel::new(p)
}

#[test]
fn t190_commands_undo_and_truncation() {
    let mut m = model_with_clip();
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![key(0, 1.0), key(90_000, 48.0)],
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),
            ease: true,
            keys: vec![key(0, 0.0), key(90_000, 2.0)],
            source: Default::default(),
        }],
    ];
    assert!(m.apply(Command::SetFxAuto { id: ClipId(1), auto: auto.clone() }).is_ok());
    assert_eq!(m.project().clips[0].fx_auto, auto);
    // битые кривые не проходят: не отсортированы / до начала материала /
    // не конечны
    for bad_keys in
        [vec![key(90_000, 1.0), key(10_000, 2.0)], vec![key(-1, 1.0)], vec![key(45_000, f64::NAN)]]
    {
        let bad = vec![vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: bad_keys,
            source: Default::default(),
        }]];
        assert!(
            m.apply(Command::SetFxAuto { id: ClipId(1), auto: bad }).is_err(),
            "битая кривая обязана отклоняться"
        );
    }
    // SetFx с укороченной цепочкой подрезает кривые лишних слотов
    assert!(m.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::blur(8)] }).is_ok());
    assert_eq!(m.project().clips[0].fx_auto.len(), 1, "кривые второго слота отпали");
    assert_eq!(m.project().clips[0].fx_auto[0][0].param, "radius");
    // undo возвращает и цепочку, и кривые
    m.undo().unwrap();
    assert_eq!(m.project().clips[0].fx.len(), 2);
    assert_eq!(m.project().clips[0].fx_auto, auto);
}

#[test]
fn t190_files_without_auto_are_byte_stable() {
    let m = model_with_clip();
    let json = serde_json::to_string(m.project()).unwrap();
    assert!(!json.contains("fx_auto"), "пустая автоматизация не сериализуется: {json}");
    // active_fx_auto выравнивается с active_fx при bypass
    let mut c = clip_with_fx();
    c.fx_auto = vec![
        vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![key(0, 1.0)],
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),
            ease: false,
            keys: vec![key(0, 1.0)],
            source: Default::default(),
        }],
    ];
    c.fx_bypass = vec![true]; // первый слот выключен
    let fx = c.active_fx();
    let auto = c.active_fx_auto();
    assert_eq!(fx.len(), 1);
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0][0].param, "strength", "кривая уехала ВМЕСТЕ со своим слотом");
}

/// Тест 214 (Р.33 С1, требование пользователя: «не хватает мастер-галки
/// отключить текущий fx-chain, в морферах и fx batch аналогично, всё
/// унифицируется»). Выключатель гасит цепочку ЦЕЛИКОМ, не трогая
/// состояние слотов, и не пишется в файл, пока включён.
#[test]
fn t214_master_switch_disables_chain_without_losing_it() {
    let mut m = model_with_clip();
    let auto = vec![vec![ParamCurve {
        param: "radius".into(),
        ease: false,
        keys: vec![key(0, 1.0), key(90_000, 48.0)],
        source: Default::default(),
    }]];
    m.apply(Command::SetFxAuto { id: ClipId(1), auto: auto.clone() }).unwrap();
    m.apply(Command::SetFxBypass { id: ClipId(1), bypass: vec![false, true] }).unwrap();
    let before = m.clip(ClipId(1)).unwrap().clone();
    assert_eq!(before.active_fx().len(), 1, "второй слот выключен bypass'ом");

    // мастер-выключатель: движок не видит НИЧЕГО
    m.apply(Command::SetFxOff { id: ClipId(1), off: true }).unwrap();
    let c = m.clip(ClipId(1)).unwrap();
    assert!(c.active_fx().is_empty(), "выключенная цепочка не доезжает до движка");
    assert!(c.active_fx_auto().is_empty(), "кривые выключенной цепочки тоже");
    // но сама цепочка, bypass и кривые целы
    assert_eq!(c.fx, before.fx, "цепочка обязана уцелеть");
    assert_eq!(c.fx_bypass, before.fx_bypass, "состояние слотов не трогается");
    assert_eq!(c.fx_auto, before.fx_auto, "кривые не трогаются");

    // обратно — как было; undo работает
    m.apply(Command::SetFxOff { id: ClipId(1), off: false }).unwrap();
    assert_eq!(m.clip(ClipId(1)).unwrap().active_fx().len(), 1);
    m.undo().unwrap();
    assert!(m.clip(ClipId(1)).unwrap().fx_off, "undo вернул выключенное состояние");

    // выключенный морфер не применяется к целям, но остаётся на дорожке
    let mut p = m.project().clone();
    p.morph_lanes = 1;
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    // Мастер-выключатель гасит цепочку СОБЫТИЯ, а не слой над ним
    // (Р.35): своя цепочка уезжает, эффект морфера действует.
    let layered = p.effective_clips().remove(0);
    assert_eq!(layered.active_fx().len(), 1, "слой поверх выключенной цепочки события");
    assert!(!layered.fx_off, "и сам выключатель к слою не относится");
    // выключенный морфер не применяется к целям, но остаётся на дорожке
    p.morphers[0].enabled = false;
    let off = p.effective_clips().remove(0);
    assert!(off.active_fx().is_empty(), "выключенный морфер не добавляет эффектов");
    assert_eq!(off.fx, before.fx, "и не трогает цепочку события");
    assert!(off.fx_off, "выключатель события остался выключателем");
    assert_eq!(p.morphers.len(), 1, "и при этом остаётся на дорожке");
}

/// Тест 215 (Р.33 С1, требование пользователя: «Не хватает копипастинга —
/// как отдельных эффектов так и fx-chain. так и автоматизации»).
/// Блоки живут в модели: их же формой (JSON) пользуется системный буфер.
#[test]
fn t215_fx_clipboard_slot_chain_and_envelope() {
    use kt_model::ops;
    let mut m = model_with_clip();
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![key(0, 1.0), key(90_000, 48.0)],
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),
            ease: true,
            keys: vec![key(0, 0.0), key(90_000, 2.0)],
            source: Default::default(),
        }],
    ];
    m.apply(Command::SetFxAuto { id: ClipId(1), auto }).unwrap();
    m.apply(Command::SetFxBypass { id: ClipId(1), bypass: vec![false, true] }).unwrap();
    let src = m.clip(ClipId(1)).unwrap().clone();

    // цель: ДРУГАЯ точка входа — ключи обязаны перевеситься на её материал
    let mut target = src.clone();
    target.id = ClipId(2);
    target.start = Tick(180_000);
    target.source_in = Tick(45_000);
    target.fx = vec![];
    target.fx_auto = vec![];
    target.fx_bypass = vec![];
    let mut p = m.project().clone();
    p.clips.push(target.clone());
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);

    // 1) цепочка целиком: слоты, bypass и кривые переезжают «вход к
    //    входу» — одной функцией, которую зовёт и UI, и пресеты (Р.48)
    let block = ops::FxChainBlock {
        slots: src
            .fx
            .iter()
            .enumerate()
            .map(|(i, f)| ops::FxSlotBlock {
                fx: f.clone(),
                bypass: src.fx_bypass[i],
                curves: src.fx_auto[i].clone(),
            })
            .collect(),
        src_in: src.source_in,
        fx_off: src.fx_off,
    };
    let target = m.clip(ClipId(2)).unwrap().clone();
    let (mut fx, mut bypass, mut auto) =
        (target.fx.clone(), target.fx_bypass.clone(), target.fx_auto.clone());
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &block, target.source_in);
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(2), fx },
        Command::SetFxBypass { id: ClipId(2), bypass },
        Command::SetFxAuto { id: ClipId(2), auto },
    ])
    .unwrap();
    let got = m.clip(ClipId(2)).unwrap().clone();
    assert_eq!(got.fx, src.fx, "цепочка не переехала");
    assert_eq!(got.fx_bypass, src.fx_bypass, "состояние слотов не переехало");
    assert_eq!(
        got.fx_auto[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![45_000, 135_000],
        "ключи обязаны встать на тот же кусок материала цели"
    );
    // undo — один шаг на весь жест
    let depth = m.undo_depth();
    m.undo().unwrap();
    assert!(m.clip(ClipId(2)).unwrap().fx.is_empty(), "undo снял вставленную цепочку");
    assert_eq!(m.undo_depth(), depth - 1, "вставка цепочки — один undo-шаг");
    m.redo().unwrap();

    // 2) ДОПИСЫВАНИЕ: второй вызов кладёт слоты в конец, не затирая
    let target = m.clip(ClipId(2)).unwrap().clone();
    let before = target.fx.len();
    let (mut fx, mut bypass, mut auto) =
        (target.fx.clone(), target.fx_bypass.clone(), target.fx_auto.clone());
    let one = ops::FxChainBlock {
        slots: vec![block.slots[0].clone()],
        src_in: block.src_in,
        fx_off: false,
    };
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &one, target.source_in);
    assert_eq!(fx.len(), before + 1, "слот не добавился");
    assert_eq!(auto.len(), fx.len(), "кривые обязаны идти параллельно слотам");

    // 3) ребейз симметричен: событие → морфер (вход = 0) и обратно
    let to_morph = ops::rebase_curves(&src.fx_auto[0], src.source_in, kt_time::Tick(0));
    assert_eq!(
        to_morph[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000],
        "в домене морфера ключи считаются от точки входа"
    );
    let back_to_clip = ops::rebase_curves(&to_morph, kt_time::Tick(0), Tick(45_000));
    assert_eq!(
        back_to_clip[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![45_000, 135_000],
        "обратный перенос возвращает ключи на материал приёмника"
    );
    // ключ левее точки входа клампится в ноль, а не уходит в минус
    let below = ops::rebase_curves(&src.fx_auto[0], Tick(90_000), Tick(0));
    assert!(below[0].keys.iter().all(|k| k.t.0 >= 0), "ключи не уходят в отрицательное время");

    // 4) блок переживает системный буфер (JSON) без потерь
    let json = serde_json::to_string(&block).unwrap();
    let back: ops::FxChainBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(back, block, "блок обязан переживать круг через системный буфер");
}

/// Тест 248 (Р.37 С3, требование пользователя: «Эффект для аудио/видео
/// можно объединять сразу, а не добавлять по отдельности»). Правило живёт
/// в МОДЕЛИ, а не в окне FX: `SetFx` прилетает ещё из вставки цепочки, из
/// bake морфера, из ридера формата и из kt-cli.
#[test]
fn t248_warp_rate_is_shared_by_the_link_group() {
    use kt_time::Tick;
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    let mk = |id: u64, kind: ClipKind| ClipEvent {
        id: ClipId(id),
        link_group: Some(GroupId(1)),
        kind,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(mk(1, ClipKind::Video));
    p.clips.push(mk(2, ClipKind::Audio));
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    let slot = |m: &TimelineModel, id: u64| -> Option<FxInstance> {
        m.clip(ClipId(id)).unwrap().fx.iter().find(|f| f.warps_time()).cloned()
    };
    let rate_of = |f: &FxInstance| match f {
        FxInstance::SlowedReverb { rate, .. } => *rate,
        _ => unreachable!(),
    };

    // одна добавка кладёт экземпляр на ОБА события пары
    let depth = m.undo_depth();
    m.apply(Command::SetFx {
        id: ClipId(2),
        fx: vec![FxInstance::gain(0.0), FxInstance::slowed_reverb(0.5, 0.2, 85.0)],
    })
    .unwrap();
    assert_eq!(m.undo_depth(), depth + 1, "пара правится ОДНИМ шагом истории");
    let v = slot(&m, 1).expect("видео-экземпляр заведён сам");
    assert_eq!(rate_of(&v), 0.5, "и с той же скоростью");
    assert_eq!(m.clip(ClipId(1)).unwrap().fx.len(), 1, "у видео только он");

    // правка rate у ВИДЕО едет аудио-партнёру
    let mut vfx = m.clip(ClipId(1)).unwrap().fx.clone();
    if let FxInstance::SlowedReverb { rate, .. } = &mut vfx[0] {
        *rate = 0.25;
    }
    m.apply(Command::SetFx { id: ClipId(1), fx: vfx }).unwrap();
    assert_eq!(rate_of(&slot(&m, 2).unwrap()), 0.25, "скорость общая в обе стороны");

    // слот ищется ПО ВИДУ, а не по индексу: у аудио он второй
    assert!(matches!(m.clip(ClipId(2)).unwrap().fx[0], FxInstance::Gain { .. }));

    // кривая rate тоже синхронна, а кривые ДРУГИХ параметров — нет
    let curve = ParamCurve {
        param: "rate".into(),
        ease: false,
        keys: vec![key(0, 1.0), key(90_000, 2.0)],
        source: Default::default(),
    };
    let mix = ParamCurve {
        param: "mix".into(),
        ease: false,
        keys: vec![key(0, 0.9)],
        source: Default::default(),
    };
    m.apply(Command::SetFxAuto { id: ClipId(2), auto: vec![vec![], vec![curve, mix]] }).unwrap();
    let va = &m.clip(ClipId(1)).unwrap().fx_auto[0];
    assert_eq!(va.len(), 1, "к партнёру едет ТОЛЬКО кривая скорости: {va:?}");
    assert_eq!(va[0].param, "rate");
    assert_eq!(va[0].keys.len(), 2);

    // реверб у видео-экземпляра свой и партнёру не навязывается
    let mut vfx = m.clip(ClipId(1)).unwrap().fx.clone();
    if let FxInstance::SlowedReverb { mix, .. } = &mut vfx[0] {
        *mix = 0.0;
    }
    m.apply(Command::SetFx { id: ClipId(1), fx: vfx }).unwrap();
    assert!(
        matches!(slot(&m, 2).unwrap(), FxInstance::SlowedReverb { mix, .. } if mix == 0.2),
        "реверб партнёра не тронут"
    );

    // удаление у одного убирает у обоих
    m.apply(Command::SetFx { id: ClipId(2), fx: vec![FxInstance::gain(0.0)] }).unwrap();
    assert!(slot(&m, 1).is_none(), "снял у одного — ушло у обоих");
    assert!(m.clip(ClipId(1)).unwrap().fx_auto.iter().all(|c| c.is_empty()));

    // РАЗРЫВ ГРУППЫ прекращает синхронизацию
    m.apply(Command::UnlinkClip { id: ClipId(1) }).unwrap();
    m.apply(Command::SetFx { id: ClipId(2), fx: vec![FxInstance::slowed_reverb(0.5, 0.2, 85.0)] })
        .unwrap();
    assert!(slot(&m, 1).is_none(), "развязанный клип больше не подхватывает");
    m.project().validate().unwrap();
}

/// Тест 248b: одиночный клип БЕЗ пары правило не трогает — на этом стоит
/// golden-проект теста 11 (там slowed на аудио-клипе без link-группы).
#[test]
fn t248_lone_clip_is_untouched_by_the_rule() {
    let mut m = model_with_clip();
    let id = m.project().clips[0].id;
    assert!(m.project().clips[0].link_group.is_none());
    m.apply(Command::SetFx { id, fx: vec![FxInstance::slowed_reverb(0.5, 0.2, 85.0)] }).unwrap();
    assert_eq!(m.project().clips.iter().filter(|c| !c.fx.is_empty()).count(), 1);
}

/// Тест 342 (Р.48, п.342): вставка цепочки — ОДНА правда на оба носителя.
///
/// До Р.48 UI дублировал ребейз инлайном в двух местах, а у морфера
/// кривые могли уйти в никуда через тихую ветку `Option`. Теперь вставку
/// делает `append_chain_block`, и её поведение зафиксировано здесь:
/// перенос «вход к входу» в ОБЕ стороны, дописывание в конец, приёмник
/// не выключается чужим `fx_off`.
#[test]
fn t342_append_chain_block_is_one_truth() {
    use kt_model::ops;
    let curves = vec![ParamCurve {
        param: "radius".into(),
        ease: false,
        keys: vec![key(45_000, 1.0), key(135_000, 48.0)],
        source: Default::default(),
    }];
    let block = ops::FxChainBlock {
        slots: vec![ops::FxSlotBlock {
            fx: FxInstance::blur(8),
            bypass: true,
            curves: curves.clone(),
        }],
        // донор — событие с точкой входа 45_000 тиков материала
        src_in: Tick(45_000),
        // мастер-выключатель донора выключен: приёмник это НЕ наследует
        fx_off: true,
    };

    // событие → МОРФЕР: у морфера домен — время блока, вход = 0
    let (mut fx, mut bypass, mut auto) = (vec![], vec![], vec![]);
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &block, Tick(0));
    assert_eq!(fx.len(), 1);
    assert_eq!(bypass, vec![true], "состояние слота обязано переехать");
    assert_eq!(
        auto[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000],
        "в морфере ключи считаются от точки входа донора"
    );

    // МОРФЕР → событие: тот же отступ от входа, но уже в материале цели
    let from_morph = ops::FxChainBlock {
        slots: vec![ops::FxSlotBlock {
            fx: FxInstance::blur(8),
            bypass: false,
            curves: auto[0].clone(),
        }],
        src_in: Tick(0),
        fx_off: false,
    };
    let (mut fx2, mut bypass2, mut auto2) = (vec![], vec![], vec![]);
    ops::append_chain_block(&mut fx2, &mut bypass2, &mut auto2, &from_morph, Tick(30_000));
    assert_eq!(
        auto2[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![30_000, 120_000],
        "обратный перенос ставит ключи на материал приёмника"
    );

    // ДОПИСЫВАНИЕ: второй блок ложится в конец, первый цел
    ops::append_chain_block(&mut fx2, &mut bypass2, &mut auto2, &block, Tick(30_000));
    assert_eq!(fx2.len(), 2, "вставка дописывает, а не заменяет");
    assert_eq!(auto2.len(), 2, "кривые идут параллельно слотам");
    assert_eq!(bypass2, vec![false, true]);

    // приёмник с непустой цепочкой и БЕЗ кривых получает их корректно
    let (mut fx3, mut bypass3, mut auto3) = (vec![FxInstance::gain(-3.0)], vec![], vec![]);
    ops::append_chain_block(&mut fx3, &mut bypass3, &mut auto3, &block, Tick(0));
    assert_eq!(auto3.len(), 2, "пустые кривые доращиваются до длины цепочки");
    assert!(auto3[0].is_empty(), "у прежнего слота кривых не появилось");
    assert!(!auto3[1].is_empty(), "у вставленного — появились");
}
