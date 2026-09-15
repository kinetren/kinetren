//! Headless-интерфейс движка: round-trip проектов (M1), офлайн-рендер в
//! wav/ppm (M2–M3), потоковый экспорт (M7). Основа golden-тестов и CI.
//!
//! Медиа-пути вида `synth:<pattern>` (sine:HZ, ramp, dc:V, impulse, silence)
//! резолвятся в SyntheticSource — рендер без FFmpeg. Реальные файлы — с M3.
//!
//! `probe <проект.otio> <от_с> <до_с>` (Р.43) отвечает на вопрос «что
//! звучит в этом месте»: эффективные события ПОСЛЕ морфер-слоёв, заказ
//! префетча и отдельной строкой чужое медиа в нём. Считается чисто
//! моделью — ни декода, ни FFmpeg, ни звукового железа. По жалобе «тут
//! звук от другого события» запускается ПЕРВЫМ: так найден п.298.

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

/// Что движок читает в диапазоне [from, to) секунд таймлайна.
///
/// Печатает ДВЕ вещи, и расхождение между ними само по себе диагноз:
/// эффективные события (после морфер-слоёв) и заказ префетча — тот самый
/// список `(медиа, бёрсты)`, по которому греется кэш и читается источник.
fn probe_range(
    project: &Project,
    project_path: Option<&str>,
    from: f64,
    to: f64,
    time_with: Option<&str>,
) -> i32 {
    let names: std::collections::HashMap<u64, &str> =
        project.media.iter().map(|m| (m.id.0, m.path.as_str())).collect();
    let short = |id: u64| -> String {
        names
            .get(&id)
            .map(|p| p.rsplit('/').next().unwrap_or(p).chars().take(46).collect())
            .unwrap_or_else(|| format!("<no media {id}>"))
    };
    let t0 = Tick((from * kt_time::TICK_HZ as f64) as i64);
    let t1 = Tick((to * kt_time::TICK_HZ as f64) as i64);
    println!(
        "project: {} clips, {} morphers, {} media",
        project.clips.len(),
        project.morphers.len(),
        project.media.len()
    );
    println!("range {from:.3}..{to:.3} s");

    println!("\n-- effective clips (after morph layers) --");
    let eff = project.effective_clips();
    let mut shown = 0;
    for c in &eff {
        let end = c.start + c.duration;
        if c.start >= t1 || end <= t0 {
            continue;
        }
        shown += 1;
        println!(
            "  {:?} {:?} lane {} {:.3}..{:.3} s  media {} ({})  in {:.3} s{}{}",
            c.id,
            c.kind,
            c.lane,
            c.start.as_secs_f64(),
            end.as_secs_f64(),
            c.media.0,
            short(c.media.0),
            c.source_in.as_secs_f64(),
            if c.reversed { "  REVERSED" } else { "" },
            if c.fx.is_empty() { String::new() } else { format!("  fx {}", c.fx.len()) }
        );
    }
    if shown == 0 {
        println!("  (none)");
    }

    // Что морфер-слои ИЗМЕНИЛИ против файла (Р.43, п.298): подменённый
    // источник и чужой варп — ровно те две беды, которые пользователь
    // слышал как «звук от другого события» и видел как «кривые не
    // работают». Сверяем кусок с его оригиналом по `slice_origin`.
    let orig: std::collections::HashMap<u64, &kt_model::ClipEvent> =
        project.clips.iter().map(|c| (c.id.0, c)).collect();
    let mut changed = Vec::new();
    for c in &eff {
        let end = c.start + c.duration;
        if c.start >= t1 || end <= t0 {
            continue;
        }
        let key = kt_model::slice_origin(c.id).unwrap_or(c.id);
        let Some(o) = orig.get(&key.0) else { continue };
        // сравниваем с НЕПРОПАТЧЕННЫМ куском того же окна, а не с целым
        // событием: резка сама по себе двигает `source_in`, и без этого
        // каждый кусок выглядел бы «изменённым»
        let plain = kt_model::slice_source_in(o, kt_time::TickRange::new(c.start, end));
        let mut what = Vec::new();
        if c.media != o.media {
            what.push(format!("media {} -> {}", o.media.0, c.media.0));
        }
        if c.source_in != plain {
            what.push(format!(
                "in {:.3} -> {:.3} s",
                plain.as_secs_f64(),
                c.source_in.as_secs_f64()
            ));
        }
        let w = |f: &[kt_model::FxInstance]| f.iter().filter(|f| f.warps_time()).count();
        if w(&c.fx) > w(&o.fx) {
            what.push("time warp added by a layer".into());
        }
        if c.fx.len() != o.fx.len() {
            what.push(format!("fx {} -> {}", o.fx.len(), c.fx.len()));
        }
        if !what.is_empty() {
            changed.push(format!("  {:?}: {}", c.id, what.join(", ")));
        }
    }
    println!("\n-- what morph layers changed here --");
    if changed.is_empty() {
        println!("  (nothing)");
    } else {
        for line in &changed {
            println!("{line}");
        }
    }

    // Стоимость сборки движка и число событий с ВАРПОМ (Р.43, п.298):
    // слой со slowed&reverb, дотянувшийся до всего проекта, раздаёт варп
    // каждому событию ниже своей строки — а варп у аудио это ресемплер и
    // пре-ролл на КАЖДОМ событии. Именно так render-поток и вставал.
    let t = std::time::Instant::now();
    let eff_all = project.effective_clips();
    let eff_ms = t.elapsed().as_secs_f64() * 1e3;
    let warped_file = project.clips.iter().filter(|c| c.fx.iter().any(|f| f.warps_time())).count();
    let warped_eff = eff_all.iter().filter(|c| c.fx.iter().any(|f| f.warps_time())).count();
    let t = std::time::Instant::now();
    let built = AudioRenderer::new(project);
    let new_ms = t.elapsed().as_secs_f64() * 1e3;
    // `sync_fx` заводит по процессору на КАЖДОЕ событие с цепочкой, а у
    // варпа внутри sinc-ресемплер: именно на этом вставал render-поток.
    let mut store = kt_engine::FxStateStore::new();
    let t = std::time::Instant::now();
    built.sync_fx(&mut store);
    let sync_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("\n-- build cost --");
    println!("  effective_clips: {} clips in {eff_ms:.0} ms", eff_all.len());
    println!("  AudioRenderer::new: {new_ms:.0} ms");
    println!("  sync_fx (FX processors): {sync_ms:.0} ms");
    println!("  clips with a time warp: {warped_file} in the file, {warped_eff} after layers");

    println!("\n-- prefetch order (what is actually read) --");
    let r = AudioRenderer::new(project);
    let frames = t0.to_audio_frames()..t1.to_audio_frames();
    // это считается на КАЖДОМ проходе префетча, то есть постоянно во
    // время игры — а не один раз при открытии
    let t = std::time::Instant::now();
    let want = r.wanted_audio_bursts(frames.clone());
    let want_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("  wanted_audio_bursts (every prefetch pass): {want_ms:.1} ms");
    if want.is_empty() {
        println!("  (empty)");
    }
    for (media, bursts) in &want {
        let s0 = bursts.start * kt_time::AUDIO_BURST_FRAMES as i64;
        let s1 = bursts.end * kt_time::AUDIO_BURST_FRAMES as i64;
        println!(
            "  media {} ({}): bursts {}..{} = source {:.3}..{:.3} s",
            media.0,
            short(media.0),
            bursts.start,
            bursts.end,
            s0 as f64 / AUDIO_SAMPLE_RATE as f64,
            s1 as f64 / AUDIO_SAMPLE_RATE as f64
        );
    }

    // ЧТО И В КАКОМ РАЗМЕРЕ будет декодировано (Р.46, п.322): ступень
    // декода клипа — функция от цепочки, натива файла и Quality, и когда
    // «зум всё равно мылит», первым делом смотреть сюда
    println!("\n-- video decode order --");
    let q = kt_media::Quality {
        width: project.settings.width.max(2) & !1,
        height: project.settings.height.max(2) & !1,
    };
    let natives = project_path.map(|p| media_natives(project, p)).unwrap_or_default();
    let vr = kt_engine::VideoRenderer::new_with_natives(project, &natives);
    println!("  quality (project): {}x{}", q.width, q.height);
    for (id, dq) in vr.video_decode_qualities(q) {
        let media = eff_all.iter().find(|c| c.id == id).map(|c| c.media.0);
        let nat = media
            .and_then(|m| natives.get(&kt_model::MediaId(m)))
            .map(|(w, h)| format!("{w}x{h}"))
            .unwrap_or_else(|| "unknown".into());
        println!(
            "  clip {} ({}): native {}, stage x{} -> decode {}x{}",
            id.0,
            media.map(short).unwrap_or_else(|| "<none>".into()),
            nat,
            (dq.width / q.width.max(1)).max(1),
            dq.width,
            dq.height
        );
    }
    let mut per_q: std::collections::BTreeMap<(u64, u32, u32), usize> = Default::default();
    for (media, _, dq) in vr.wanted_frames(kt_time::TickRange::new(t0, t1), q) {
        *per_q.entry((media.0, dq.width, dq.height)).or_default() += 1;
    }
    if per_q.is_empty() {
        println!("  (no video frames in this range)");
    }
    for ((media, w, h), n) in per_q {
        println!("  media {} ({}): {n} frames at {w}x{h}", media, short(media));
    }

    // главный вопрос жалобы: не заказано ли медиа, которого в этом месте
    // нет ни у одного события
    let placed: std::collections::BTreeSet<u64> = eff
        .iter()
        .filter(|c| c.start < t1 && c.start + c.duration > t0)
        .map(|c| c.media.0)
        .collect();
    let foreign: Vec<u64> =
        want.iter().map(|(m, _)| m.0).filter(|m| !placed.contains(m)).collect();
    if foreign.is_empty() {
        println!("\nno foreign media in the order");
    } else {
        for m in foreign {
            println!("\nFOREIGN MEDIA IN THE ORDER: {} ({})", m, short(m));
        }
    }

    // --time: УСПЕВАЕТ ЛИ движок отрендерить это место (Р.43, п.301).
    // После п.294 голодание кольца («правый underrun») больше не тормозит
    // часы — оно слышно тишиной, и без числа его не с чем сравнить. Здесь
    // это число: доля реального времени, которую съедает ОДИН аудио-рендер
    // этого места, на прогретом кэше и без UI.
    if let Some(path) = time_with {
        // ЧЕРЕЗ КЭШ, как в программе: без него замер считал бы декод
        // заново на каждый бёрст, а у реверса чтение идёт НАЗАД по файлу —
        // это совсем другой порядок цены и совсем не то, что переживает
        // пользователь на повторе (Р.43, п.302).
        let cache = std::sync::Arc::new(kt_cache::BurstCache::new(kt_cache::CacheConfig::default()));
        let mut fetcher = match open_sources(project, path, false, Some(cache)) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
        let (fa, fb) = (t0.to_audio_frames(), t1.to_audio_frames());
        let mut store = kt_engine::FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; kt_time::AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
        let step = kt_time::AUDIO_BURST_FRAMES as i64;
        // первый круг греет кэш и заводит состояния FX
        let mut pos = fa;
        while pos < fb {
            r.render_frames_with(&mut fetcher, &mut store, pos, &mut out);
            pos += step;
        }
        let start = std::time::Instant::now();
        let mut under = 0u64;
        let mut pos = fa;
        while pos < fb {
            under += r.render_frames_with(&mut fetcher, &mut store, pos, &mut out).underruns;
            pos += step;
        }
        let ms = start.elapsed().as_secs_f64() * 1e3;
        let span_ms = (to - from) * 1000.0;
        println!("\n-- audio render cost (warm cache, no UI) --");
        println!(
            "  linear: {ms:.0} ms for {:.3} s = {:.0}% of real time, underruns {under}",
            to - from,
            ms / span_ms * 100.0
        );

        // ТО ЖЕ МЕСТО НА ПОВТОРЕ: регион играется через `render_looped_with`,
        // и заворот стоит своего (Р.43). Короткий регион заворачивается чаще,
        // поэтому «сколько стоит секунда» у него может отличаться от
        // линейного прогона — именно на это жаловался пользователь
        // («регион больше реверсов — как по маслу, регион внутри события —
        // растут underruns»).
        let laps = 4i64;
        let mut store = kt_engine::FxStateStore::new();
        r.sync_fx(&mut store);
        let mut pos = fa;
        let total = (fb - fa) * laps;
        let mut done = 0i64;
        while done < total.min(fb - fa) {
            r.render_looped_with(&mut fetcher, &mut store, fa..fb, pos, &mut out);
            pos = fa + (pos - fa + step).rem_euclid(fb - fa);
            done += step;
        }
        let start = std::time::Instant::now();
        let mut under_l = 0u64;
        let mut done = 0i64;
        while done < total {
            under_l += r.render_looped_with(&mut fetcher, &mut store, fa..fb, pos, &mut out).underruns;
            pos = fa + (pos - fa + step).rem_euclid(fb - fa);
            done += step;
        }
        let ms_l = start.elapsed().as_secs_f64() * 1e3;
        let span_l = span_ms * laps as f64;
        println!(
            "  looped ({laps} laps): {ms_l:.0} ms for {:.3} s = {:.0}% of real time, underruns {under_l}",
            (to - from) * laps as f64,
            ms_l / span_l * 100.0
        );
        if ms > span_ms || ms_l > span_l {
            println!("  OVER BUDGET: audio alone cannot keep up here");
        }
    }
    0
}

