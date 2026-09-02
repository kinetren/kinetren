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
use kt_model::SlowedKeyframe;
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
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
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
        fx,
        opacity: 1.0,
    });
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
        fx: vec![],
        opacity: 1.0,
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
    // SOURCE-домен (п.1): kf.t — позиция в МАТЕРИАЛЕ. Выход обязан следовать
    // профилю: на выходном времени τ звучит rate(S(τ)). Чек-точки выходного
    // времени вычисляются через T(s) = output_time_for_source.
    let dur = 40 * AUDIO_SAMPLE_RATE as i64;
    let automation = vec![
        SlowedKeyframe { t: 0.20, rate: 1.0, mix: 0.0 },
        SlowedKeyframe { t: 0.40, rate: 0.5, mix: 0.0 },
        SlowedKeyframe { t: 0.50, rate: 0.5, mix: 0.0 },
        SlowedKeyframe { t: 0.65, rate: 1.0, mix: 0.0 },
    ];
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        automation,
    }];
    let p = fx_project(fx, "synth:sine:240", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 240.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    // рендер ОДНИМ куском — как офлайн-экспорт (там и была регрессия)
    let mut out = vec![0f32; dur as usize * CH];
    assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);

    let win = AUDIO_SAMPLE_RATE as usize; // окно 1 c
    // выходные позиции чек-точек — через T(s) от source-позиций профиля
    let t_of_s = |s: f64| kt_fx::output_time_for_source(1.0, &automation_rate_profile(), s);
    let checks = [
        (0.10, 1.0),          // материал до первой точки
        (t_of_s(0.30), 0.75), // середина спада (материал s=0.30)
        (t_of_s(0.45), 0.5),  // центр плато (материал s=0.45)
        (0.95, 1.0),          // далеко после последнего kf
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

fn automation_rate_profile() -> Vec<SlowedKeyframe> {
    vec![
        SlowedKeyframe { t: 0.20, rate: 1.0, mix: 0.0 },
        SlowedKeyframe { t: 0.40, rate: 0.5, mix: 0.0 },
        SlowedKeyframe { t: 0.50, rate: 0.5, mix: 0.0 },
        SlowedKeyframe { t: 0.65, rate: 1.0, mix: 0.0 },
    ]
}

fn automation_probe() -> &'static [SlowedKeyframe] {
    use std::sync::OnceLock;
    static A: OnceLock<Vec<SlowedKeyframe>> = OnceLock::new();
    A.get_or_init(|| {
        vec![
            SlowedKeyframe { t: 0.2, rate: 1.0, mix: 0.0 },
            SlowedKeyframe { t: 0.3, rate: 0.35, mix: 0.0 },
            SlowedKeyframe { t: 0.7, rate: 0.6, mix: 0.0 },
            SlowedKeyframe { t: 0.8, rate: 1.0, mix: 0.0 },
        ]
    })
}

#[test]
fn t4_automation_consumes_source_by_integral() {
    // «Оборванный конец»: при duration = src/avg_rate источник обязан быть
    // дочитан до конца (± задержка ресемплера), а не до avg-доли константы.
    let automation = automation_probe().to_vec();
    let avg = kt_fx::avg_rate(1.0, &automation);
    let dur = 20 * AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        automation,
    }];
    let p = fx_project(fx, "synth:ramp", dur);
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

