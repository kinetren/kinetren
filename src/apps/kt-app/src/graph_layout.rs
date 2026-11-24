//! The layout of a branched chain in the node editor (fx-graph-dag D8, the owner's U8: automatic by
//! processing order, nothing stored). The Output's main path runs on the lane's first row; every arm
//! is a row under it from the column after the value it reads to the column before what reads it; a
//! Merge stands where its last input arrives. A wire that no row carries runs on a thin track under
//! the rows, so no wire crosses a node.

use kt_model::fx_graph::{Node, Socket};
use kt_model::fx_strip::Strip;

/// How a wire goes from its output to its socket.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Via {
    /// Neighbours: straight through the gap (or along the first row).
    Direct,
    /// Along this row of the lane.
    Row(usize),
    /// Along this track under the lane's rows.
    Track(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Edge {
    pub from: Node,
    pub to: usize,
    pub socket: Socket,
    pub via: Via,
}

/// One lane laid out: a column and a row per slot of the strip (slots of the other lane keep 0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaneLayout {
    pub rows: usize,
    pub tracks: usize,
    pub row: Vec<usize>,
    pub edges: Vec<Edge>,
}

/// The columns of every slot: the longest path from the Source; a device's halves (`pairs`) share a
/// column where the two orders allow it.
pub fn columns(s: &Strip, pairs: &[(usize, usize)]) -> Vec<usize> {
    let n = s.links.len();
    let solve = |floor: &[usize]| -> Vec<usize> {
        let mut col = vec![0usize; n];
        // the strip is in processing order: an input is always earlier
        for i in 0..n {
            let after = s.links[i].each().map(|(x, _)| match x {
                Node::Slot(j) => col[j] + 1,
                Node::Source => 0,
            });
            col[i] = after.max().unwrap_or(0).max(floor[i]);
        }
        col
    };
    let mut floor = vec![0usize; n];
    for _ in 0..=n {
        let col = solve(&floor);
        let mut same = true;
        for &(a, b) in pairs {
            let m = col[a].max(col[b]);
            same &= col[a] == col[b];
            floor[a] = m;
            floor[b] = m;
        }
        if same {
            return col;
        }
    }
    // the halves stand in crossing orders: each lane keeps its own columns
    solve(&vec![0; n])
}

