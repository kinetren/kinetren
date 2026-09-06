//! Headless-интерфейс движка: round-trip проектов (M1), офлайн-рендер в
//! wav/ppm (M2–M3), потоковый экспорт (M7). Основа golden-тестов и CI.
//!
//! Медиа-пути вида `synth:<pattern>` (sine:HZ, ramp, dc:V, impulse, silence)
//! резолвятся в SyntheticSource — рендер без FFmpeg. Реальные файлы — с M3.

mod wav;

use kt_engine::{AudioRenderer, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::{Project, ProjectSettings};
use kt_otio::Retained;
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code);
}

fn synth_pattern(path: &str) -> Option<AudioPattern> {
    let rest = path.strip_prefix("synth:")?;
    let mut it = rest.split(':');
    match it.next()? {
        "silence" => Some(AudioPattern::Silence),
        "ramp" => Some(AudioPattern::Ramp),
        "impulse" => Some(AudioPattern::Impulse),
        "sine" => Some(AudioPattern::Sine { hz: it.next()?.parse().ok()? }),
        "dc" => Some(AudioPattern::Dc { value: it.next()?.parse().ok()? }),
        _ => None,
    }
}

fn render_wav(project: &Project, project_path: &str, output: &str) -> i32 {
    let base = std::path::Path::new(project_path)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let mut fetcher = DirectFetcher::new(None);
    for m in &project.media {
        if let Some(pattern) = synth_pattern(&m.path) {
            fetcher.add_source(m.id, Box::new(SyntheticSource::new(Tick(90_000 * 3600), pattern)));
            continue;
        }
        // относительные пути — от файла проекта (как в kt-app)
        let resolved = if std::path::Path::new(&m.path).is_absolute() {
            m.path.clone()
        } else {
            base.join(&m.path).to_string_lossy().to_string()
        };
        // sidecar-БД проекта: пакетный индекс вместо полного скана (тест 34)
        let mut idx_db = kt_media::index::MediaIndexDb::open(
            &std::path::Path::new(project_path).with_extension("ktindex.db"),
        )
        .ok();
        match kt_media::ffmpeg::FfmpegSource::open_indexed(&resolved, idx_db.as_mut()) {
            Ok(src) => fetcher.add_source(m.id, Box::new(src)),
            Err(e) => {
                eprintln!("{resolved}: {e}");
                return 1;
            }
        }
    }
    let end_frame =
        project.clips.iter().map(|c| (c.start + c.duration).to_audio_frames()).max().unwrap_or(0);
    if end_frame == 0 {
        eprintln!("empty project - nothing to render");
        return 1;
    }
    let renderer = AudioRenderer::new(project);
    let mut samples = vec![0f32; end_frame as usize * AUDIO_CHANNELS];
    let stats = renderer.render_frames(&mut fetcher, 0, &mut samples);
    let file = match std::fs::File::create(output) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("create {output}: {e}");
            return 1;
        }
    };
    if let Err(e) = wav::write_wav_f32(
        std::io::BufWriter::new(file),
        AUDIO_SAMPLE_RATE,
        AUDIO_CHANNELS as u16,
        &samples,
    ) {
        eprintln!("write {output}: {e}");
        return 1;
    }
    eprintln!(
        "ok: {} frames ({:.3} s), underruns: {}",
        end_frame,
        end_frame as f64 / AUDIO_SAMPLE_RATE as f64,
        stats.underruns
    );
    0
}

/// Чтение .otio-проекта (Р.24: .ktproj как проект не открывается —
/// только `kt-cli convert`).
fn load_otio(input: &str) -> Result<Project, i32> {
    let text = match std::fs::read_to_string(input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("read {input}: {e}");
            return Err(1);
        }
    };
    let base = std::path::Path::new(input).parent().map(|p| p.to_path_buf());
    match kt_otio::load_project(&text, base.as_deref()) {
        Ok(b) => {
            for n in &b.notices {
                eprintln!("note: {n}");
            }
            Ok(b.project)
        }
        Err(kt_otio::Error::NotOtio) => {
            eprintln!("{input}: legacy .ktproj is import-only — use `kt-cli convert`");
            Err(1)
        }
        Err(e) => {
            eprintln!("{input}: {e}");
            Err(1)
        }
    }
}

fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("version") | None => {
            println!("kt-cli {} (tick rate {} Hz)", kt_version::FULL, kt_time::TICK_HZ);
            0
        }
        Some("new") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: kt-cli new <out.otio>");
                return 2;
            };
            let p = Project::new(ProjectSettings {
                fps: 30,
                width: 1920,
                height: 1080,
                sample_rate: 48000,
            });
            let base = std::path::Path::new(path).parent().map(|d| d.to_path_buf());
            let out = kt_otio::save_project(&p, &Retained::default(), base.as_deref());
            if let Err(e) = std::fs::write(path, out) {
                eprintln!("write {path}: {e}");
                return 1;
            }
            0
        }
        Some("convert") => {
            // Р.24 п.12: единственный путь для старых .ktproj
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli convert <in.ktproj> <out.otio>");
                return 2;
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("read {input}: {e}");
                    return 1;
                }
            };
            let (p, notices) = match kt_otio::import_legacy(&text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{input}: {e}");
                    return 1;
                }
            };
            for n in &notices {
                eprintln!("note: {n}");
            }
            let base = std::path::Path::new(output).parent().map(|d| d.to_path_buf());
            let out = kt_otio::save_project(&p, &Retained::default(), base.as_deref());
            if let Err(e) = std::fs::write(output, out) {
                eprintln!("write {output}: {e}");
                return 1;
            }
            eprintln!("ok: {} media, {} clips, {} transitions", p.media.len(), p.clips.len(), p.transitions.len());
            0
        }
        Some("demo") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: kt-cli demo <out.otio>");
                return 2;
            };
            let p = demo_project();
            let base = std::path::Path::new(path).parent().map(|d| d.to_path_buf());
            let out = kt_otio::save_project(&p, &Retained::default(), base.as_deref());
            if let Err(e) = std::fs::write(path, out) {
                eprintln!("write {path}: {e}");
                return 1;
            }
            0
        }
        Some("roundtrip") => {
            let Some(input) = args.get(1) else {
                eprintln!("usage: kt-cli roundtrip <in.otio> [out.otio]");
                return 2;
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("read {input}: {e}");
                    return 1;
                }
            };
            let base = std::path::Path::new(input).parent().map(|p| p.to_path_buf());
            let bundle = match kt_otio::load_project(&text, base.as_deref()) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{input}: {e}");
                    return 1;
                }
            };
            let project = bundle.project;
            let out = kt_otio::save_project(&project, &bundle.retained, base.as_deref());
            match args.get(2) {
                Some(dst) => {
                    if let Err(e) = std::fs::write(dst, &out) {
                        eprintln!("write {dst}: {e}");
                        return 1;
                    }
                }
                None => print!("{out}"),
            }
            eprintln!(
                "ok: {} media, {} clips, byte-stable: {}",
                project.media.len(),
                project.clips.len(),
                out == text
            );
            0
        }
        Some("render") => {
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli render <in.otio> <out.wav>");
                return 2;
            };
            let project = match load_otio(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            render_wav(&project, input, output)
        }
        Some("export") => {
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli export <in.otio> <out.mp4> [encoder]");
                return 2;
            };
            // третьим аргументом можно закрепить видео-энкодер (иначе
            // автовыбор: nvenc → vaapi → qsv → libx264). Нужен там, где
            // рендер обязан быть воспроизводимым — например в кросс-рендер
            // конформансе (тест 158).
            let encoder = args.get(3).cloned();
            let project = match load_otio(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            type Sources = Vec<(kt_model::MediaId, Box<dyn kt_media::MediaSource>)>;
            let open_all = || -> Result<Sources, String> {
                let base =
                    std::path::Path::new(input).parent().unwrap_or(std::path::Path::new("."));
                let mut out = Vec::new();
                for m in &project.media {
                    let src: Box<dyn kt_media::MediaSource> =
                        if let Some(p) = synth_pattern(&m.path) {
                            Box::new(SyntheticSource::new(Tick(90_000 * 3600), p))
                        } else {
                            let resolved = if std::path::Path::new(&m.path).is_absolute() {
                                m.path.clone()
                            } else {
                                base.join(&m.path).to_string_lossy().to_string()
                            };
                            {
                                let mut idx_db = kt_media::index::MediaIndexDb::open(
                                    &std::path::Path::new(input).with_extension("ktindex.db"),
                                )
                                .ok();
                                Box::new(
                                    kt_media::ffmpeg::FfmpegSource::open_indexed(
                                        &resolved,
                                        idx_db.as_mut(),
                                    )
                                    .map_err(|e| format!("{resolved}: {e}"))?,
                                )
                            }
                        };
                    out.push((m.id, src));
                }
                Ok(out)
            };
            let (audio_sources, video_sources) = match (open_all(), open_all()) {
                (Ok(a), Ok(v)) => (a, v),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("{e}");
                    return 1;
                }
            };
            let opts = kt_engine::ExportOptions {
                force_encoder: encoder,
                ..Default::default()
            };
            // Р.29: рендер на GPU по умолчанию (headless-девайс, тот же
            // код композиции); адаптера нет → CPU с пометкой в stderr,
            // KT_VIDEO_GPU=0 — принудительный CPU
            let video_gpu = if std::env::var("KT_VIDEO_GPU").as_deref() == Ok("0") {
                None
            } else {
                match kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new) {
                    Ok(e) => {
                        use kt_engine::VideoGpuExec as _;
                        eprintln!("video: GPU ({})", e.info().name);
                        Some(e as std::sync::Arc<dyn kt_engine::VideoGpuExec>)
                    }
                    Err(err) => {
                        eprintln!("video: CPU (no GPU: {err})");
                        None
                    }
                }
            };
            match kt_engine::export_mp4_with_backends(
                &project,
                audio_sources,
                video_sources,
                output,
                None,
                &opts,
                &kt_engine::ExportBackends { video_gpu },
                &std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                &std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                |_| {},
            ) {
                Ok(stats) => {
                    eprintln!(
                        "ok: {} video frames, {} audio frames, underruns {}, missing layers {}, encoder {}",
                        stats.video_frames,
                        stats.audio_frames,
                        stats.underruns,
                        stats.missing_video_layers,
                        stats.encoder
                    );
                    0
                }
                Err(e) => {
                    eprintln!("export: {e}");
                    1
                }
            }
        }
        Some(cmd) => {
            eprintln!("unknown command: {cmd} (available: version, new, roundtrip, render, export)");
            2
        }
    }
}