/// Нативные размеры видео-медиа (Р.46, п.322): открывается только
/// КОНТЕЙНЕР (probe), кадры не декодируются. Недоступный файл — не ошибка:
/// его ступень просто неизвестна, и в отчёте это видно.
fn media_natives(
    project: &Project,
    project_path: &str,
) -> std::collections::HashMap<kt_model::MediaId, (u32, u32)> {
    let mut out = std::collections::HashMap::new();
    for m in &project.media {
        if synth_pattern(&m.path).is_some() {
            continue;
        }
        let mut idx_db = kt_media::index::MediaIndexDb::open(
            &std::path::Path::new(project_path).with_extension("ktindex.db"),
        )
        .ok();
        if let Ok(src) = kt_media::ffmpeg::FfmpegSource::open_indexed(&m.path, idx_db.as_mut()) {
            let i = kt_media::MediaSource::info(&src);
            if i.has_video {
                out.insert(m.id, (i.width, i.height));
            }
        }
    }
    out
}

/// Открыть все источники проекта. Общее у офлайн-рендера и `probe --time`.
///
/// `strict` — недоступный файл это ошибка (рендер: молчаливая тишина в
/// выходном файле хуже отказа). Диагностика ходит сюда с `false`: в бине
/// проекта могут лежать медиа, которых на этой машине нет, а замеряемый
/// диапазон их не касается.
fn open_sources(
    project: &Project,
    project_path: &str,
    strict: bool,
    cache: Option<std::sync::Arc<kt_cache::BurstCache>>,
) -> Result<DirectFetcher, String> {
    let mut fetcher = DirectFetcher::new(cache);
    for m in &project.media {
        if let Some(pattern) = synth_pattern(&m.path) {
            fetcher.add_source(m.id, Box::new(SyntheticSource::new(Tick(90_000 * 3600), pattern)));
            continue;
        }
        // путь УЖЕ разрешён при загрузке (kt_otio::load_project резолвит
        // относительные target_url от каталога проекта, п.316) — второй
        // join удваивал префикс и «терял» файл при относительном вызове
        let resolved = m.path.clone();
        let mut idx_db = kt_media::index::MediaIndexDb::open(
            &std::path::Path::new(project_path).with_extension("ktindex.db"),
        )
        .ok();
        match kt_media::ffmpeg::FfmpegSource::open_indexed(&resolved, idx_db.as_mut()) {
            Ok(src) => fetcher.add_source(m.id, Box::new(src)),
            Err(e) if strict => return Err(format!("{resolved}: {e}")),
            Err(e) => eprintln!("  skipped (not on this machine): {resolved}: {e}"),
        }
    }
    Ok(fetcher)
}

