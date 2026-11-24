//! fx-graph-dag v2d (capability `ui-reach`): the strip the node editor edits — a gesture re-wires by
//! address, the processing order is found after it, and a line stays the empty list.

use kt_model::fx_graph::{self, FxIn, GraphWhy, MatteFrom, Node, SlotInputs, Socket};
use kt_model::fx_strip::{links_of, line_links, without_lead, Strip, To};

fn line(n: usize) -> Strip {
    let lane = vec![0u8; n];
    Strip::new(&line_links(&lane), &lane)
}

/// The strip settled, with the order its slots came in.
fn settled(mut s: Strip) -> (Strip, Vec<usize>) {
    s.settle().expect("settles");
    let from = s.from.clone();
    (s, from)
}

/// Test t3296 (fx-graph-dag D1, D8): the gestures' wiring — an arm from the middle leaves the Output
/// last; a Merge takes the Output as its Background; a wire is an address, so the order follows it;
/// a removal gives the Output back to what the Merge read; a loop and a slot after the Output are
/// refused; a line keeps the empty list through every gesture that leaves it a line.
#[test]
fn t3296_the_strip_s_gestures_and_their_order() {
    // lead(0) blur(1): an arm from the lead, a Merge before Out, the arm into its Foreground
    let mut s = line(2);
    let arm = s.branch(0, Node::Slot(0));
    let (mut s, from) = settled(s);
    assert_eq!(from, vec![0, 2, 1], "the arm's slot {arm} stands before the Output");
    assert_eq!(s.out[0], Some(2));
    assert_eq!(s.wiring(0), vec![SlotInputs::default(), SlotInputs::default(), SlotInputs { main: FxIn::Slot(0), ..Default::default() }]);
    assert_eq!(s.live(0), vec![true, false, true], "an unmerged arm reaches nothing");
    let m = s.merge(0, Node::Slot(2));
    assert_eq!((s.links[m].main, s.links[m].second), (Node::Slot(2), None), "from the Output: the Background only");
    s.connect(m, Socket::Matte, Node::Slot(1));
    let (s, _) = settled(s);
    assert_eq!(s.live(0), vec![true; 4]);
    let w = s.wiring(0);
    assert_eq!(w[3].matte.map(|x| (x.from, x.by)), Some((FxIn::Slot(1), MatteFrom::Luma)));
    assert_eq!(links_of(&w, 4, 0), s.links, "the chain's wiring reads back as the strip's links");

    // the Merge removed: the Output is the Background again though the arm stands before it
    let mut r = s.clone();
    r.remove(3);
    let (r, _) = settled(r);
    assert_eq!((r.out[0], r.live(0)), (Some(2), vec![true, false, true]));
    // …and when the arm stands right before the Merge, the Background moves to the end
    let lane = vec![0u8; 3];
    let w = vec![SlotInputs::default(), SlotInputs { main: FxIn::Source, ..Default::default() }, SlotInputs { main: FxIn::Slot(0), second: Some(FxIn::Prev), matte: None }];
    let mut r = Strip::new(&links_of(&w, 3, 0), &lane);
    r.remove(2);
    let (r, from) = settled(r);
    assert_eq!((from, r.out[0]), (vec![1, 0], Some(1)), "the Background is the Output");

    // a loop and a reader of the Output are refused and change nothing
    let mut l = s.clone();
    l.connect(1, Socket::Main, Node::Slot(3));
    let was = l.clone();
    assert_eq!(l.settle(), Err(GraphWhy::Forward));
    assert_eq!(l, was);

    // a line through the line's gestures: move, insert, remove — always the empty list
    let mut a = line(4);
    a.shift(0, To::After(2));
    let (mut a, from) = settled(a);
    assert_eq!(from, vec![1, 2, 0, 3]);
    assert!(a.wiring(0).is_empty() && a.is_line(0));
    a.shift(3, To::Before(0));
    let (mut a, _) = settled(a);
    assert!(a.wiring(0).is_empty(), "{:?}", a.wiring(0));
    a.remove(1);
    a.insert(0, To::After(0));
    let (a, _) = settled(a);
    assert!(a.wiring(0).is_empty(), "{:?}", a.wiring(0));

    // a block on an arm takes the arm's readers and keeps its own wiring; on a line it appends
    let mut b = s.clone();
    let got = b.block(0, Some(1), &[], 2);
    let (b, _) = settled(b);
    assert_eq!((got.len(), b.live(0)), (2, vec![true; 6]), "{:?}", b.wiring(0));
    // a block whose last slot skips the one before keeps that one dangling, as it was copied
    let mut b = s.clone();
    b.block(0, Some(1), &w[..2], 2);
    let (b, _) = settled(b);
    assert_eq!(b.live(0).iter().filter(|x| !**x).count(), 1, "{:?}", b.wiring(0));
    let mut b = line(2);
    b.block(0, Some(1), &[], 2);
    let (b, _) = settled(b);
    assert!(b.wiring(0).is_empty() && b.out[0] == Some(3));

    // two lanes interleaved: each keeps its places and its own wiring
    let lane = vec![0u8, 1, 0, 1];
    let mut t = Strip::new(&line_links(&lane), &lane);
    t.branch(0, Node::Slot(0));
    let (t, _) = settled(t);
    assert!(t.wiring(1).is_empty() && !t.wiring(0).is_empty());
    assert_eq!(t.lane.iter().filter(|l| **l == 0).count(), 3);
}

/// Test t3297 (fx-graph-dag D2): the leading Geometry node stands on the OUTPUT of what it reads —
/// a branch at the value right before it reads the frame after it too; taken off a strip it gives
/// the same wiring back, and after a branch it is refused.
#[test]
fn t3297_the_lead_stands_before_every_branch() {
    // blur(0), then two readers of the blur; the lead between them
    let w = vec![
        SlotInputs::default(),
        SlotInputs::default(),
        SlotInputs { main: FxIn::Slot(0), ..Default::default() },
        SlotInputs { main: FxIn::Slot(1), second: Some(FxIn::Prev), matte: None },
    ];
    let mut with = w.clone();
    fx_graph::insert_lead(&mut with, 4, 1);
    assert_eq!(with[3].main, FxIn::Slot(1), "the second reader of the blur skips the lead: {with:?}");
    assert_eq!(without_lead(&with, 5, 1), Ok(w.clone()), "taken off, the wiring is not what it was");
    // pos 0: the Source's readers read the lead
    let src = vec![SlotInputs::default(), SlotInputs { main: FxIn::Source, ..Default::default() }, SlotInputs { main: FxIn::Slot(0), second: Some(FxIn::Prev), matte: None }];
    let mut with = src.clone();
    fx_graph::insert_lead(&mut with, 3, 0);
    assert_eq!(without_lead(&with, 4, 0), Ok(src));
    // a wire that skips the lead
    let mut skip = with.clone();
    skip[2].main = FxIn::Source;
    assert_eq!(without_lead(&skip, 4, 0), Err(GraphWhy::LeadInArm));
    // a line has no branch to stand after
    assert_eq!(without_lead(&[], 3, 2), Ok(vec![]));
}