/// Тест 31 (п.1): кейфреймы приклеены к МАТЕРИАЛУ. Добавление РАННЕГО
/// кейфрейма (ускорение) сдвигает выходное время, но не материал, на
/// котором срабатывает поздний кейфрейм замедления. При старой (выходной)
/// семантике материал точки замедления в сценарии B другой — тест падает.
#[test]
fn t31_keyframes_anchor_to_source_material() {
    let dur = 20 * AUDIO_SAMPLE_RATE as i64; // L, кадры
    let profile_a = vec![
        SlowedKeyframe { t: 0.40, rate: 1.0, mix: 0.0 },
        SlowedKeyframe { t: 0.50, rate: 0.5, mix: 0.0 },
        SlowedKeyframe { t: 0.70, rate: 0.5, mix: 0.0 },
        SlowedKeyframe { t: 0.75, rate: 1.0, mix: 0.0 },
    ];
    let mut profile_b = vec![
        SlowedKeyframe { t: 0.10, rate: 1.0, mix: 0.0 },
        SlowedKeyframe { t: 0.18, rate: 2.0, mix: 0.0 },
        SlowedKeyframe { t: 0.26, rate: 2.0, mix: 0.0 },
        SlowedKeyframe { t: 0.34, rate: 1.0, mix: 0.0 },
    ];
    profile_b.extend(profile_a.iter().cloned());

    let render = |automation: Vec<SlowedKeyframe>| -> Vec<f32> {
        let fx = vec![FxInstance::SlowedReverb {
            rate: 1.0,
            mix: 0.0, // ramp не должен загрязняться ревербом
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            automation,
        }];
        let p = fx_project(fx, "synth:ramp", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Ramp);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);
        out
    };
    let out_a = render(profile_a);
    let out_b = render(profile_b);

    // выходной кадр, где играет материал src (ramp кодирует source-кадр)
    let x_of_material = |out: &[f32], src: f64| -> usize {
        let target = (src * 1e-6) as f32;
        out.chunks_exact(CH).position(|fr| fr[0] >= target).expect("материал достижим")
    };
    // фактическая скорость НА материале: сколько источника за окно выхода
    let rate_at_material = |out: &[f32], src: f64| -> f64 {
        let x = x_of_material(out, src);
        let w = AUDIO_SAMPLE_RATE as usize; // 1 c
        let v0 = out[x * CH] as f64;
        let v1 = out[(x + w) * CH] as f64;
        (v1 - v0) / (w as f64 * 1e-6)
    };

    // 1) ЯКОРЬ: материал за кейфреймом s=0.5 замедлен в ОБОИХ сценариях
    let m = 0.55 * dur as f64;
    let ra = rate_at_material(&out_a, m);
    let rb = rate_at_material(&out_b, m);
    assert!((ra - 0.5).abs() < 0.05, "A:材 s=0.55 обязан играть на 0.5, вышло {ra:.3}");
    assert!(
        (rb - 0.5).abs() < 0.05,
        "B: ранний кейфрейм НЕ должен сдвигать материал замедления, вышло {rb:.3}"
    );

    // 2) бамп активен на своём материале
    let r_bump = rate_at_material(&out_b, 0.22 * dur as f64);
    assert!((r_bump - 2.0).abs() < 0.2, "материал s=0.22 ускорен ×2, вышло {r_bump:.3}");

    // 3) ускорение сдвигает ВРЕМЯ достижения материала, не материал
    let xa = x_of_material(&out_a, 0.5 * dur as f64);
    let xb = x_of_material(&out_b, 0.5 * dur as f64);
    let dt_pred = kt_fx::output_time_for_source(1.0, &[], 0.0); // 0 — только для типа
    let _ = dt_pred;
    assert!(xa > xb, "с ранним ускорением материал s=0.5 достигается РАНЬШЕ: xa={xa}, xb={xb}");
    let shift = (xa - xb) as f64 / dur as f64;
    // предсказание: T_A(0.5) − T_B(0.5)
    let ta = {
        let a = vec![
            SlowedKeyframe { t: 0.40, rate: 1.0, mix: 0.0 },
            SlowedKeyframe { t: 0.50, rate: 0.5, mix: 0.0 },
            SlowedKeyframe { t: 0.70, rate: 0.5, mix: 0.0 },
            SlowedKeyframe { t: 0.75, rate: 1.0, mix: 0.0 },
        ];
        kt_fx::output_time_for_source(1.0, &a, 0.5)
    };
    let tb = {
        let mut b = vec![
            SlowedKeyframe { t: 0.10, rate: 1.0, mix: 0.0 },
            SlowedKeyframe { t: 0.18, rate: 2.0, mix: 0.0 },
            SlowedKeyframe { t: 0.26, rate: 2.0, mix: 0.0 },
            SlowedKeyframe { t: 0.34, rate: 1.0, mix: 0.0 },
        ];
        b.push(SlowedKeyframe { t: 0.40, rate: 1.0, mix: 0.0 });
        b.push(SlowedKeyframe { t: 0.50, rate: 0.5, mix: 0.0 });
        b.push(SlowedKeyframe { t: 0.70, rate: 0.5, mix: 0.0 });
        b.push(SlowedKeyframe { t: 0.75, rate: 1.0, mix: 0.0 });
        kt_fx::output_time_for_source(1.0, &b, 0.5)
    };
    let pred = ta - tb;
    assert!(
        (shift - pred).abs() < 8192.0 / dur as f64,
        "сдвиг времени {shift:.5} должен совпадать с предсказанием карты {pred:.5}"
    );
}
