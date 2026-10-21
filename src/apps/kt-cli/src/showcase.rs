//! `kt-cli showcase` (readme-showcase): a dense, valid project cut from a
//! directory of real footage — the timeline the README screenshots show.
//!
//! Everything is a pure function of the sorted file names and the probed
//! media facts: no clock, no randomness. The same directory gives the same
//! bytes (t824). Media are referenced by absolute path, never copied.
//!
//! Layout (design D2 of the change): lane 0 of the video tracks is the TOP
//! layer, so titles and inserts live there, the montage on lane 1 and the
//! blurred backdrop for non-16:9 sources on lane 2, under the montage.

use std::path::PathBuf;

use kt_media::MediaSource;
use kt_model::*;
use kt_time::Tick;

/// A probed source file.
pub struct Source {
    pub path: String,
    pub dur: f64,
    /// a picture (not a still); `false` = an audio-only file — music
    pub video: bool,
    pub audio: bool,
    pub aspect: f64,
}

/// Regular files directly in `dir` that open as video (not stills) or as
/// audio only (music for the beds), by name.
pub fn scan(dir: &str) -> Result<Vec<Source>, String> {
    let root = std::fs::canonicalize(dir).map_err(|e| format!("{dir}: {e}"))?;
    if !root.is_dir() {
        return Err(format!("{dir}: not a directory"));
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&root)
        .map_err(|e| format!("{dir}: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        // sidecar index files live next to media; they are never footage
        .filter(|p| !p.to_string_lossy().contains(".ktindex.db"))
        // text next to the footage (credits, checksums, projects): FFmpeg's
        // text demuxer would happily open a .txt as a "video"
        .filter(|p| {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase());
            !matches!(
                ext.as_deref(),
                Some("txt" | "md" | "json" | "sha1" | "kt" | "ktproj" | "otio" | "log" | "part")
            )
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let path = f.to_string_lossy().into_owned();
        let Ok(src) = kt_media::ffmpeg::FfmpegSource::open(&path) else { continue };
        let info = src.info();
        let video = info.has_video && !info.still;
        if !(video || info.has_audio) || info.duration.0 <= 0 {
            continue;
        }
        let (w, h) = info.view_size();
        out.push(Source {
            path,
            dur: info.duration.0 as f64 / 90_000.0,
            video,
            audio: info.has_audio,
            aspect: if h > 0.0 { w / h } else { 16.0 / 9.0 },
        });
    }
    if !out.iter().any(|s| s.video) {
        return Err(format!("{dir}: no readable video file"));
    }
    if !out.iter().any(|s| s.audio) {
        return Err(format!("{dir}: no file with an audio stream (the music bed needs one)"));
    }
    Ok(out)
}

/// Free-layout positions of the bin cards: three columns per group, groups
/// one under another with room for their frame titles.
fn free_grid(groups: &[&Vec<MediaId>]) -> Vec<(MediaId, f32, f32)> {
    const COLS: usize = 3;
    let (cell_w, cell_h, gap) = (142.0, 100.0, 34.0);
    let mut out = Vec::new();
    let mut y = 18.0;
    for g in groups.iter().filter(|g| !g.is_empty()) {
        for (i, m) in g.iter().enumerate() {
            out.push((*m, (i % COLS) as f32 * cell_w, y + (i / COLS) as f32 * cell_h));
        }
        y += g.len().div_ceil(COLS) as f32 * cell_h + gap;
    }
    out
}

/// The id of the nested reel (media and reels share one id space; the
/// sources and generators stay far below it).
const REEL: MediaId = MediaId(1000);

fn t(s: f64) -> Tick {
    Tick((s * 90_000.0).round() as i64)
}

/// Golden-ratio sequence in [0, 1): a deterministic spread of in-points.
fn spread(i: usize) -> f64 {
    (i as f64 * 0.618_033_988_75).fract()
}

#[allow(clippy::too_many_arguments)]
fn clip(
    id: u64,
    kind: ClipKind,
    lane: u32,
    st: f64,
    du: f64,
    si: f64,
    m: u64,
    gain: f32,
) -> ClipEvent {
    ClipEvent {
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
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn title(text: &str, size: f64, pos_y: f64) -> GeneratorKind {
    let mut k = GeneratorKind::default_text();
    if let GeneratorKind::Text { text: tx, size: sz, pos_y: py, bold, shadow_a, .. } = &mut k {
        *tx = text.to_string();
        *sz = size;
        *py = pos_y;
        *bold = true;
        *shadow_a = 160;
    }
    k
}

/// The four transition kinds the montage cycles through (≥ 3 distinct).
fn transition_kind(i: usize) -> TransitionKind {
    let pick = |want: fn(&TransitionKind) -> bool| {
        TransitionKind::ALL.iter().find(|k| want(k)).cloned().unwrap_or_default()
    };
    match i % 4 {
        0 => TransitionKind::Crossfade,
        1 => pick(|k| matches!(k, TransitionKind::Push { .. })),
        2 => pick(|k| matches!(k, TransitionKind::Iris { .. })),
        _ => pick(|k| matches!(k, TransitionKind::Whip { .. })),
    }
}

/// Montage FX cycle (every fifth cut is styled).
fn montage_fx(i: usize) -> Vec<FxInstance> {
    match i % 4 {
        0 => vec![FxInstance::glow(0.6)],
        1 => vec![FxInstance::vhs(6, 0.5)],
        2 => vec![FxInstance::noise(0.08)],
        _ => vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::edges(40)],
    }
}

fn slowed() -> FxInstance {
    FxInstance::SlowedReverb {
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
    }
}

/// `rate` and `mix` curves of the music bed, keys in MATERIAL ticks
/// (`source_in .. source_in + dur`); `mix` carries several segment shapes
/// so the graph reads as curves, not a polyline.
fn slowed_curves(si: f64, dur: f64) -> Vec<Vec<ParamCurve>> {
    let at = |u: f64| t(si + u * dur);
    // `rate` drives the warp map, which only takes straight segments
    // (validate: RateShapeMustBeLinear); the shaped segments live on `mix`
    let rate = AutoKey::linear(vec![
        AutoKey::at(at(0.0), 1.0),
        AutoKey::at(at(0.22), 0.86),
        AutoKey::at(at(0.45), 0.72),
        AutoKey::at(at(0.62), 0.9),
        AutoKey::at(at(0.8), 0.66),
        AutoKey::at(at(1.0), 0.8),
    ]);
    let mix = vec![
        AutoKey::shaped(at(0.0), 0.12, SegShape::EaseOut { tension: 1.0 }),
        AutoKey::at(at(0.22), 0.34),
        AutoKey::shaped(
            at(0.45),
            0.55,
            SegShape::Zigzag { periods: 4.0, amplitude: 0.06, profile: ZigProfile::Sine },
        ),
        AutoKey::shaped(at(0.62), 0.3, SegShape::Hold),
        AutoKey::shaped(at(0.8), 0.62, SegShape::EaseIn { tension: 1.0 }),
        AutoKey::at(at(1.0), 0.4),
    ];
    vec![vec![
        ParamCurve { param: "rate".into(), keys: rate, source: Default::default() },
        ParamCurve { param: "mix".into(), keys: mix, source: Default::default() },
    ]]
}

/// Build the showcase project from probed sources (already sorted by name).
pub fn build(src: &[Source]) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: true,
    });
    let n = src.len();
    // the montage, the picture-in-picture and the inserts take pictures;
    // audio-only files (music) only feed the beds
    let vids: Vec<usize> = (0..n).filter(|&i| src[i].video).collect();
    let nv = vids.len();
    for (i, s) in src.iter().enumerate() {
        p.media.push(MediaRef::with_duration(MediaId(i as u64 + 1), s.path.clone(), t(s.dur)));
    }
    let mut next_id = 1u64;
    let mut id = || {
        let v = next_id;
        next_id += 1;
        v
    };
    let mut clips: Vec<ClipEvent> = Vec::new();
    let mut transitions: Vec<TransitionEvent> = Vec::new();
    let mut add_transition = |a: u64, b: u64, off: f64, kind: TransitionKind| {
        transitions.push(TransitionEvent {
            id: TransitionId(transitions.len() as u64 + 1),
            a: Some(ClipId(a)),
            b: Some(ClipId(b)),
            in_offset: t(off),
            out_offset: t(off),
            kind,
            audio: Default::default(),
            auto: vec![],
            invert: false,
        });
    };

    // ---- V lane 1 "MAIN": the montage, with linked audio on A lane 0 ----
    const CUTS: usize = 40;
    const PATTERN: [f64; 8] = [2.4, 3.2, 1.6, 4.0, 2.0, 2.8, 3.6, 1.2];
    let mut cursor = 0.0;
    // (video id, audio id, handle) of the previous cut, for transitions
    let mut prev: Option<(u64, Option<u64>, f64)> = None;
    let mut wide_cuts = Vec::new(); // main cuts of non-16:9 sources → backdrop
    let mut main_ids = Vec::new();
    for i in 0..CUTS {
        let si = vids[(i + i / nv) % nv];
        let s = &src[si];
        let handle = (s.dur * 0.1).min(0.5);
        let len = PATTERN[i % PATTERN.len()].min(s.dur * 0.4);
        let lo = handle;
        let hi = (s.dur - len - handle).max(lo);
        // inside 10 %–85 % of the file: intros and credits stay out
        let want = s.dur * (0.10 + 0.75 * spread(i + 1));
        let src_in = want.clamp(lo, hi);
        let start = cursor;
        cursor += len;
        let vid = id();
        let mut v = clip(vid, ClipKind::Video, 1, start, len, src_in, si as u64 + 1, 0.0);
        if i % 5 == 4 {
            v.fx = montage_fx(i / 5);
        }
        let aid = if s.audio {
            let aid = id();
            let g = GroupId(vid);
            v.link_group = Some(g);
            let mut a = clip(aid, ClipKind::Audio, 0, start, len, src_in, si as u64 + 1, -16.0);
            a.link_group = Some(g);
            clips.push(a);
            Some(aid)
        } else {
            None
        };
        if (s.aspect - 16.0 / 9.0).abs() > 0.05 {
            wide_cuts.push((start, len, src_in, si));
        }
        main_ids.push((vid, start, len));
        clips.push(v);
        if let Some((pv, pa, ph)) = prev
            && i % 4 == 0
        {
            let off = ph.min(handle).min(len / 3.0);
            add_transition(pv, vid, off, transition_kind(i / 4));
            if let (Some(pa), Some(aid)) = (pa, aid) {
                add_transition(pa, aid, off, TransitionKind::Crossfade);
            }
        }
        prev = Some((vid, aid, handle));
    }
    let total = cursor;

    // ---- V lane 2 "BACKDROP": blurred fill behind non-16:9 cuts ----
    for (start, len, src_in, si) in &wide_cuts {
        let mut b = clip(id(), ClipKind::Video, 2, *start, *len, *src_in, *si as u64 + 1, 0.0);
        b.geometry.rule = Conform::Fill;
        b.fx = vec![FxInstance::blur(28)];
        b.opacity = 0.85;
        clips.push(b);
    }

    // ---- V lane 0 "TITLES": titles and inserts over the montage ----
    let gen_media = |p: &mut Project, k: &GeneratorKind| -> u64 {
        let mid = p.media.len() as u64 + 1;
        p.media.push(MediaRef::new(MediaId(mid), gen_kind_path(k)));
        mid
    };
    // a nested reel "STINGER" — the title over a tilted picture-in-picture —
    // used twice (intro and outro), so the timeline shows the ×2 badge and
    // the screenshot can expand one instance in place
    let pip_src = vids
        .iter()
        .copied()
        .min_by(|&a, &b| {
            let d = |i: usize| (src[i].aspect - 1.0).abs();
            d(a).total_cmp(&d(b)).then(a.cmp(&b))
        })
        .unwrap_or(0);
    // The reel's content is twice as long as an instance: each instance
    // plays the middle of it, so an expanded instance shows veiled tails on
    // both sides. Three parameters are published, and each instance sets
    // its own: the title text, the glow and the picture.
    let reel_len = (total * 0.06).min(5.0);
    let content = reel_len * 2.0;
    let t1 = title("KINETREN", 0.16, 0.45);
    let m1 = gen_media(&mut p, &t1);
    let mut reel = Sequence::new(REEL, "STINGER");
    reel.ensure_tracks(LaneKind::Video, 2);
    let title_id = id();
    let mut c = clip(title_id, ClipKind::Video, 0, 0.0, content, 0.0, m1, 0.0);
    c.r#gen = Some(t1);
    reel.clips.push(c);
    let s_in = src[pip_src].dur * 0.35;
    let pic_id = id();
    let mut c = clip(pic_id, ClipKind::Video, 1, 0.0, content.min(src[pip_src].dur * 0.3), s_in, pip_src as u64 + 1, 0.0);
    c.geometry.zoom = 0.42;
    c.geometry.pos_x = -0.28;
    c.geometry.pos_y = 0.2;
    c.geometry.rot = 6.0;
    c.fx = vec![FxInstance::glow(0.5)];
    reel.clips.push(c);
    let (p_title, p_glow, p_pic) = (PubId(1), PubId(2), PubId(3));
    reel.published = vec![
        PublishedParam {
            id: p_title,
            name: "Title".into(),
            target: ParamAddr::new(ClipId(title_id), ParamOwner::Generator, "text"),
            ty: ParamType::Text { multiline: false },
            range: PubRange::default(),
        },
        PublishedParam {
            id: p_glow,
            name: "Glow".into(),
            target: ParamAddr::new(ClipId(pic_id), ParamOwner::FxSlot { slot: 0 }, "strength"),
            ty: ParamType::Num,
            range: PubRange::default(),
        },
        PublishedParam {
            id: p_pic,
            name: "Picture".into(),
            target: ParamAddr::new(ClipId(pic_id), ParamOwner::ClipAttr, "media"),
            ty: ParamType::Media,
            range: PubRange::default(),
        },
    ];
    p.sequences.push(reel);
    // intro: the reel as authored; middle of the edit: its own title,
    // a stronger glow and another picture
    clips.push(clip(id(), ClipKind::Video, 0, 0.0, reel_len, reel_len * 0.5, REEL.0, 0.0));
    let other_pic = vids[(vids.iter().position(|&v| v == pip_src).unwrap_or(0) + 3) % nv];
    let mut c = clip(id(), ClipKind::Video, 0, total * 0.5, reel_len, reel_len * 0.5, REEL.0, 0.0);
    c.overrides = vec![
        Override::value(p_title, ParamValue::Text("REELS".into())),
        Override::value(p_glow, ParamValue::Num(0.9)),
        Override::value(p_pic, ParamValue::Media(MediaId(other_pic as u64 + 1))),
    ];
    clips.push(c);
    let t2 = title("nonlinear video editor", 0.06, 0.82);
    let m2 = gen_media(&mut p, &t2);
    let at2 = total * 0.4;
    let mut c = clip(id(), ClipKind::Video, 0, at2, (total * 0.05).min(4.0), 0.0, m2, 0.0);
    c.r#gen = Some(t2);
    clips.push(c);
    // picture-in-picture: the source closest to square, scaled into a corner
    let pip = vids
        .iter()
        .copied()
        .min_by(|&a, &b| {
            let d = |i: usize| (src[i].aspect - 1.0).abs();
            d(a).total_cmp(&d(b)).then(a.cmp(&b))
        })
        .unwrap_or(0);
    let pip_at = total * 0.6;
    let pip_len = (total * 0.08).min(8.0).min(src[pip].dur * 0.6);
    let mut c =
        clip(id(), ClipKind::Video, 0, pip_at, pip_len, src[pip].dur * 0.2, pip as u64 + 1, 0.0);
    c.geometry.zoom = 0.34;
    c.geometry.pos_x = 0.3;
    c.geometry.pos_y = -0.26;
    c.geometry.rot = -4.0;
    c.fx = vec![FxInstance::glow(0.4)];
    clips.push(c);
    // a VHS insert
    let ins = vids[(vids.iter().position(|&v| v == pip).unwrap_or(0) + 1) % nv];
    let ins_at = total * 0.78;
    let ins_len = (total * 0.03).min(2.4).min(src[ins].dur * 0.4);
    let mut c =
        clip(id(), ClipKind::Video, 0, ins_at, ins_len, src[ins].dur * 0.5, ins as u64 + 1, 0.0);
    c.fx = vec![FxInstance::vhs(8, 0.6), FxInstance::noise(0.12)];
    c.opacity = 0.9;
    clips.push(c);

    // ---- A lane 1 "MUSIC": beds from the longest music files (audio-only
    // sources), or from the longest videos with sound when there is none ----
    let music: Vec<usize> = (0..n).filter(|&i| !src[i].video).collect();
    let mut beds: Vec<usize> =
        if music.is_empty() { (0..n).filter(|&i| src[i].audio).collect() } else { music.clone() };
    beds.sort_by(|&a, &b| src[b].dur.total_cmp(&src[a].dur).then(a.cmp(&b)));
    let split = total * 0.55;
    let mut at = 0.0;
    let mut k = 0usize;
    let mut prev_bed: Option<(u64, f64)> = None;
    let mut first_bed = true;
    while at < total - 1e-6 {
        let bi = beds[k % beds.len()];
        let s = &src[bi];
        let src_in = if first_bed { s.dur * 0.08 } else { s.dur * 0.3 }.min(s.dur * 0.5);
        let end_goal = if first_bed { split } else { total };
        let len = (end_goal - at).min(s.dur - src_in).max(0.04);
        let bid = id();
        let mut b = clip(bid, ClipKind::Audio, 1, at, len, src_in, bi as u64 + 1, -6.0);
        if first_bed {
            b.fx = vec![slowed()];
            b.fx_auto = slowed_curves(src_in, len);
        }
        clips.push(b);
        if let Some((pb, plen)) = prev_bed {
            // a quarter of the SHORTER neighbour: two crossfades around a
            // short bed must not meet (TransitionWindowsOverlap)
            let off = (len.min(plen) / 4.0).min(2.0).min(src_in);
            add_transition(pb, bid, off, TransitionKind::Crossfade);
        }
        prev_bed = Some((bid, len));
        if at + len >= split - 1e-6 {
            first_bed = false;
        }
        at += len;
        k += 1;
    }

    // ---- A lane 2 "AMBIENCE": a quiet bed from the shortest audio source ----
    // the shortest video with sound (its own soundtrack under the music),
    // else the shortest bed
    let amb = vids
        .iter()
        .copied()
        .filter(|&i| src[i].audio)
        .min_by(|&a, &b| src[a].dur.total_cmp(&src[b].dur).then(a.cmp(&b)))
        .unwrap_or_else(|| *beds.last().unwrap_or(&0));
    let amb_len = (total * 0.5).min(src[amb].dur * 0.8);
    clips.push(clip(
        id(),
        ClipKind::Audio,
        2,
        total * 0.25,
        amb_len,
        src[amb].dur * 0.1,
        amb as u64 + 1,
        -22.0,
    ));

    p.clips = clips;
    p.transitions = transitions;

    // ---- morpher over the "BREAK" section: a glow batch on the montage ----
    let (m0, m1) = (total * 0.66, total * 0.8);
    let targets: Vec<ClipId> = main_ids
        .iter()
        .filter(|(_, s, l)| *s < m1 && s + l > m0)
        .map(|(v, _, _)| ClipId(*v))
        .collect();
    // the morpher swaps in the brightest-looking choice: the third video
    let swap = vids[2 % nv];
    p.ensure_tracks(LaneKind::Video, 3);
    p.ensure_tracks(LaneKind::Audio, 3);
    p.ensure_tracks(LaneKind::Morph, 1);
    p.morphers = vec![MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: t(m0),
        duration: t(m1 - m0),
        targets: MorphTargets::Clips { ids: targets },
        // what the batch does: another source in place of the targets, and
        // a VHS look with glow on top
        patch: MorphPatch {
            media: Some(MediaId(swap as u64 + 1)),
            absolute_source_in: Some(t(src[swap].dur * 0.3)),
            video_fx: vec![FxInstance::vhs(8, 0.6), FxInstance::glow(0.6)],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }];

    for (u, name) in
        [(0.0, "INTRO"), (0.25, "CHASE"), (0.5, "DROP"), (0.66, "BREAK"), (0.88, "OUTRO")]
    {
        p.markers.push(Marker::named(t(total * u), name));
    }
    p.region = Some(Region { start: t(total * 0.25), end: t(total * 0.5) });
    p.loop_enabled = true;

    for (kind, lane, name) in [
        (LaneKind::Video, 0, "TITLES"),
        (LaneKind::Video, 1, "MAIN"),
        (LaneKind::Video, 2, "BACKDROP"),
        (LaneKind::Audio, 0, "CAMERA"),
        (LaneKind::Audio, 1, "MUSIC"),
        (LaneKind::Audio, 2, "AMBIENCE"),
        (LaneKind::Morph, 0, "BATCH"),
    ] {
        if let Some(tr) = p.track_at_mut(kind, lane) {
            tr.name = name.to_string();
        }
    }
    // compact rows: with the reel expanded in place every track, its own
    // included, fits the timeline panel of a 1080p window
    for tr in p.tracks.iter_mut().chain(p.sequences.iter_mut().flat_map(|s| s.tracks.iter_mut())) {
        if tr.kind != LaneKind::Morph {
            tr.height = 24.0;
        }
    }

    let media_ids: Vec<MediaId> = p.media.iter().map(|m| m.id).collect();
    let footage: Vec<MediaId> = vids.iter().map(|&i| media_ids[i]).collect();
    let tracks: Vec<MediaId> = music.iter().map(|&i| media_ids[i]).collect();
    let generated: Vec<MediaId> = media_ids[n..].to_vec();
    p.ui = Some(UiState {
        view_start: 0.0,
        // the whole edit in ≈ 1500 px of timeline
        ticks_per_px: (t(total).0 as f64 / 1500.0).max(1.0),
        // the cursor sits on the picture-in-picture: four layers on screen
        cursor: t(pip_at + pip_len * 0.5).0,
        bin_free: true,
        bin_layout: free_grid(&[&footage, &tracks, &generated]),
        bin_groups: [("FOOTAGE", footage), ("MUSIC", tracks), ("GENERATED", generated)]
            .into_iter()
            .filter(|(_, m)| !m.is_empty())
            .map(|(name, members)| BinGroup { name: name.into(), members, locked: false })
            .collect(),
        ..Default::default()
    });
    p.normalize();
    p
}

/// `kt-cli showcase <out.kt> --sources <dir> [--force]`.
pub fn run(args: &[String], force: bool) -> i32 {
    let out = args.get(1).filter(|a| !a.starts_with("--"));
    let dir = args.iter().position(|a| a == "--sources").and_then(|i| args.get(i + 1));
    let (Some(out), Some(dir)) = (out, dir) else {
        eprintln!("usage: kt-cli showcase <out.kt> --sources <dir> [--force]");
        return 2;
    };
    let src = match scan(dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let p = build(&src);
    if let Err(e) = p.validate() {
        eprintln!("showcase project is invalid: {e:?}");
        return 1;
    }
    let text = crate::project_text(&p, &kt_otio::Retained::default());
    if let Err(c) = crate::write_project(out, &text, force) {
        return c;
    }
    println!(
        "{out}: {} sources, {} events, {} transitions, {:.1} s",
        src.len(),
        p.clips.len(),
        p.transitions.len(),
        p.clips.iter().map(|c| (c.start + c.duration).0).max().unwrap_or(0) as f64 / 90_000.0
    );
    0
}