/// Демо-монтаж для скриншотов сайта (порт gen_project из site/screenshots.py —
/// убирает второго писателя формата, Р.24 этап 11): шесть именованных
/// дорожек, метки, регион с loop, FX-цепочки, морфер.
fn demo_project() -> Project {
    use kt_model::*;
    let t = |s: f64| Tick((s * 90_000.0).round() as i64);
    let clip = |id: u64, kind: ClipKind, lane: u32, st: f64, du: f64, si: f64, m: u64,
                gain: f32, fx: Vec<FxInstance>| ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start: t(st),
        duration: t(du),
        source_in: t(si),
        media: MediaId(m),
        gain_db: gain,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    };
    let pan = FxInstance::Pan { mode: PanMode::Fill, pan_x: 0.02, pan_y: -0.03, zoom: 1.18 };
    // тот же ход камеры, что был кейфреймами CamMotion, — теперь кривыми
    // на Pan (Р.34 К2: механизм анимации в программе один)
    let cam = FxInstance::Pan { mode: PanMode::Fill, pan_x: -0.12, pan_y: 0.05, zoom: 1.05 };
    let slowed = FxInstance::SlowedReverb {
        rate: 0.86,
        mix: 0.34,
        reverb_delay_ms: 150.0,
        damp: 0.28,
        decay: 0.86,
        wet_gain: 1.1,
        predelay_ms: 0.0,
        width: 1.0,
        automation: vec![],
    };
    // автоматизация slowed — общими кривыми (Р.33): ключи в тиках
    // МАТЕРИАЛА клипа 25 (source_in = 0, длина 74 с)
    let slowed_auto = |dur: f64| -> Vec<Vec<ParamCurve>> {
        let key = |u: f64, v: f64| AutoKey { t: t(u * dur), v };
        vec![vec![
            ParamCurve {
                param: "rate".into(),
                ease: false,
                keys: vec![
                    key(0.0, 1.0),
                    key(0.28, 0.86),
                    key(0.62, 0.72),
                    key(0.84, 0.9),
                    key(1.0, 0.66),
                ],
            },
            ParamCurve {
                param: "mix".into(),
                ease: false,
                keys: vec![
                    key(0.0, 0.12),
                    key(0.28, 0.34),
                    key(0.62, 0.55),
                    key(0.84, 0.3),
                    key(1.0, 0.62),
                ],
            },
        ]]
    };
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 1920, height: 1080, sample_rate: 48000 });
    for (id, path) in [
        (1u64, "media/drone_coast.mp4"),
        (2, "media/city_night_broll.mp4"),
        (3, "media/interview_camA.mp4"),
        (4, "media/colorbars_plate.mp4"),
        (5, "media/gradient_wash.mp4"),
        (6, "media/music_bed.flac"),
        (7, "media/voiceover_take3.wav"),
        (8, "media/room_tone.wav"),
    ] {
        p.media.push(MediaRef { id: MediaId(id), path: path.into() });
    }
    p.clips = vec![
        clip(1, ClipKind::Video, 0, 0.0, 12.0, 2.0, 1, 0.0, vec![pan.clone()]),
        clip(3, ClipKind::Video, 0, 12.0, 22.0, 3.0, 3, 0.0,
             vec![pan.clone(), FxInstance::noise(0.05)]),
        clip(5, ClipKind::Video, 0, 34.0, 14.0, 20.0, 1, 0.0, vec![cam]),
        clip(7, ClipKind::Video, 0, 48.0, 18.0, 4.0, 2, 0.0, vec![]),
        clip(9, ClipKind::Video, 0, 66.0, 14.0, 2.0, 5, 0.0, vec![]),
        clip(11, ClipKind::Video, 0, 80.0, 20.0, 24.0, 3, 0.0, vec![pan]),
        clip(13, ClipKind::Video, 1, 6.0, 6.0, 1.0, 4, 0.0,
             vec![FxInstance::EventGrid { cols: 3, rows: 2, center: true }]),
        clip(15, ClipKind::Video, 1, 26.0, 8.0, 10.0, 2, 0.0,
             vec![FxInstance::glow(0.72)]),
        clip(17, ClipKind::Video, 1, 55.0, 7.0, 8.0, 5, 0.0,
             vec![FxInstance::vhs(6, 0.5)]),
        clip(19, ClipKind::Video, 2, 88.0, 6.0, 6.0, 4, 0.0,
             vec![FxInstance::Mirror { horizontal: true, vertical: false },
                  FxInstance::edges(42)]),
        clip(21, ClipKind::Audio, 0, 12.0, 22.0, 2.0, 7, -1.5, vec![]),
        clip(23, ClipKind::Audio, 0, 80.0, 20.0, 28.0, 7, -1.5, vec![]),
        {
            let mut c = clip(25, ClipKind::Audio, 1, 0.0, 74.0, 0.0, 6, -8.0, vec![slowed]);
            c.fx_auto = slowed_auto(74.0);
            c
        },
        clip(27, ClipKind::Audio, 1, 74.0, 31.0, 20.0, 6, -10.0, vec![]),
        clip(29, ClipKind::Audio, 2, 0.0, 30.0, 0.0, 8, -18.0, vec![]),
        clip(31, ClipKind::Audio, 2, 45.0, 30.0, 0.0, 8, -18.0, vec![]),
    ];
    // ход камеры демо-клипа — кривыми Pan (было тремя кейфреймами
    // CamMotion; Р.34 К2: механизм анимации один на программу)
    if let Some(c) = p.clips.iter_mut().find(|c| c.id == ClipId(5)) {
        let key = |u: f64, v: f64| AutoKey { t: t(20.0 + u * 14.0), v };
        c.fx_auto = vec![vec![
            ParamCurve {
                param: "pan_x".into(),
                ease: true,
                keys: vec![key(0.0, -0.12), key(0.55, 0.0), key(1.0, 0.14)],
            },
            ParamCurve {
                param: "pan_y".into(),
                ease: true,
                keys: vec![key(0.0, 0.05), key(0.55, 0.0), key(1.0, -0.06)],
            },
            ParamCurve {
                param: "zoom".into(),
                ease: true,
                keys: vec![key(0.0, 1.05), key(0.55, 1.28), key(1.0, 1.45)],
            },
        ]];
    }
    for (at, name) in [
        (0.0, "COLD OPEN"),
        (12.0, "INTERVIEW IN"),
        (34.0, "B-ROLL"),
        (48.0, "NIGHT CITY"),
        (66.0, "MUSIC SWELL"),
        (80.0, "OUTRO"),
    ] {
        p.markers.push(Marker::named(t(at), name));
    }
    // бывший нахлёст music-клипов — теперь честный кроссфейд-переход
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(25)),
        b: Some(ClipId(27)),
        in_offset: t(2.0),
        out_offset: t(2.0),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.region = Some(Region { start: t(34.0), end: t(66.0) });
    p.loop_enabled = true;
    p.video_lanes = 3;
    p.audio_lanes = 3;
    p.morph_lanes = 1;
    p.morphers = vec![MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: t(48.0),
        duration: t(32.0),
        targets: vec![ClipId(7), ClipId(9)],
        kind: MorphKind::EffectBatch {
            video_fx: vec![FxInstance::glow(0.5)],
            audio_fx: vec![],
            video_bypass: vec![],
            audio_bypass: vec![],
        },
        enabled: true,
    }];
    p.ui = Some(UiState {
        view_start: 0.0,
        ticks_per_px: 5_400.0,
        cursor: t(52.0).0,
        bin_free: true,
        bin_layout: vec![
            (MediaId(1), 0.0, 18.0),
            (MediaId(3), 0.0, 114.0),
            (MediaId(2), 0.0, 210.0),
            (MediaId(5), 0.0, 306.0),
            (MediaId(4), 0.0, 402.0),
            (MediaId(6), 0.0, 524.0),
            (MediaId(7), 0.0, 620.0),
            (MediaId(8), 0.0, 716.0),
        ],
        bin_groups: vec![
            BinGroup {
                name: "CAMERA".into(),
                members: vec![MediaId(1), MediaId(3), MediaId(2), MediaId(5), MediaId(4)],
                locked: false,
            },
            BinGroup { name: "SOUND".into(), members: vec![MediaId(6), MediaId(7), MediaId(8)], locked: true },
        ],
        lane_names: vec![
            (LaneKind::Video, 0, "MAIN".into()),
            (LaneKind::Video, 1, "OVERLAY".into()),
            (LaneKind::Video, 2, "PLATES".into()),
            (LaneKind::Audio, 0, "DIALOG".into()),
            (LaneKind::Audio, 1, "MUSIC".into()),
            (LaneKind::Audio, 2, "AMBIENCE".into()),
            (LaneKind::Morph, 0, "BATCH".into()),
        ],
        ..Default::default()
    });
    p.normalize();
    p.validate().expect("demo project is valid");
    p
}
