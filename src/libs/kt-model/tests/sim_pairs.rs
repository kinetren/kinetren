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

/// t1461b (rewritten by fx-graph D2): a patch HOLDS both halves of a device as slots — a layer given
/// VHS Tape with a tracking curve gets the tape's sound with the curve at the same position; each
/// half switches off alone (the picture off keeps the sound, the sound off keeps the picture); a
/// patch given only the sound gets the picture's tape too.
#[test]
fn t1461b_a_morpher_patch_hands_the_other_half_on() {
    kt_plugins_builtin::install();
    let tape = FxInstance::plugin("vhs_tape").expect("vhs_tape");
    let curve = ParamCurve {
        param: "tracking_um".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 20.0)]),
        source: Default::default(),
    };
    let patch = MorphPatch { video_fx: vec![tape], video_auto: vec![vec![curve]], ..Default::default() };
    let mut m = TimelineModel::new(morph_project());
    m.apply(Command::AddMorpher(layer(patch))).unwrap();
    let held = patch_of(&m);
    let a = held.chain_for(ClipKind::Audio);
    assert_eq!(a.iter().map(|f| f.tag()).collect::<Vec<_>>(), ["vhs_tape_sound"]);
    let auto = held.auto_for(ClipKind::Audio);
    assert_eq!(auto.len(), a.len(), "the curves are not in the slots' order");
    assert_eq!(auto[0][0].param, "tracking_um");
    // the picture switched off: the sound stays (each half its own switch)
    let mut p = held.clone();
    p.video_bypass = vec![true];
    assert_eq!(p.chain_for(ClipKind::Audio).len(), 1, "the picture's switch took the sound");
    assert!(p.chain_for(ClipKind::Video).is_empty());
    // the sound switched off: none on the audio, the tape on the video
    let mut p = held.clone();
    p.audio_bypass = vec![true];
    assert!(p.chain_for(ClipKind::Audio).is_empty(), "the sound's switch did not hold");
    assert_eq!(p.chain_for(ClipKind::Video).iter().map(|f| f.tag()).collect::<Vec<_>>(), ["vhs_tape"]);
    // a patch given only the sound holds the picture's tape too
    let mut m = TimelineModel::new(morph_project());
    let only_sound = MorphPatch { audio_fx: vec![FxInstance::plugin("vhs_tape_sound").unwrap()], ..Default::default() };
    m.apply(Command::AddMorpher(layer(only_sound))).unwrap();
    assert_eq!(patch_of(&m).chain_for(ClipKind::Video).iter().map(|f| f.tag()).collect::<Vec<_>>(), ["vhs_tape"]);
}

fn morph_project() -> Project {
    let mut p = project();
    p.ensure_tracks(LaneKind::Morph, 1);
    p
}

fn layer(patch: MorphPatch) -> MorpherEvent {
    MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(10), ClipId(11)] },
        patch,
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }
}

fn patch_of(m: &TimelineModel) -> MorphPatch {
    m.project().morphers[0].patch.clone()
}

fn chain_ids(fx: &[FxInstance]) -> Vec<&str> {
    fx.iter().map(|f| f.tag()).collect()
}

