//! Headless interface of the engine: project round-trip (M1), offline rendering to
//! wav/ppm (M2–M3), streaming export (M7). The basis of golden tests and CI.
//!
//! Media paths of the form `synth:<pattern>` (sine:HZ, ramp, dc:V, impulse, silence)
//! resolve to SyntheticSource — rendering without FFmpeg. Real files — from M3.
//!
//! The project file is its OWN `.kt` format (R.70); OpenTimelineIO is a door in
//! (`import`) and out (`export-otio`), and `roundtrip` checks the door itself.
//!
//! `probe <project.kt> <from_s> <to_s>` (R.43) answers the question “what
//! sounds at this place”: the effective events AFTER the morpher layers, the prefetch
//! order and, on a separate line, the foreign media in it. It is computed purely
//! by the model — no decode, no FFmpeg, no audio hardware. For a complaint “here
//! the sound comes from another event” it is run FIRST: that is how item 298 was found.
//!
//! `ffmpeg` (windows-build) prints the FFmpeg search report — the folder (or SONAME) and
//! version that loaded, and every rejected candidate with its reason — and exits non-zero
//! when nothing loaded.

mod showcase;
mod wav;

use kt_engine::{AudioRenderer, DirectFetcher};
use kt_model::{Project, ProjectSettings};
use kt_otio::Retained;
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code);
}

// Parsing of pseudo-paths (`synth:`, `gen:`) — the SINGLE factory of kt-media
// (R.52). A second, case-SENSITIVE copy of `synth_pattern` lived here, and
// the copies were already drifting apart (footnote of item 211).

