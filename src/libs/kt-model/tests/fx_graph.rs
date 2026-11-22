//! fx-graph-dag v2a (capabilities `project-format`, `video-plan`): a chain's wiring in the model —
//! validation with the slot's address, the canonical line, the one removal rule, the reads a time
//! kind in an arm opens, and the pair's speed staying the trunk's.

use kt_model::fx_graph::{self, FxIn, GraphWhy, Matte, MatteFrom, SlotInputs, MAX_HELD, MAX_READS};
use kt_model::*;
use kt_time::Tick;

fn clip(id: u64, kind: ClipKind, group: Option<u64>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: group.map(GroupId),
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
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
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
    p.clips.push(clip(10, ClipKind::Video, Some(1)));
    p.clips.push(clip(11, ClipKind::Audio, Some(1)));
    p
}

fn fx(id: &str) -> FxInstance {
    FxInstance::plugin(id).unwrap_or_else(|| panic!("{id} is not registered"))
}

fn merge() -> FxInstance {
    FxInstance::Merge { mode: MergeMode::Blend(BlendMode::Screen), mix: 1.0 }
}

fn main(main: FxIn) -> SlotInputs {
    SlotInputs { main, ..Default::default() }
}

fn merged(main: FxIn, second: FxIn) -> SlotInputs {
    SlotInputs { main, second: Some(second), matte: None }
}

fn get(m: &TimelineModel, id: u64) -> ClipEvent {
    m.project().clips.iter().find(|c| c.id == ClipId(id)).unwrap().clone()
}

fn why(r: Result<(), CommandError>) -> Option<(usize, GraphWhy)> {
    match r {
        Err(CommandError::InvalidClip(ProjectError::BadGraph(_, slot, why))) => Some((slot, why)),
        _ => None,
    }
}

