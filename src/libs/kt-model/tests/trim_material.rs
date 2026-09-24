//! Тест 748c (Р.63 п.2): правый край события упирается в КОНЕЦ МАТЕРИАЛА.
//!
//! До Р.63 `trim_max_end` у незеркального события отдавал `i64::MAX / 4`:
//! край можно было утащить сколько угодно вправо. Ровно так у владельца
//! событие стало длиной 316.967 с при файле 226.268 с — последние 11.5 с
//! материала «звучали» тишиной и считались underrun'ами. Предел ставится
//! ПО МАТЕРИАЛУ: замедленное событие упирается позже, ускоренное раньше.
//! Уже существующие события длиннее материала не укорачиваются сами
//! (правило 20) — они законны, а честную тишину им даёт движок.

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;

fn scene(media_len: Option<Tick>, rate: Option<f64>) -> TimelineModel {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(match media_len {
        Some(d) => MediaRef::with_duration(MediaId(1), "synth:ramp", d),
        None => MediaRef::new(MediaId(1), "synth:ramp"),
    });
    let mut c = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(2 * SEC),
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
    if let Some(r) = rate {
        c.fx = vec![FxInstance::slowed_reverb(r, 0.0, 120.0)];
        c.fx_bypass = vec![false];
    }
    p.clips.push(c);
    p.normalize();
    p.validate().unwrap();
    TimelineModel::new(p)
}

fn end_of(m: &TimelineModel) -> i64 {
    let c = &m.project().clips[0];
    (c.start + c.duration).0
}

#[test]
fn t748c_trim_end_stops_at_the_material_end() {
    // 1) материал 10 с, скорость 1 — дальше 10 с не пускает
    let mut m = scene(Some(Tick(10 * SEC)), None);
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(30 * SEC) }).unwrap();
    assert_eq!(
        end_of(&m),
        10 * SEC,
        "край не упёрся в конец материала (на прежнем коде — 30 с)"
    );
    // внутри материала трим по-прежнему свободен
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(6 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 6 * SEC, "трим внутри материала обязан работать");

    // 2) ЗАМЕДЛЕНИЕ: 10 с материала при rate 0.5 — это 20 с на таймлайне
    let mut m = scene(Some(Tick(10 * SEC)), Some(0.5));
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(60 * SEC) }).unwrap();
    let got = end_of(&m);
    assert!(
        (got - 20 * SEC).abs() <= SEC / 30,
        "у замедленного события предел считается по МАТЕРИАЛУ: ждали ≈20 с, вышло {} с",
        got as f64 / SEC as f64
    );

    // 3) УСКОРЕНИЕ: те же 10 с материала при rate 2 — это 5 с
    let mut m = scene(Some(Tick(10 * SEC)), Some(2.0));
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(60 * SEC) }).unwrap();
    let got = end_of(&m);
    assert!(
        (got - 5 * SEC).abs() <= SEC / 30,
        "у ускоренного события предел ближе: ждали ≈5 с, вышло {} с",
        got as f64 / SEC as f64
    );

    // 4) длина НЕИЗВЕСТНА (генератор, лента, файл ещё не открывали) —
    //    предела нет, как было всегда
    let mut m = scene(None, None);
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(30 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 30 * SEC, "без известной длины предела быть не должно");

    // 5) событие УЖЕ длиннее материала (файл владельца) — законно и
    //    само не укорачивается (правило 20)
    let mut m = scene(Some(Tick(10 * SEC)), None);
    {
        let mut p = m.project().clone();
        p.clips[0].duration = Tick(30 * SEC);
        p.validate().expect("событие длиннее материала — законно");
        m = TimelineModel::new(p);
    }
    assert_eq!(end_of(&m), 30 * SEC);
    // его можно ПОДРЕЗАТЬ (предел не тянет край назад скачком)
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(25 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 25 * SEC, "подрезка события за материалом обязана работать");
    // но растить ДАЛЬШЕ уже нельзя
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(40 * SEC) }).unwrap();
    assert_eq!(end_of(&m), 25 * SEC, "событие за материалом не имеет права расти дальше");
}
