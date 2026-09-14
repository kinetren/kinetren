//! Тесты 4–6 (docs/porting-notes.md) — регрессии на хрипы slowed&reverb.
//!
//! 4 — спектральная чистота варпа: 440 Гц при rate 0.5 → пик на 220 Гц,
//!     паразитные компоненты < −60 dB (ловит и «46.9 Гц треск» (дефект c),
//!     и пропуски burst'ов (дефект a) старого движка);
//! 5 — сглаживание параметров: смена mix посреди воспроизведения не даёт
//!     скачка сэмпл-к-сэмплу (дефект e — зануление буферов в setParams);
//! 6 — хвост реверба живёт за границей клипа и переживает пересборку графа
//!     (дефекты f и «rebuildGraph убивает FX-состояние»).

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

/// п.8.1 (Р.28): рендер с ПОЛНОСТЬЮ доступным источником обязан идти без
/// единого underrun'а — иначе рендер спрашивает материал за пределами
/// собственного окна, а на живом кэше это тишина и растущий счётчик.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun при полностью доступном источнике");
}

const CH: usize = AUDIO_CHANNELS;

fn fx_project(fx: Vec<FxInstance>, pattern_path: &str, dur_frames: i64) -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000, pitch_engine: Default::default() });
    p.media.push(MediaRef { id: MediaId(1), path: pattern_path.into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick::from_audio_frames(dur_frames),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// Кривая скорости в НОВОМ домене (Р.33): точки (доля материала, rate)
/// переводятся в тики материала клипа `0..dur_frames` с `source_in = 0`.
fn rate_curve(points: &[(f64, f64)], dur_frames: i64) -> Vec<Vec<ParamCurve>> {
    vec![vec![ParamCurve {
        param: "rate".into(),
        ease: false,
        keys: points
            .iter()
            .map(|(s, r)| AutoKey {
                t: Tick::from_audio_frames((s * dur_frames as f64).round() as i64),
                v: *r,
            })
            .collect(),
    }]]
}

/// Проект с автоматизацией на единственном слоте.
fn fx_project_auto(
    fx: Vec<FxInstance>,
    auto: Vec<Vec<ParamCurve>>,
    pattern_path: &str,
    dur_frames: i64,
) -> Project {
    let mut p = fx_project(fx, pattern_path, dur_frames);
    p.clips[0].fx_auto = auto;
    p.validate().unwrap();
    p
}

fn direct(pattern: AudioPattern) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
    f
}

/// Амплитудный спектр моно-сигнала (окно Ханна), дБ относительно пика.
fn spectrum_db(samples: &[f32]) -> Vec<f32> {
    use realfft::RealFftPlanner;
    let n = samples.len();
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let mut input: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos();
            s * w
        })
        .collect();
    let mut out = fft.make_output_vec();
    fft.process(&mut input, &mut out).unwrap();
    let mags: Vec<f32> = out.iter().map(|c| c.norm()).collect();
    let peak = mags.iter().cloned().fold(f32::MIN, f32::max).max(1e-12);
    mags.iter().map(|m| 20.0 * (m / peak).max(1e-12).log10()).collect()
}

// ---- Тест 4 -----------------------------------------------------------------