/// Test t3209 (fx-graph-dag D1; capability `project-format`): a graph is accepted only when every
/// input names an earlier slot, a second input and a matte stand on a Merge, the lead stands before
/// the first branch, at most `MAX_HELD` frames are held and `MAX_READS` reads made; a refusal names
/// the slot; a line written by hand reads as the empty list.
#[test]
fn t3209_a_graph_is_checked_with_the_slot_s_address() {
    kt_plugins_builtin::install();
    let mut m = TimelineModel::new(project());
    let chain = vec![fx("blur"), fx("glow"), merge()];
    m.apply(Command::SetFx { id: ClipId(10), fx: chain.clone() }).unwrap();

    // the highlights: blur and glow both from the Source, merged — accepted, one undo step
    let wiring = vec![main(FxIn::Prev), main(FxIn::Source), merged(FxIn::Prev, FxIn::Slot(0))];
    m.apply(Command::SetFxIn { id: ClipId(10), fx_in: wiring.clone() }).unwrap();
    assert_eq!(get(&m, 10).fx_in, vec![SlotInputs::default(), main(FxIn::Source), merged(FxIn::Prev, FxIn::Slot(0))]);
    assert_eq!(fx_graph::trunk_len(&get(&m, 10).fx_in, 3), 0, "the Source is read twice: no trunk");
    assert_eq!(fx_graph::held_peak(&get(&m, 10).fx_in, 3), 2, "the Source and the blur are held");
    m.undo().unwrap();
    assert!(get(&m, 10).fx_in.is_empty(), "one undo did not take the wiring off");

    // a line written out by hand is the empty list: nothing to write, nothing to hash
    m.apply(Command::SetFxIn { id: ClipId(10), fx_in: vec![main(FxIn::Prev), main(FxIn::Slot(0)), main(FxIn::Prev)] })
        .unwrap();
    assert!(get(&m, 10).fx_in.is_empty(), "a line's entries were kept");

    // a forward input and a loop are refused at their slot
    let bad = vec![main(FxIn::Slot(2)), main(FxIn::Prev), main(FxIn::Prev)];
    assert_eq!(why(m.apply(Command::SetFxIn { id: ClipId(10), fx_in: bad })), Some((0, GraphWhy::Forward)));
    let own = vec![main(FxIn::Prev), main(FxIn::Slot(1))];
    assert_eq!(why(m.apply(Command::SetFxIn { id: ClipId(10), fx_in: own })), Some((1, GraphWhy::Forward)));
    // a Foreground or a matte on an effect that has no such socket
    let plain = vec![main(FxIn::Prev), merged(FxIn::Prev, FxIn::Source)];
    assert_eq!(why(m.apply(Command::SetFxIn { id: ClipId(10), fx_in: plain })), Some((1, GraphWhy::SocketOnPlain)));
    let matte = SlotInputs { matte: Some(Matte { from: FxIn::Source, by: MatteFrom::Luma }), ..Default::default() };
    assert_eq!(
        why(m.apply(Command::SetFxIn { id: ClipId(10), fx_in: vec![matte] })),
        Some((0, GraphWhy::SocketOnPlain))
    );
    // the leading Geometry node after a branch: it places the frame for the whole layer
    m.apply(Command::SetGeometry { id: ClipId(10), geometry: Default::default(), pos: 1, auto: vec![] }).unwrap();
    assert_eq!(why(m.apply(Command::SetFxIn { id: ClipId(10), fx_in: wiring.clone() })), Some((1, GraphWhy::LeadInArm)));
    m.apply(Command::SetGeometry { id: ClipId(10), geometry: Default::default(), pos: 0, auto: vec![] }).unwrap();
    m.apply(Command::SetFxIn { id: ClipId(10), fx_in: wiring }).unwrap();

    // more frames held than MAX_HELD: five arms from the Source merged one by one
    let n = 9;
    let mut many = vec![fx("blur"); 5];
    many.extend(std::iter::repeat_n(merge(), 4));
    let mut w: Vec<SlotInputs> = (0..5).map(|i| main(if i == 0 { FxIn::Prev } else { FxIn::Source })).collect();
    w.extend((0..4).map(|k| merged(FxIn::Prev, FxIn::Slot(k))));
    assert!(fx_graph::held_peak(&w, n) > MAX_HELD, "the scene holds {} frames", fx_graph::held_peak(&w, n));
    assert_eq!(fx_graph::check(&many, &w, 0).map_err(|e| e.why), Err(GraphWhy::TooManyHeld));

    // more reads than MAX_READS: a Datamosh in each of five arms
    // (merged one at a time, so at most three frames are held)
    let mosh = fx("datamosh");
    let moshed = vec![
        mosh.clone(), mosh.clone(), merge(), mosh.clone(), merge(), mosh.clone(), merge(), mosh, merge(),
    ];
    let w: Vec<SlotInputs> = vec![
        SlotInputs::default(),
        main(FxIn::Source),
        merged(FxIn::Prev, FxIn::Slot(0)),
        main(FxIn::Source),
        merged(FxIn::Slot(2), FxIn::Prev),
        main(FxIn::Source),
        merged(FxIn::Slot(4), FxIn::Prev),
        main(FxIn::Source),
        merged(FxIn::Slot(6), FxIn::Prev),
    ];
    assert!(fx_graph::held_peak(&w, 9) <= MAX_HELD, "scene: {} held", fx_graph::held_peak(&w, 9));
    assert_eq!(fx_graph::read_count(&moshed, &w), 1 + 5);
    assert!(fx_graph::read_count(&moshed, &w) > MAX_READS);
    assert_eq!(fx_graph::check(&moshed, &w, 0).map_err(|e| e.why), Err(GraphWhy::TooManyReads));

    // the file: a hand-written forward input is refused by the reader with the slot
    let p = m.document();
    let text = project_file::to_text(&p, None);
    let broken = text.replacen("\"source\"", "{\"slot\": 7}", 1);
    assert_ne!(broken, text, "scene: the wiring is in the file");
    assert!(project_file::from_text(&broken).is_err(), "a forward input was read");
}