thread_local! {
    /// How many lanes were laid out on this thread: a hover lays out nothing (t3221).
    pub static LAID: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

pub fn lane(s: &Strip, lane: u8, col: &[usize]) -> LaneLayout {
    LAID.with(|c| c.set(c.get() + 1));
    let n = s.links.len();
    let mine: Vec<usize> = (0..n).filter(|i| s.lane[*i] == lane).collect();
    let mut row = vec![0usize; n];
    let mut placed = vec![false; n];
    // the Output's main path
    let mut at = s.out[usize::from(lane == 1)];
    while let Some(i) = at {
        placed[i] = true;
        at = match s.links[i].main {
            Node::Slot(j) => Some(j),
            Node::Source => None,
        };
    }
    // the arms, each on the first row free over its columns: (first, last) taken per row
    let mut taken: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut free_row = |a: usize, b: usize| -> usize {
        let free = taken.iter().position(|t| t.iter().all(|(x, y)| b < *x || a > *y));
        let r = free.unwrap_or_else(|| {
            taken.push(Vec::new());
            taken.len() - 1
        });
        taken[r].push((a, b));
        r + 1
    };
    let before = |x: Node| match x {
        Node::Slot(j) => col[j] + 1,
        Node::Source => 0,
    };
    let mut tail = vec![false; n];
    for &i in &mine {
        if placed[i] {
            continue;
        }
        let mut arm = vec![i];
        while let Some(k) = mine.iter().copied().find(|k| !placed[*k] && s.links[*k].main == Node::Slot(arm[arm.len() - 1]) && !arm.contains(k)) {
            arm.push(k);
        }
        let last = arm[arm.len() - 1];
        tail[last] = true;
        let end = s.readers(lane, Node::Slot(last)).iter().map(|(r, _)| col[*r]).max().map_or(col[last], |c| c.saturating_sub(1).max(col[last]));
        let r = free_row(before(s.links[i].main).min(col[i]), end);
        for k in arm {
            placed[k] = true;
            row[k] = r;
        }
    }
    let rows = 1 + taken.len();
    // the wires
    let mut tracks: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut edges = Vec::new();
    for &i in &mine {
        for (from, socket) in s.links[i].each() {
            let (c0, r0) = (before(from), if let Node::Slot(j) = from { row[j] } else { 0 });
            let gap = col[i] > c0;
            // nothing stands between them on their common row
            let clear = r0 == row[i] && !mine.iter().any(|k| row[*k] == r0 && (c0..col[i]).contains(&col[*k]));
            let via = if !gap || clear {
                Via::Direct
            } else if let Node::Slot(j) = from
                && tail[j]
            {
                Via::Row(r0)
            } else if row[i] > 0 && socket == Socket::Main && from == s.links[i].main && !mine.iter().any(|k| row[*k] == row[i] && (c0..col[i]).contains(&col[*k])) {
                Via::Row(row[i])
            } else {
                let (a, b) = (c0, col[i] - 1);
                let t = tracks.iter().position(|t| t.iter().all(|(x, y)| b < *x || a > *y)).unwrap_or_else(|| {
                    tracks.push(Vec::new());
                    tracks.len() - 1
                });
                tracks[t].push((a, b));
                Via::Track(t)
            };
            edges.push(Edge { from, to: i, socket, via });
        }
    }
    LaneLayout { rows, tracks: tracks.len(), row, edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kt_model::fx_strip::line_links;

    /// Test t3218b (fx-graph-dag D8): the layout — a line is one row in its order; an arm is a row
    /// under the trunk between the value it reads and the Merge it reaches; the Merge stands after
    /// its longest input; a dry wire into a Merge runs on a track, never across the nodes.
    #[test]
    fn t3218b_the_arms_are_rows_under_the_trunk() {
        let lanes = vec![0u8; 3];
        let s = Strip::new(&line_links(&lanes), &lanes);
        let col = columns(&s, &[]);
        let l = lane(&s, 0, &col);
        assert_eq!((col, l.rows, l.tracks, l.row.clone()), (vec![0, 1, 2], 1, 0, vec![0; 3]));
        assert!(l.edges.iter().all(|e| e.via == Via::Direct));
        // lead, blur, glow; an arm of one from the lead, merged; the dry lead into the matte
        let mut s = s;
        let arm = s.branch(0, Node::Slot(0));
        let m = s.merge(0, Node::Slot(arm));
        s.connect(m, Socket::Matte, Node::Slot(0));
        s.settle().unwrap();
        // lead 0, arm 1, blur 2, glow 3, merge 4
        let order: Vec<usize> = s.from.clone();
        let pos = |id: usize| order.iter().position(|x| *x == id).unwrap();
        let col = columns(&s, &[]);
        let l = lane(&s, 0, &col);
        assert_eq!((col[pos(3)], l.row[pos(3)]), (1, 1), "the arm stands under the trunk after the lead");
        assert_eq!((col[pos(4)], l.row[pos(4)]), (3, 0), "the Merge stands after the glow on the trunk");
        assert_eq!(l.rows, 2);
        let via = |to: usize, sk: Socket| l.edges.iter().find(|e| e.to == to && e.socket == sk).unwrap().via;
        assert_eq!(via(pos(4), Socket::Second), Via::Row(1));
        assert_eq!((via(pos(4), Socket::Matte), l.tracks), (Via::Track(0), 1));
        // a device's halves in two lanes share a column though one lane is shorter
        let lanes = vec![0u8, 0, 1];
        let s = Strip::new(&line_links(&lanes), &lanes);
        assert_eq!(columns(&s, &[(1, 2)]), vec![0, 1, 1]);
    }
}