#[test]
fn t4_slowed_halves_pitch_without_artifacts() {
    let dur = 5 * AUDIO_SAMPLE_RATE as i64; // 5 c
    let p = fx_project(vec![FxInstance::slowed_reverb(0.5, 0.0, 140.0)], "synth:sine:440", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let n = 1 << 16; // 65536 сэмплов анализа
    let skip = 8192; // пропускаем переходный процесс ресемплера
    let mut out = vec![0f32; (skip + n) * CH];
    let stats = r.render_frames_with(&mut f, &mut store, 0, &mut out);
    assert_eq!(stats.underruns, 0);

    let mono: Vec<f32> = out[skip * CH..].chunks_exact(CH).map(|fr| fr[0]).collect();
    let db = spectrum_db(&mono);
    let bin_hz = AUDIO_SAMPLE_RATE as f32 / n as f32;
    let peak_bin = db.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
    let peak_hz = peak_bin as f32 * bin_hz;
    assert!(
        (peak_hz - 220.0).abs() < 2.0 * bin_hz,
        "rate 0.5 обязан дать 220 Гц, получили {peak_hz:.2} Гц"
    );
    // Всё вне ±16 бинов (±12 Гц — скирт окна Ханна) — тише −60 dB.
    // Артефакты старого движка лежали далеко за этим окном: треск 46.9 Гц
    // дал бы компоненты на peak±47 Гц (±64 бина), пропуск burst'ов — гребёнку
    // по всей полосе.
    for (bin, &v) in db.iter().enumerate() {
        if (bin as i64 - peak_bin as i64).abs() > 16 && v > -60.0 {
            let hz = bin as f32 * bin_hz;
            panic!("паразитная компонента {v:.1} dB на {hz:.1} Гц (bin {bin})");
        }
    }
}

#[test]
fn t4_warp_consumes_source_at_rate() {
    // rate 0.5 на клипе длиной D требует D/2 кадров источника: ramp в конце
    // окна источника должен дойти ровно до source_in + D*rate (±чанк ресемплера).
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let p = fx_project(vec![FxInstance::slowed_reverb(0.5, 0.0, 140.0)], "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    // последний выходной сэмпл ≈ значение ramp на кадре dur*0.5 источника
    let got = out[(dur as usize - 1) * CH];
    let expect = SyntheticSource::sample_at(AudioPattern::Ramp, dur / 2, 0);
    // допуск = групповая задержка sinc-ресемплера + пара чанков; главное,
    // что нет НАКОПЛЕННОГО дрейфа (старый движок терял burst каждые ~230 мс,
    // что за 4 c дало бы ~17 000 кадров)
    let tolerance = 4096.0 * 1e-6;
    assert!(
        (got - expect).abs() < tolerance,
        "источник потреблялся не с rate 0.5: конец на {got} vs {expect}"
    );
}

// ---- Тест 5 -----------------------------------------------------------------

#[test]
fn t5_param_step_does_not_click() {
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let mk = |mix: f32| {
        fx_project(vec![FxInstance::slowed_reverb(1.0, mix, 140.0)], "synth:sine:220", dur)
    };
    let mut f = direct(AudioPattern::Sine { hz: 220.0 });
    let mut store = FxStateStore::new();

    let half = dur as usize / 2;
    let mut out = vec![0f32; dur as usize * CH];

    // первая половина с mix 0.1
    let p1 = mk(0.1);
    let r1 = AudioRenderer::new(&p1);
    r1.sync_fx(&mut store);
    no_underruns(r1.render_frames_with(&mut f, &mut store, 0, &mut out[..half * CH]));

    // ступенька: mix 0.9 (та же структура цепочки → тот же state, сглаживание)
    let p2 = mk(0.9);
    let r2 = AudioRenderer::new(&p2);
    r2.sync_fx(&mut store);
    no_underruns(r2.render_frames_with(&mut f, &mut store, half as i64, &mut out[half * CH..]));

    // максимальный скачок сэмпл-к-сэмплу вокруг ступеньки не больше, чем
    // естественный наклон синуса с запасом (сглаживание ~20 мс)
    let natural = 2.0 * std::f32::consts::PI * 220.0 / AUDIO_SAMPLE_RATE as f32; // ~0.0288 при ампл. 1
    let window = &out[(half - 2000) * CH..(half + 4000) * CH];
    let mut max_jump = 0f32;
    for w in window.chunks_exact(CH).collect::<Vec<_>>().windows(2) {
        max_jump = max_jump.max((w[1][0] - w[0][0]).abs());
    }
    assert!(
        max_jump < natural * 1.5,
        "ступенька mix дала щелчок: max sample-to-sample jump {max_jump}"
    );
}

// ---- Тест 6 -----------------------------------------------------------------

#[test]
fn t6_reverb_tail_rings_past_clip_end_and_decays() {
    let dur = AUDIO_SAMPLE_RATE as i64; // 1 c
    let p = fx_project(vec![FxInstance::slowed_reverb(1.0, 0.6, 140.0)], "synth:impulse", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Impulse);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let tail = store.get_mut(ClipId(1)).unwrap().tail_frames();
    assert!(tail > 0);
    let total = (dur + tail) as usize;
    let mut out = vec![0f32; total * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));

    // энергия сразу за границей клипа есть (хвост дозванивает)
    let after: f32 = out[dur as usize * CH..(dur as usize + 4800) * CH]
        .iter()
        .map(|s| s.abs())
        .fold(0.0, f32::max);
    assert!(after > 1e-4, "хвост реверба обрезан на границе клипа: {after}");

    // к концу tail затухает ниже −60 dB от пика хвоста
    let tail_last: f32 = out[(total - 4800) * CH..].iter().map(|s| s.abs()).fold(0.0, f32::max);
    assert!(tail_last < after * 0.001, "хвост не затух за tail(): пик {after}, конец {tail_last}");
}

#[test]
fn t6_graph_rebuild_mid_tail_preserves_state() {
    // Правка ДРУГОГО клипа (пересборка AudioRenderer) посреди звучания
    // не должна прерывать/менять реверб (старый rebuildGraph убивал state).
    let dur = AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::slowed_reverb(1.0, 0.6, 140.0)];
    let p1 = fx_project(fx.clone(), "synth:sine:330", dur);

    // эталон: непрерывный рендер одним графом
    let r1 = AudioRenderer::new(&p1);
    let mut store_a = FxStateStore::new();
    r1.sync_fx(&mut store_a);
    let mut fa = direct(AudioPattern::Sine { hz: 330.0 });
    let mut full = vec![0f32; dur as usize * CH];
    no_underruns(r1.render_frames_with(&mut fa, &mut store_a, 0, &mut full));

    // тот же рендер, но посреди — «правка»: добавлен посторонний клип,
    // граф пересобран, store тот же
    let mut store_b = FxStateStore::new();
    r1.sync_fx(&mut store_b);
    let mut fb = direct(AudioPattern::Sine { hz: 330.0 });
    let half = dur as usize / 2;
    let mut split = vec![0f32; dur as usize * CH];
    no_underruns(r1.render_frames_with(&mut fb, &mut store_b, 0, &mut split[..half * CH]));

    let mut p2 = p1.clone();
    p2.media.push(MediaRef { id: MediaId(2), path: "synth:silence".into() });
    p2.clips.push(ClipEvent {
        id: ClipId(99),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 3,
        start: Tick::from_audio_frames(10 * dur),
        duration: Tick::from_audio_frames(dur),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    let r2 = AudioRenderer::new(&p2);
    r2.sync_fx(&mut store_b); // state клипа 1 обязан пережить пересборку
    no_underruns(r2.render_frames_with(&mut fb, &mut store_b, half as i64, &mut split[half * CH..]));

    assert_eq!(full, split, "пересборка графа не должна менять звук (state сохранён)");
}

// ---- Тест 4в: автоматизация rate реально меняет скорость во времени -------
// (регрессия: оценка автоматизации один раз на буфер давала константное
// замедление; поймано на слух пользователем — фикс: по-чанковая оценка)

/// Мгновенная частота сигнала в окне по нулевым пересечениям (канал 0).
fn window_freq_hz(out: &[f32], from_frame: usize, frames: usize) -> f64 {
    let mut crossings = 0u32;
    let mut prev = out[from_frame * CH];
    for f in (from_frame + 1)..(from_frame + frames) {
        let v = out[f * CH];
        if (prev < 0.0 && v >= 0.0) || (prev > 0.0 && v <= 0.0) {
            crossings += 1;
        }
        prev = v;
    }
    crossings as f64 / 2.0 * AUDIO_SAMPLE_RATE as f64 / frames as f64
}

#[test]
fn t4_automation_changes_rate_over_time() {
    // ДОМЕН СОБЫТИЯ (Р.37): kf.t — позиция в материале, но ОСЬ линейна, то
    // есть на выходном времени τ звучит ровно rate(τ). Чек-точки берутся
    // прямо из профиля — тесту больше не нужна карта, которую он сторожит.
    let dur = 40 * AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }];
    let p = fx_project_auto(
        fx,
        rate_curve(&automation_rate_profile(), dur),
        "synth:sine:240",
        dur,
    );
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 240.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    // рендер ОДНИМ куском — как офлайн-экспорт (там и была регрессия)
    let mut out = vec![0f32; dur as usize * CH];
    assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);

    let win = AUDIO_SAMPLE_RATE as usize; // окно 1 c
    // (профиль здесь и в кривой клипа — один и тот же)
    let checks = [
        (0.10, 1.0),  // до первой точки профиля
        (0.30, 0.75), // середина спада
        (0.45, 0.5),  // центр плато
        (0.95, 1.0),  // далеко после последнего kf
    ];
    for (t, expected_rate) in checks {
        let center = (dur as f64 * t) as usize;
        let freq = window_freq_hz(&out, center - win / 2, win);
        let expected = 240.0 * expected_rate;
        assert!(
            (freq - expected).abs() < expected * 0.05,
            "t={t}: частота {freq:.1} Гц, ожидалось ~{expected:.1} Гц — \
             автоматизация обязана менять скорость по профилю"
        );
    }
}

