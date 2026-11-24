//! Addresses of an event's effect SLOTS follow the slots (found-defects 906). A slot is addressed by its
//! number in three places: a reel's published parameter (`ParamOwner::FxSlot`, a slot's mask), a link's
//! text (`12/fx0/radius`, `self/fx0.mask1/cx`) in any curve, and the parallel lists of the event itself
//! (their writers move those). A chain edit that moves, adds in the middle or removes slots hands the map
//! to `Command::MapFxSlots`; the model's own edits (a pair's mirror) call `remap` directly.

use crate::links::LinkPath;
use crate::published::{ParamOwner, PublishedParam};
use crate::{ClipEvent, ClipId, CurveSource, FxInstance, ParamCurve, Project};

/// Where each slot of a chain went: `to[old]` is its new number, `None` — it is gone.
pub type SlotMap = Vec<Option<usize>>;

/// The map of `from` (which old slot each new one was, `None` — a new slot) for a chain of `old` slots.
pub fn map_of(from: &[Option<usize>], old: usize) -> SlotMap {
    let mut to = vec![None; old];
    for (new, f) in from.iter().enumerate() {
        if let Some(o) = f
            && *o < old
        {
            to[*o] = Some(new);
        }
    }
    to
}

/// The map of a removal of slot `i` from `old` slots.
pub fn without(old: usize, i: usize) -> SlotMap {
    (0..old).map(|k| match k.cmp(&i) {
        std::cmp::Ordering::Less => Some(k),
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => Some(k - 1),
    }).collect()
}

/// Whether a map moves anything that stood.
pub fn is_identity(to: &SlotMap) -> bool {
    to.iter().enumerate().all(|(i, t)| *t == Some(i))
}

/// Which slot of `now` each slot of `then` is (a window's Cancel gives an earlier chain back and has no
/// record of what moved): the equal instance first, then the same kind in order. Indexed by `then`.
pub fn matching(then: &[FxInstance], now: &[FxInstance]) -> Vec<Option<usize>> {
    let mut used = vec![false; now.len()];
    let mut out = vec![None; then.len()];
    let mut pass = |here: bool, same: &dyn Fn(&FxInstance, &FxInstance) -> bool| {
        for (i, f) in then.iter().enumerate() {
            if out[i].is_some() {
                continue;
            }
            let fits = |k: &usize| !used[*k] && same(f, &now[*k]);
            let hit = if here { Some(i).filter(|k| *k < now.len()).filter(fits) } else { (0..now.len()).find(fits) };
            if let Some(k) = hit {
                used[k] = true;
                out[i] = Some(k);
            }
        }
    };
    // its own place first, then elsewhere: equal twins are not swapped
    pass(true, &|a, b| a == b);
    pass(false, &|a, b| a == b);
    pass(true, &|a, b| a.tag() == b.tag());
    pass(false, &|a, b| a.tag() == b.tag());
    out
}

fn owner(o: &mut ParamOwner, to: &SlotMap) {
    // a gone slot is an address nothing answers: the publication is dropped by the same step's
    // reconciliation, as for an event that is gone
    let at = |s: usize| to.get(s).copied().unwrap_or(Some(s)).unwrap_or(usize::MAX);
    match o {
        ParamOwner::FxSlot { slot } => *slot = at(*slot),
        ParamOwner::Mask { slot: Some(slot), .. } => *slot = at(*slot),
        _ => {}
    }
}

/// The link's text with the map, or `None` when it does not address a moved slot of `id`. `me` — the
/// event a `self` address means. A gone leader: `Some(None)`.
fn path(text: &str, me: Option<ClipId>, id: ClipId, to: &SlotMap) -> Option<Option<String>> {
    let mut p = LinkPath::parse(text)?;
    if p.clip.or(me) != Some(id) {
        return None;
    }
    let slot = match &p.owner {
        ParamOwner::FxSlot { slot } | ParamOwner::Mask { slot: Some(slot), .. } => *slot,
        _ => return None,
    };
    match to.get(slot).copied().unwrap_or(Some(slot)) {
        Some(n) if n == slot => None,
        Some(_) => {
            owner(&mut p.owner, to);
            Some(Some(p.to_text()))
        }
        None => Some(None),
    }
}

