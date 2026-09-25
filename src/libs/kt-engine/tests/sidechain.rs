//! Тесты 769-771, 778 (Р.65): ЖИВОЙ САЙДЧЕЙН на дорожке.
//!
//! Дакинг перестал быть снимком кривой: музыка приглушается компрессором,
//! детектор которого слушает дорожку речи. Здесь проверяется движок —
//! порядок стадий, окно ключа, мёртвый и заглушённый ключ, цена ветки.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::LaneKind::Audio as A;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const D: i64 = AUDIO_BURST_FRAMES as i64;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip(id: u64, lane: u32, start_f: i64, dur_f: i64, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick::from_audio_frames(start_f),
        duration: Tick::from_audio_frames(dur_f),
        source_in: Tick::ZERO,
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

fn project(clips: Vec<ClipEvent>) -> Project {
    let mut p = Project::new(settings());
    let mut ids: Vec<MediaId> = clips.iter().map(|c| c.media).collect();
    ids.sort();
    ids.dedup();
    for id in ids {
        p.media.push(MediaRef::new(id, format!("synth:{}", id.0)));
    }
    p.clips = clips;
    p.normalize();
    p.validate().unwrap();
    p
}

fn direct(sources: &[(u64, AudioPattern)]) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for &(id, pat) in sources {
        f.add_source(MediaId(id), Box::new(SyntheticSource::new(Tick(90_000 * 60), pat)));
    }
    f
}

fn stream(p: &Project, f: &mut dyn AudioFetcher, bursts: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::default();
    r.sync_fx(&mut store);
    let mut all = Vec::new();
    let mut buf = vec![0f32; D as usize * CH];
    for i in 0..bursts {
        r.render_frames_with(f, &mut store, i * D, &mut buf);
        all.extend_from_slice(&buf);
    }
    all
}

fn rms(v: &[f32], burst: usize) -> f32 {
    let sl = &v[burst * D as usize * CH..(burst + 1) * D as usize * CH];
    (sl.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / sl.len() as f64).sqrt() as f32
}

/// МУЗЫКА ОТДЕЛЬНО: микс линеен, поэтому вклад речи вычитается тем же
/// рендером без музыкального события. Так «сдвинулся ли дакинг по
/// времени» меряется на самой музыке, а не на сумме, где речь громче.
fn music_only(p: &Project, bursts: i64) -> Vec<f32> {
    let full = stream(p, &mut sources(), bursts);
    let mut sp = p.clone();
    sp.clips.retain(|c| c.lane != 0);
    sp.validate().unwrap();
    let speech = stream(&sp, &mut sources(), bursts);
    full.iter().zip(speech.iter()).map(|(a, b)| a - b).collect()
}

fn ducker(key: Option<TrackId>) -> FxInstance {
    FxInstance::Compressor {
        threshold_db: -30.0,
        ratio: 12.0,
        attack_ms: 3.0,
        release_ms: 10.0,
        makeup_db: 0.0,
        key,
    }
}

/// Сцена: музыка (дорожка 0) звучит непрерывно, речь (дорожка 1) — двумя
/// «репликами» с паузой; на музыке компрессор с ключом на речь.
fn scene(key_on_music: bool) -> (Project, TrackId, TrackId) {
    let clips = vec![
        clip(1, 0, 0, 12 * D, 1),  // музыка: ровный тон весь прогон
        clip(2, 1, D, 2 * D, 2),   // речь: реплика 1
        clip(3, 1, 7 * D, 2 * D, 2), // речь: реплика 2
    ];
    let mut p = project(clips);
    p.ensure_tracks(A, 2);
    let music = p.track_at(A, 0).unwrap().id;
    let speech = p.track_at(A, 1).unwrap().id;
    if key_on_music {
        p.track_at_mut(A, 0).unwrap().fx = vec![ducker(Some(speech))];
    }
    p.validate().unwrap();
    (p, music, speech)
}

fn sources() -> DirectFetcher {
    direct(&[
        (1, AudioPattern::Dc { value: 0.30 }), // музыка тише порога сама по себе
        (2, AudioPattern::Dc { value: 0.90 }), // речь громкая
    ])
}

/// Тест 769 (Р.65): музыка проседает под речью и возвращается между.
///
/// Меряется МИКС против того же микса БЕЗ ключа: речь в обоих одна и та
/// же, значит вся разница — это музыка. Мерить «музыку отдельно» было бы
/// проверкой внутренностей, а не того, что слышно.
#[test]
fn t769_music_ducks_under_speech_live() {
    let (p, _, _) = scene(true);
    let out = stream(&p, &mut sources(), 12);
    let (p_plain, _, _) = scene(false);
    let plain = stream(&p_plain, &mut sources(), 12);

    // ПОД РЕЧЬЮ: та же речь, но микс тише — просела музыка
    for b in [1usize, 2, 7, 8] {
        assert!(
            rms(&out, b) < rms(&plain, b) * 0.9,
            "бёрст {b}: музыка не просела под речью ({} против {})",
            rms(&out, b),
            rms(&plain, b)
        );
    }
    // МЕЖДУ РЕЧЬЮ: вернулась к своему уровню
    for b in [5usize, 6, 11] {
        assert!(
            (rms(&out, b) - rms(&plain, b)).abs() < rms(&plain, b) * 0.05,
            "бёрст {b}: между репликами уровень обязан вернуться ({} против {})",
            rms(&out, b),
            rms(&plain, b)
        );
    }
    // и до первой реплики дакинга ещё нет
    assert!((rms(&out, 0) - rms(&plain, 0)).abs() < 1e-6, "дакинг начался раньше речи");
}