fn automation_rate_profile() -> Vec<(f64, f64)> {
    vec![(0.20, 1.0), (0.40, 0.5), (0.50, 0.5), (0.65, 1.0)]
}

fn automation_probe() -> &'static [(f64, f64)] {
    &[(0.2, 1.0), (0.3, 0.35), (0.7, 0.6), (0.8, 1.0)]
}

#[test]
fn t4_automation_consumes_source_by_integral() {
    // «Оборванный конец»: при duration = src/avg_rate источник обязан быть
    // дочитан до конца (± задержка ресемплера), а не до avg-доли константы.
    let avg = kt_fx::avg_rate(1.0, automation_probe());
    let dur = 20 * AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }];
    let p = fx_project_auto(fx, rate_curve(automation_probe(), dur), "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));

    // пробуем не самый край (последние ~64 сэмпла легально затухают:
    // sinc-ресемплер видит нули за окном источника), а точку чуть раньше
    // плюс несколько промежуточных — интеральный маппинг по всему клипу
    for frac in [0.35f64, 0.55, 0.75, 0.95, 0.995] {
        let x = (dur as f64 * frac) as usize;
        let got = out[x * CH];
        let expect_src = dur as f64 * kt_fx::source_progress(1.0, automation_probe(), frac);
        let expect = SyntheticSource::sample_at(AudioPattern::Ramp, expect_src as i64, 0);
        let tolerance = 4096.0 * 1e-6;
        assert!(
            (got - expect).abs() < tolerance,
            "t={frac}: источник на {:.0}, ожидалось {:.0} (интеграл профиля)",
            got as f64 / 1e-6,
            expect_src
        );
    }
    let _ = avg;
}

