//! t1460 (sim-audio-pairs; capability `plugins`): a simulated device's two halves are one passport.
//! The registry refuses, by name and in either registration order, a pair whose passports differ, a
//! pair that is not an audio effect, an audio kind that declares a pair, a pair declared by a kind
//! that is not a video effect, and two video kinds naming one audio kind; `counterpart` answers both
//! ways and `unpaired` names a pair whose audio half never came.

use std::sync::Arc;

use kt_plugin_api::{Decl, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, Registry, VideoClass};

struct P(Manifest);

impl Plugin for P {
    fn manifest(&self) -> &Manifest {
        &self.0
    }
}

fn rows(default: f64) -> Vec<ParamSpec> {
    vec![ParamSpec::num("spacing_um", 0.0, 5.0, default, 0.01, ParamUnit::Um, ParamKind::Smooth)]
}

fn video(id: &'static str, pair: Option<&'static str>, default: f64) -> Arc<dyn Plugin> {
    Arc::new(P(Manifest {
        id: id.into(),
        media: Media::VideoFx,
        passport: rows(default),
        rows: Vec::new(),
        decl: Decl { video_class: Some(VideoClass::PointWise), builtin: false, pair, ..Decl::default() },
        ui: Default::default(),
    }))
}

fn audio(id: &'static str, pair: Option<&'static str>, default: f64) -> Arc<dyn Plugin> {
    Arc::new(P(Manifest {
        id: id.into(),
        media: Media::AudioFx,
        passport: rows(default),
        rows: Vec::new(),
        decl: Decl { state: true, builtin: false, pair, ..Decl::default() },
        ui: Default::default(),
    }))
}

fn refused(reg: &mut Registry, p: Arc<dyn Plugin>, what: &str) {
    let id = p.manifest().id.to_string();
    let e = reg.register(p).expect_err(&format!("{id} must be refused: {what}"));
    assert!(e.reason.contains(what), "{id}: refused for '{}', expected '{what}'", e.reason);
}

#[test]
fn t1460_a_pair_is_one_passport() {
    // video first, then its sound: paired both ways
    let mut reg = Registry::new();
    reg.register(video("tape", Some("tape_sound"), 0.05)).unwrap();
    assert_eq!(reg.unpaired(), vec![("tape".to_string(), "tape_sound")]);
    reg.register(audio("tape_sound", None, 0.05)).unwrap();
    assert!(reg.unpaired().is_empty());
    assert_eq!(reg.counterpart("tape"), Some("tape_sound"));
    assert_eq!(reg.counterpart("tape_sound"), Some("tape"));

    // a second video naming the same sound
    refused(&mut reg, video("tape2", Some("tape_sound"), 0.05), "two video kinds name the same audio pair");
    // a pair declared by an audio kind
    refused(&mut reg, audio("echo", Some("tape"), 0.05), "not a video effect");

    // the sound first, a passport that differs: refused when the video comes
    let mut reg = Registry::new();
    reg.register(audio("deck_sound", None, 0.3)).unwrap();
    refused(&mut reg, video("deck", Some("deck_sound"), 0.05), "passport differs");
    // the video first, the sound differs: refused when the sound comes
    let mut reg = Registry::new();
    reg.register(video("deck", Some("deck_sound"), 0.05)).unwrap();
    refused(&mut reg, audio("deck_sound", None, 0.3), "passport differs");
    // the named pair is a video kind
    let mut reg = Registry::new();
    reg.register(video("other", None, 0.05)).unwrap();
    refused(&mut reg, video("deck", Some("other"), 0.05), "not an audio effect");
    // an audio pair that declares a pair itself
    let mut reg = Registry::new();
    reg.register(video("deck", Some("deck_sound"), 0.05)).unwrap();
    refused(&mut reg, audio("deck_sound", Some("deck"), 0.05), "not a video effect");
    // an unpaired kind has no counterpart
    assert_eq!(reg.counterpart("deck"), None);
}