/// t2990 (fx-graph D2; capabilities `morpher`, `audio-fx`): the sound half of a layer's device is a
/// slot with its own switch. VHS Tape put on a layer's picture holds its sound with the curve; the
/// sound switched off leaves the audio events without the device and the video events with it; the
/// picture's edit reaches the switched-off half's values; removing the tape takes both; undo steps
/// back through each state in one step each.
#[test]
fn t2990_a_layer_s_device_sound_switches_off_alone() {
    kt_plugins_builtin::install();
    let mut m = TimelineModel::new(morph_project());
    m.apply(Command::AddMorpher(layer(MorphPatch::default()))).unwrap();
    let curve = ParamCurve {
        param: "tracking_um".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(450_000), 25.0)]),
        source: Default::default(),
    };
    // the person adds the tape to the layer's picture (the window sends the whole layer)
    let mut ev = m.project().morphers[0].clone();
    ev.patch.video_fx = vec![FxInstance::plugin("vhs_tape").unwrap()];
    ev.patch.video_auto = vec![vec![curve]];
    m.apply(Command::SetMorpher(ev)).unwrap();
    let p = patch_of(&m);
    assert_eq!(chain_ids(&p.audio_fx), ["vhs_tape_sound"], "the layer does not hold the sound half");
    assert_eq!(p.audio_auto[0][0].param, "tracking_um", "the curve did not reach the sound half");
    // the sound half switched off
    let mut ev = m.project().morphers[0].clone();
    ev.patch.audio_bypass = vec![true];
    m.apply(Command::SetMorpher(ev)).unwrap();
    let eff = m.project().effective_clips();
    let a = eff.iter().find(|c| c.kind == ClipKind::Audio).unwrap();
    let v = eff.iter().find(|c| c.kind == ClipKind::Video).unwrap();
    assert!(a.active_fx().iter().all(|f| f.tag() != "vhs_tape_sound"), "the switched-off sound still plays: {:?}", ids(a));
    assert!(v.active_fx().iter().any(|f| f.tag() == "vhs_tape"), "the picture lost the tape: {:?}", ids(v));
    // the picture's edit reaches the switched-off half, which stays off
    let mut ev = m.project().morphers[0].clone();
    ev.patch.video_fx[0].set_param("tracking_um", 7.0);
    m.apply(Command::SetMorpher(ev)).unwrap();
    let p = patch_of(&m);
    assert_eq!(p.audio_fx[0].get_param("tracking_um"), Some(7.0), "the device's rows split");
    assert_eq!(p.audio_bypass, vec![true], "the edit switched the sound back on");
    // removing the tape takes both halves
    let mut ev = m.project().morphers[0].clone();
    ev.patch.video_fx.clear();
    ev.patch.video_auto.clear();
    m.apply(Command::SetMorpher(ev)).unwrap();
    let p = patch_of(&m);
    assert!(p.audio_fx.is_empty() && p.video_fx.is_empty(), "a half outlived its device: {:?}", chain_ids(&p.audio_fx));
    // undo walks back one state per step
    m.undo().unwrap();
    assert_eq!(patch_of(&m).audio_fx[0].get_param("tracking_um"), Some(7.0));
    m.undo().unwrap();
    assert_eq!(patch_of(&m).audio_fx[0].get_param("tracking_um"), patch_of(&m).video_fx[0].get_param("tracking_um"));
    m.undo().unwrap();
    assert!(!patch_of(&m).audio_bypass.first().copied().unwrap_or(false), "undo did not bring the sound back on");
    m.undo().unwrap();
    assert!(patch_of(&m).audio_fx.is_empty(), "undo of the addition left the sound half");
}

/// t2991 (fx-graph D2; capability `morpher`): a project written before the patch held both halves
/// is given them on reading, where the render used to derive them — the effective chains are the
/// ones the derivation gave; the reading is idempotent (a second save and read adds nothing); a half
/// held switched off stays the one held and stays off after a save and a read.
#[test]
fn t2991_an_old_patch_gets_its_halves_on_reading() {
    kt_plugins_builtin::install();
    let curve = ParamCurve {
        param: "tracking_um".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 3.0), AutoKey::at(Tick(450_000), 9.0)]),
        source: Default::default(),
    };
    let gain = FxInstance::plugin("gain").unwrap();
    // as a file before fx-graph held it: the picture's device alone, the sound chain with a gain
    let mut old = morph_project();
    old.morphers.push(layer(MorphPatch {
        video_fx: vec![FxInstance::plugin("datamosh").unwrap()],
        audio_fx: vec![gain],
        ..Default::default()
    }));
    let mut tape = layer(MorphPatch {
        video_fx: vec![FxInstance::plugin("vhs_tape").unwrap()],
        video_auto: vec![vec![curve]],
        ..Default::default()
    });
    tape.id = MorphId(2);
    old.morphers.push(tape);
    let text = project_file::to_text(&old, None);
    let read = project_file::from_text(&text).unwrap().project;
    let mosh = &read.morphers[0].patch;
    assert_eq!(chain_ids(&mosh.audio_fx), ["gain", "datamosh_sound"], "the derived half is not where the render put it");
    assert_eq!(chain_ids(&mosh.chain_for(ClipKind::Audio)), ["gain", "datamosh_sound"]);
    let tp = &read.morphers[1].patch;
    assert_eq!(chain_ids(&tp.audio_fx), ["vhs_tape_sound"]);
    assert_eq!(tp.auto_for(ClipKind::Audio)[0][0].param, "tracking_um", "the derived half lost its curve");
    // idempotent
    let again = project_file::from_text(&project_file::to_text(&read, None)).unwrap().project;
    assert_eq!(again.morphers, read.morphers, "a second reading changed the patches");
    // a half held switched off stays off across a save and a read
    let mut m = TimelineModel::new(read);
    let mut ev = m.project().morphers[1].clone();
    ev.patch.audio_bypass = vec![true];
    m.apply(Command::SetMorpher(ev)).unwrap();
    let back = project_file::from_text(&project_file::to_text(m.project(), None)).unwrap().project;
    let tp = &back.morphers[1].patch;
    assert_eq!(chain_ids(&tp.audio_fx), ["vhs_tape_sound"], "the reading derived a second sound beside the switched-off one");
    assert!(tp.chain_for(ClipKind::Audio).is_empty(), "the switched-off sound came back on after a save");
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