/// Тест 31 (Р.33; ПЕРЕВЁРНУТ в Р.37 по решению пользователя). Раньше он
/// утверждал обратное: «ранний бамп скорости не двигает МАТЕРИАЛ, на
/// котором срабатывают поздние ключи, но двигает ВРЕМЯ». С Р.37 ось
/// автоматизации — собственный размах события, поэтому верно зеркальное:
/// **ранний бамп не двигает МОМЕНТ срабатывания поздних ключей, но
/// двигает материал, играющий в этот момент.**
///
/// Тест не ослаблен, а перевёрнут: проверок столько же, и добавлена проба
/// в ХВОСТЕ. Проба только на плато (τ=0.55) давала бы ложно-зелёное — там
/// старая и новая семантика совпадают; расходятся они на τ=0.90.
#[test]
fn t31_keyframes_anchor_to_event_time() {
    let dur = 20 * AUDIO_SAMPLE_RATE as i64; // L, кадры
    let profile_a: Vec<(f64, f64)> = vec![(0.40, 1.0), (0.50, 0.5), (0.70, 0.5), (0.75, 1.0)];
    let mut profile_b: Vec<(f64, f64)> =
        vec![(0.10, 1.0), (0.18, 2.0), (0.26, 2.0), (0.34, 1.0)];
    profile_b.extend(profile_a.iter().copied());

    let render = |profile: Vec<(f64, f64)>| -> Vec<f32> {
        let fx = vec![FxInstance::SlowedReverb {
            rate: 1.0,
            mix: 0.0, // ramp не должен загрязняться ревербом
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            width: 1.0,
            frames: Default::default(),
            automation: vec![],
        }];
        let p = fx_project_auto(fx, rate_curve(&profile, dur), "synth:ramp", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Ramp);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);
        out
    };
    let out_a = render(profile_a.clone());
    let out_b = render(profile_b.clone());

    // материал, играющий на ВЫХОДНОМ прогрессе τ (ramp кодирует кадр)
    let material_at_out = |out: &[f32], tau: f64| -> f64 {
        out[((tau * dur as f64) as usize) * CH] as f64 / 1e-6
    };
    // фактическая скорость на ВЫХОДНОМ прогрессе τ (окно 0.5 c)
    let rate_at_out = |out: &[f32], tau: f64| -> f64 {
        let x = (tau * dur as f64) as usize;
        let w = AUDIO_SAMPLE_RATE as usize / 2;
        (out[(x + w) * CH] as f64 - out[x * CH] as f64) / (w as f64 * 1e-6)
    };

    // 1) ЯКОРЬ: на одном и том же выходном времени играет одна и та же
    //    скорость — ранний бамп момент НЕ сдвинул
    for (tau, want) in [(0.55, 0.5), (0.60, 0.5), (0.64, 0.5), (0.85, 1.0), (0.90, 1.0)] {
        let ra = rate_at_out(&out_a, tau);
        let rb = rate_at_out(&out_b, tau);
        assert!(
            (ra - want).abs() < 0.06,
            "A: на τ={tau} обязана играть скорость {want}, вышло {ra:.3}"
        );
        assert!(
            (rb - want).abs() < 0.06,
            "B: ранний кейфрейм НЕ должен сдвигать МОМЕНТ срабатывания, вышло {rb:.3}"
        );
    }

    // 2) бамп активен на СВОЁМ выходном времени, и только у B
    assert!((rate_at_out(&out_b, 0.22) - 2.0).abs() < 0.2, "B: τ=0.22 ускорено ×2");
    assert!((rate_at_out(&out_a, 0.22) - 1.0).abs() < 0.1, "A: τ=0.22 без ускорения");

    // 3) ускорение сдвигает МАТЕРИАЛ, играющий в этот момент, а не момент;
    //    величина сдвига предсказывается картой: S_B(τ) − S_A(τ)
    let ma = material_at_out(&out_a, 0.5);
    let mb = material_at_out(&out_b, 0.5);
    assert!(mb > ma, "с ранним ускорением к τ=0.5 съедено БОЛЬШЕ материала: {mb} vs {ma}");
    let pred = kt_fx::source_progress(1.0, &profile_b, 0.5)
        - kt_fx::source_progress(1.0, &profile_a, 0.5);
    let got = (mb - ma) / dur as f64;
    assert!(
        (got - pred).abs() < 8192.0 / dur as f64,
        "сдвиг материала {got:.5} должен совпадать с предсказанием карты {pred:.5}"
    );

    // 4) к концу события съедено ровно avg_rate материала — на этом стоят
    //    окно источника и префетч
    for (out, prof) in [(&out_a, &profile_a), (&out_b, &profile_b)] {
        let end = material_at_out(out, 0.995) / dur as f64;
        let want = kt_fx::avg_rate(1.0, prof);
        assert!(
            (end - want).abs() < 0.03,
            "потреблено {end:.4} материала, карта обещает {want:.4}"
        );
    }
}

