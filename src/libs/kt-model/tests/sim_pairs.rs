//! t1461 (sim-audio-pairs; capability `audio-fx`): a simulated device is ONE across an A/V link
//! group. Adding VHS Tape to the picture puts its sound on the linked audio; a value, a curve and
//! the presence follow both ways in the same undo step; the sound's own bypass and the other slots
//! stay; a morpher patch hands the other half to the events of the other kind.

use kt_model::*;
use kt_time::Tick;

fn clip(id: u64, kind: ClipKind, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: Some(GroupId(1)),
        kind,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    }
}

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "a.mp4"));
    let gain = FxInstance::plugin("gain").expect("gain");
    p.clips.push(clip(10, ClipKind::Video, vec![]));
    p.clips.push(clip(11, ClipKind::Audio, vec![gain]));
    p
}

fn get(m: &TimelineModel, id: u64) -> ClipEvent {
    m.project().clips.iter().find(|c| c.id == ClipId(id)).unwrap().clone()
}

fn ids(c: &ClipEvent) -> Vec<String> {
    c.fx.iter().map(|f| f.tag().to_string()).collect()
}

#[test]
fn t1461_the_device_is_one_across_the_link_group() {
    kt_plugins_builtin::install();
    let mut m = TimelineModel::new(project());
    // add VHS Tape to the picture: its sound lands on the linked audio, after the audio's own gain
    let mut tape = FxInstance::plugin("vhs_tape").expect("vhs_tape");
    tape.set_param("rec_a_spacing", 3.0);
    m.apply(Command::SetFx { id: ClipId(10), fx: vec![tape.clone()] }).unwrap();
    let a = get(&m, 11);
    assert_eq!(ids(&a), ["gain", "vhs_tape_sound"], "the pair did not land on the audio");
    assert_eq!(a.fx[1].get_param("rec_a_spacing"), Some(3.0), "the device's rows differ");
    // one undo removes both halves: the mirror is in the same step
    m.undo().unwrap();
    assert_eq!(ids(&get(&m, 11)), ["gain"]);
    m.redo().unwrap();
    // the sound's slot edits the same device: the picture follows
    let mut a = get(&m, 11);
    a.fx[1].set_param("tracking_um", 12.0);
    m.apply(Command::SetFx { id: ClipId(11), fx: a.fx.clone() }).unwrap();
    assert_eq!(get(&m, 10).fx[0].get_param("tracking_um"), Some(12.0), "the picture's deck did not follow");
    // a curve on the picture's slot reaches the sound's
    let curve = ParamCurve {
        param: "tracking_um".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(900_000), 30.0)]),
        source: Default::default(),
    };
    m.apply(Command::SetFxAuto { id: ClipId(10), auto: vec![vec![curve.clone()]] }).unwrap();
    let a = get(&m, 11);
    assert_eq!(a.fx_auto.get(1).map(|c| c.len()), Some(1), "the curve did not reach the sound");
    assert_eq!(a.fx_auto[1][0].param, "tracking_um");
    assert!(a.fx_auto[0].is_empty(), "the gain got a curve");
    // switching the sound off is its own: the picture's slot stays on
    m.apply(Command::SetFxBypass { id: ClipId(11), bypass: vec![false, true] }).unwrap();
    assert!(get(&m, 10).fx_bypass.iter().all(|b| !b), "the sound's switch turned the picture off");
    assert_eq!(get(&m, 11).fx_bypass, vec![false, true]);
    // a dub of a dub: two tapes on the picture, two sounds on the audio
    let v = get(&m, 10);
    m.apply(Command::SetFx { id: ClipId(10), fx: vec![v.fx[0].clone(), tape.clone()] }).unwrap();
    assert_eq!(ids(&get(&m, 11)), ["gain", "vhs_tape_sound", "vhs_tape_sound"]);
    // removing the device from the picture removes its sound; the gain stays
    m.apply(Command::SetFx { id: ClipId(10), fx: vec![] }).unwrap();
    let a = get(&m, 11);
    assert_eq!(ids(&a), ["gain"], "the sound outlived its device");
    assert!(a.fx_bypass.len() <= 1, "a bypass flag outlived its slot");
}