// ---- track-device-sound (kkt-26.11.18b line 29 "Дать звук отдельно") ----

fn track(m: &TimelineModel, id: TrackId) -> Track {
    m.project().track(id).cloned().unwrap()
}

fn track_ids(t: &Track) -> Vec<String> {
    t.fx.iter().map(|f| f.tag().to_string()).collect()
}

/// A project with Video 1, Video 2, Audio 1 (a gain), Audio 2.
fn track_project() -> (Project, [TrackId; 4]) {
    let mut p = project();
    p.ensure_tracks(LaneKind::Video, 2);
    p.ensure_tracks(LaneKind::Audio, 2);
    let id = |p: &Project, k, l| p.track_at(k, l).unwrap().id;
    let ids = [id(&p, LaneKind::Video, 0), id(&p, LaneKind::Video, 1), id(&p, LaneKind::Audio, 0), id(&p, LaneKind::Audio, 1)];
    p.track_mut(ids[2]).unwrap().fx = vec![FxInstance::plugin("gain").unwrap()];
    (p, ids)
}

/// t3230 (capability `audio-fx`): VHS Tape on a whole video track sounds on its sound track — the
/// audio track of the same number by default; values both ways, each half's switch its own, removal
/// takes both, each one undo step.
#[test]
fn t3230_a_track_device_sounds_on_its_sound_track() {
    kt_plugins_builtin::install();
    let (p, [v1, _v2, a1, _a2]) = track_project();
    let mut m = TimelineModel::new(p);
    let tape = FxInstance::plugin("vhs_tape").unwrap();
    m.apply(Command::SetTrackFx { id: v1, fx: vec![tape], bypass: vec![] }).unwrap();
    assert_eq!(track(&m, v1).sound, Some(a1), "the default sound track is the audio track of the same number");
    assert_eq!(track_ids(&track(&m, a1)), ["gain", "vhs_tape_sound"]);
    m.undo().unwrap();
    assert_eq!(track(&m, v1).sound, None, "one undo takes the link too");
    assert_eq!(track_ids(&track(&m, a1)), ["gain"]);
    m.redo().unwrap();
    // the picture's tracking reaches the sound half
    let mut v = track(&m, v1);
    v.fx[0].set_param("tracking_um", 12.0);
    m.apply(Command::SetTrackFx { id: v1, fx: v.fx, bypass: v.fx_bypass }).unwrap();
    assert_eq!(track(&m, a1).fx[1].get_param("tracking_um"), Some(12.0));
    // the sound half's audio output, set on the audio track, reaches the picture half
    let mut a = track(&m, a1);
    a.fx[1].set_param("audio_out", 1.0);
    m.apply(Command::SetTrackFx { id: a1, fx: a.fx.clone(), bypass: a.fx_bypass.clone() }).unwrap();
    assert_eq!(track(&m, v1).fx[0].get_param("audio_out"), Some(1.0), "the deck is one");
    // the sound half switched off alone
    m.apply(Command::SetTrackFx { id: a1, fx: a.fx.clone(), bypass: vec![false, true] }).unwrap();
    assert_eq!(track(&m, a1).fx_bypass, [false, true]);
    assert_eq!(track_ids(&track(&m, a1)), ["gain", "vhs_tape_sound"], "a switched-off half stays");
    assert!(track(&m, v1).fx_bypass.iter().all(|b| !b), "the picture half stays on");
    // removal takes both
    m.apply(Command::SetTrackFx { id: v1, fx: vec![], bypass: vec![] }).unwrap();
    assert_eq!(track_ids(&track(&m, a1)), ["gain"]);
    // Camera ISP pairs on a track too; Datamosh does not: a stream slot on a track stage is skipped
    // by the engine, so its sound would be the sound of a device that is not on the picture
    let isp = FxInstance::plugin("camera_isp").unwrap();
    let mosh = FxInstance::plugin("datamosh").unwrap();
    m.apply(Command::SetTrackFx { id: v1, fx: vec![mosh, isp], bypass: vec![] }).unwrap();
    assert_eq!(track_ids(&track(&m, a1)), ["gain", "camera_isp_sound"]);
}