/// Тест 213 (Р.33 С1): АУДИО-параметры автоматизируются общим механизмом.
/// До этого раунда кривые существовали только у видео-FX, а из аудио —
/// лишь собственная механика slowed&reverb; громкость автоматизировать
/// было нечем (требование пользователя: «нужна автоматизация всех
/// параметров всех эффектов»).
#[test]
fn t213_audio_gain_follows_its_curve() {
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::gain(0.0)];
    // кривая громкости: тишина → 0 dB → тишина, ключи в тиках материала
    let auto = vec![vec![ParamCurve {
        param: "db".into(),
        ease: false,
        keys: vec![
            AutoKey { t: Tick::from_audio_frames(0), v: -60.0 },
            AutoKey { t: Tick::from_audio_frames(dur / 2), v: 0.0 },
            AutoKey { t: Tick::from_audio_frames(dur), v: -60.0 },
        ],
    }]];
    let p = fx_project_auto(fx, auto, "synth:sine:440", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));

    let rms = |from: usize, n: usize| -> f64 {
        let s: f64 = out[from * CH..(from + n) * CH].iter().map(|v| (*v as f64) * (*v as f64)).sum();
        (s / (n * CH) as f64).sqrt()
    };
    let win = AUDIO_SAMPLE_RATE as usize / 4;
    let head = rms(win / 2, win);
    let mid = rms(dur as usize / 2 - win / 2, win);
    let tail = rms(dur as usize - win * 2, win);
    assert!(mid > head * 8.0, "громкость обязана расти к середине: {head:.4} → {mid:.4}");
    assert!(mid > tail * 8.0, "и падать к концу: {mid:.4} → {tail:.4}");
    // на самом пике — примерно исходная амплитуда синуса (0 dB): 0.5/√2.
    // Окно узкое: широкое усредняет склон вокруг пика и занижает RMS, а
    // сглаживание параметра (one-pole ~20 мс) чуть отстаёт от кривой —
    // это не дефект, а то, ради чего оно и стоит (без него тут щёлкало бы)
    let peak = rms(dur as usize / 2 - 375, 750);
    assert!(
        (peak - 0.3536).abs() < 0.03,
        "в 0 dB ожидается исходный уровень, вышло {peak:.4}"
    );

    // без кривой звук байт-в-байт прежний (константный путь не тронут)
    let plain = fx_project(vec![FxInstance::gain(0.0)], "synth:sine:440", dur);
    let r2 = AudioRenderer::new(&plain);
    let mut f2 = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store2 = FxStateStore::new();
    r2.sync_fx(&mut store2);
    let mut out2 = vec![0f32; dur as usize * CH];
    no_underruns(r2.render_frames_with(&mut f2, &mut store2, 0, &mut out2));
    let src: Vec<f32> = (0..dur)
        .flat_map(|i| {
            let v = SyntheticSource::sample_at(AudioPattern::Sine { hz: 440.0 }, i, 0);
            [v, v]
        })
        .collect();
    assert_eq!(out2, src, "без кривой громкость не должна трогать сигнал вовсе");
}

/// Кейфреймы slowed&reverb старого образца переезжают в общий механизм
/// (Р.33): собственного домена у них больше нет, но проект пользователя
/// обязан звучать так же.
#[test]
fn t213_legacy_slowed_automation_migrates_to_curves() {
    let dur = 20 * AUDIO_SAMPLE_RATE as i64;
    let mut p = fx_project(
        vec![FxInstance::SlowedReverb {
            rate: 1.0,
            mix: 0.0,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            width: 1.0,
            frames: Default::default(),
            automation: vec![
                SlowedKeyframe { t: 0.2, rate: 1.0, mix: 0.0 },
                SlowedKeyframe { t: 0.8, rate: 0.5, mix: 0.0 },
            ],
        }],
        "synth:ramp",
        dur,
    );
    assert!(kt_model::migrate_legacy_slowed(&mut p.clips[0]), "миграция обязана сработать");
    let c = &p.clips[0];
    // легаси-поле опустело, кривые встали в тиках материала
    assert!(
        matches!(&c.fx[0], FxInstance::SlowedReverb { automation, .. } if automation.is_empty()),
        "старое поле обязано опустеть — иначе оно уедет обратно в файл"
    );
    let rate = c.fx_auto[0].iter().find(|k| k.param == "rate").expect("кривая rate");
    assert_eq!(
        rate.keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![
            (0.2 * c.duration.0 as f64).round() as i64,
            (0.8 * c.duration.0 as f64).round() as i64
        ],
        "доли материала обязаны стать тиками материала"
    );
    assert!(c.fx_auto[0].iter().any(|k| k.param == "mix"), "mix тоже переезжает");
    // и проект играет: профиль скорости продолжает действовать
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    let end_src = out[(dur as usize - 1) * CH] as f64 / 1e-6;
    assert!(
        end_src < dur as f64 * 0.95,
        "замедление обязано остаться: съедено {end_src:.0} из {dur}"
    );
}