/// What the engine reads in the range [from, to) seconds of the timeline.
///
/// It prints TWO things, and a mismatch between them is by itself a diagnosis:
/// the effective events (after the morpher layers) and the prefetch order — that very
/// list of `(media, bursts)` by which the cache warms and the source is read.
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

    // TRACKS (R.62): the first thing to ask about a complaint “the track can’t be
    // heard” and “the picture is lighter than in the preview”. ALL rows are printed,
    // not only the special ones: “the track is in place and untouched by anything” is
    // the same answer as “muted”.
    println!("\n-- tracks --");
    let sil = kt_model::silenced_tracks(project);
    for (kind, lane) in project.rows() {
        let Some(t) = project.track_at(kind, lane) else { continue };
        let mut flags: Vec<String> = Vec::new();
        if t.mute {
            flags.push("MUTE".into());
        }
        if t.solo {
            flags.push("SOLO".into());
        }
        if t.lock {
            flags.push("LOCK".into());
        }
        // “stage” — the cost and the place: for audio this is its own chain on the SUM of
        // events, for video an implicit reel in the plan (the composite of the track
        // as a whole). Its presence explains both the sound and the cost.
        let stage = match kind {
            kt_model::LaneKind::Audio if t.needs_audio_stage() => " stage:audio",
            kt_model::LaneKind::Video if t.needs_video_stage() => " stage:video",
            _ => "",
        };
        let level = match kind {
            kt_model::LaneKind::Audio => format!("{:+.1} dB", t.gain_db),
            kt_model::LaneKind::Video => format!("opacity {:.2}", t.opacity),
            _ => "—".into(),
        };
        let fx = if t.fx.is_empty() {
            String::new()
        } else {
            format!("  fx {}{}", t.fx.len(), if t.fx_off { " (off)" } else { "" })
        };
        println!(
            "  {:<8} {:?} {}  blend {:?}  [{}]{}{}",
            t.label(lane),
            kind,
            level,
            t.blend,
            flags.join(" "),
            fx,
            stage
        );
    }
    // SOLO — on a separate line: “one track is audible” is worked out by it, and not
    // by listing the switches on every row.
    if !sil.solo.is_empty() {
        println!("  solo active: {} — other tracks of these kinds are silent", sil.solo.join(", "));
    }
    if !sil.muted.is_empty() {
        println!("  muted: {} — they will not be in the file either", sil.muted.join(", "));
    }

    // NESTING (R.50): “to understand where the frame comes from, enter N levels” —
    // that is our minus, not a feature. The tree is printed at once and in full.
    if !project.sequences.is_empty() {
        println!("\n-- nested reels --");
        fn print_reel(p: &kt_model::Project, id: kt_model::MediaId, depth: usize, path: &str) {
            let Some(s) = p.sequence(id) else { return };
            let here =
                if path.is_empty() { s.name.clone() } else { format!("{path} / {}", s.name) };
            println!(
                "  {:indent$}{} (media {}) — {} clips, {} places, ends {:.3} s",
                "",
                here,
                id.0,
                s.clips.len(),
                p.sequence_uses(id),
                s.end().as_secs_f64(),
                indent = depth * 2
            );
            // PUBLIC PARAMETERS (R.59, stage 3): the first thing to ask about a complaint
            // “two releases play the same title”.
            for pp in &s.published {
                println!(
                    "  {:indent$}  published {:?} '{}' -> clip {:?} {}",
                    "",
                    pp.id,
                    pp.name,
                    pp.target.clip,
                    pp.target.param,
                    indent = depth * 2
                );
            }
            for c in &s.clips {
                if p.sequence(c.media).is_some() {
                    print_reel(p, c.media, depth + 1, &here);
                }
            }
        }
        for s in &project.sequences {
            // root reels — those that stand directly in the edit
            if project.clips.iter().any(|c| c.media == s.id) {
                print_reel(project, s.id, 0, "");
            }
        }
        let orphan: Vec<&str> = project
            .sequences
            .iter()
            .filter(|s| project.sequence_uses(s.id) == 0)
            .map(|s| s.name.as_str())
            .collect();
        if !orphan.is_empty() {
            println!("  unused: {}", orphan.join(", "));
        }
    }

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
        // SIGNED SPEED (R.57): from a single “warped” one cannot see either the
        // direction, or which piece of material the event occupies —
        // and a complaint “it plays the wrong thing” is worked out precisely by these numbers.
        if let Some(w) = kt_model::clip_warp(c) {
            let (lo, hi) = w.span();
            let rates: Vec<f64> =
                (0..=10).map(|i| kt_model::eff_rate(w.base(), w.prof(), i as f64 / 10.0)).collect();
            let rmin = rates.iter().cloned().fold(f64::INFINITY, f64::min);
            let rmax = rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            println!(
                "      warped: rate {rmin:.2}..{rmax:.2}, material {:.3}..{:.3} s, {:.3} s consumed",
                kt_time::Tick(c.source_in.0 + lo).as_secs_f64(),
                kt_time::Tick(c.source_in.0 + hi).as_secs_f64(),
                kt_time::Tick(w.used()).as_secs_f64(),
            );
        }
        // TAIL BEYOND THE MATERIAL (R.63 item 2): the first thing to ask about a
        // complaint “silence at the end”. The length is not always known (a generator,
        // a reel, a file that is not open) — we stay silent only then.
        if let Some(at) =
            kt_model::material_end_on_timeline(c, project.media_duration(c.media))
        {
            println!(
                "      material ends at {:.3} s (event goes to {:.3} s): \
                 silence and a held frame past it, not starvation",
                kt_time::Tick(at).as_secs_f64(),
                (c.start + c.duration).as_secs_f64(),
            );
        }
    }
    if shown == 0 {
        println!("  (none)");
    }
    // PATH of each event of an attachment: without it, “sound from another event”
    // in a nested edit cannot be parsed (R.50, B6).
    for c in &project.clips {
        let end = c.start + c.duration;
        if c.start >= t1 || end <= t0 {
            continue;
        }
        let Some(s) = project.sequence(c.media) else { continue };
        // The ACTUAL window, not the length on the timeline (R.56, item 448): for a
        // stretched instance the reel is consumed not at the same speed, and
        // a “complaint about another place” is parsed precisely by these numbers.
        // The mirrored one plays the same window IN THE OPPOSITE direction — the bounds are the same.
        // The bounds are the material’s TRAVEL (R.57): with a signed speed
        // “consumed” is more than what has passed, and by it the window would be wider than the
        // real one.
        let warp = kt_model::clip_warp(c);
        let (lo, hi) = match &warp {
            Some(w) => w.span(),
            None => (0, c.duration.0),
        };
        println!(
            "  {:?} is a NESTED REEL '{}' — its content plays at {:.3}..{:.3} s of the reel{}{}",
            c.id,
            s.name,
            kt_time::Tick(c.source_in.0 + lo).as_secs_f64(),
            kt_time::Tick(c.source_in.0 + hi).as_secs_f64(),
            match &warp {
                Some(w) =>
                    format!("  (warped, {:.3} s consumed)", kt_time::Tick(w.used()).as_secs_f64()),
                None => String::new(),
            },
            if c.reversed { "  REVERSED" } else { "" }
        );
        // Overrides of THIS instance — in the same list: “why is there
        // a different text here” is answered by one line.
        for o in &c.overrides {
            let name = s
                .published
                .iter()
                .find(|x| x.id == o.pub_id)
                .map(|x| x.name.as_str())
                .unwrap_or("<dangling>");
            println!("    override {:?} '{}' = {:?}", o.pub_id, name, o.value);
        }
    }

    // What the morpher layers CHANGED against the file (R.43, item 298): a substituted
    // source and a foreign warp — exactly the two troubles that the user
    // heard as “sound from another event” and saw as “the curves don’t
    // work”. We compare the piece with its original by `slice_origin`.
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
        // we compare against the UNPATCHED piece of the same window, not against the whole
        // event: the cut itself moves `source_in`, and without this
        // every piece would look “changed”
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

    // The cost of building the engine and the number of events with a WARP (R.43, item 298):
    // a slowed&reverb layer that reaches the whole project hands the warp to
    // every event below its line — and warp on audio is a resampler and
    // a pre-roll on EVERY event. That is exactly where the render thread used to stall.
    let t = std::time::Instant::now();
    let eff_all = project.effective_clips();
    let eff_ms = t.elapsed().as_secs_f64() * 1e3;
    let warped_file = project.clips.iter().filter(|c| c.fx.iter().any(|f| f.warps_time())).count();
    let warped_eff = eff_all.iter().filter(|c| c.fx.iter().any(|f| f.warps_time())).count();
    let t = std::time::Instant::now();
    let built = AudioRenderer::new(project);
    let new_ms = t.elapsed().as_secs_f64() * 1e3;
    // `sync_fx` creates a processor for EVERY event with a chain, and a
    // warp inside has a sinc resampler: exactly there the render thread stalled.
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
    // this is counted on EVERY prefetch pass, that is, constantly during
    // playback — and not once when opening
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

    // WHAT AND AT WHAT SIZE will be decoded (R.46, item 322): the decode scale step
    // of a clip is a function of the chain, the native size of the file and Quality, and when
    // “zoom still smears” — look here first
    println!("\n-- video decode order --");
    let q = kt_media::Quality {
        width: project.settings.width.max(2) & !1,
        height: project.settings.height.max(2) & !1,
    };
    let facts = project_path.map(|p| media_facts(project, p)).unwrap_or_default();
    let vr = kt_engine::VideoRenderer::new_with_facts(project, &facts);
    println!("  quality (project): {}x{}", q.width, q.height);
    for (id, dq) in vr.video_decode_qualities(q) {
        let media = eff_all.iter().find(|c| c.id == id).map(|c| c.media.0);
        let nat = media
            .and_then(|m| facts.natives.get(&kt_model::MediaId(m)))
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
    // CONFORM RULE (R.62): for a complaint “the picture is stretched/cropped/with
    // margins” look here — the event rule (allowed by the project), the kind of
    // source (native × SAR), the frame on the canvas and the decode size
    println!("\n-- conform (frame rule) --");
    println!("  project rule: {:?}", project.settings.conform);
    for c in eff_all.iter().filter(|c| c.kind == kt_model::ClipKind::Video) {
        if c.start >= t1 || c.start + c.duration <= t0 {
            continue;
        }
        let rule = c.geometry.rule.resolve(project.settings.conform);
        let view = facts.views.get(&c.media).copied();
        let canvas = (q.width as f64, q.height as f64);
        let frame = kt_fx::conform::conform_dest(view, canvas, rule);
        let g = kt_fx::conform::conform_geometry(
            view,
            (q.width, q.height),
            rule,
            [c.geometry.crop_l, c.geometry.crop_t, c.geometry.crop_r, c.geometry.crop_b],
        );
        println!(
            "  conform clip {}: rule={:?} ({:?} on the event) view={} frame={:.0}x{:.0} decode={}x{} node@{}{}",
            c.id.0,
            rule,
            c.geometry.rule,
            view.map(|(w, h)| format!("{w:.0}x{h:.0}")).unwrap_or_else(|| "unknown/stretch".into()),
            frame.0,
            frame.1,
            g.decode.0,
            g.decode.1,
            c.geometry_pos,
            if c.geometry.params_identity() { "" } else { " (node not identity)" }
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

    // the main question of the complaint: is there not a media ordered here that
    // is absent at this place for every event
    let placed: std::collections::BTreeSet<u64> = eff
        .iter()
        .filter(|c| c.start < t1 && c.start + c.duration > t0)
        .map(|c| c.media.0)
        .collect();
    let foreign: Vec<u64> = want.iter().map(|(m, _)| m.0).filter(|m| !placed.contains(m)).collect();
    if foreign.is_empty() {
        println!("\nno foreign media in the order");
    } else {
        for m in foreign {
            println!("\nFOREIGN MEDIA IN THE ORDER: {} ({})", m, short(m));
        }
    }

    // --time: DOES THE ENGINE KEEP UP rendering this place (R.43, item 301).
    // After item 294 the starvation of the ring (“right underrun”) no longer stalls
    // the clock — it is heard as silence, and without a number there is nothing to compare it with. Here
    // this is the number: the share of real time eaten by ONE audio render
    // of this place, on a warm cache and without the UI.
    if let Some(path) = time_with {
        // THROUGH THE CACHE, as in the program: without it the measurement would count
        // decoding anew for every burst, and for reverse the reading goes BACK along the file —
        // that is a completely different cost order and not what the user experiences
        // on repeat (R.43, item 302).
        let cache =
            std::sync::Arc::new(kt_cache::BurstCache::new(kt_cache::CacheConfig::default()));
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
        // the first lap warms the cache and sets up the FX states
        let mut pos = fa;
        while pos < fb {
            r.render_frames_with(&mut fetcher, &mut store, pos, &mut out);
            pos += step;
        }
        let start = std::time::Instant::now();
        let mut under = 0u64;
        let mut warm = 0u64;
        let mut pos = fa;
        while pos < fb {
            let st = r.render_frames_with(&mut fetcher, &mut store, pos, &mut out);
            under += st.underruns;
            warm += st.nest_warmups;
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
        // THE COST OF THE SUB-MIX (R.55, C4): bursts computed for nothing during
        // the pre-roll of the nested reel producer’s move. They are not heard, but
        // by them one sees the cost of reverse and stretch on an attachment — and whether
        // the order window has diverged from what the producer reads (then they
        // grow systematically).
        if warm > 0 {
            println!(
                "  nest submix warmups: {warm} bursts ({:.0} ms of reel time)",
                warm as f64 * kt_time::AUDIO_BURST_FRAMES as f64 / 48.0
            );
        }

        // THE SAME PLACE ON REPEAT: a region is played through `render_looped_with`,
        // and the wrap-around has its own cost (R.43). A short region wraps more often,
        // so “how much a second costs” there may differ from
        // a linear run — it is exactly this that the user complained about
        // (“a region with more reverses — smooth as butter, a region inside an event —
        // the underruns grow”).
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
            under_l +=
                r.render_looped_with(&mut fetcher, &mut store, fa..fb, pos, &mut out).underruns;
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

/// Facts about the project’s sources: native (R.46, item 322) and proceduralness
/// (R.56, item 448). Only the CONTAINER is opened (probe), the frames are not
/// decoded. An unavailable file is not an error: its step is simply
/// unknown, and the report shows this.
fn media_facts(project: &Project, project_path: &str) -> kt_engine::SourceFacts {
    let mut out = kt_engine::SourceFacts::default();
    for m in &project.media {
        // synthetics are skipped: their info().width/height are fictitious;
        // generators (`gen:`) pass — their native is an honest “reserve of
        // detail” for the step (GEN_NATIVE, R.52)
        if kt_media::factory::synth_pattern(&m.path).is_some() {
            continue;
        }
        let mut idx_db = kt_media::index::MediaIndexDb::open(
            &std::path::Path::new(project_path).with_extension("ktindex.db"),
        )
        .ok();
        if let Ok(src) = kt_media::factory::open_indexed(&m.path, idx_db.as_mut()) {
            out.observe(m.id, kt_media::MediaSource::info(&*src));
        }
    }
    // generators stand at the addresses of EVENTS (R.59, stage 2), and their facts are
    // asked at the same addresses — otherwise the decode step of a title would be
    // counted as “native unknown”, and a slowed title would again become
    // stepped
    for (addr, state) in kt_engine::gen_sources(project) {
        let src = kt_media::factory::open_gen(&state);
        out.observe(addr, kt_media::MediaSource::info(&*src));
    }
    out
}

/// Open all sources of the project. Common to the offline render and `probe --time`.
///
/// `strict` — an unavailable file is an error (render: silent silence in the
/// output file is worse than a refusal). The diagnostic goes here with `false`: the project’s
/// bin may hold media that are absent on this machine, and the measured
/// range does not touch them.
fn open_sources(
    project: &Project,
    project_path: &str,
    strict: bool,
    cache: Option<std::sync::Arc<kt_cache::BurstCache>>,
) -> Result<DirectFetcher, String> {
    let mut fetcher = DirectFetcher::new(cache);
    for m in &project.media {
        if let Some(pseudo) = kt_media::factory::open_pseudo(&m.path) {
            match pseudo {
                Ok(s) => fetcher.add_source(m.id, s),
                Err(e) if strict => return Err(e),
                Err(e) => eprintln!("  skipped (bad pseudo source): {e}"),
            }
            continue;
        }
        // the path is ALREADY resolved on load (kt_otio::load_project resolves relative
        // target_url from the project directory, item 316) — a second
        // join doubled the prefix and “lost” the file on a relative call
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
    // see `media_facts`: the sources of generators are addressed by events
    for (addr, state) in kt_engine::gen_sources(project) {
        fetcher.add_source(addr, kt_media::factory::open_gen(&state));
    }
    Ok(fetcher)
}

/// Say WHAT WILL NOT GET INTO THE FILE (R.62): muted tracks and an active
/// solo — before the render, with the same wording as the render window. There is no refusal:
/// mute and solo are the content of the project (the owner’s decision), but staying silent about them
/// is not allowed (rule 16).
fn report_silenced(project: &Project) {
    let s = kt_model::silenced_tracks(project);
    if !s.muted.is_empty() {
        println!(
            "notice: muted tracks ({}) will not be in the file: {}",
            s.muted.len(),
            s.muted.join(", ")
        );
    }
    if !s.solo.is_empty() {
        println!(
            "notice: solo is on ({}): only these tracks will be in the file: {}",
            s.solo.len(),
            s.solo.join(", ")
        );
    }
}

fn render_wav(project: &Project, project_path: &str, output: &str, allow_missing: bool) -> i32 {
    report_silenced(project);
    // Opening of sources — ONE for render and diagnostics (R.49, item 359):
    // a second copy of the same loop lived here, and it would have drifted apart silently.
    // `--allow-missing` renders silence instead of a refusal: sometimes that is exactly
    // what is needed (to check the edit without foreign media), but NOT by default.
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

/// Reading the PROJECT file (R.70): only its own format. An exchange file and a legacy one
/// are not a project — the error names the door (`kt-cli import`), just as “Open” in the
/// app names the menu item. It also returns a foreign passthrough — it is
/// needed by `export-otio`.
fn load_project_file(input: &str) -> Result<(Project, Retained), i32> {
    use kt_model::project_file::{FileKind, ProjectFileError, from_text};
    let text = match std::fs::read_to_string(input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("read {input}: {e}");
            return Err(1);
        }
    };
    match from_text(&text) {
        Ok(f) => {
            let retained = match f.otio {
                Some(v) => match Retained::from_json(&v) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("{input}: passthrough field unreadable: {e}");
                        return Err(1);
                    }
                },
                None => Retained::default(),
            };
            Ok((f.project, retained))
        }
        Err(ProjectFileError::NotOurs { looks_like: Some(kind) }) => {
            let what = match kind {
                FileKind::Otio => "an OpenTimelineIO file",
                FileKind::LegacyKtproj => "a legacy .ktproj",
                FileKind::FormerName => "a project of Kinotrain (the former name of Kinetren)",
            };
            eprintln!("{input}: {what}, not a project — run `kt-cli import {input} <out.kt>` first");
            Err(1)
        }
        Err(e) => {
            eprintln!("{input}: {e}");
            Err(1)
        }
    }
}

/// The project for render/probe: relative media paths are resolved from the
/// project directory ONCE, here (t307: otherwise `sub/p.kt` looked for `sub/sub/clip.mp4`
/// from the directory above it). In the file they stay
/// relative — the app does this on every access, and kt-cli
/// — on input.
fn load_project_resolved(input: &str) -> Result<Project, i32> {
    let (mut p, _) = load_project_file(input)?;
    kt_otio::resolve_project_media(&mut p, std::path::Path::new(input).parent());
    Ok(p)
}

/// The text of the project file: the model plus a foreign passthrough, if there is one — the
/// same body as Ctrl+S in the app (`kt_model::project_file`).
fn project_text(p: &Project, retained: &Retained) -> String {
    let otio = (!retained.is_empty()).then(|| retained.to_json());
    kt_model::project_file::to_text(p, otio.as_ref())
}

fn run(args: &[String]) -> i32 {
    // permission to overwrite an existing file — one for all subcommands
    let force = args.iter().any(|a| a == "--force");
    // the FFmpeg folder the person chose in the app (View → FFmpeg… → Locate folder…)
    // is the first step of the search here too (windows-build)
    let download = cfg!(windows).then(|| kt_dirs::ffmpeg_download_dir().to_string_lossy().into_owned());
    kt_ffmpeg::set_search_dirs(ffmpeg_user_choice(), download);
    match args.first().map(String::as_str) {
        Some("ffmpeg") => {
            // the search report (media-index): what was loaded, what was rejected and why
            let ok = kt_ffmpeg::rescan().is_ok();
            print!("{}", kt_ffmpeg::search_report());
            if ok { 0 } else { 1 }
        }
        Some("version") | None => {
            println!("kt-cli {} (tick rate {} Hz)", kt_version::FULL, kt_time::TICK_HZ);
            0
        }
        Some("new") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: kt-cli new <out.kt> [--force]");
                return 2;
            };
            let p = Project::new(ProjectSettings {
                fps: 30,
                width: 1920,
                height: 1080,
                sample_rate: 48000,
                pitch_engine: Default::default(),

                default_shape: Default::default(),

                conform: Default::default(),

                settings_explicit: false,
            });
            let out = project_text(&p, &Retained::default());
            if let Err(c) = write_project(path, &out, force) {
                return c;
            }
            0
        }
        // THE IMPORT DOOR (R.70): a legacy .ktproj, our .otio of any version of the
        // envelope (fixes — `import_older`) and a foreign .otio — into a file
        // of the project in its own format. The source is not touched.
        Some("import") => {
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli import <in.otio|in.ktproj> <out.kt> [--force]");
                return 2;
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("read {input}: {e}");
                    return 1;
                }
            };
            let in_base = std::path::Path::new(input).parent();
            let b = match kt_otio::import_interchange(&text, in_base) {
                Ok(b) => b,
                Err(kt_otio::Error::ProjectFile) => {
                    eprintln!("{input}: already a Kinetren project — nothing to import");
                    return 1;
                }
                Err(e) => {
                    eprintln!("{input}: {e}");
                    return 1;
                }
            };
            let (mut p, retained, notices) = (b.project, b.retained, b.notices);
            for n in &notices {
                eprintln!("note: {n}");
            }
            // media paths are resolved from the SOURCE (they are absolute); in the project file
            // they are relative to IT
            kt_otio::relativize_project_media(&mut p, std::path::Path::new(output).parent());
            let out = project_text(&p, &retained);
            if let Err(c) = write_project(output, &out, force) {
                return c;
            }
            eprintln!(
                "ok: {} media, {} clips, {} transitions, {} reels",
                p.media.len(),
                p.clips.len(),
                p.transitions.len(),
                p.sequences.len()
            );
            0
        }
        // EXPORT DOOR (R.70): a self-contained exchange file — the body of the reel
        // inline in each instance, the foreign passthrough is returned.
        Some("export-otio") => {
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli export-otio <in.kt> <out.otio> [--force]");
                return 2;
            };
            let (mut p, retained) = match load_project_file(input) {
                Ok(v) => v,
                Err(code) => return code,
            };
            // absolute in memory — relative to the exchange file on output
            // (the OTIO writer itself does this by `out_base`)
            kt_otio::resolve_project_media(&mut p, std::path::Path::new(input).parent());
            let out_base = std::path::Path::new(output).parent();
            let out = kt_otio::save_project(&p, &retained, out_base);
            if let Err(c) = write_project(output, &out, force) {
                return c;
            }
            0
        }
        // README showcase (readme-showcase): a dense project cut from real
        // footage in a directory; media by absolute path, never copied
        Some("showcase") => showcase::run(args, force),
        Some("demo") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: kt-cli demo <out.kt> [--files] [--force]");
                return 2;
            };
            // By default the demo refers to SYNTHETIC sources (R.49,
            // item 359). Eight paths `media/*` do not exist anywhere in the distribution —
            // `demo` and `render` in a row did not work for anyone who had not assembled
            // these files themselves. `synth:` is a full-fledged source across the whole
            // program; `--files` brings back the old names.
            let p = demo_project(!args.iter().any(|a| a == "--files"));
            let out = project_text(&p, &Retained::default());
            if let Err(c) = write_project(path, &out, force) {
                return c;
            }
            0
        }
        Some("roundtrip") => {
            let Some(input) = args.get(1) else {
                // check of the exchange DOOR: exchange file → model → exchange file
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
        // Diagnostics of “what sounds at this place” (R.43): for a complaint of the kind
        // “here the sound comes from another event” the first thing is to find out which
        // MEDIA the engine is going to read at all in this range. The answer is
        // purely model-based — no decode, no FFmpeg, no audio hardware.
        Some("probe") => {
            let (Some(input), Some(from), Some(to)) = (args.get(1), args.get(2), args.get(3))
            else {
                eprintln!("usage: kt-cli probe <in.kt> <from_s> <to_s> [--time]");
                return 2;
            };
            let (Ok(from), Ok(to)) = (from.parse::<f64>(), to.parse::<f64>()) else {
                eprintln!("from/to are seconds");
                return 2;
            };
            let project = match load_project_resolved(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            let timed = args.iter().any(|a| a == "--time");
            // We measure the MATERIAL LENGTHS ourselves (R.63 item 2): in projects prior to this
            // round they are not in the file, and it is precisely by them that probe answers the
            // complaint “silence at the end”. We open the same sources as for
            // the order; unavailable ones simply stay silent.
            let mut project = project;
            if let Ok(mut f) = open_sources(&project, input.as_str(), false, None) {
                let ids: Vec<kt_model::MediaId> = project.media.iter().map(|m| m.id).collect();
                for id in ids {
                    if let Some(src) = f.source_mut(id) {
                        let d = src.info().duration;
                        if d.0 > 0 {
                            if let Some(m) = project.media.iter_mut().find(|m| m.id == id) {
                                m.duration = Some(d);
                            }
                        }
                    }
                }
            }
            probe_range(&project, Some(input.as_str()), from, to, timed.then_some(input.as_str()))
        }
        Some("render") => {
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli render <in.kt> <out.wav> [--allow-missing]");
                return 2;
            };
            let project = match load_project_resolved(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            render_wav(&project, input, output, args.iter().any(|a| a == "--allow-missing"))
        }
        Some("export") => {
            let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: kt-cli export <in.kt> <out.mp4> [encoder]");
                return 2;
            };
            // a third argument can pin the video encoder (otherwise
            // auto-selection: nvenc → vaapi → qsv → libx264). It is needed where
            // the render MUST be reproducible — for example in the cross-render
            // conformance (test 158).
            let encoder = args.get(3).cloned();
            let project = match load_project_resolved(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            type Sources = Vec<(kt_model::MediaId, Box<dyn kt_media::MediaSource>)>;
            let open_all = || -> Result<Sources, String> {
                let mut out = Vec::new();
                for m in &project.media {
                    let src: Box<dyn kt_media::MediaSource> =
                        if let Some(pseudo) = kt_media::factory::open_pseudo(&m.path) {
                            pseudo?
                        } else {
                            // the path is already resolved on load (item 316)
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
            report_silenced(&project);
            let (audio_sources, video_sources) = match (open_all(), open_all()) {
                (Ok(a), Ok(v)) => (a, v),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("{e}");
                    return 1;
                }
            };
            let opts = kt_engine::ExportOptions { force_encoder: encoder, ..Default::default() };
            // R.29: rendering on the GPU by default (headless device, the same
            // composition code); no adapter → CPU with a note in stderr,
            // KT_VIDEO_GPU=0 — forced CPU
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
            eprintln!(
                "unknown command: {cmd} (available: version, new, import, export-otio, demo, showcase, roundtrip, probe, render, export)"
            );
            2
        }
    }
}

/// A demo edit for the site screenshots (port of gen_project from site/screenshots.py —
/// it removes a second writer of the format, R.24 stage 11): six named
/// tracks, markers, a region with loop, FX chains, a morpher.
/// Write the project atomically, with the same code as the GUI (R.49, item 359).
/// A bare `fs::write` left a stub right at the file’s place if the write
/// was interrupted. An existing file is not overwritten without `--force`: kt-cli
/// has neither an asterisk nor a dialog — there is nothing to stop a silent overwrite
/// of someone else’s project.
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

/// The project is busy with another copy of the program — warn (R.49, item 359).
/// Nothing to forbid: kt-cli reads, and writes on an explicit request.
fn warn_if_locked(path: &str) {
    if !std::path::Path::new(path).exists() {
        return;
    }
    let lock = kt_lock_holder(path);
    if let Some(pid) = lock {
        eprintln!("note: {path} is open in Kinetren (process {pid}) — it may overwrite changes");
    }
}

/// The per-user data directory of the app — the same resolver as kt-app (`kt-dirs`).
fn app_data_dir() -> Option<std::path::PathBuf> {
    Some(kt_dirs::data_dir())
}

/// The FFmpeg folder chosen in the app: one line in `<data_dir>/ffmpeg-dir`.
fn ffmpeg_user_choice() -> Option<String> {
    let text = std::fs::read_to_string(app_data_dir()?.join("ffmpeg-dir")).ok()?;
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_string())
}

/// The process number of the lock holder for the project, if there is one. The format of the
/// lock file is the same as kt-app writes (`project_lock.rs`): the first line is the pid.
fn kt_lock_holder(project: &str) -> Option<u32> {
    let data = app_data_dir()?;
    let lock = kt_dirs::locks::lock_file(&data, std::path::Path::new(project));
    if let Some(pid) = kt_dirs::locks::read_holder(&lock) {
        return Some(pid);
    }
    // an app of the former name (Linux only) that has not migrated yet keeps its locks
    // in the former directory next to `kinetren`
    if cfg!(target_os = "linux") && !kt_dirs::is_fallback() {
        let former = data.parent()?.join("kinotrain").join("locks").join(lock.file_name()?);
        return kt_dirs::locks::read_holder(&former);
    }
    None
}

fn demo_project(synth: bool) -> Project {
    use kt_model::*;
    let t = |s: f64| Tick((s * 90_000.0).round() as i64);
    let clip = |id: u64,
                kind: ClipKind,
                lane: u32,
                st: f64,
                du: f64,
                si: f64,
                m: u64,
                gain: f32,
                fx: Vec<FxInstance>| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    let pan = FxInstance::geometry_at(-0.002, 0.002, 1.18);
    // the same camera move that used to be CamMotion keyframes — now curves
    // on Pan (R.34 K2: the animation mechanism is one in the program)
    let cam = FxInstance::geometry_at(0.003, -0.001, 1.05);
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
    // slowed automation — with shared curves (R.33): keys in the ticks of the
    // MATERIAL of clip 25 (source_in = 0, length 74 s)
    let slowed_auto = |dur: f64| -> Vec<Vec<ParamCurve>> {
        let key = |u: f64, v: f64| AutoKey::at(t(u * dur), v);
        vec![vec![
            ParamCurve {
                param: "rate".into(),

                keys: AutoKey::linear(vec![
                    key(0.0, 1.0),
                    key(0.28, 0.86),
                    key(0.62, 0.72),
                    key(0.84, 0.9),
                    key(1.0, 0.66),
                ]),
                source: Default::default(),
            },
            ParamCurve {
                param: "mix".into(),

                keys: AutoKey::linear(vec![
                    key(0.0, 0.12),
                    key(0.28, 0.34),
                    key(0.62, 0.55),
                    key(0.84, 0.3),
                    key(1.0, 0.62),
                ]),
                source: Default::default(),
            },
        ]]
    };
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
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
    // synthetics chosen by role: different notes for music and voice are heard
    // separately, room tone — a quiet constant component
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
        p.media.push(MediaRef::new(MediaId(id), path));
    }
    p.clips = vec![
        clip(1, ClipKind::Video, 0, 0.0, 12.0, 2.0, 1, 0.0, vec![pan.clone()]),
        clip(
            3,
            ClipKind::Video,
            0,
            12.0,
            22.0,
            3.0,
            3,
            0.0,
            vec![pan.clone(), FxInstance::noise(0.05)],
        ),
        clip(5, ClipKind::Video, 0, 34.0, 14.0, 20.0, 1, 0.0, vec![cam]),
        clip(7, ClipKind::Video, 0, 48.0, 18.0, 4.0, 2, 0.0, vec![]),
        clip(9, ClipKind::Video, 0, 66.0, 14.0, 2.0, 5, 0.0, vec![]),
        clip(11, ClipKind::Video, 0, 80.0, 20.0, 24.0, 3, 0.0, vec![pan]),
        clip(
            13,
            ClipKind::Video,
            1,
            6.0,
            6.0,
            1.0,
            4,
            0.0,
            vec![FxInstance::EventGrid { cols: 3, rows: 2, center: true }],
        ),
        clip(15, ClipKind::Video, 1, 26.0, 8.0, 10.0, 2, 0.0, vec![FxInstance::glow(0.72)]),
        clip(17, ClipKind::Video, 1, 55.0, 7.0, 8.0, 5, 0.0, vec![FxInstance::vhs(6, 0.5)]),
        clip(
            19,
            ClipKind::Video,
            2,
            88.0,
            6.0,
            6.0,
            4,
            0.0,
            vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::edges(42)],
        ),
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
    // the camera move of the demo clip — by Pan curves (it was three keyframes
    // of CamMotion; R.34 K2: one animation mechanism per program)
    if let Some(c) = p.clips.iter_mut().find(|c| c.id == ClipId(5)) {
        let key = |u: f64, v: f64| AutoKey::at(t(20.0 + u * 14.0), v);
        c.fx_auto = vec![vec![
            ParamCurve {
                param: "pos_x".into(),
                keys: vec![key(0.0, -0.12), key(0.55, 0.0), key(1.0, 0.14)],
                source: Default::default(),
            },
            ParamCurve {
                param: "pos_y".into(),
                keys: vec![key(0.0, 0.05), key(0.55, 0.0), key(1.0, -0.06)],
                source: Default::default(),
            },
            ParamCurve {
                param: "zoom".into(),
                keys: vec![key(0.0, 1.05), key(0.55, 1.28), key(1.0, 1.45)],
                source: Default::default(),
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
    // the former overlap of the music clips — now an honest crossfade transition
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(25)),
        b: Some(ClipId(27)),
        in_offset: t(2.0),
        out_offset: t(2.0),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.region = Some(Region { start: t(34.0), end: t(66.0) });
    p.loop_enabled = true;
    p.ensure_tracks(kt_model::LaneKind::Video, 3);
    p.ensure_tracks(kt_model::LaneKind::Audio, 3);
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
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
            BinGroup {
                name: "SOUND".into(),
                members: vec![MediaId(6), MediaId(7), MediaId(8)],
                locked: true,
            },
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
