//! The built-in DEMO project (welcome-window-and-tour D8): one body for kt-cli's `demo` command
//! and the GUI's lesson projects (rule 17). It needs no media file: the sources are synthetic
//! (`synth:`, the kt-cli default since R.49 item 359), the old `media/*` names (`--files`), or —
//! for the lessons — generators for every picture, so a lesson opens with real thumbnails and no
//! missing source (the F-HLP-029 defect "eight nonexistent media" does not apply).

use crate::*;
use kt_time::Tick;

/// Where the demo's eight media come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoSources {
    Synth,
    Files,
    Generators,
}

pub fn demo_project(sources: DemoSources) -> Project {
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
    // generators by role (the lesson project, D8): every picture is drawn by the program, the
    // sound stays synthetic — no media file anywhere
    let gens = [
        (1u64, "gradient"),
        (2, "noise"),
        (3, "corners"),
        (4, "smpte"),
        (5, "checker"),
        (6, "synth:sine:220"),
        (7, "synth:sine:660"),
        (8, "synth:dc:0.05"),
    ];
    let mut gen_kinds: Vec<(MediaId, GeneratorKind)> = Vec::new();
    let table: Vec<(u64, String)> = match sources {
        DemoSources::Synth => synths.iter().map(|(i, p)| (*i, p.to_string())).collect(),
        DemoSources::Files => files.iter().map(|(i, p)| (*i, p.to_string())).collect(),
        DemoSources::Generators => gens
            .iter()
            .map(|(i, tag)| match GeneratorKind::default_for_tag(tag) {
                Some(kind) => {
                    gen_kinds.push((MediaId(*i), kind.clone()));
                    (*i, gen_kind_path(&kind))
                }
                None => (*i, tag.to_string()),
            })
            .collect(),
    };
    for (id, path) in table {
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
    p.ensure_tracks(LaneKind::Video, 3);
    p.ensure_tracks(LaneKind::Audio, 3);
    p.ensure_tracks(LaneKind::Morph, 1);
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
    // a generator's values live on its EVENT (R.59 stage 2)
    for c in &mut p.clips {
        if let Some((_, kind)) = gen_kinds.iter().find(|(m, _)| *m == c.media) {
            c.r#gen = Some(kind.clone());
        }
    }
    p.normalize();
    p.validate().expect("demo project is valid");
    p
}