/// Тест 229 (Р.33 С5): новые параметры АУДИО-эффектов. Умолчания
/// байт-в-байт держит GOLDEN_AUDIO; здесь — что сдвинутый параметр
/// действительно слышен и слышен правильно.
#[test]
fn t229_new_audio_params() {
    let dur = 2 * AUDIO_SAMPLE_RATE as i64;
    let render = |fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>| -> Vec<f32> {
        let p = fx_project_auto(fx, auto, "synth:sine:440", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Sine { hz: 440.0 });
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let rms = |out: &[f32], ch: usize| -> f64 {
        let mut s = 0.0;
        for f in out.chunks_exact(CH) {
            s += (f[ch] as f64).powi(2);
        }
        (s / (out.len() / CH) as f64).sqrt()
    };

    // ---- Gain.pan: баланс ------------------------------------------------
    let flat = render(vec![FxInstance::gain(0.0)], vec![]);
    assert_eq!(
        flat,
        render(vec![FxInstance::Gain { db: 0.0, pan: 0.0 }], vec![]),
        "баланс 0 — прежний путь байт-в-байт"
    );
    let left = render(vec![FxInstance::Gain { db: 0.0, pan: -1.0 }], vec![]);
    assert!(rms(&left, 1) < 1e-9, "полный сдвиг влево гасит правый канал");
    assert!((rms(&left, 0) - rms(&flat, 0)).abs() < 1e-6, "левый канал не тронут");
    let half = render(vec![FxInstance::Gain { db: 0.0, pan: 0.5 }], vec![]);
    assert!(
        (rms(&half, 0) / rms(&flat, 0) - 0.5).abs() < 0.01,
        "баланс +0.5 обязан вдвое приглушить левый: {}",
        rms(&half, 0) / rms(&flat, 0)
    );

    // ---- Gain.pan автоматизируется тем же механизмом ---------------------
    let curve = vec![vec![ParamCurve {
        param: "pan".into(),
        ease: false,
        keys: vec![
            AutoKey { t: Tick(0), v: -1.0 },
            AutoKey { t: Tick::from_audio_frames(dur), v: 1.0 },
        ],
    }]];
    let swept = render(vec![FxInstance::Gain { db: 0.0, pan: 0.0 }], curve);
    let half_len = swept.len() / 2;
    let (l1, r1) = (rms(&swept[..half_len], 0), rms(&swept[..half_len], 1));
    let (l2, r2) = (rms(&swept[half_len..], 0), rms(&swept[half_len..], 1));
    assert!(l1 > r1 && l2 < r2, "кривая баланса обязана переехать слева направо");

    // ---- SlowedReverb: пред-задержка и ширина ----------------------------
    let rev = |predelay_ms: f32, width: f32| {
        vec![FxInstance::SlowedReverb {
            rate: 1.0,
            mix: 1.0,
            reverb_delay_ms: 85.0,
            damp: 0.2,
            decay: 0.84,
            wet_gain: 1.0,
            predelay_ms,
            width,
            frames: Default::default(),
            automation: vec![],
        }]
    };
    let plain = render(rev(0.0, 1.0), vec![]);
    // хвост слышен ПОСЛЕ конца материала — сравниваем начало отклика
    let onset = |out: &[f32]| -> usize {
        out.chunks_exact(CH).position(|f| f[0].abs() > 0.02).unwrap_or(usize::MAX)
    };
    let delayed = render(rev(100.0, 1.0), vec![]);
    assert!(
        onset(&delayed) > onset(&plain),
        "пред-задержка обязана сдвинуть отклик: {} vs {}",
        onset(&delayed),
        onset(&plain)
    );
    // ширина 0 — каналы хвоста совпадают (моно), 1 — расходятся
    let mono = render(rev(0.0, 0.0), vec![]);
    let diff = |out: &[f32]| -> f64 {
        out.chunks_exact(CH).map(|f| (f[0] - f[1]).abs() as f64).sum::<f64>()
    };
    assert!(
        diff(&mono) < diff(&plain) * 0.5,
        "ширина 0 обязана свести хвост в моно: {} vs {}",
        diff(&mono),
        diff(&plain)
    );
}

/// Тест 246 (Р.37 С1): потребление источника — ИНТЕГРАЛ кривой скорости по
/// ВЫХОДНОМУ времени. Прямая проверка новой семантики, независимая от
/// GOLDEN_AUDIO: раньше эталон был единственным, кто это сторожил.
#[test]
fn t246_consumption_is_the_integral_over_event_time() {
    let dur = 10 * AUDIO_SAMPLE_RATE as i64;
    // разгон 1.0 → 2.0 ровно за событие: интеграл = средняя скорость = 1.5
    let profile: Vec<(f64, f64)> = vec![(0.0, 1.0), (1.0, 2.0)];
    assert!(
        (kt_fx::avg_rate(1.0, &profile) - 1.5).abs() < 1e-15,
        "карта: линейный разгон 1→2 съедает ровно 1.5 длины события"
    );

    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }];
    let p = fx_project_auto(fx, rate_curve(&profile, dur), "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);

    // ramp кодирует номер кадра источника — читаем его прямо с выхода
    let src_at = |tau: f64| out[((tau * dur as f64) as usize) * CH] as f64 / 1e-6;
    for tau in [0.25f64, 0.5, 0.75, 0.99] {
        let want = dur as f64 * kt_fx::source_progress(1.0, &profile, tau);
        let got = src_at(tau);
        assert!(
            (got - want).abs() < 8192.0,
            "τ={tau}: съедено {got:.0} материала, интеграл обещает {want:.0}"
        );
    }
    // квадратичность видна невооружённо: к половине события съедено 0.625
    // длины (парабола), а не 0.75 (что дала бы линейная интерполяция)
    assert!((src_at(0.5) / dur as f64 - 0.625).abs() < 0.01);
}

