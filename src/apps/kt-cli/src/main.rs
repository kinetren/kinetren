//! Headless-интерфейс движка: round-trip проектов (M1), офлайн-рендер в
//! wav/ppm (M2–M3), потоковый экспорт (M7). Основа golden-тестов и CI.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version") | None => {
            println!("kt-cli {} (tick rate {} Hz)", env!("CARGO_PKG_VERSION"), kt_time::TICK_HZ);
        }
        Some(cmd) => {
            eprintln!("unknown command: {cmd} (доступно: version; roundtrip/render — с M1/M2)");
            std::process::exit(2);
        }
    }
}