fn render_wav(project: &Project, project_path: &str, output: &str, allow_missing: bool) -> i32 {
    // Открытие источников — ОДНО на рендер и диагностику (Р.49, п.359):
    // тут лежала вторая копия того же цикла, и разъехалась она бы молча.
    // `--allow-missing` рендерит тишиной вместо отказа: иногда это ровно
    // то, что нужно (проверить монтаж без чужих медиа), но НЕ по умолчанию.
    let mut fetcher = match open_sources(project, project_path, !allow_missing, None) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
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
    // разрешение перезаписать существующий файл — одно на все подкоманды
    let force = args.iter().any(|a| a == "--force");
    match args.first().map(String::as_str) {
        Some("version") | None => {
            println!("kt-cli {} (tick rate {} Hz)", kt_version::FULL, kt_time::TICK_HZ);
            0
        }
        Some("new") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: kt-cli new <out.otio> [--force]");
                return 2;
            };
            let p = Project::new(ProjectSettings {
                fps: 30,
                width: 1920,
                height: 1080,
                sample_rate: 48000, pitch_engine: Default::default() });
            let base = std::path::Path::new(path).parent().map(|d| d.to_path_buf());
            let out = kt_otio::save_project(&p, &Retained::default(), base.as_deref());
            if let Err(c) = write_project(path, &out, force) {
                return c;
            }
            0
        }
        Some("convert") => {
            // Р.24 п.12: единственный путь для старых .ktproj
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli convert <in.ktproj> <out.otio> [--force]");
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
            if let Err(c) = write_project(output, &out, force) {
                return c;
            }
            eprintln!("ok: {} media, {} clips, {} transitions", p.media.len(), p.clips.len(), p.transitions.len());
            0
        }
        Some("demo") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: kt-cli demo <out.otio> [--files] [--force]");
                return 2;
            };
            // По умолчанию демо ссылается на СИНТЕТИЧЕСКИЕ источники (Р.49,
            // п.359). Восемь путей `media/*` не существуют нигде в поставке —
            // `demo` и `render` подряд не работали ни у кого, кто не собрал
            // эти файлы сам. `synth:` — полноправный источник во всей
            // программе; `--files` возвращает прежние имена.
            let p = demo_project(!args.iter().any(|a| a == "--files"));
            let base = std::path::Path::new(path).parent().map(|d| d.to_path_buf());
            let out = kt_otio::save_project(&p, &Retained::default(), base.as_deref());
            if let Err(c) = write_project(path, &out, force) {
                return c;
            }
            0
        }
        Some("roundtrip") => {
            let Some(input) = args.get(1) else {
                eprintln!("usage: kt-cli roundtrip <in.otio> [out.otio] [--force]");
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
                    if let Err(c) = write_project(dst, &out, force) {
                        return c;
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
        // Диагностика «что звучит в этом месте» (Р.43): по жалобе вида
        // «тут звук от другого события» первым делом надо узнать, какое
        // МЕДИА движок вообще собирается читать в этом диапазоне. Ответ
        // чисто модельный — ни декода, ни FFmpeg, ни звукового железа.
        Some("probe") => {
            let (Some(input), Some(from), Some(to)) = (args.get(1), args.get(2), args.get(3))
            else {
                eprintln!("usage: kt-cli probe <in.otio> <from_s> <to_s> [--time]");
                return 2;
            };
            let (Ok(from), Ok(to)) = (from.parse::<f64>(), to.parse::<f64>()) else {
                eprintln!("from/to are seconds");
                return 2;
            };
            let project = match load_otio(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            let timed = args.iter().any(|a| a == "--time");
            probe_range(&project, Some(input.as_str()), from, to, timed.then_some(input.as_str()))
        }
        Some("render") => {
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli render <in.otio> <out.wav> [--allow-missing]");
                return 2;
            };
            let project = match load_otio(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            render_wav(&project, input, output, args.iter().any(|a| a == "--allow-missing"))
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
                let mut out = Vec::new();
                for m in &project.media {
                    let src: Box<dyn kt_media::MediaSource> =
                        if let Some(p) = synth_pattern(&m.path) {
                            Box::new(SyntheticSource::new(Tick(90_000 * 3600), p))
                        } else {
                            // путь уже разрешён при загрузке (п.316)
                            let resolved = m.path.clone();
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
            eprintln!("unknown command: {cmd} (available: version, new, roundtrip, probe, render, export)");
            2
        }
    }
}

/// Демо-монтаж для скриншотов сайта (порт gen_project из site/screenshots.py —
/// убирает второго писателя формата, Р.24 этап 11): шесть именованных
/// дорожек, метки, регион с loop, FX-цепочки, морфер.
/// Записать проект атомарно, тем же кодом, что и GUI (Р.49, п.359).
/// Голый `fs::write` оставлял обрубок прямо на месте файла, если запись
/// прервалась. Существующий файл не перезаписывается без `--force`: у kt-cli
/// нет ни звёздочки, ни диалога — молча затереть чужой проект тут нечему
/// помешать.
fn write_project(path: &str, text: &str, force: bool) -> Result<(), i32> {
    let p = std::path::Path::new(path);
    warn_if_locked(path);
    if p.exists() && !force {
        eprintln!("{path}: file exists (use --force to overwrite)");
        return Err(1);
    }
    match kt_otio::write_atomic(p, text) {
        Ok(w) => {
            if !w.dir_synced {
                eprintln!("note: {path} written, but the directory could not be synced");
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("write {path}: {e}");
            Err(1)
        }
    }
}

/// Проект занят другой копией программы — предупредить (Р.49, п.359).
/// Запрещать нечего: kt-cli читает, а пишет по явной просьбе.
fn warn_if_locked(path: &str) {
    if !std::path::Path::new(path).exists() {
        return;
    }
    let lock = kt_lock_holder(path);
    if let Some(pid) = lock {
        eprintln!("note: {path} is open in Kinotrain (process {pid}) — it may overwrite changes");
    }
}

/// Номер процесса-держателя замка на проект, если он есть. Формат файла
/// замка — тот же, что кладёт kt-app (`project_lock.rs`): первая строка pid.
fn kt_lock_holder(project: &str) -> Option<u32> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))?;
    let abs = std::path::absolute(project).ok()?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in abs.to_string_lossy().as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    let f = base.join("kinotrain").join("locks").join(format!("{h:016x}.lock"));
    let text = std::fs::read_to_string(f).ok()?;
    text.lines().next()?.trim().parse().ok()
}

fn demo_project(synth: bool) -> Project {
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
        frames: Default::default(),
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
        Project::new(ProjectSettings { fps: 30, width: 1920, height: 1080, sample_rate: 48000, pitch_engine: Default::default() });
    let files = [
        (1u64, "media/drone_coast.mp4"),
        (2, "media/city_night_broll.mp4"),
        (3, "media/interview_camA.mp4"),
        (4, "media/colorbars_plate.mp4"),
        (5, "media/gradient_wash.mp4"),
        (6, "media/music_bed.flac"),
        (7, "media/voiceover_take3.wav"),
        (8, "media/room_tone.wav"),
    ];
    // синтетика подобрана по роли: разные ноты у музыки и голоса слышны
    // по отдельности, комнатный тон — тихая постоянная составляющая
    let synths = [
        (1u64, "synth:ramp"),
        (2, "synth:impulse"),
        (3, "synth:sine:440"),
        (4, "synth:ramp"),
        (5, "synth:dc:0.25"),
        (6, "synth:sine:220"),
        (7, "synth:sine:660"),
        (8, "synth:dc:0.05"),
    ];
    for (id, path) in if synth { synths } else { files } {
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
        auto: vec![],
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
        targets: MorphTargets::Clips { ids: vec![ClipId(7), ClipId(9)] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::glow(0.5)],
            audio_fx: vec![],
            video_bypass: vec![],
            audio_bypass: vec![],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
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