/// Тест 246b (Р.37 С1): кривая ЛЮБОГО параметра тоже живёт в выходном
/// времени. Раньше `mix`/`gain`/`pan` индексировались по потреблённому
/// материалу, и ранний ключ скорости сдвигал момент их срабатывания —
/// эту половину сторожил только GOLDEN_AUDIO.
#[test]
fn t246_gain_curve_peaks_at_event_middle_under_warp() {
    let dur = 8 * AUDIO_SAMPLE_RATE as i64;
    let t = |frac: f64| Tick::from_audio_frames((dur as f64 * frac) as i64);
    // клип замедлен вдвое; громкость — треугольник с пиком в СЕРЕДИНЕ
    let fx = vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0), FxInstance::gain(0.0)];
    let auto = vec![
        vec![],
        vec![ParamCurve {
            param: "db".into(),
            ease: false,
            keys: vec![
                AutoKey { t: t(0.0), v: -60.0 },
                AutoKey { t: t(0.5), v: 0.0 },
                AutoKey { t: t(1.0), v: -60.0 },
            ],
        }],
    ];
    let p = fx_project_auto(fx, auto, "synth:sine:440", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);

    let win = AUDIO_SAMPLE_RATE as usize / 2;
    let rms = |center: usize| -> f64 {
        let (a, b) = (center.saturating_sub(win / 2), (center + win / 2).min(dur as usize));
        let s: f64 = out[a * CH..b * CH].iter().map(|v| (*v as f64).powi(2)).sum();
        (s / ((b - a) * CH) as f64).sqrt()
    };
    let mid = rms(dur as usize / 2);
    let quarter = rms(dur as usize / 4);
    let three_q = rms(dur as usize * 3 / 4);
    // пик — в СЕРЕДИНЕ СОБЫТИЯ. На прежней семантике ключ «середина
    // материала» приходился на КОНЕЦ события (0.5 материала при 0.5×
    // съедается за всё событие), и здесь была бы монотонно растущая рампа
    assert!(mid > quarter * 3.0, "пик громкости обязан быть в середине: {mid:.4} vs {quarter:.4}");
    assert!(mid > three_q * 3.0, "и спадать к концу: {mid:.4} vs {three_q:.4}");
    assert!(
        (quarter / three_q).ln().abs() < 0.7,
        "треугольник симметричен: {quarter:.5} vs {three_q:.5}"
    );
}

// ---- Тест 320/321 (Р.47, п.325): честный порядок аудио-слотов ---------------

/// Тест 320: обработка идёт В ПОРЯДКЕ ЦЕПОЧКИ. Автоматизированный Gain
/// ПЕРЕД ревербом — это фейд ВХОДА: хвост реверба обязан дозвонить после
/// того, как вход ушёл в ноль. До Р.47 конвейер был прибит (громкость
/// всегда после реверба), и тот же фейд душил ХВОСТ — red-proof: на
/// коммите до узловой цепочки RMS последней четверти ~0.
#[test]
fn t320_slot_order_is_honest_gain_before_reverb_fades_the_input() {
    let dur = 48_000 * 4;
    let p = fx_project_auto(
        vec![
            FxInstance::gain(0.0),
            FxInstance::SlowedReverb {
                rate: 1.0,
                mix: 1.0,
                reverb_delay_ms: 100.0,
                damp: 0.1,
                decay: 0.95,
                wet_gain: 1.0,
                predelay_ms: 0.0,
                width: 1.0,
                frames: Default::default(),
                automation: vec![],
            },
        ],
        vec![
            vec![ParamCurve {
                param: "db".into(),
                ease: false,
                keys: vec![
                    AutoKey { t: Tick(0), v: 0.0 },
                    AutoKey { t: Tick::from_audio_frames(dur / 4), v: -60.0 },
                ],
            }],
            vec![],
        ],
        "synth:sine:330",
        dur,
    );
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 330.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    let rms = |s: &[f32]| -> f64 {
        (s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / s.len() as f64).sqrt()
    };
    let frames = |a: f64, b: f64| {
        ((a * 48_000.0) as usize * CH)..((b * 48_000.0) as usize * CH)
    };
    let head = rms(&out[frames(0.0, 0.5)]);
    // окно сразу после фейда входа (2.4..3.6 c): хвост реверба от громкой
    // первой половины ещё жирный (decay 0.95 ≈ t60 5 c)
    let tail = rms(&out[frames(1.2, 2.4)]);
    assert!(head > 0.03, "start must sound: {head}");
    assert!(
        tail > head * 0.05,
        "reverb tail must ring after the INPUT fade (slot order): tail {tail}, head {head}"
    );
}

