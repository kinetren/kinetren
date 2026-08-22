//! Headless-интерфейс движка: round-trip проектов (M1), офлайн-рендер в
//! wav/ppm (M2–M3), потоковый экспорт (M7). Основа golden-тестов и CI.

use kt_model::{Project, ProjectSettings};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code);
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
        Some(cmd) => {
            eprintln!("unknown command: {cmd} (доступно: version, new, roundtrip)");
            2
        }
    }
}