/// Test t3209b (fx-graph-dag D2, D6): a time kind in an arm opens a read made of its ancestors; on
/// the trunk it opens none; a removal re-wires the readers to the removed slot's input; an
/// insertion on a wire keeps a line a line.
#[test]
fn t3209b_reads_and_the_one_removal_rule() {
    kt_plugins_builtin::install();
    // Slowed in an arm: blur from the Source, Slowed from the Source, merged
    let slowed = FxInstance::slowed_reverb(0.5, 0.35, 100.0);
    let chain = vec![fx("blur"), slowed.clone(), merge()];
    let wiring = vec![SlotInputs::default(), main(FxIn::Source), merged(FxIn::Prev, FxIn::Slot(0))];
    let reads = fx_graph::reads(&chain, &wiring);
    assert_eq!(reads.len(), 1, "Slowed in an arm opens one read");
    assert_eq!((reads[0].slot, reads[0].keep.clone()), (1, vec![1]), "the read is Slowed alone from the Source");
    assert!(reads[0].fx_in.is_empty() && reads[0].nested.is_empty());
    assert_eq!(fx_graph::read_count(&chain, &wiring), 2);
    // a blur before the branch is the read's ancestor: the read reruns it on its own frame
    let chain2 = vec![fx("glow"), fx("blur"), slowed.clone(), merge()];
    let wiring2 = vec![SlotInputs::default(), main(FxIn::Prev), main(FxIn::Slot(0)), merged(FxIn::Prev, FxIn::Slot(1))];
    let r = fx_graph::reads(&chain2, &wiring2);
    assert_eq!(r[0].keep, vec![0, 2], "the read is the glow and Slowed");
    // on the trunk it is today's reading: no read
    let trunk = vec![slowed.clone(), fx("blur"), fx("glow"), merge()];
    let w3 = vec![SlotInputs::default(), SlotInputs::default(), main(FxIn::Slot(0)), merged(FxIn::Prev, FxIn::Slot(1))];
    assert!(fx_graph::reads(&trunk, &w3).is_empty(), "Slowed on the trunk opened a read");
    // a read is a LINE: Slowed after a Merge of two arms is refused with its slot
    let after_merge = vec![fx("blur"), fx("glow"), merge(), slowed.clone()];
    let w5 = vec![SlotInputs::default(), main(FxIn::Source), merged(FxIn::Slot(0), FxIn::Prev), SlotInputs::default()];
    assert_eq!(fx_graph::check(&after_merge, &w5, 0).map_err(|e| (e.slot, e.why)), Err((3, GraphWhy::ReadBranches)));
        // a dangling Datamosh opens nothing
    let dangling = vec![fx("blur"), fx("datamosh"), fx("glow")];
    let w4 = vec![SlotInputs::default(), main(FxIn::Source), main(FxIn::Slot(0))];
    assert!(!fx_graph::reaches_output(&w4, 3)[1]);
    assert!(fx_graph::reads(&dangling, &w4).is_empty());

    // removing the blur: the Merge's Foreground reads the blur's input, the Source
    let mut w = wiring.clone();
    fx_graph::remove_slot(&mut w, 3, 0);
    assert_eq!(w, vec![SlotInputs::default(), merged(FxIn::Prev, FxIn::Source)]);
    // removing the arm's Slowed: the Merge's Background reads the Source
    let mut w = wiring.clone();
    fx_graph::remove_slot(&mut w, 3, 1);
    assert_eq!(w, vec![SlotInputs::default(), merged(FxIn::Source, FxIn::Prev)]);
    // a line stays the empty list both ways
    let mut line = Vec::new();
    fx_graph::insert_slot(&mut line, 3, 1);
    fx_graph::remove_slot(&mut line, 4, 2);
    assert!(line.is_empty());
    // an insertion on the wire into the Merge's Background
    let mut w = wiring.clone();
    fx_graph::insert_slot(&mut w, 3, 2);
    assert_eq!(w, vec![SlotInputs::default(), main(FxIn::Source), SlotInputs::default(), merged(FxIn::Prev, FxIn::Slot(0))]);
    // a forward wire re-sorts stably; a loop has no order
    assert_eq!(fx_graph::topo_order(3, |i| if i == 0 { vec![2] } else { vec![] }), Some(vec![1, 2, 0]));
    assert_eq!(fx_graph::topo_order(2, |i| vec![1 - i]), None);
}