#[test]
fn t1461b_a_morpher_patch_hands_the_other_half_on() {
    kt_plugins_builtin::install();
    let tape = FxInstance::plugin("vhs_tape").expect("vhs_tape");
    let mut patch = MorphPatch { video_fx: vec![tape.clone()], ..Default::default() };
    let curve = ParamCurve {
        param: "tracking_um".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 20.0)]),
        source: Default::default(),
    };
    patch.video_auto = vec![vec![curve]];
    let a = patch.chain_for(ClipKind::Audio);
    assert_eq!(a.iter().map(|f| f.tag()).collect::<Vec<_>>(), ["vhs_tape_sound"]);
    let auto = patch.auto_for(ClipKind::Audio);
    assert_eq!(auto.len(), a.len(), "the curves are not in the slots' order");
    assert_eq!(auto[0][0].param, "tracking_um");
    // a bypassed picture slot hands nothing on; a patch holding the sound itself gets no second one
    patch.video_bypass = vec![true];
    assert!(patch.chain_for(ClipKind::Audio).is_empty());
    patch.video_bypass = vec![];
    patch.audio_fx = vec![FxInstance::plugin("vhs_tape_sound").unwrap()];
    assert_eq!(patch.chain_for(ClipKind::Audio).len(), 1);
    // and the sound in a patch hands the picture's tape to the video events
    let only_sound = MorphPatch { audio_fx: vec![FxInstance::plugin("vhs_tape_sound").unwrap()], ..Default::default() };
    assert_eq!(only_sound.chain_for(ClipKind::Video).iter().map(|f| f.tag()).collect::<Vec<_>>(), ["vhs_tape"]);
}

/// t1461c: a preset or a pasted chain goes through the same door as the picker (`append_chain_block`,
/// then SetFx and SetFxAuto): the device lands on both halves, its curve too, and a preset holding
/// the sound puts the tape on the picture.
#[test]
fn t1461c_a_preset_or_a_paste_carries_the_pair() {
    kt_plugins_builtin::install();
    let mut m = TimelineModel::new(project());
    let curve = ParamCurve {
        param: "tracking_um".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(900_000), 30.0)]),
        source: Default::default(),
    };
    let block = ops::FxChainBlock {
        slots: vec![ops::FxSlotBlock {
            fx: FxInstance::plugin("vhs_tape").unwrap(),
            bypass: false,
            curves: vec![curve],
            masks: Vec::new(),
        }],
        src_in: Tick(0),
        fx_off: false,
    };
    let v = get(&m, 10);
    let (mut fx, mut bypass, mut auto) = (v.fx.clone(), v.fx_bypass.clone(), v.fx_auto.clone());
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &block, v.source_in);
    m.apply(Command::SetFx { id: ClipId(10), fx }).unwrap();
    m.apply(Command::SetFxAuto { id: ClipId(10), auto }).unwrap();
    let a = get(&m, 11);
    assert_eq!(ids(&a), ["gain", "vhs_tape_sound"], "the paste did not reach the audio");
    assert_eq!(a.fx_auto[1][0].param, "tracking_um", "the pasted curve did not reach the sound");

    // a preset of the sound, on the audio: the picture gets the tape
    let mut m = TimelineModel::new(project());
    let a = get(&m, 11);
    let block = ops::FxChainBlock {
        slots: vec![ops::FxSlotBlock { fx: FxInstance::plugin("vhs_tape_sound").unwrap(), bypass: false, curves: vec![], masks: Vec::new() }],
        src_in: Tick(0),
        fx_off: false,
    };
    let (mut fx, mut bypass, mut auto) = (a.fx.clone(), a.fx_bypass.clone(), a.fx_auto.clone());
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &block, a.source_in);
    m.apply(Command::SetFx { id: ClipId(11), fx }).unwrap();
    assert_eq!(ids(&get(&m, 10)), ["vhs_tape"], "the sound's preset did not reach the picture");
}

/// t1485 (capability `project-format`): a deck's sound row off its default writes the project as
/// format 4 — an older build refuses it instead of dropping the device's sound settings; a deck at
/// its defaults, or a sound slot's own rows, leave the former format and text.
#[test]
fn t1485_a_device_s_sound_rows_raise_the_format() {
    kt_plugins_builtin::install();
    let mut p = project();
    let tape = FxInstance::plugin("vhs_tape").unwrap();
    p.clips[0].fx = vec![tape.clone()];
    assert!(!p.has_device_sound());
    let plain = project_file::to_text(&p, None);
    assert!(plain.contains("\"kinetren\": 1"), "a deck at its defaults moved the format");
    assert!(!plain.contains("audio_out"), "a default sound row was written");
    let mut normal = tape;
    normal.set_param("audio_out", 1.0);
    p.clips[0].fx = vec![normal];
    assert!(p.has_device_sound());
    let text = project_file::to_text(&p, None);
    assert!(text.contains("\"kinetren\": 4"), "the deck's sound row did not raise the format");
    assert!(text.contains("\"audio_out\": \"normal\""));
    let back = project_file::from_text(&text).unwrap().project;
    assert_eq!(back, p);
    // a build that reads up to 3 refuses it by the same check
    let newer = text.replace("\"kinetren\": 4", &format!("\"kinetren\": {}", project_file::PROJECT_FILE_READS + 1));
    assert!(matches!(project_file::from_text(&newer), Err(project_file::ProjectFileError::TooNew { .. })));
}
