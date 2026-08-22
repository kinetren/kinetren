//! Headless-интерфейс движка: round-trip проектов (M1), офлайн-рендер в
//! wav/ppm (M2–M3), потоковый экспорт (M7). Основа golden-тестов и CI.
//!
//! Медиа-пути вида `synth:<pattern>` (sine:HZ, ramp, dc:V, impulse, silence)
//! резолвятся в SyntheticSource — рендер без FFmpeg. Реальные файлы — с M3.

mod wav;

use kt_engine::{AudioRenderer, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::{Project, ProjectSettings};
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

fn render_wav(project: &Project, output: &str) -> i32 {
    let mut fetcher = DirectFetcher::new(None);
    for m in &project.media {
        if let Some(pattern) = synth_pattern(&m.path) {
            fetcher.add_source(m.id, Box::new(SyntheticSource::new(Tick(90_000 * 3600), pattern)));
            continue;
        }
        match kt_media::ffmpeg::FfmpegSource::open(&m.path) {
            Ok(src) => fetcher.add_source(m.id, Box::new(src)),
            Err(e) => {
                eprintln!("{}: {e}", m.path);
                return 1;
            }
        }
    }
    let end_frame =
        project.clips.iter().map(|c| (c.start + c.duration).to_audio_frames()).max().unwrap_or(0);
    if end_frame == 0 {
        eprintln!("пустой проект — нечего рендерить");
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
        "ok: {} кадров ({:.3} c), underruns: {}",
        end_frame,
        end_frame as f64 / AUDIO_SAMPLE_RATE as f64,
        stats.underruns
    );
    0
}

fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("version") | None => {
            println!("kt-cli {} (tick rate {} Hz)", env!("CARGO_PKG_VERSION"), kt_time::TICK_HZ);
            0
        }
        Some("new") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: kt-cli new <out.ktproj>");
                return 2;
            };
            let p = Project::new(ProjectSettings {
                fps: 30,
                width: 1920,
                height: 1080,
                sample_rate: 48000,
            });
            if let Err(e) = std::fs::write(path, p.to_json()) {
                eprintln!("write {path}: {e}");
                return 1;
            }
            0
        }
        Some("roundtrip") => {
            let Some(input) = args.get(1) else {
                eprintln!("usage: kt-cli roundtrip <in.ktproj> [out.ktproj]");
                return 2;
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("read {input}: {e}");
                    return 1;
                }
            };
            let project = match Project::from_json(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{input}: {e}");
                    return 1;
                }
            };
            let out = project.to_json();
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
                eprintln!("usage: kt-cli render <in.ktproj> <out.wav>");
                return 2;
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("read {input}: {e}");
                    return 1;
                }
            };
            let project = match Project::from_json(&text) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{input}: {e}");
                    return 1;
                }
            };
            render_wav(&project, output)
        }
        Some(cmd) => {
            eprintln!("unknown command: {cmd} (доступно: version, new, roundtrip, render)");
            2
        }
    }
}