/// Test t3209c (fx-graph-dag D2): the pair's speed is the trunk's. Slowed moved into an arm of the
/// picture leaves the linked sound without it, in the same batch that wired it; back on the trunk,
/// the sound slows again.
#[test]
fn t3209c_an_arm_s_speed_is_not_the_pair_s() {
    kt_plugins_builtin::install();
    let mut m = TimelineModel::new(project());
    let slowed = FxInstance::slowed_reverb(0.5, 0.35, 100.0);
    let chain = vec![fx("blur"), slowed, merge()];
    m.apply(Command::SetFx { id: ClipId(10), fx: chain.clone() }).unwrap();
    assert!(get(&m, 11).fx.iter().any(FxInstance::warps_time), "scene: a line's Slowed reaches the sound");
    let wiring = vec![SlotInputs::default(), main(FxIn::Source), merged(FxIn::Prev, FxIn::Slot(0))];
    m.apply(Command::SetFxIn { id: ClipId(10), fx_in: wiring }).unwrap();
    assert!(!get(&m, 11).fx.iter().any(FxInstance::warps_time), "an arm's Slowed slowed the sound");
    // one batch from the line state: the editor's gesture
    m.undo().unwrap();
    m.undo().unwrap();
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(10), fx: chain },
        Command::SetFxIn {
            id: ClipId(10),
            fx_in: vec![SlotInputs::default(), main(FxIn::Source), merged(FxIn::Prev, FxIn::Slot(0))],
        },
    ])
    .unwrap();
    assert!(!get(&m, 11).fx.iter().any(FxInstance::warps_time), "the batch left the sound slowed");
    // the line again: the trunk's Slowed is the pair's
    m.apply(Command::SetFxIn { id: ClipId(10), fx_in: vec![] }).unwrap();
    assert!(get(&m, 11).fx.iter().any(FxInstance::warps_time), "the trunk's Slowed did not come back to the sound");
}

/// Test t3210 (fx-graph-dag D7; capability `project-format`): a project of lines keeps its text and
/// version; an arm and a Merge raise it to format 8, written as `fx_in` and the kind `merge`, and it
/// reads back equal; a build that reads only 7 is told the version.
#[test]
fn t3210_a_graph_raises_the_format_a_line_does_not_move() {
    kt_plugins_builtin::install();
    let mut m = TimelineModel::new(project());
    m.apply(Command::SetFx { id: ClipId(10), fx: vec![fx("blur"), fx("glow")] }).unwrap();
    let line = m.document();
    let text = project_file::to_text(&line, None);
    assert_eq!(project_file::version_for(&line), project_file::PROJECT_FILE_VERSION, "a line is format 1");
    assert!(!text.contains("fx_in") && !text.contains("merge"), "a line wrote the wiring");
    // the same chain wired as a line by hand writes the same bytes
    m.apply(Command::SetFxIn { id: ClipId(10), fx_in: vec![SlotInputs::default(), main(FxIn::Slot(0))] }).unwrap();
    assert_eq!(project_file::to_text(&m.document(), None), text, "a canonical line moved the file");

    m.apply(Command::SetFx { id: ClipId(10), fx: vec![fx("blur"), fx("glow"), merge()] }).unwrap();
    m.apply(Command::SetFxIn {
        id: ClipId(10),
        fx_in: vec![SlotInputs::default(), main(FxIn::Source), merged(FxIn::Prev, FxIn::Slot(0))],
    })
    .unwrap();
    let graph = m.document();
    assert_eq!(project_file::version_for(&graph), project_file::PROJECT_FILE_VERSION_FX_GRAPH);
    assert_eq!(project_file::PROJECT_FILE_VERSION_FX_GRAPH, 8);
    let gtext = project_file::to_text(&graph, None);
    assert!(gtext.contains("\"fx_in\"") && gtext.contains("\"merge\""), "the wiring is not in the file");
    let back = project_file::from_text(&gtext).expect("the graph reads back").project;
    assert_eq!(back, graph, "the graph did not read back equal");
    assert_eq!(project_file::to_text(&back, None), gtext, "a second write moved the bytes");
    // an older build's ceiling: the version is named, nothing is read in part
    let older = gtext.replacen(
        &format!("\"{}\": 8", project_file::MARKER),
        &format!("\"{}\": {}", project_file::MARKER, project_file::PROJECT_FILE_READS + 1),
        1,
    );
    assert_ne!(older, gtext, "scene: the version is in the file");
    assert!(matches!(project_file::from_text(&older), Err(project_file::ProjectFileError::TooNew { .. })));
}

