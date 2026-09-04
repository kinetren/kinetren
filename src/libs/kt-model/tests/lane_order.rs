//! Тест 49 (docs/porting-notes.md, п.6 v1.4): display-порядок дорожек.
//! normalize_lane_order: пусто → блочный V..A..M; дубли/мусор вычищаются,
//! недостающие дописываются. lane_order_insert/remove — shift-aware
//! синхронизация вектора при вставке/удалении дорожки. Serde-roundtrip
//! UiState.lane_order; старые файлы без поля читаются (default пусто).

use kt_model::LaneKind::{Audio as A, Camera as C, Morph as M, Video as V};
use kt_model::*;

#[test]
fn t49_normalize_lane_order() {
    // пусто → блочный дефолт
    assert_eq!(
        normalize_lane_order(&[], 2, 2, 1, 0),
        vec![(V, 0), (V, 1), (A, 0), (A, 1), (M, 0)]
    );
    // произвольный валидный порядок сохраняется
    let mixed = vec![(V, 0), (A, 0), (V, 1), (M, 0), (A, 1)];
    assert_eq!(normalize_lane_order(&mixed, 2, 2, 1, 0), mixed);
    // дубли и несуществующие вычищаются, недостающие дописываются в хвост
    let dirty = vec![(V, 0), (V, 0), (A, 7), (M, 0), (V, 1)];
    assert_eq!(normalize_lane_order(&dirty, 2, 1, 1, 0), vec![(V, 0), (M, 0), (V, 1), (A, 0)]);
    // сжатие числа дорожек выбрасывает хвостовые
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 0), vec![(V, 0), (A, 0)]);
}

#[test]
fn t49_insert_remove_shift_aware() {
    // вставка V-дорожки в позицию 1: (V,1) старая → (V,2), новая (V,1)
    // встаёт прямо над ней, чужие виды не тронуты
    let mut rows = vec![(V, 0), (A, 0), (V, 1), (M, 0)];
    lane_order_insert(&mut rows, V, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (V, 2), (M, 0)]);
    // вставка в конец вида (позиции at нет) — в хвост вектора
    lane_order_insert(&mut rows, A, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (V, 2), (M, 0), (A, 1)]);
    // удаление сдвигает большие lane вниз, строка исчезает
    lane_order_remove(&mut rows, V, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (M, 0), (A, 1)]);
    // нормализация после операций — идемпотентна (вектор уже корректен)
    assert_eq!(normalize_lane_order(&rows, 2, 2, 1, 0), rows);
}

#[test]
fn t49_lane_order_serde_roundtrip() {
    let mut p = Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.ui = Some(UiState {
        view_start: 0.0,
        ticks_per_px: 900.0,
        cursor: 0,
        lane_order: vec![(V, 0), (A, 0), (M, 0)],
        ..Default::default()
    });
    let json = serde_json::to_string(&p).unwrap();
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(loaded, p, "round-trip с lane_order");
    // старый файл без lane_order: поле по умолчанию пусто
    let mut p2 = p.clone();
    p2.ui = Some(UiState { view_start: 0.0, ticks_per_px: 900.0, cursor: 0, lane_order: vec![], ..Default::default() });
    let json2 = serde_json::to_string(&p2).unwrap();
    assert!(!json2.contains("lane_order"), "пустой вектор не сериализуется");
    assert_eq!(Project::from_json(&json2).unwrap(), p2);
}

/// Р.29: камерные дорожки — четвёртый вид в display-порядке (хвост блока
/// V..A..M..C), insert/remove работают той же машинерией.
#[test]
fn t188_camera_lanes_in_display_order() {
    assert_eq!(
        normalize_lane_order(&[], 1, 1, 1, 2),
        vec![(V, 0), (A, 0), (M, 0), (C, 0), (C, 1)]
    );
    // произвольный валидный порядок с камерами сохраняется
    let mixed = vec![(C, 0), (V, 0), (A, 0)];
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 1), mixed);
    // несуществующая камера вычищается
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 0), vec![(V, 0), (A, 0)]);
    let mut rows = vec![(V, 0), (C, 0)];
    lane_order_insert(&mut rows, C, 0);
    assert_eq!(rows, vec![(V, 0), (C, 0), (C, 1)]);
    lane_order_remove(&mut rows, C, 0);
    assert_eq!(rows, vec![(V, 0), (C, 0)]);
}