/// Тест 770 (Р.65): порядок стадий по ключу, выравнивание, детерминизм.
#[test]
fn t770_stage_order_alignment_and_determinism() {
    // ключ НИЖЕ цели по рангу (речь — дорожка 1, музыка — 0): порядок
    // обязан определяться зависимостью, а не рангом строки
    let (mut p, _, _speech) = scene(true);
    // у дорожки-ключа СВОЯ цепочка — её выход слышит и микс, и детектор
    p.track_at_mut(A, 1).unwrap().fx = vec![FxInstance::gain(-2.0)];
    p.validate().unwrap();
    let a = stream(&p, &mut sources(), 12);

    // тот же рендер ДРУГИМИ границами чанков — байт-в-байт
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::default();
    r.sync_fx(&mut store);
    let mut f = sources();
    let mut b = Vec::new();
    let mut half = vec![0f32; (D as usize / 2) * CH];
    for i in 0..24 {
        r.render_frames_with(&mut f, &mut store, i * (D / 2), &mut half);
        b.extend_from_slice(&half);
    }
    assert_eq!(a, b, "рендер зависит от границ чанков — это не детерминизм");

    // УПРЕЖДЕНИЕ у цели: лимитер в цепочке музыки сдвигает её окно входа,
    // а ключ обязан приехать на ТО ЖЕ окно — дакинг не «уезжает» по времени.
    // Меряем МУЗЫКУ ОТДЕЛЬНО: на сумме речь громче и просадку маскирует.
    let (mut p2, _, sp2) = scene(false);
    p2.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::limiter(), ducker(Some(sp2))];
    p2.validate().unwrap();
    let with_key = music_only(&p2, 12);
    let (mut p3, _, _) = scene(false);
    // эталон — ТА ЖЕ цепочка без прибора: компрессор с молчащим ключом
    // обязан быть прозрачен, а с ключом-речью — приглушать
    p3.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::limiter()];
    p3.validate().unwrap();
    let no_key = music_only(&p3, 12);
    for b in [2usize, 8] {
        assert!(
            rms(&with_key, b) < rms(&no_key, b) * 0.5,
            "бёрст {b}: с упреждением в цепочке цели дакинг разъехался с речью ({} против {})",
            rms(&with_key, b),
            rms(&no_key, b)
        );
    }
    // …а в паузе ключ молчит, и с ключом звучит ровно то же, что без него
    assert!(
        (rms(&with_key, 6) - rms(&no_key, 6)).abs() < rms(&no_key, 6) * 0.05,
        "в паузе ключ обязан быть прозрачен: {} против {}",
        rms(&with_key, 6),
        rms(&no_key, 6)
    );
}

/// Тест 771 (Р.65): мёртвый и заглушённый ключ.
#[test]
fn t771_dead_and_muted_key() {
    // --- МЁРТВЫЙ КЛЮЧ: дорожки с таким id нет -------------------------
    let (mut p, _, _) = scene(false);
    p.track_at_mut(A, 0).unwrap().fx = vec![ducker(Some(TrackId(9999)))];
    p.validate().unwrap();
    let dead = stream(&p, &mut sources(), 12);
    assert!(dead.iter().all(|s| s.is_finite()), "мёртвый ключ дал не-числа");

    // …и прибор работает ОБЫЧНЫМ компрессором: свой вход выше порога,
    // значит музыка сжата и в ПАУЗЕ тоже (ровно, без дакинга)
    let (mut p_self, _, _) = scene(false);
    p_self.track_at_mut(A, 0).unwrap().fx = vec![ducker(None)];
    p_self.validate().unwrap();
    let selfkey = stream(&p_self, &mut sources(), 12);
    for b in [5usize, 6, 11] {
        assert!(
            (rms(&dead, b) - rms(&selfkey, b)).abs() < 1e-6,
            "бёрст {b}: с мёртвым ключом звук обязан совпасть с обычным компрессором ({} против {})",
            rms(&dead, b),
            rms(&selfkey, b)
        );
    }

    // --- ЗАГЛУШЁННЫЙ КЛЮЧ: дакинга нет -------------------------------
    let (mut p2, _, _) = scene(true);
    p2.track_at_mut(A, 1).unwrap().mute = true;
    p2.validate().unwrap();
    let muted = stream(&p2, &mut sources(), 12);
    let lvl = rms(&muted, 0);
    for b in 1..12usize {
        assert!(
            (rms(&muted, b) - lvl).abs() < lvl * 0.05,
            "бёрст {b}: заглушённый ключ не должен приглушать музыку ({} против {lvl})",
            rms(&muted, b)
        );
    }
}

/// Тест 778 (Р.65): цена ключевой ветки измерена, а не объявлена.
#[test]
fn t778_key_branch_cost_is_measured() {
    let (p_key, _, _) = scene(true);
    let (p_plain, _, _) = scene(false);
    let run = |p: &Project| {
        let t = std::time::Instant::now();
        let _ = stream(p, &mut sources(), 12);
        t.elapsed().as_secs_f64()
    };
    // прогрев (первый прогон тянет декод и аллокации)
    let _ = run(&p_key);
    let _ = run(&p_plain);
    let with = (0..3).map(|_| run(&p_key)).fold(f64::MAX, f64::min);
    let without = (0..3).map(|_| run(&p_plain)).fold(f64::MAX, f64::min);
    // ключевая ветка — это второй прогон дорожки речи: потолок щедрый,
    // утверждение здесь про ПОРЯДОК цены, а не про микросекунды
    assert!(
        with < without * 4.0 + 0.05,
        "ключевая ветка стоит слишком дорого: {with:.4} c против {without:.4} c без ключа"
    );
}
