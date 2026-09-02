//! Тест 179 (п.179 porting-notes): кросс-рендер НАСТОЯЩЕГО фильма.
//!
//! Предложение пользователя: «в интернете я где-то встречал opensource otio
//! файлы общественных фильмов — предлагаю взять какой-то фильм у которого
//! исходники поменьше весят и тоже прорендерить двумя программами».
//!
//! Взят Netflix Open Content «Sol Levante» (CC BY 4.0), аниматик: 63
//! склейки на одной дорожке, 23.976 fps, старт таймкода с часа, один
//! исходник H.264 1080p. Синтетика такого не даёт: настоящий декод с
//! B-кадрами, NTSC-частота и реальные длины планов (7…347 кадров).
//!
//! Ни .otio, ни медиа НЕ коммитятся (320 МБ, у репозитория с .otio нет
//! лицензии): `src/scripts/fetch-oc-media.sh` кладёт их в
//! fixtures/otio-oc/, без них тест молча пропускается.
//! KT_REQUIRE_OC_RENDER=1 превращает пропуск в провал.
//!
//! Сверка не может быть попиксельной: два разных энкодера на настоящей
//! картинке никогда не дадут одинаковых байтов. Поэтому метрика
//! САМОКАЛИБРУЮЩАЯСЯ: кадр нашего рендера обязан быть похож на кадр melt
//! в тот же момент СИЛЬНЕЕ, чем на кадр melt в полутора секундах оттуда.
//! Уехавшая склейка ломает именно это отношение, а разница кодеков — нет.

use kt_media::MediaSource;
use kt_time::Tick;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Сколько секунд монтажа рендерим. Весь фильм — 162 с 1080p в двух
/// энкодерах; для проверки склеек хватает окна, а склеек в нём ~12.
const WINDOW_S: f64 = 30.0;
/// Отступ от склейки при выборе момента сверки (доля плана).
const EDGE: f64 = 0.25;

struct Env {
    python: PathBuf,
    otio: PathBuf,
    scripts: PathBuf,
}

fn env() -> Option<&'static Env> {
    static ENV: OnceLock<Option<Env>> = OnceLock::new();
    ENV.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let python = root.join("target/otio-venv/bin/python3");
        let marker = root.join("target/otio-venv/.kt-ok");
        let dir = root.join("fixtures/otio-oc");
        let otio = dir.join("SolLevante_Animatics_ver1_H264.otio");
        let mov = dir.join("SolLevante_Animatics_ver1_H264.mov");
        if !Command::new("melt").arg("-version").output().is_ok_and(|o| o.status.success()) {
            return fail("melt not found (apt install melt)");
        }
        if !python.exists() || !marker.exists() {
            return fail("no otio venv (run src/scripts/otio-mlt-venv.sh)");
        }
        if !otio.exists() || !mov.exists() {
            return fail("no fixtures/otio-oc (run src/scripts/fetch-oc-media.sh)");
        }
        Some(Env { python, otio, scripts: root.join("scripts") })
    })
    .as_ref()
}

fn fail(why: &str) -> Option<Env> {
    if std::env::var_os("KT_REQUIRE_OC_RENDER").is_some() {
        panic!("open-content cross-render required: {why}");
    }
    eprintln!("skipped: {why}");
    None
}

struct Prep {
    prepared: PathBuf,
    mlt: PathBuf,
    rate: f64,
    duration_s: f64,
    cuts: Vec<f64>,
}

fn prepare(env: &Env, out: &Path) -> Result<Prep, String> {
    let o = Command::new(&env.python)
        .arg(env.scripts.join("oc-prepare.py"))
        .arg(&env.otio)
        .arg(out)
        .arg(WINDOW_S.to_string())
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    let m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap())
            .map_err(|e| e.to_string())?;
    let mlt = m["mlt"]
        .as_str()
        .ok_or_else(|| format!("MLT-адаптер отказался: {}", m["mlt_error"]))?;
    Ok(Prep {
        prepared: PathBuf::from(m["prepared"].as_str().unwrap()),
        mlt: PathBuf::from(mlt),
        rate: m["rate"].as_f64().unwrap(),
        duration_s: m["duration_seconds"].as_f64().unwrap(),
        cuts: m["cuts"].as_array().unwrap().iter().filter_map(|v| v.as_f64()).collect(),
    })
}

fn render_ours(prepared: &Path, out: &Path) -> Result<(), String> {
    let o = Command::new(env!("CARGO_BIN_EXE_kt-cli"))
        .arg("export")
        .arg(prepared)
        .arg(out)
        .arg("libx264")
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(())
}