/// Тест 321: байт-якоря одно-слотовых цепочек после узлового рефакторинга.
/// [Gain −6] — ровно источник × 10^(−6/20) поэлементно (константный путь —
/// один и тот же множитель, нарезка на чанки не меняет float-умножение).
#[test]
fn t321_single_gain_is_exact_scalar_multiply() {
    let dur = 48_000;
    let dry = {
        let p = fx_project(vec![], "synth:ramp", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Ramp);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let gained = {
        let p = fx_project(vec![FxInstance::gain(-6.0)], "synth:ramp", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Ramp);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let k = 10f32.powf(-6.0 / 20.0);
    for (i, (d, g)) in dry.iter().zip(gained.iter()).enumerate() {
        assert_eq!(*g, d * k, "sample {i}: gain must be one exact multiply");
    }
}

// ---- Тесты 323/324 (Р.47, п.327): пятёрка Ка2 -------------------------------

/// Тест 323: порядок слотов на НЕЛИНЕЙНОСТИ. «Тихо → перегруз» чист (драйв
/// почти не кусает −40 дБ), «перегруз → тихо» — грязный, но тихий: спектры
/// и RMS обязаны различаться в разы. На линейных эффектах порядок слышен
/// только с кривыми (t320) — дисторшн доказывает его на константах.
#[test]
fn t323_slot_order_matters_on_nonlinear_chains() {
    let dur = 48_000;
    let render = |fx: Vec<FxInstance>| -> Vec<f32> {
        let p = fx_project(fx, "synth:sine:440", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Sine { hz: 440.0 });
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let quiet_then_drive =
        render(vec![FxInstance::gain(-40.0), FxInstance::Distortion {
            drive_db: 24.0,
            tone: 1.0,
            mix: 1.0,
        }]);
    let drive_then_quiet =
        render(vec![
            FxInstance::Distortion { drive_db: 24.0, tone: 1.0, mix: 1.0 },
            FxInstance::gain(-40.0),
        ]);
    let rms = |s: &[f32]| -> f64 {
        (s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / s.len() as f64).sqrt()
    };
    let (a, b) = (rms(&quiet_then_drive), rms(&drive_then_quiet));
    assert!(
        a / b > 2.0 || b / a > 2.0,
        "nonlinear chain must depend on slot order: {a} vs {b}"
    );
}

/// Тест 324: спектральные пробы пятёрки Ка2 — формулы делают то, что
/// написано на ручках.
#[test]
fn t324_ka2_spectral_probes() {
    let dur = 48_000;
    let render = |fx: Vec<FxInstance>, hz: f64| -> Vec<f32> {
        let p = fx_project(fx, "synth:sine:1000", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Sine { hz });
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let rms_mono = |out: &[f32]| -> f64 {
        let mono: Vec<f64> =
            out.chunks_exact(CH).map(|f| (f[0] as f64 + f[1] as f64) * 0.5).collect();
        (mono.iter().map(|v| v * v).sum::<f64>() / mono.len() as f64).sqrt()
    };
    let lp = |cutoff: f32| FxInstance::Filter {
        mode: FilterMode::Lowpass,
        cutoff_hz: cutoff,
        resonance: 0.707,
    };

    // LP 800: 200 Гц проходит, 8 кГц глушится >= 40 дБ
    let pass = rms_mono(&render(vec![lp(800.0)], 200.0));
    let dry = rms_mono(&render(vec![], 200.0));
    assert!((pass / dry - 1.0).abs() < 0.1, "LP passband: {pass} vs {dry}");
    let cut = rms_mono(&render(vec![lp(800.0)], 8000.0));
    let dry8 = rms_mono(&render(vec![], 8000.0));
    assert!(cut / dry8 < 0.01, "LP must kill 8 kHz by 40 dB: {}", cut / dry8);

    // EQ: low_gain −12 глушит 100 Гц вдвое-с-лишним, 8 кГц не трогает
    let mut eq = FxInstance::eq();
    eq.set_param("low_gain_db", -12.0);
    let low = rms_mono(&render(vec![eq.clone()], 100.0));
    let dry100 = rms_mono(&render(vec![], 100.0));
    let ratio = low / dry100;
    assert!((ratio - 0.25).abs() < 0.1, "EQ low shelf -12 dB: {ratio}");
    let high = rms_mono(&render(vec![eq], 8000.0));
    assert!((high / dry8 - 1.0).abs() < 0.1, "EQ low shelf must not touch 8 kHz");

    // Bitcrusher bits 3 поднимает шумовой пол на чистом синусе
    let crushed = render(
        vec![FxInstance::Bitcrusher { bits: 3.0, rate_hz: 48_000.0, mix: 1.0 }],
        1000.0,
    );
    let clean = render(vec![], 1000.0);
    let mono: Vec<f32> =
        crushed.chunks_exact(CH).map(|f| (f[0] + f[1]) * 0.5).collect();
    let mono_clean: Vec<f32> =
        clean.chunks_exact(CH).map(|f| (f[0] + f[1]) * 0.5).collect();
    let sp = spectrum_db(&mono[8192..16384]);
    let sp_clean = spectrum_db(&mono_clean[8192..16384]);
    // квант ПЕРИОДИКИ — гребёнка гармоник, не сплошной пол: меряем ПИК
    // в полосе вдали от тона (бин = 48000/8192 ≈ 5.86 Гц; 1000..3000 =
    // 5.9..17.6 кГц — там живут нечётные гармоники кванта)
    let peak = |sp: &[f32]| -> f32 {
        sp[1000..3000].iter().cloned().fold(f32::MIN, f32::max)
    };
    assert!(
        peak(&sp) > peak(&sp_clean) + 60.0,
        "3-bit quantization must spawn harmonics: {} vs {}",
        peak(&sp),
        peak(&sp_clean)
    );

    // StereoWidth 0 — моно: каналы равны; 2 — s-энергия растёт
    let p = fx_project(vec![FxInstance::StereoWidth { width: 0.0 }], "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    for fr in out.chunks_exact(CH) {
        assert!((fr[0] - fr[1]).abs() < 1e-6, "width 0 must be mono");
    }
}
