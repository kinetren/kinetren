//! Test 151 (item 151, R.24; R.70 — its own format): kt-cli on
//! a `.kt` project file and two exchange doors.
//!
//! new/demo write a project file of their own format; export-otio gives a
//! self-contained OTIO, roundtrip on it is byte-stable; import reads
//! a legacy .ktproj and .otio into `.kt`; render/probe reject an exchange file and
//! a legacy one with a hint toward import.

use std::process::Command;

/// The kt-cli binary. A cross-built test archive runs on another machine (windows-build: the
/// Windows CI job), where the compile-time `CARGO_BIN_EXE_kt-cli` is a path of the build
/// container; nextest gives the remapped path at run time.
fn kt_cli_exe() -> std::ffi::OsString {
    std::env::var_os("NEXTEST_BIN_EXE_kt-cli")
        .or_else(|| std::env::var_os("NEXTEST_BIN_EXE_kt_cli"))
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_kt-cli").into())
}

fn cli() -> Command {
    Command::new(kt_cli_exe())
}

#[test]
fn t151_cli_otio_workflows() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t151-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // new → a project file of ITS OWN format: the marker and version in the first line
    let new_path = dir.join("empty.kt");
    let st = cli().args(["new", new_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let text = std::fs::read_to_string(&new_path).unwrap();
    assert!(text.starts_with("{\n  \"kinetren\": 1"), "{text:.80}");
    assert!(!text.contains("OTIO_SCHEMA"), "a project file is not an interchange file");

    // demo → a rich project (transition, markers, region, morpher)
    let demo_path = dir.join("demo.kt");
    let st = cli().args(["demo", demo_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let demo = std::fs::read_to_string(&demo_path).unwrap();
    assert!(demo.contains("\"transitions\""));
    assert!(demo.contains("\"markers\""));

    // export-otio → a self-contained exchange file
    let otio_path = dir.join("demo.otio");
    let st = cli()
        .args(["export-otio", demo_path.to_str().unwrap(), otio_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let otio = std::fs::read_to_string(&otio_path).unwrap();
    assert!(otio.starts_with("{\n    \"OTIO_SCHEMA\": \"Timeline.1\""));
    assert!(otio.contains("\"OTIO_SCHEMA\": \"Transition.1\""));
    assert!(otio.contains("\"OTIO_SCHEMA\": \"Marker.2\""));

    // roundtrip of the exchange door: byte-stable
    let rt_path = dir.join("demo-rt.otio");
    let st = cli()
        .args(["roundtrip", otio_path.to_str().unwrap(), rt_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success());
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(err.contains("byte-stable: true"), "{err}");
    assert_eq!(std::fs::read_to_string(&rt_path).unwrap(), otio);

    // import: exchange file → project file, and it is field-for-field equal to the source
    let back_path = dir.join("back.kt");
    let st = cli()
        .args(["import", otio_path.to_str().unwrap(), back_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let back = kt_model::project_file::from_text(&std::fs::read_to_string(&back_path).unwrap())
        .unwrap()
        .project;
    let orig = kt_model::project_file::from_text(&demo).unwrap().project;
    assert_eq!(back.clips, orig.clips, "round trip kt → otio → kt loses events");
    assert_eq!(back.transitions, orig.transitions);
    assert_eq!(back.morphers, orig.morphers);

    // import: legacy fixture → project file
    let legacy = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo.ktproj");
    let conv_path = dir.join("converted.kt");
    let st = cli().args(["import", legacy, conv_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(std::fs::read_to_string(&conv_path).unwrap().starts_with("{\n  \"kinetren\": "));

    // render/probe reject a legacy file and an exchange file with a hint toward import
    let wav = dir.join("out.wav");
    for (what, input) in [("legacy", legacy.to_string()), ("otio", otio_path.display().to_string())]
    {
        let st = cli().args(["render", &input, wav.to_str().unwrap()]).output().unwrap();
        assert!(!st.status.success(), "{what}: render must refuse");
        let said = String::from_utf8_lossy(&st.stderr);
        assert!(said.contains("kt-cli import"), "{what}: a hint on import is required: {said}");
        assert!(!said.contains("convert"), "{what}: the old “convert” hint is dead: {said}");
    }
    // import of a project file — “already a project”, not a silent success
    let st = cli()
        .args(["import", demo_path.to_str().unwrap(), dir.join("x.kt").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!st.status.success());
    assert!(String::from_utf8_lossy(&st.stderr).contains("already a Kinetren project"));

    std::fs::remove_dir_all(&dir).ok();
}

/// Test 290 (R.43, item 298): smoke test of the `probe` tool.
///
/// Parsing the user’s complaint (“before, the sound came from a source
/// ‘far away’, and with the problems — a ‘salute’”) would take minutes instead of
/// hours if such an answer could be obtained in one command. Here the tool itself
/// is held — all four of its sections are in place and it does not crash on
/// a real project. THE BEHAVIOR ITSELF (a layer does not reach an event outside
/// its window) is guarded by t288 in kt-model: a test on printing cannot
/// replace it.
#[test]
fn t290_probe_reports_what_layers_changed() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t290-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("probe.kt");

    // the demo project carries a morpher; we replace it with a layer FAR from the event
    let st = cli().args(["demo", path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    let probe = |from: &str, to: &str| -> String {
        let out = cli().args(["probe", path.to_str().unwrap(), from, to]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    };

    // the demo project has events in the first second and no foreign media
    let head = probe("0", "2");
    assert!(head.contains("effective clips"), "probe must show the events:\n{head}");
    assert!(
        head.contains("no foreign media in the order"),
        "there must be no foreign media in a sound place:\n{head}"
    );
    assert!(
        head.contains("clips with a time warp"),
        "probe must name the number of events with a warp — that is how a foreign layer is seen:\n{head}"
    );
    // Test 318 (R.46, item 322): for the complaint “zoom still smears” one must look
    // at the decode step, and probe must name it — together with
    // the native size of the file, because of which the step may not exist at all
    assert!(
        head.contains("video decode order"),
        "probe must show the VIDEO order, not only the audio:\n{head}"
    );
    assert!(head.contains("stage x"), "the video order must include a decode scale step:\n{head}");

    // Test 452b (R.50, B6): for a nested edit, “enter N levels to understand where
    // the frame comes from” is our minus. The reel tree and the PATH of an
    // event are printed at once.
    let nested = dir.join("nested.kt");
    {
        let text = std::fs::read_to_string(&path).unwrap();
        let mut pr = kt_model::project_file::from_text(&text).unwrap().project;
        let mut n = kt_model::ops::NestCounters::of(&pr);
        let ids: Vec<kt_model::ClipId> =
            pr.clips.iter().filter(|c| c.start.0 < 2 * 90_000).map(|c| c.id).collect();
        let (cmds, rep) = kt_model::ops::collapse_into_sequence(&pr, &ids, "Intro", n.ids());
        let mut m = kt_model::TimelineModel::new(pr);
        m.apply_batch(cmds).expect("collapsing in the demo project");
        assert!(rep.seq.is_some());
        pr = m.project().clone();
        std::fs::write(&nested, kt_model::project_file::to_text(&pr, None)).unwrap();
    }
    let out = cli().args(["probe", nested.to_str().unwrap(), "0", "2"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let nest_head = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(nest_head.contains("nested reels"), "probe must print the reel tree:\n{nest_head}");
    assert!(
        nest_head.contains("Intro") && nest_head.contains("places"),
        "the reel tree must have the name and the number of slots:\n{nest_head}"
    );
    assert!(
        nest_head.contains("is a NESTED REEL"),
        "probe must name the PATH of the nested event:\n{nest_head}"
    );

    // R.56 (item 448): for a STRETCHED instance the window of content is not equal to its
    // length on the timeline — printing the length would lie exactly at the place
    // for which probe is run (“here is a frame from another place”).
    let warped = dir.join("nested_warped.kt");
    {
        let text = std::fs::read_to_string(&nested).unwrap();
        let mut pr = kt_model::project_file::from_text(&text).unwrap().project;
        let id = pr
            .clips
            .iter()
            .find(|c| pr.sequence(c.media).is_some() && c.kind == kt_model::ClipKind::Video)
            .map(|c| c.id)
            .expect("reel instance is present");
        let c = pr.clips.iter_mut().find(|c| c.id == id).unwrap();
        c.reversed = true;
        c.fx.push(kt_model::FxInstance::slowed_reverb(0.5, 0.0, 85.0));
        pr.validate().expect("a mirrored slowed instance is legal");
        std::fs::write(&warped, kt_model::project_file::to_text(&pr, None)).unwrap();
    }
    let out = cli().args(["probe", warped.to_str().unwrap(), "0", "2"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let warp_head = String::from_utf8_lossy(&out.stdout).to_string();
    let line = warp_head
        .lines()
        .find(|l| l.contains("is a NESTED REEL") && l.contains("REVERSED"))
        .unwrap_or_else(|| panic!("the mirrored instance is not named mirrored:\n{warp_head}"));
    assert!(
        line.contains("warped") && line.contains("consumed"),
        "for a stretched instance probe must print the ACTUALLY consumed \
         reel window, not the length of the event: {line}"
    );

    // the layer of the demo project aims at events 7 and 9; at the end of the timeline
    // there are none, so there must be no edits — but the events must exist, otherwise
    // “nothing changed” would hold by itself
    let far = probe("95", "99");
    assert!(far.contains("ClipId("), "events must be present at this place:\n{far}");
    let changed = far.split("what morph layers changed here").nth(1).unwrap_or("");
    let changed = changed.split("\n\n").next().unwrap_or("");
    assert!(
        changed.contains("(nothing)"),
        "the layer has nothing to do with this place, yet probe showed edits: {changed:?}"
    );

    // --time answers the question “does the engine keep up HERE” (R.43, item 301):
    // after item 294 the starvation of the ring is heard as silence, not as slowdown, and without a
    // number there is nothing to compare it with
    let timed = cli().args(["probe", path.to_str().unwrap(), "0", "2", "--time"]).output().unwrap();
    assert!(timed.status.success(), "{}", String::from_utf8_lossy(&timed.stderr));
    let timed = String::from_utf8_lossy(&timed.stdout);
    assert!(
        timed.contains("% of real time"),
        "probe --time must name the share of real time:\n{timed}"
    );
    // synthetics are by definition cheaper than real time — “we don’t keep up” must not
    // be printed here, otherwise the line means nothing
    assert!(
        !timed.contains("OVER BUDGET"),
        "the synthetic demo project must fit the budget:\n{timed}"
    );
}

/// Test 307 (item 316, R.45): relative media paths are NOT doubled.
/// `kt_otio::load_project` already resolves target_url from the project directory;
/// kt-cli on top of this joined the base a SECOND time, and a call with a relative
/// path to the .otio (`kt-cli export sub/p.otio …`) lost the media
/// (“sub/sub/clip.mp4: decode failed”), silently rendering empty layers.
#[test]
fn t307_relative_media_paths_resolve_once() {
    kt_plugins_builtin::install();
    if !Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    {
        eprintln!("skipped: ffmpeg binary unavailable");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-cli-t307-{}", std::process::id()));
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let st = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=30"])
        .args(["-t", "1", "-pix_fmt", "yuv420p", "-c:v", "libx264"])
        .arg(sub.join("clip.mp4"))
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    let mut p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "clip.mp4"));
    p.clips.push(kt_model::ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: kt_model::ClipId(1),
        link_group: None,
        kind: kt_model::ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: kt_model::MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    std::fs::write(sub.join("p.kt"), kt_model::project_file::to_text(&p, None)).unwrap();

    // a RELATIVE path to the project from the directory ABOVE it — before the fix the media
    // resolved to sub/sub/clip.mp4 and the layer silently disappeared
    let out = cli()
        .current_dir(&dir)
        .args(["export", "sub/p.kt", "sub/out.mp4", "libx264"])
        .output()
        .unwrap();
    let all =
        format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{all}");
    assert!(!all.contains("decode failed"), "media lost: {all}");
    assert!(all.contains("missing layers 0"), "empty layers: {all}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 387 (R.49 A11): kt-cli does not overwrite an existing file silently. It has
/// no asterisk and no dialog — there is nobody to stop it; therefore overwriting
/// only by an explicit `--force`.
#[test]
fn t387_cli_does_not_overwrite_without_force() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t387-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("p.kt");
    std::fs::write(&path, "the user’s work").unwrap();

    for cmd in [["new"], ["demo"]] {
        let st = cli().args([cmd[0], path.to_str().unwrap()]).output().unwrap();
        assert!(!st.status.success(), "{} overwrote an existing file", cmd[0]);
        let err = String::from_utf8_lossy(&st.stderr);
        assert!(err.contains("--force"), "{}: it is not said how to resolve it: {err}", cmd[0]);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "the user’s work",
            "{} rewrote the file after all",
            cmd[0]
        );
    }

    // with --force it writes, and writes ATOMICALLY: no temporary file is left behind
    let st = cli().args(["new", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(std::fs::read_to_string(&path).unwrap().contains("\"kinetren\""));
    let leftovers: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "a temporary file was left: {leftovers:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 388 (R.49 A11): `demo` and `render` work in a row WITHOUT a single
/// media file. Eight paths `media/*` that the demo referred to do not
/// exist anywhere in the distribution — that is, the demo was not rendered by anyone who
/// had not assembled these files themselves. `synth:` is a full-fledged source across the
/// whole program, and now it is in the demo by default.
#[test]
fn t388_demo_renders_without_any_media_files() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t388-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("demo.kt");
    let wav = dir.join("demo.wav");

    let st = cli().args(["demo", proj.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let text = std::fs::read_to_string(&proj).unwrap();
    assert!(text.contains("synth:"), "the default demo does not refer to synthetics");
    assert!(!text.contains("media/"), "non-existent files remain in the demo: {text:.400}");

    let st =
        cli().args(["render", proj.to_str().unwrap(), wav.to_str().unwrap()]).output().unwrap();
    assert!(
        st.status.success(),
        "the demo does not render out of the box: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    let len = std::fs::metadata(&wav).unwrap().len();
    assert!(len > 1_000_000, "audio is suspiciously short: {len} bytes");

    // the former file names remain available by an explicit flag
    let files = dir.join("demo-files.kt");
    let st = cli().args(["demo", files.to_str().unwrap(), "--files"]).output().unwrap();
    assert!(st.status.success());
    assert!(std::fs::read_to_string(&files).unwrap().contains("media/"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 389 (R.49 A11): rendering with unavailable media is a REFUSAL, not a silent
/// silence in the output file; `--allow-missing` switches this on consciously.
#[test]
fn t389_render_refuses_missing_media_unless_allowed() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t389-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("p.kt");
    let wav = dir.join("p.wav");
    let st = cli().args(["demo", proj.to_str().unwrap(), "--files"]).output().unwrap();
    assert!(st.status.success());

    let st =
        cli().args(["render", proj.to_str().unwrap(), wav.to_str().unwrap()]).output().unwrap();
    assert!(!st.status.success(), "render with missing media must refuse");
    assert!(!wav.exists(), "a file was written in which silence stands in place of the material");

    let st = cli()
        .args(["render", proj.to_str().unwrap(), wav.to_str().unwrap(), "--allow-missing"])
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(wav.exists(), "with --allow-missing the render must succeed");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 727 (R.62): probe names the conform rule, the kind of source, the
/// decode size and the frame of each video event in the window — the first thing
/// that will be asked about a complaint “the picture is stretched/cropped”.
#[test]
fn t727_probe_reports_conform_geometry() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t727-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("probe.kt");
    let st = cli().args(["demo", path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let out = cli().args(["probe", path.to_str().unwrap(), "0", "2"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("conform"), "probe must print the conform:\n{text}");
    assert!(text.contains("rule="), "…with the event rule:\n{text}");
    assert!(text.contains("decode="), "…and the decode size:\n{text}");
}

/// Test 740b (R.62, change B): kt-cli says WHAT WILL NOT GET INTO THE FILE —
/// muted tracks and an active solo, with the same wording as the
/// render window, and BEFORE the render. There is no refusal: mute and solo are the content of the project
/// (the owner’s decision), but staying silent about them is not allowed.
#[test]
fn t740b_cli_reports_muted_and_solo_before_render() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t740b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.kt");
    let st = cli().args(["demo", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    // without muted tracks — not a single extra line
    let wav = dir.join("out.wav");
    let out = cli()
        .args(["render", path.to_str().unwrap(), wav.to_str().unwrap(), "--allow-missing"])
        .output()
        .unwrap();
    let clean = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!clean.contains("muted tracks"), "extra line on a project without mute: {clean}");

    // we mute the first audio track and enable solo on video — both lines
    let text = std::fs::read_to_string(&path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    {
        // the track objects of the model: line order top to bottom (video, then audio)
        v["project"]["tracks"] = serde_json::json!([
            {"kind": "video", "id": 1, "solo": true},
            {"kind": "audio", "id": 2, "mute": true},
        ]);
    }
    let muted_path = dir.join("muted.kt");
    std::fs::write(&muted_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let out = cli()
        .args(["render", muted_path.to_str().unwrap(), wav.to_str().unwrap(), "--allow-missing"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "render must proceed, not refuse: {said}");
    assert!(said.contains("muted tracks"), "kt-cli is silent about a muted track: {said}");
    assert!(said.contains("solo is on"), "kt-cli is silent about an active solo: {said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 746 (R.62, change B, 4.6): `probe` prints the TRACKS. Complaints
/// “the track can’t be heard” and “the picture is lighter than in the preview” are parsed
/// precisely by these lines: switches, level, blend mode, and whether the
/// track has a stage (a chain on the sum of events for audio, an implicit reel for
/// video). ALL rows are printed: “the track is in place and untouched by anything” is
/// the same answer as “muted”.
#[test]
fn t746_probe_prints_tracks_with_switches_level_blend_and_stage() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t746-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.kt");
    let st = cli().args(["demo", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    let plain = String::from_utf8_lossy(
        &cli().args(["probe", path.to_str().unwrap(), "0", "5"]).output().unwrap().stdout,
    )
    .to_string();
    assert!(plain.contains("-- tracks --"), "probe has no tracks section: {plain}");
    assert!(plain.contains("V1"), "a track without novelties is not shown — it must be visible too");
    assert!(!plain.contains("MUTE"), "switch on an untouched track: {plain}");
    assert!(!plain.contains("stage:"), "stage on a track without chain, level and mode");

    // a track with level, mode, muting and solo
    let text = std::fs::read_to_string(&path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    {
        v["project"]["tracks"] = serde_json::json!([
            {"kind": "video", "id": 1, "name": "Titles", "opacity": 0.5,
             "blend": "multiply", "solo": true},
            {"kind": "audio", "id": 2, "gain_db": -6.0, "mute": true},
        ]);
    }
    let p2 = dir.join("tracks.kt");
    std::fs::write(&p2, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let said = String::from_utf8_lossy(
        &cli().args(["probe", p2.to_str().unwrap(), "0", "5"]).output().unwrap().stdout,
    )
    .to_string();
    let tracks = said
        .split("-- tracks --")
        .nth(1)
        .unwrap_or("")
        .split("-- ")
        .next()
        .unwrap_or("")
        .to_string();
    for want in [
        "Titles",        // the track name, not a formula
        "opacity 0.50", // video level
        "Multiply",     // blend mode
        "SOLO",
        "-6.0 dB", // audio level
        "MUTE",
        "stage:audio", // −6 dB on audio = the track’s stage
        "stage:video", // opacity and mode = an implicit reel
    ] {
        assert!(tracks.contains(want), "the tracks section has no “{want}”:\n{tracks}");
    }
    assert!(
        said.contains("solo active") && said.contains("muted:"),
        "probe did not say what from this will not reach the file: {said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 748b (R.63 item 2): `probe` names the end of the material.
///
/// For the complaint “silence at the end” the first thing asked is probe — and it must
/// say that the material ended before the event, and not make one count
/// this in one’s head from the file length and the speed.
#[test]
fn t748b_probe_names_the_material_end() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t748b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("edge.kt");
    let st = cli().args(["demo", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    // the event is LONGER than the material: the synthetic source lasts an hour
    // (`SYNTH_DURATION`), so we stretch the event to an hour and a half
    let text = std::fs::read_to_string(&path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut patched = 0usize;
    fn stretch(x: &mut serde_json::Value, n: &mut usize) {
        match x {
            serde_json::Value::Object(m) => {
                // an event of the model: it has both `source_in` and `duration`
                if m.contains_key("source_in")
                    && let Some(val) = m.get_mut("duration")
                {
                    *val = serde_json::json!(5400i64 * 90_000);
                    *n += 1;
                }
                for (_, val) in m.iter_mut() {
                    stretch(val, n);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(|val| stretch(val, n)),
            _ => {}
        }
    }
    stretch(&mut v, &mut patched);
    assert!(patched > 0, "scene degenerated: there are no events in the demo project");
    let long = dir.join("long.kt");
    std::fs::write(&long, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let out = cli().args(["probe", long.to_str().unwrap(), "0", "2"]).output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        said.contains("material ends at"),
        "probe is silent about the end of the material — for the complaint “silence at the end” there is nothing to ask:\n{said}"
    );
    assert!(
        said.contains("not starvation"),
        "probe did not say that silence past the edge is not starvation: {said}"
    );

    // the event is INSIDE the material — not a single extra line (rule 9)
    let out = cli().args(["probe", path.to_str().unwrap(), "0", "2"]).output().unwrap();
    let clean = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        !clean.contains("material ends at"),
        "an event inside the material must not have an end line: {clean}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Copies of the generated fixture videos in a fresh directory (gen.sh runs
/// once per process: the tests run in parallel).
fn showcase_sources(tag: &str) -> std::path::PathBuf {
    static GEN: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    // on Windows the fixtures come generated with the test archive (no bash to run gen.sh)
    let pregenerated = cfg!(windows) && fixtures.join("out").is_dir();
    GEN.get_or_init(|| {
        if pregenerated {
            return;
        }
        let st = Command::new("bash")
            .arg(fixtures.join("gen.sh"))
            .current_dir(&fixtures)
            .status()
            .expect("run fixtures/gen.sh (needs ffmpeg in PATH)");
        assert!(st.success(), "gen.sh failed");
    });
    let dir = std::env::temp_dir().join(format!("kt-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("sources");
    std::fs::create_dir_all(&src).unwrap();
    for e in std::fs::read_dir(fixtures.join("out")).unwrap() {
        let p = e.unwrap().path();
        // the videos, and one audio-only file: music for the beds
        if p.extension().is_some_and(|x| x == "mp4")
            || p.file_name().is_some_and(|n| n == "sine48k_stereo.flac")
        {
            std::fs::copy(&p, src.join(p.file_name().unwrap())).unwrap();
        }
    }
    dir
}

/// t824 (readme-showcase): `showcase` cuts a dense, valid, deterministic
/// project from a directory of real footage — the README's timeline.
#[test]
fn t824_showcase_is_rich_valid_and_deterministic() {
    kt_plugins_builtin::install();
    use kt_model::{ClipKind, GeneratorKind};
    let dir = showcase_sources("t824");
    let src = dir.join("sources");
    let run = |out: &std::path::Path| {
        let st = cli()
            .args(["showcase", out.to_str().unwrap(), "--sources", src.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
        std::fs::read_to_string(out).unwrap()
    };
    let a = run(&dir.join("a.kt"));
    let b = run(&dir.join("b.kt"));
    assert!(a == b, "two runs on the same directory must give the same bytes");
    let p = kt_model::project_file::from_text(&a).unwrap().project;
    p.validate().expect("the reloaded showcase validates");

    let canon = std::fs::canonicalize(&src).unwrap();
    let files: Vec<String> = std::fs::read_dir(&src)
        .unwrap()
        .map(|e| e.unwrap().path().file_name().unwrap().to_string_lossy().into_owned())
        .filter(|f| f.ends_with(".mp4"))
        .collect();
    for m in p.media.iter().filter(|m| !kt_model::is_pseudo_path(&m.path)) {
        let path = std::path::Path::new(&m.path);
        assert!(
            path.is_absolute() && path.starts_with(&canon),
            "absolute, into the dir: {}",
            m.path
        );
    }

    let lane = |kind: ClipKind, lane: u32| -> Vec<&kt_model::ClipEvent> {
        p.clips.iter().filter(|c| c.kind == kind && c.lane == lane).collect()
    };
    // main montage: ≥ 24 cuts drawn from every source file
    let main = lane(ClipKind::Video, 1);
    assert!(main.len() >= 24, "montage cuts: {}", main.len());
    for f in &files {
        let id = p.media.iter().find(|m| m.path.ends_with(f.as_str())).map(|m| m.id);
        assert!(main.iter().any(|c| Some(c.media) == id), "{f} is used in the montage");
    }
    // linked audio partners
    assert!(main.iter().any(|v| v.link_group.is_some()
        && p.clips.iter().any(|a| a.kind == ClipKind::Audio && a.link_group == v.link_group)));
    // ≥ 3 transition kinds — by the file's `kind` tag (the plugin kinds share one enum variant)
    let mut kinds: Vec<String> = p
        .transitions
        .iter()
        .map(|t| serde_json::to_value(&t.kind).unwrap()["kind"].as_str().unwrap_or("crossfade").to_string())
        .collect();
    kinds.sort();
    kinds.dedup();
    assert!(kinds.len() >= 3, "transition kinds: {}", kinds.len());
    // overlay with on-screen geometry, a title generator, ≥ 3 video FX kinds
    let videos: Vec<&kt_model::ClipEvent> =
        p.clips.iter().filter(|c| c.kind == ClipKind::Video).collect();
    assert!(videos.iter().any(|c| c.lane == 0 && c.geometry.zoom != 1.0));
    assert!(videos.iter().any(|c| matches!(c.r#gen, Some(GeneratorKind::Text { .. }))));
    let mut fx: Vec<String> = videos
        .iter()
        .flat_map(|c| c.fx.iter())
        // the KIND is the tag: every plugin slot shares one enum variant
        .map(|f| f.tag().to_string())
        .collect();
    fx.sort();
    fx.dedup();
    assert!(fx.len() >= 3, "video FX kinds: {fx:?}");
    // Slowed+Reverb on the music track with `rate` and `mix` curves ≥ 4 keys
    let music = lane(ClipKind::Audio, 1);
    let slowed = music
        .iter()
        .find(|c| c.fx.iter().any(|f| f.warps_time()))
        .expect("a Slowed+Reverb bed on the music track");
    let slot = slowed.fx.iter().position(|f| f.warps_time()).unwrap();
    let music_file = p.media.iter().find(|m| m.path.ends_with("sine48k_stereo.flac")).map(|m| m.id);
    assert_eq!(Some(slowed.media), music_file, "an audio-only file is the music bed");
    assert!(
        !main.iter().any(|c| Some(c.media) == music_file),
        "music never lands on the picture track"
    );
    let curves = &slowed.fx_auto[slot];
    for param in ["rate", "mix"] {
        let c = curves.iter().find(|c| c.param == param).expect(param);
        assert!(c.keys.len() >= 4, "{param}: {} keys", c.keys.len());
    }
    assert_eq!(p.morphers.len(), 1, "one morpher batch");
    // a nested reel with content, placed twice
    assert_eq!(p.sequences.len(), 1, "one nested reel");
    let reel = &p.sequences[0];
    assert!(reel.clips.len() >= 2, "the reel has content");
    assert_eq!(p.clips.iter().filter(|c| c.media == reel.id).count(), 2, "two instances");
    // published parameters, set per instance; the reel is longer than an
    // instance, so an expanded instance shows tails on both sides
    assert!(reel.published.len() >= 3, "the reel publishes parameters");
    let instances: Vec<_> = p.clips.iter().filter(|c| c.media == reel.id).collect();
    assert!(instances.iter().any(|c| c.overrides.len() >= 3), "an instance sets its own values");
    let content = reel.clips.iter().map(|c| (c.start + c.duration).0).max().unwrap();
    assert!(instances.iter().all(|c| c.source_in.0 > 0 && (c.source_in + c.duration).0 < content));
    // the morpher swaps the source of its targets and adds a look
    assert!(p.morphers[0].patch.media.is_some(), "the morpher swaps the source");
    assert!(!p.morphers[0].patch.video_fx.is_empty());
    assert!(p.markers.iter().filter(|m| !m.name.is_empty()).count() >= 4);
    assert!(p.region.is_some() && p.loop_enabled, "a looped region");
    let names: Vec<String> = p.tracks.iter().map(|t| t.name.clone()).collect();
    assert!(names.iter().filter(|n| !n.is_empty()).count() >= 7, "named tracks: {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t1412 (readme-shots-showcase): the showcase's looks are the striking ones the README shows
/// (the owner, 2026-11-03: "the effects used are dullish, the new simulation effects are not
/// shown"): the styled montage cuts carry at least six kinds, none of them a plain blur, noise or
/// gain; a simulation is on the timeline and in the morpher's look; the VHS look-alike is not
/// offered where the VHS Tape simulation exists.
#[test]
fn t1412_showcase_looks_are_striking() {
    kt_plugins_builtin::install();
    use kt_model::ClipKind;
    let dir = showcase_sources("t1412");
    let out = dir.join("s.kt");
    let st = cli()
        .args(["showcase", out.to_str().unwrap(), "--sources", dir.join("sources").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let p = kt_model::project_file::from_text(&std::fs::read_to_string(&out).unwrap()).unwrap().project;
    const DULL: [&str; 3] = ["blur", "noise", "gain"];
    const SIMULATIONS: [&str; 3] = ["vhs_tape", "camera_isp", "datamosh"];
    let tags = |fx: &[kt_model::FxInstance]| -> Vec<String> { fx.iter().map(|f| f.tag().to_string()).collect() };
    let mut montage: Vec<String> = p
        .clips
        .iter()
        .filter(|c| c.kind == ClipKind::Video && c.lane == 1)
        .flat_map(|c| tags(&c.fx))
        .collect();
    for t in &montage {
        assert!(!DULL.contains(&t.as_str()), "a dull look on a montage cut: {t}");
    }
    montage.sort();
    montage.dedup();
    assert!(montage.len() >= 6, "montage looks: {montage:?}");
    let all: Vec<String> = p.clips.iter().filter(|c| c.kind == ClipKind::Video).flat_map(|c| tags(&c.fx)).collect();
    assert!(all.iter().any(|t| SIMULATIONS.contains(&t.as_str())), "a simulation on the timeline: {all:?}");
    assert!(!all.iter().any(|t| t == "vhs"), "the VHS look-alike on the timeline: {all:?}");
    let morph = tags(&p.morphers[0].patch.video_fx);
    assert!(morph.iter().any(|t| SIMULATIONS.contains(&t.as_str())), "the morpher's look: {morph:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t825 (readme-showcase): a missing or empty sources directory and an
/// existing output are refused; `--force` writes atomically.
#[test]
fn t825_showcase_refuses_missing_sources_and_existing_output() {
    kt_plugins_builtin::install();
    let dir = showcase_sources("t825");
    let out = dir.join("s.kt");
    let showcase = |src: &std::path::Path, force: bool| {
        let mut c = cli();
        c.args(["showcase", out.to_str().unwrap(), "--sources", src.to_str().unwrap()]);
        if force {
            c.arg("--force");
        }
        c.output().unwrap()
    };
    let missing = dir.join("no-such-dir");
    let st = showcase(&missing, false);
    assert!(!st.status.success());
    assert!(String::from_utf8_lossy(&st.stderr).contains("no-such-dir"), "names the directory");
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::write(empty.join("notes.txt"), "not a video").unwrap();
    let st = showcase(&empty, false);
    assert!(!st.status.success());
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(err.contains("empty") && err.contains("no readable video"), "{err}");
    assert!(!out.exists(), "nothing written on refusal");

    std::fs::write(&out, "keep me").unwrap();
    let st = showcase(&dir.join("sources"), false);
    assert!(!st.status.success());
    assert!(String::from_utf8_lossy(&st.stderr).contains("--force"));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep me");
    let st = showcase(&dir.join("sources"), true);
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(std::fs::read_to_string(&out).unwrap().starts_with("{\n  \"kinetren\""));
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "no .tmp remains");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t829 (kinetren-rename): kt-cli refuses a project of the former name with
/// a hint and `import` brings it in byte-equal to today's file; the source
/// is not touched.
#[test]
fn t829_cli_imports_a_project_of_the_former_name() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t829-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let today = dir.join("today.kt");
    let st = cli().args(["demo", today.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let text = std::fs::read_to_string(&today).unwrap();
    let old = dir.join("old.kt");
    std::fs::write(&old, text.replacen("\"kinetren\"", "\"kinotrain\"", 1)).unwrap();
    let before = std::fs::read(&old).unwrap();

    let st = cli().args(["probe", old.to_str().unwrap(), "0", "1"]).output().unwrap();
    assert!(!st.status.success());
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(err.contains("former name") && err.contains("kt-cli import"), "{err}");

    let new = dir.join("new.kt");
    let st = cli().args(["import", old.to_str().unwrap(), new.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert_eq!(std::fs::read_to_string(&new).unwrap(), text, "import == today's file");
    assert_eq!(std::fs::read(&old).unwrap(), before, "the source is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t831 (kinetren-rename): kt-cli finds the app's project lock in the same
/// data directory as the app (`kinetren/locks`), and still sees a lock of a
/// not yet migrated app of the former name (`kinotrain/locks`).
#[test]
fn t831_cli_and_app_agree_on_the_lock_directory() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t831-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let project = dir.join("locked.kt");
    std::fs::write(&project, "{}").unwrap();
    // the lock name: FNV-1a of the (on Windows normalised) absolute path, kt_dirs::locks —
    // the one function kt-app and kt-cli share
    let abs = std::path::absolute(&project).unwrap();
    let h = kt_dirs::locks::stem(&abs.to_string_lossy(), kt_dirs::Os::NATIVE);
    // the former name existed only on Linux; on Windows the data directory is redirected
    // with KINETREN_DATA_DIR (the known folders cannot be redirected per process)
    let cases: &[(&str, &str)] =
        if cfg!(windows) { &[("kinetren", "4242")] } else { &[("kinetren", "4242"), ("kinotrain", "4343")] };
    for &(data, pid) in cases {
        let xdg = dir.join(format!("xdg-{data}"));
        let locks = xdg.join(data).join("locks");
        std::fs::create_dir_all(&locks).unwrap();
        std::fs::write(locks.join(format!("{h}.lock")), format!("{pid}\n")).unwrap();
        let mut cmd = cli();
        cmd.args(["demo", project.to_str().unwrap(), "--force"]);
        if cfg!(windows) {
            cmd.env("KINETREN_DATA_DIR", xdg.join(data));
        } else {
            cmd.env("XDG_DATA_HOME", &xdg);
        }
        let st = cmd.output().unwrap();
        assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
        let err = String::from_utf8_lossy(&st.stderr);
        assert!(err.contains(&format!("process {pid}")), "{data}: the holder is reported: {err}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// t849 (windows-build, media-index): `kt-cli ffmpeg` prints the search report — the empty
/// folder of `KINETREN_FFMPEG_DIR` rejected with its reason and the FFmpeg that loaded; with
/// the system fallback off there is nothing to load, the report keeps the reason, gives the
/// hint of the OS and exits non-zero.
#[test]
fn t849_cli_prints_the_ffmpeg_search_report() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t849-{}", std::process::id()));
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let run = |system: &str| {
        cli()
            .arg("ffmpeg")
            .env("KINETREN_FFMPEG_DIR", &empty)
            .env("KINETREN_FFMPEG_SYSTEM", system)
            .env("XDG_DATA_HOME", dir.join("xdg"))
            .output()
            .unwrap()
    };
    let empty_s = empty.to_string_lossy().to_string();

    let out = run("1");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&format!("{empty_s}: no avcodec")), "the empty folder is rejected: {text}");
    // with the system fallback on, Linux loads the system FFmpeg; Windows has none
    if cfg!(target_os = "linux") && kt_media::ffmpeg::backend_info().is_some() {
        assert!(out.status.success(), "{text}");
        assert!(text.starts_with("loaded: FFmpeg "), "the loaded candidate and version: {text}");
    }

    let out = run("0");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "nothing loaded → non-zero: {text}");
    assert!(text.contains("FFmpeg not loaded"), "{text}");
    assert!(text.contains(&format!("{empty_s}: no avcodec")), "{text}");
    let hint = if cfg!(windows) { "Download" } else { "from your distribution" };
    assert!(text.contains(hint), "the hint of the OS: {text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 940 (still-image-frame-rule): `kt-cli probe` tells the same truth as the render.
///
/// Before the change the probe read the source's view directly and printed a fit frame for a
/// square still, while the renderer dropped the view of every "procedural" source and stretched
/// the still over the canvas. Now both call `lead_view`: the still prints its view and the fit
/// frame, a missing picture (the placeholder) prints `unknown/stretch` and the whole canvas.
#[test]
fn t940_probe_conform_line_is_the_render_s() {
    kt_plugins_builtin::install();
    use kt_model::*;
    let dir = showcase_sources("t940"); // runs fixtures/gen.sh
    let still = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/out/still_square.png")
        .canonicalize()
        .expect("the square still fixture");
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: ConformRule::Fit,
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), still.to_string_lossy().to_string()));
    p.media.push(MediaRef::new(MediaId(2), dir.join("gone.png").to_string_lossy().to_string()));
    for (id, media, lane) in [(1u64, 1u64, 0u32), (2, 2, 1)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane,
            start: kt_time::Tick(0),
            duration: kt_time::Tick(90_000),
            source_in: kt_time::Tick(0),
            media: MediaId(media),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.ensure_tracks(LaneKind::Video, 2);
    p.validate().unwrap();
    let path = dir.join("t940.kt");
    std::fs::write(&path, kt_model::project_file::to_text(&p, None)).unwrap();
    let out = cli().args(["probe", path.to_str().unwrap(), "0", "1"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    let line = |id: u64| {
        text.lines()
            .find(|l| l.contains(&format!("conform clip {id}:")))
            .unwrap_or_else(|| panic!("no conform line for clip {id}:\n{text}"))
            .to_string()
    };
    let s = line(1);
    assert!(
        s.contains("view=176x176") && s.contains("frame=180x180"),
        "the still's conform line is not the fit the render uses: {s}"
    );
    // ...and it is what the RENDERER orders: the decode size of the conform line equals the size
    // in the video decode order (before the change: 180×180 printed, 320×180 ordered)
    let decode = s.split("decode=").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or("?");
    let ordered = text
        .lines()
        .find(|l| l.trim_start().starts_with("media 1 ("))
        .unwrap_or_else(|| panic!("no decode order for the still:\n{text}"));
    assert!(
        ordered.ends_with(&format!("frames at {decode}")),
        "the probe's conform decode {decode} is not what the renderer orders: {ordered}"
    );
    let g = line(2);
    assert!(
        g.contains("view=unknown/stretch") && g.contains("frame=320x180"),
        "the missing picture's conform line: {g}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The owner's case of morpher-event-grid-cost (ktprompts 26.10.18 item 8), the same scene as
/// t946 in kt-engine: a synthetic 960×540 source, a morpher block with an event grid automated
/// 1×1 → 12×30 → 12×10 → 1×1 and fades at both ends.
fn owner_grid_case() -> kt_model::Project {
    // the event grid is a plugin since plugin-api-stage1 (D11.3)
    kt_plugins_builtin::install();
    use kt_model::*;
    use kt_time::Tick;
    const SEC: i64 = 90_000;
    let curve = |param: &str, vals: [f64; 4]| ParamCurve {
        param: param.into(),
        keys: AutoKey::linear(
            [0, 2, 4, 6].iter().zip(vals).map(|(s, v)| AutoKey::at(Tick(s * SEC), v)).collect(),
        ),
        source: Default::default(),
    };
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 960,
        height: 540,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(6 * SEC),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.ensure_tracks(LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(6 * SEC),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            video_fx: vec![kt_model::FxInstance::plugin_with("event_grid", &[1.0, 1.0, 0.0]).unwrap()],
            video_auto: vec![vec![
                curve("cols", [1.0, 12.0, 12.0, 1.0]),
                curve("rows", [1.0, 30.0, 10.0, 1.0]),
            ]],
            ..Default::default()
        },
        timed: true,
        strength: 1.0,
        fade_in: Tick(SEC / 2),
        fade_out: Tick(SEC / 2),
        enabled: true,
    });
    p.validate().unwrap();
    p
}

/// Test 947, the probe half (morpher-event-grid-cost, D1): `probe --time` tells the VIDEO
/// compose cost — the backend that composed, frames, compose ms, passes and sub-samples — and
/// both backends count the same program. Fields and counts are asserted, never times.
/// `KT_T947_SAVE=<path>` also keeps the project for the release measurement (task 1.4).
#[test]
fn t947_probe_time_tells_the_video_cost() {
    kt_plugins_builtin::install();
    let dir = std::env::temp_dir().join(format!("kt-cli-t947-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let text = kt_model::project_file::to_text(&owner_grid_case(), None);
    let path = dir.join("t947.kt");
    std::fs::write(&path, &text).unwrap();
    if let Some(keep) = std::env::var_os("KT_T947_SAVE") {
        std::fs::write(keep, &text).unwrap();
    }
    let run = |gpu: Option<&str>| -> String {
        let mut c = cli();
        c.args(["probe", path.to_str().unwrap(), "0", "1", "--time"]);
        if let Some(v) = gpu {
            c.env("KT_VIDEO_GPU", v);
        }
        let out = c.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let s = String::from_utf8_lossy(&out.stdout).to_string();
        s.split("-- video compose cost").nth(1).unwrap_or_else(|| panic!("no video section:\n{s}")).to_string()
    };
    let max_subs = |s: &str| -> u64 {
        s.split("(max ").nth(1).and_then(|r| r.split(')').next()).and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("no peak sub-samples in:\n{s}"))
    };
    let cpu = run(Some("0"));
    assert!(cpu.contains("backend: CPU"), "KT_VIDEO_GPU=0 composed on: {cpu}");
    assert!(cpu.contains("frames: 30 (incomplete 0)"), "one second at 30 fps, all complete: {cpu}");
    assert!(cpu.contains("compose ms: p50 "), "no compose time: {cpu}");
    // the peak frame of the first second read the full 16-tap pass before the tile rule;
    // now no frame does (a 3-cell axis keeps 3×4 taps); the fade's strength slot snapshots
    let peak = max_subs(&cpu);
    assert!(peak > 0 && peak < 960 * 540 * 16, "the peak frame's sub-samples on the CPU: {cpu}");
    assert!(cpu.contains("snapshots "), "{cpu}");
    // the default backend (the GPU where there is an adapter) counts the same program
    let def = run(None);
    assert_eq!(max_subs(&def), max_subs(&cpu), "the backends count different work:\n{def}\n{cpu}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// t964 (hw-encode-context D6, media-index): `kt-cli ffmpeg` names the hardware encoding devices.
///
/// "No hardware encoder" has a device cause — no render node in a sandbox, no driver, a driver
/// that cannot encode — that only the machine can name; the Flatpak guide sends people here.
/// `KT_DRI_DIR` stands for `/dev/dri`: a regular file named `renderD128` is not a VAAPI device.
#[test]
fn t964_cli_names_the_hardware_encoding_devices() {
    kt_plugins_builtin::install();
    if !(cfg!(target_os = "linux") && kt_media::ffmpeg::backend_info().is_some()) {
        eprintln!("t964 skipped: needs FFmpeg on Linux");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-cli-t964-{}", std::process::id()));
    let dri = dir.join("dri");
    std::fs::create_dir_all(&dri).unwrap();
    std::fs::write(dri.join("renderD128"), b"not a device").unwrap();
    let run = |probe: bool| {
        let mut c = cli();
        c.arg("ffmpeg")
            .env("KT_DRI_DIR", &dri)
            .env_remove("KINETREN_VAAPI_DEVICE")
            .env("XDG_DATA_HOME", dir.join("xdg"));
        if probe {
            c.arg("--probe-encoders");
        }
        c.output().unwrap()
    };
    let out = run(false);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("hardware encoding:"), "no hardware section: {text}");
    assert!(
        text.contains("hardware context functions: present") || text.contains("hardware context functions: missing "),
        "{text}"
    );
    let node = format!("{}/renderD128: does not open as a VAAPI device (", dri.display());
    assert!(text.contains(&node), "the fake node is not named with a reason: {text}");
    assert!(!text.contains("hardware encoders (opened"), "without the flag nothing is opened: {text}");

    let out = run(true);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    for name in ["h264_nvenc", "h264_vaapi", "h264_qsv"] {
        let line = text.lines().find(|l| l.trim_start().starts_with(&format!("{name}:")));
        let line = line.unwrap_or_else(|| panic!("no probe line for {name}: {text}"));
        assert!(line.contains(": opens") || line.contains(": fails: "), "{line}");
    }
    let vaapi = text.lines().find(|l| l.trim_start().starts_with("h264_vaapi:")).unwrap();
    assert!(
        vaapi.contains("opens") || vaapi.contains("no VAAPI device") || vaapi.contains("not in this FFmpeg build"),
        "the VAAPI line does not name its failing part: {vaapi}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The fake VA driver of `fixtures/fake_va`, built for this test run, and a "render node" that is
/// a regular file: the real libva and the real FFmpeg on a machine with no graphics card.
/// `None` where the scene cannot exist (it says why): not Linux, or an FFmpeg without VAAPI.
struct FakeVa {
    dir: std::path::PathBuf,
    driver: std::path::PathBuf,
}

impl FakeVa {
    fn new(test: &str) -> Option<FakeVa> {
        kt_plugins_builtin::install();
        if !(cfg!(target_os = "linux") && kt_media::ffmpeg::backend_info().is_some()) {
            eprintln!("{test} skipped: needs FFmpeg on Linux");
            return None;
        }
        let dir = std::env::temp_dir().join(format!("kt-cli-{test}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("dri")).unwrap();
        std::fs::write(dir.join("dri").join("renderD128"), b"a regular file").unwrap();
        let driver = dir.join("ktfake_drv_video.so");
        let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/fake_va/ktfake_drv_video.c");
        let built = ["cc", "clang"].iter().any(|cc| {
            Command::new(cc)
                .args(["-shared", "-fPIC", "-O1", "-w", "-o"])
                .arg(&driver)
                .arg(source)
                .status()
                .is_ok_and(|s| s.success())
        });
        assert!(built, "no C compiler built the fake driver");
        let fake = FakeVa { dir, driver };
        let out = fake.run("", &["ffmpeg", "--probe-encoders"]);
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let line = text.lines().find(|l| l.trim_start().starts_with("h264_vaapi:")).unwrap_or("");
        if line.contains("not in this FFmpeg build") {
            eprintln!("{test} skipped: this FFmpeg has no h264_vaapi");
            return None;
        }
        assert!(line.contains(": opens on "), "the healthy fake driver does not open: {text}");
        Some(fake)
    }

    fn command(&self, mode: &str) -> Command {
        let mut c = cli();
        c.env("KT_FAKE_VA", mode)
            .env("KT_FAKE_VA_LOG", self.dir.join("driver.log"))
            .env("KT_DRI_DIR", self.dir.join("dri"))
            .env_remove("KINETREN_VAAPI_DEVICE")
            .env("LD_PRELOAD", &self.driver)
            .env("LIBVA_DRIVERS_PATH", &self.dir)
            .env("LIBVA_DRIVER_NAME", "ktfake")
            .env("XDG_DATA_HOME", self.dir.join("xdg"));
        c
    }

    fn run(&self, mode: &str, args: &[&str]) -> std::process::Output {
        self.command(mode).args(args).output().unwrap()
    }

    /// A short synthetic project, its path.
    fn project(&self) -> String {
        let mut p = owner_grid_case();
        p.morphers.clear();
        p.clips[0].duration = kt_time::Tick(90_000);
        p.validate().unwrap();
        let path = self.dir.join("short.kt");
        std::fs::write(&path, kt_model::project_file::to_text(&p, None)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn out(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }

    /// A forced `h264_vaapi` with `mode`: it ended by itself with an error, and left no file. The
    /// output, for the reason.
    fn forced_fails(&self, mode: &str) -> String {
        let out = self.out(&format!("forced-{mode}.mp4"));
        let r = self.run(mode, &["export", &self.project(), &out, "h264_vaapi"]);
        let all = format!("{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
        assert_eq!(r.status.code(), Some(1), "{mode}: the process did not end with an error: {:?} {all}", r.status);
        assert!(!std::path::Path::new(&out).exists(), "{mode}: a file was left behind");
        all
    }

    /// The cascade with `mode`: the render still comes out, from another encoder, and the log says
    /// why. The output.
    fn auto_renders(&self, mode: &str) -> String {
        let out = self.out(&format!("auto-{mode}.mp4"));
        let r = self.run(mode, &["export", &self.project(), &out]);
        let all = format!("{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
        assert!(r.status.success(), "{mode}: the cascade did not render: {:?} {all}", r.status);
        assert!(all.contains("encoder h264_vaapi skipped: "), "{mode}: the skip is not in the log: {all}");
        assert!(!all.contains("encoder: h264_vaapi (hardware"), "{mode}: it was opened: {all}");
        assert!(std::fs::metadata(&out).unwrap().len() > 0, "{mode}: no file");
        all
    }
}

impl Drop for FakeVa {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// t1551 (hw-encode-context D10, ktprompts v26.10 T16): a driver that FAULTS — on load, in the
/// encoder's open, on the first picture — ends a throwaway process, not the render. Forced, the
/// export is an error naming the crash; left to the cascade, the render comes out of the next
/// encoder; the report that diagnoses such a driver survives it too.
#[test]
fn t1551_a_driver_that_crashes_does_not_end_the_render() {
    let Some(fake) = FakeVa::new("t1551") else { return };
    for mode in ["init-abort", "context-abort", "frame-abort"] {
        let all = fake.forced_fails(mode);
        assert!(all.contains("the driver crashed in a test process (signal 6)"), "{mode}: {all}");
        fake.auto_renders(mode);
        let r = fake.run(mode, &["ffmpeg", "--probe-encoders"]);
        let text = String::from_utf8_lossy(&r.stdout);
        assert!(r.status.success(), "{mode}: the report died: {:?} {text}", r.status);
        assert!(text.contains("h264_vaapi: fails: the driver crashed"), "{mode}: {text}");
    }
    let text = String::from_utf8_lossy(&fake.run("init-abort", &["ffmpeg"]).stdout).into_owned();
    assert!(text.contains("does not open as a VAAPI device (the driver crashed"), "{text}");
}

/// t1552 (D3, D10): a driver that REFUSES, at each part of the one step: the forced error names
/// the part, the cascade goes on to the next encoder. A driver that opens and cannot encode is
/// found before the render starts — "first frames" — and does not fail the export half way.
#[test]
fn t1552_each_refused_part_is_named_and_the_cascade_goes_on() {
    let Some(fake) = FakeVa::new("t1552") else { return };
    for (mode, part) in [
        ("init-refuse", "no VAAPI device ("),
        ("surfaces-refuse", "frames context: code "),
        ("context-refuse", "encoder: code "),
        ("frame-refuse", "first frames: encoder send"),
        ("frame-empty", "first frames: the encoder produced no data"),
    ] {
        let all = fake.forced_fails(mode);
        assert!(all.contains(part), "{mode}: the error does not name “{part}”: {all}");
        let all = fake.auto_renders(mode);
        assert!(all.contains(part), "{mode}: the log does not name “{part}”: {all}");
    }
}

/// t1553 (D10): a driver that HANGS is left behind: the test process is killed at its time limit,
/// the export goes on, and nobody waits for it for ever.
#[test]
fn t1553_a_driver_that_hangs_is_left_behind() {
    let Some(fake) = FakeVa::new("t1553") else { return };
    let started = std::time::Instant::now();
    let out = fake.out("hang.mp4");
    let r = fake
        .command("context-hang")
        .env("KT_HW_TRIAL_TIMEOUT_MS", "1000")
        .args(["export", &fake.project(), &out])
        .output()
        .unwrap();
    let all = format!("{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    assert!(r.status.success(), "{all}");
    assert!(all.contains("encoder h264_vaapi skipped: the driver did not answer in 1 s"), "{all}");
    assert!(std::path::Path::new(&out).exists(), "no file: {all}");
    assert!(started.elapsed().as_secs() < 60, "{:?}", started.elapsed());
}

/// t1554 (the export spec's cancel-and-error scenario on a hardware encoder): a driver that passes
/// the trial and fails in the MIDDLE of the render is a clear error, with no partial file — the
/// frames of a stream cannot move to another encoder half way.
#[test]
fn t1554_a_failure_in_the_middle_is_an_error_and_no_file() {
    let Some(fake) = FakeVa::new("t1554") else { return };
    let all = fake.forced_fails("frame-refuse-after-8");
    assert!(all.contains("encoder: h264_vaapi (hardware"), "it did not start on VAAPI: {all}");
    assert!(all.contains("encoder send"), "the error does not say what failed: {all}");
}

/// t1555 (D3, D4, D10): a healthy driver: left to the cascade the hardware encoder is chosen, and
/// the device is named; every frame of the pipelined export reaches the driver in THIS process,
/// after the throwaway process met it first. Each one logged its pictures when it closed.
#[test]
fn t1555_a_healthy_driver_gets_every_frame_after_the_trial() {
    let Some(fake) = FakeVa::new("t1555") else { return };
    let _ = std::fs::remove_file(fake.dir.join("driver.log"));
    let out = fake.out("healthy.mp4");
    let r = fake.run("", &["export", &fake.project(), &out]);
    let all = format!("{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    assert!(r.status.success(), "{all}");
    assert!(all.contains("encoder: h264_vaapi (hardware, nv12)"), "{all}");
    assert!(all.contains("encoder h264_vaapi\n") || all.trim_end().ends_with("encoder h264_vaapi"), "{all}");
    let log = std::fs::read_to_string(fake.dir.join("driver.log")).unwrap();
    assert!(log.contains("frames 3\n"), "the trial's three pictures: {log}");
    assert!(log.contains("frames 30\n"), "every frame in the render's own process: {log}");
}