/// Whether a curve addresses a moved slot (a question before a write).
fn touches(c: &ParamCurve, me: Option<ClipId>, id: ClipId, to: &SlotMap) -> bool {
    c.source.link_path().is_some_and(|t| path(t, me, id, to).is_some())
}

fn curves(list: &mut Vec<ParamCurve>, me: Option<ClipId>, id: ClipId, to: &SlotMap) {
    list.retain_mut(|c| {
        let CurveSource::Link { path: text, .. } = &mut c.source else { return true };
        match path(text, me, id, to) {
            None => true,
            Some(Some(new)) => {
                *text = new;
                true
            }
            // the leader is gone: the follower keeps its own keys, or its base value
            Some(None) => {
                c.source = CurveSource::Keys;
                !c.keys.is_empty()
            }
        }
    });
}

/// Every curve of an event that can hold a link, read: (curve, the event `self` means).
fn event_touched(c: &ClipEvent, published: &dyn Fn(crate::MediaId) -> Vec<PublishedParam>, id: ClipId, to: &SlotMap) -> bool {
    let own = Some(c.id);
    c.fx_auto.iter().flatten().chain(&c.gen_auto).chain(&c.geometry_auto).any(|x| touches(x, own, id, to))
        || c.masks.iter().any(|(_, m)| m.auto.iter().any(|x| touches(x, own, id, to)))
        || (!c.overrides.is_empty() && {
            let set = published(c.media);
            c.overrides.iter().any(|o| {
                let target = set.iter().find(|p| p.id == o.pub_id).map(|p| p.target.clip);
                o.curves.iter().any(|x| touches(x, target, id, to))
            })
        })
}

fn event(c: &mut ClipEvent, set: &[PublishedParam], id: ClipId, to: &SlotMap) {
    let own = Some(c.id);
    for l in &mut c.fx_auto {
        curves(l, own, id, to);
    }
    curves(&mut c.gen_auto, own, id, to);
    curves(&mut c.geometry_auto, own, id, to);
    let masks = &mut c.masks;
    for m in masks.picture.iter_mut().chain(masks.slots.iter_mut().flatten()) {
        curves(&mut m.auto, own, id, to);
    }
    for o in &mut c.overrides {
        // an override's curve lives in the reel: its `self` is the published parameter's event
        let target = set.iter().find(|p| p.id == o.pub_id).map(|p| p.target.clip);
        curves(&mut o.curves, target, id, to);
    }
}

/// Move every address of `id`'s slots by `to`. The slots themselves (and the event's parallel lists) are
/// the caller's.
pub fn remap(p: &mut Project, id: ClipId, to: &SlotMap) {
    if is_identity(to) {
        return;
    }
    for x in p.published.iter_mut().chain(p.sequences.iter_mut().flat_map(|s| s.published.iter_mut())) {
        if x.target.clip == id {
            owner(&mut x.target.owner, to);
        }
    }
    // the sets as the overrides' `self` reads them, taken before the events are written
    let root: Vec<PublishedParam> = p.published.clone();
    let sets: Vec<(crate::MediaId, Vec<PublishedParam>)> = p.sequences.iter().map(|s| (s.id, s.published.clone())).collect();
    // the reel one stands in keeps its set in the root's place (the working view), its record another's
    let set = |m: crate::MediaId| -> Vec<PublishedParam> {
        let own = sets.iter().find(|(i, _)| *i == m).map(|(_, s)| s.as_slice()).unwrap_or(&[]);
        own.iter().chain(root.iter().filter(|r| !own.iter().any(|o| o.id == r.id))).cloned().collect()
    };
    for c in p.clips.iter_mut().chain(p.sequences.iter_mut().flat_map(|s| s.clips.iter_mut())) {
        if event_touched(c, &set, id, to) {
            let of = set(c.media);
            event(c, &of, id, to);
        }
    }
}