/// t3231 (capability `audio-fx`): the link moves the halves, a cleared link stays cleared, an audio
/// track is the sound of one video track, removing the sound track clears the link.
#[test]
fn t3231_the_link_moves_clears_and_stays_one_to_one() {
    kt_plugins_builtin::install();
    let (p, [v1, v2, a1, a2]) = track_project();
    let mut m = TimelineModel::new(p);
    m.apply(Command::SetTrackFx { id: v1, fx: vec![FxInstance::plugin("vhs_tape").unwrap()], bypass: vec![] }).unwrap();
    let props = |m: &TimelineModel, id, s| TrackProps { sound: s, ..track(m, id).props() };
    m.apply(Command::SetTrackProps { id: v1, props: props(&m, v1, Some(a2)) }).unwrap();
    assert_eq!(track_ids(&track(&m, a1)), ["gain"], "the half left the former sound track");
    assert_eq!(track_ids(&track(&m, a2)), ["vhs_tape_sound"]);
    m.apply(Command::SetTrackProps { id: v1, props: props(&m, v1, None) }).unwrap();
    assert!(track(&m, a2).fx.is_empty(), "clearing takes the half off");
    // an edit of the device does not bring the link back (rule 20)
    let mut v = track(&m, v1);
    v.fx[0].set_param("tracking_um", 5.0);
    m.apply(Command::SetTrackFx { id: v1, fx: v.fx, bypass: v.fx_bypass }).unwrap();
    assert_eq!(track(&m, v1).sound, None);
    assert!(track(&m, a1).fx.len() == 1 && track(&m, a2).fx.is_empty());
    // one-to-one: Video 2 cannot name Audio 1 while Video 1 does
    m.apply(Command::SetTrackProps { id: v1, props: props(&m, v1, Some(a1)) }).unwrap();
    assert!(m.apply(Command::SetTrackProps { id: v2, props: props(&m, v2, Some(a1)) }).is_err());
    // nor a video track, nor itself
    assert!(m.apply(Command::SetTrackProps { id: v2, props: props(&m, v2, Some(v1)) }).is_err());
    // removing the sound track clears the link
    m.apply(Command::RemoveTrack { kind: LaneKind::Audio, at: 0 }).unwrap();
    assert_eq!(track(&m, v1).sound, None);
}

/// t3233 (capability `project-format`): a sound link writes format 6 and reads back; without it the
/// file keeps its version; a link to a video track is refused with the track's address.
#[test]
fn t3233_a_track_sound_link_raises_the_format() {
    kt_plugins_builtin::install();
    let (mut p, [v1, v2, a1, _]) = track_project();
    let plain = project_file::to_text(&p, None);
    assert!(plain.contains("\"kinetren\": 1"));
    assert!(!plain.contains("\"sound\""));
    p.track_mut(v1).unwrap().sound = Some(a1);
    let text = project_file::to_text(&p, None);
    assert!(text.contains("\"kinetren\": 6"), "the link did not raise the format");
    assert_eq!(project_file::from_text(&text).unwrap().project, p);
    let newer = text.replace("\"kinetren\": 6", &format!("\"kinetren\": {}", project_file::PROJECT_FILE_READS + 1));
    assert!(matches!(project_file::from_text(&newer), Err(project_file::ProjectFileError::TooNew { .. })));
    p.track_mut(v1).unwrap().sound = Some(v2);
    assert!(matches!(p.validate(), Err(ProjectError::BadTrackSound(t)) if t == v1));
}
