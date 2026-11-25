//! loudness-norms (project-format): the loudness target and the program stage in the file.

use kt_model::*;

fn project() -> Project {
    Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    })
}

fn text(p: &Project) -> String {
    project_file::to_text(p, None)
}

/// t3966: the target and the stage round trip and raise the format to 9 by content; a project at the
/// defaults writes not a byte more; a reader of the former version refuses; the commands refuse what is
/// out of range and undo; a reel standing as the root has no program.
#[test]
fn t3966_the_loudness_is_written_by_content() {
    kt_plugins_builtin::install();
    let p = project();
    assert_eq!(project_file::version_for(&p), 1, "the defaults keep format 1");
    let plain = text(&p);
    assert!(!plain.contains("loudness") && !plain.contains("program"), "the defaults are written: {plain}");

    let mut q = p.clone();
    q.loudness = LoudnessTarget::of(LoudnessStandard::Streaming);
    q.program = Some(ProgramStage {
        gain_db: 5.5,
        measured: Some(ProgramMeasured { lufs: -19.5, dbtp: 1.0, reached: -14.0, target: -14.0, ceiling: -1.0, gain_db: 5.5, sound: u64::MAX - 1 }),
    });
    q.validate().unwrap();
    assert_eq!(project_file::version_for(&q), project_file::PROJECT_FILE_VERSION_LOUDNESS);
    assert_eq!(project_file::PROJECT_FILE_VERSION_LOUDNESS, 9);
    let t = text(&q);
    assert_eq!(project_file::from_text(&t).unwrap().project, q, "the round trip");
    // a reader of a lower version is told the version: the same text, its number past what is read
    let above = t.replacen("\": 9", &format!("\": {}", project_file::PROJECT_FILE_READS + 1), 1);
    assert_ne!(above, t);
    let refused = format!("{:?}", project_file::from_text(&above).expect_err("a newer format is refused"));
    assert!(refused.contains(&(project_file::PROJECT_FILE_READS + 1).to_string()), "the refusal names the version: {refused}");
    // the target alone and the stage alone raise it too
    let mut only = p.clone();
    only.program = Some(ProgramStage::default());
    assert_eq!(project_file::version_for(&only), 9);

    let mut m = TimelineModel::new(p.clone());
    m.apply(Command::SetLoudness { target: q.loudness }).unwrap();
    m.apply(Command::SetProgram { stage: q.program }).unwrap();
    assert_eq!((m.project().loudness, m.project().program), (q.loudness, q.program));
    let off = LoudnessTarget { standard: LoudnessStandard::Streaming, target_lufs: -20.0, ceiling_dbtp: -1.0 };
    assert!(m.apply(Command::SetLoudness { target: off }).is_err(), "a standard carries its own numbers");
    let deaf = LoudnessTarget { standard: LoudnessStandard::Custom, target_lufs: f64::NAN, ceiling_dbtp: -1.0 };
    assert!(m.apply(Command::SetLoudness { target: deaf }).is_err());
    assert!(m.apply(Command::SetProgram { stage: Some(ProgramStage { gain_db: 99.0, measured: None }) }).is_err());
    m.undo().and_then(|_| m.undo()).unwrap();
    assert_eq!((m.project().loudness, m.project().program), (p.loudness, p.program), "one undo each");

    let mut reels = q.clone();
    reels.sequences.push(Sequence::new(MediaId(7), "R"));
    assert_eq!(reels.reel_as_root(MediaId(7)).unwrap().program, None, "a reel is not the program");
}