/// melt по профилю с ДРОБНОЙ частотой: 24000/1001, а не округлённые 24 —
/// иначе чужой рендер сам поедет на 0.1 % и сверка меряла бы наш дрейф
/// вместе с чужим.
fn render_theirs(mlt: &Path, rate: f64, dir: &Path, out: &Path) -> Result<(), String> {
    let (num, den) = ntsc_ratio(rate);
    let profile = dir.join("kt-profile");
    std::fs::write(
        &profile,
        format!(
            "description=kt oc render\nframe_rate_num={num}\nframe_rate_den={den}\n\
             width=1920\nheight=1080\nprogressive=1\nsample_aspect_num=1\n\
             sample_aspect_den=1\ndisplay_aspect_num=16\ndisplay_aspect_den=9\n\
             colorspace=709\n"
        ),
    )
    .map_err(|e| e.to_string())?;
    let o = Command::new("melt")
        .arg("-silent")
        .arg("-profile")
        .arg(&profile)
        .arg(mlt)
        .arg("-consumer")
        .arg(format!("avformat:{}", out.display()))
        .args(["vcodec=libx264", "preset=veryfast", "crf=18", "g=15", "pix_fmt=yuv420p", "an=1", "real_time=0"])
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() || !out.exists() {
        return Err(format!("melt failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(())
}

/// Дробная частота как отношение целых: 23.976→24000/1001, 24→24/1.
fn ntsc_ratio(rate: f64) -> (u32, u32) {
    let r = rate.round();
    if (rate - r).abs() < 1e-6 { (r as u32, 1) } else { ((r * 1000.0) as u32, 1001) }
}

/// Сигнатура кадра: яркость по сетке 16×9. Устойчива к разнице кодеков,
/// но ломается, если показан ДРУГОЙ план.
fn signature(f: &kt_media::VideoFrame) -> Vec<f64> {
    let (w, h) = (f.width as usize, f.height as usize);
    let mut out = vec![0.0; 16 * 9];
    let mut n = vec![0.0; 16 * 9];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            if i + 2 >= f.rgba.len() {
                continue;
            }
            let lum = 0.2126 * f.rgba[i] as f64
                + 0.7152 * f.rgba[i + 1] as f64
                + 0.0722 * f.rgba[i + 2] as f64;
            let cell = (y * 9 / h) * 16 + (x * 16 / w);
            out[cell] += lum;
            n[cell] += 1.0;
        }
    }
    for (v, c) in out.iter_mut().zip(n) {
        if c > 0.0 {
            *v /= c;
        }
    }
    out
}

fn diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f64>() / a.len() as f64
}

fn frame_at(src: &mut dyn MediaSource, t: f64) -> Option<kt_media::VideoFrame> {
    src.read_video_frame(Tick((t * 90_000.0).round() as i64), None).ok()
}

#[test]
fn t179_real_film_cross_render() {
    let Some(env) = env() else { return };
    let dir = std::env::temp_dir().join(format!("kt-oc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let prep = prepare(env, &dir).expect("подготовка настоящего фильма");
    let ours_path = dir.join("ours.mp4");
    let theirs_path = dir.join("theirs.mp4");
    render_ours(&prep.prepared, &ours_path).expect("наш рендер");
    render_theirs(&prep.mlt, prep.rate, &dir, &theirs_path).expect("рендер melt");

    let mut ours =
        kt_media::ffmpeg::FfmpegSource::open(ours_path.to_str().unwrap()).expect("наш рендер");
    let mut theirs =
        kt_media::ffmpeg::FfmpegSource::open(theirs_path.to_str().unwrap()).expect("рендер melt");

    // длительность: сверяем в СЕКУНДАХ — число кадров у нас и у melt разное
    // по построению (наша сетка 24, у melt 24000/1001, п.178)
    let (od, td) = (
        ours.info().duration.0 as f64 / 90_000.0,
        theirs.info().duration.0 as f64 / 90_000.0,
    );
    let frame = 1.0 / prep.rate;
    assert!(
        (od - td).abs() <= 2.0 * frame,
        "длительности разошлись: наш {od:.3} c, melt {td:.3} c (монтаж {:.3} c)",
        prep.duration_s
    );

    // моменты сверки: внутри каждого плана, с отступом от склеек
    let mut probes: Vec<f64> = Vec::new();
    let mut prev = 0.0;
    for &c in &prep.cuts {
        let (a, b) = (prev, c.min(prep.duration_s));
        if b - a > 4.0 * frame {
            probes.push(a + (b - a) * EDGE);
            probes.push(a + (b - a) * (1.0 - EDGE));
        }
        prev = c;
    }
    assert!(probes.len() >= 8, "мало точек сверки: {}", probes.len());

    // Сколько кадров разбега терпим. Наша сетка 24, у melt 24000/1001
    // (п.178): к концу третьей минуты набегает ~4 кадра, на окне 30 с —
    // не больше двух. Больше — это уже уехавшая склейка, а не дрейф.
    const MAX_SHIFT: i64 = 2;

    let mut bad: Vec<String> = Vec::new();
    let mut worst = 0i64;
    for &t in &probes {
        let Some(a) = frame_at(&mut ours, t) else {
            bad.push(format!("{t:.3} c: наш кадр не прочитался"));
            continue;
        };
        let sa = signature(&a);
        // ищем, каким кадром melt наш кадр объясняется ЛУЧШЕ ВСЕГО:
        // смещение в кадрах — это и есть измеренный разбег
        let mut best: Option<(i64, f64)> = None;
        for k in -4i64..=4 {
            let tt = t + k as f64 * frame;
            if tt < 0.0 || tt >= prep.duration_s - frame {
                continue;
            }
            if let Some(f) = frame_at(&mut theirs, tt) {
                let d = diff(&sa, &signature(&f));
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((k, d));
                }
            }
        }
        match best {
            None => bad.push(format!("{t:.3} c: кадры melt не прочитались")),
            Some((k, d)) if d > 24.0 => bad.push(format!(
                "{t:.3} c: ни один кадр melt не похож на наш (лучший {d:.1}/255 при сдвиге {k})"
            )),
            Some((k, _)) if k.abs() > MAX_SHIFT => bad.push(format!(
                "{t:.3} c: наш кадр совпал с melt только при сдвиге {k} кадров — уехала склейка"
            )),
            Some((k, _)) => worst = worst.max(k.abs()),
        }
    }
    assert!(
        bad.is_empty(),
        "точек сверки {}, расхождений {}:\n{}",
        probes.len(),
        bad.len(),
        bad.join("\n")
    );
    eprintln!(
        "t179: {} точек сверки на {:.1} c монтажа — сошлись, худший разбег {worst} кадр(а) \
         (наша сетка {:.0} против {:.3} у melt, п.178)",
        probes.len(),
        prep.duration_s,
        prep.rate.round(),
        prep.rate
    );
}