/// Test t3224 (fx-graph-dag D6; capability `project-format`): a block of slots with an arm and a
/// Merge, copied from one event, pasted into another event's chain and into a morpher patch: the
/// inputs inside the block are re-based, an input from outside the copied slots became the block's
/// input, and one undo takes the paste off.
#[test]
fn t3224_copy_and_paste_carry_the_wiring() {
    kt_plugins_builtin::install();
    let mut p = project();
    p.clips.push(clip(20, ClipKind::Video, None));
    let mut m = TimelineModel::new(p);
    // the source: [glow, blur, sharpen←Source, merge(blur, sharpen)]
    let src = vec![fx("glow"), fx("blur"), fx("sharpen"), merge()];
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(10), fx: src.clone() },
        Command::SetFxIn {
            id: ClipId(10),
            fx_in: vec![SlotInputs::default(), SlotInputs::default(), main(FxIn::Slot(0)), merged(FxIn::Slot(1), FxIn::Prev)],
        },
    ])
    .unwrap();
    // copy the last three: the sharpen read the glow, which is outside: it now reads the block's input
    let c = get(&m, 10);
    let keep = [1usize, 2, 3];
    let block = ops::FxChainBlock {
        slots: keep
            .iter()
            .map(|&i| ops::FxSlotBlock { fx: c.fx[i].clone(), bypass: false, curves: vec![], masks: vec![] })
            .collect(),
        src_in: c.source_in,
        fx_off: false,
        fx_in: fx_graph::derive(&c.fx_in, &keep),
    };
    assert_eq!(block.fx_in, vec![SlotInputs::default(), main(FxIn::Source), merged(FxIn::Slot(0), FxIn::Prev)]);
    // the clipboard text carries it
    let text = serde_json::to_string(&block).unwrap();
    assert_eq!(serde_json::from_str::<ops::FxChainBlock>(&text).unwrap(), block);

    // paste after the receiver's own [vignette]: the block's Source is the vignette
    m.apply(Command::SetFx { id: ClipId(20), fx: vec![fx("vignette")] }).unwrap();
    let r = get(&m, 20);
    let (mut f, mut b, mut a, mut w) = (r.fx.clone(), r.fx_bypass.clone(), r.fx_auto.clone(), r.fx_in.clone());
    let first = f.len();
    ops::append_chain_block(&mut f, &mut b, &mut a, &block, r.source_in);
    ops::append_chain_wiring(&mut w, first, &block);
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(20), fx: f },
        Command::SetFxIn { id: ClipId(20), fx_in: w },
    ])
    .unwrap();
    let r = get(&m, 20);
    assert_eq!(r.fx.iter().map(|f| f.tag()).collect::<Vec<_>>(), ["vignette", "blur", "sharpen", "merge"]);
    assert_eq!(r.fx_in, vec![SlotInputs::default(), SlotInputs::default(), main(FxIn::Slot(0)), merged(FxIn::Slot(1), FxIn::Prev)]);
    m.undo().unwrap();
    assert_eq!(get(&m, 20).fx.len(), 1, "one undo did not take the paste off");
    assert!(get(&m, 20).fx_in.is_empty());

    // into an empty morpher patch: the block's Source is the patch's, the target's Output
    let mut patch = MorphPatch::default();
    let first = patch.video_fx.len();
    ops::append_chain_block(&mut patch.video_fx, &mut patch.video_bypass, &mut patch.video_auto, &block, Tick(0));
    ops::append_chain_wiring(&mut patch.video_in, first, &block);
    assert_eq!(patch.video_in, block.fx_in, "a block pasted first keeps its wiring as is");
}
