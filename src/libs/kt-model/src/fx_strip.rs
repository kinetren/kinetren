//! The editor's side of an effect graph (fx-graph-dag D8): one strip of slots holding one or two
//! lanes, wired by ADDRESS OF A SLOT, not by "the slot before" — so a gesture re-wires first and the
//! processing order is found after it (`settle`, the stable topological order of design D1). A lane
//! without branches settles to the order it already had.

use crate::fx_graph::{canonical, encode, reads_of, resolve, topo_order, trunk_len, GraphWhy, Matte, MatteFrom, Node, SlotInputs, Socket};

/// A slot's inputs by address: the lane's Source or a slot of the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Links {
    pub main: Node,
    pub second: Option<Node>,
    pub matte: Option<(Node, MatteFrom)>,
}

impl Links {
    pub fn line(main: Node) -> Links {
        Links { main, second: None, matte: None }
    }

    /// Every input with its socket.
    pub fn each(&self) -> impl Iterator<Item = (Node, Socket)> + use<> {
        std::iter::once((self.main, Socket::Main))
            .chain(self.second.map(|n| (n, Socket::Second)))
            .chain(self.matte.map(|(n, _)| (n, Socket::Matte)))
    }

    fn map(&mut self, f: impl Fn(Node) -> Node) {
        self.main = f(self.main);
        self.second = self.second.map(&f);
        self.matte = self.matte.map(|(n, by)| (f(n), by));
    }

    pub fn get(&self, s: Socket) -> Option<Node> {
        match s {
            Socket::Main => Some(self.main),
            Socket::Second => self.second,
            Socket::Matte => self.matte.map(|m| m.0),
        }
    }

    fn set(&mut self, s: Socket, n: Node) {
        match s {
            Socket::Main => self.main = n,
            Socket::Second => self.second = Some(n),
            Socket::Matte => self.matte = Some((n, self.matte.map_or(MatteFrom::Luma, |m| m.1))),
        }
    }
}

/// The links of a chain of `n` slots wired by `fx_in`, its slots standing at `base..base + n`.
pub fn links_of(fx_in: &[SlotInputs], n: usize, base: usize) -> Vec<Links> {
    let at = |x: Node| match x {
        Node::Source => Node::Source,
        Node::Slot(j) => Node::Slot(base + j),
    };
    (0..n)
        .map(|i| {
            let mut l = Links::line(Node::Source);
            for (node, socket) in reads_of(fx_in, i) {
                l.set(socket, at(node));
            }
            if let (Some(m), Some(x)) = (l.matte.as_mut(), crate::fx_graph::inputs(fx_in, i).matte) {
                m.1 = x.by;
            }
            l
        })
        .collect()
}

/// The links of lanes without branches: every slot reads the one before it in its lane.
pub fn line_links(lane: &[u8]) -> Vec<Links> {
    let mut last = [Node::Source; 2];
    lane.iter()
        .enumerate()
        .map(|(i, &l)| {
            let k = usize::from(l == 1);
            let x = Links::line(last[k]);
            last[k] = Node::Slot(i);
            x
        })
        .collect()
}

/// Where a moved slot goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum To {
    /// On the main wire into this slot.
    Before(usize),
    /// On this slot's output: every reader of it reads the moved slot.
    After(usize),
    /// On one wire: the reader's socket.
    Wire(usize, Socket),
    /// On the wire into the lane's Output.
    Out,
}

/// The strip: one or two lanes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strip {
    pub links: Vec<Links>,
    /// 0 the picture (or the only lane), 1 the sound.
    pub lane: Vec<u8>,
    /// Which slot of the strip each one was; a slot added by the gesture has the next free number.
    pub from: Vec<usize>,
    /// The slot each lane's Output reads (`None` — an empty lane).
    pub out: [Option<usize>; 2],
    next: usize,
}

impl Strip {
    /// A strip as the chain holds it: the Output is the lane's last slot.
    pub fn new(links: &[Links], lane: &[u8]) -> Strip {
        let mut out = [None; 2];
        for (i, l) in lane.iter().enumerate() {
            out[usize::from(*l == 1)] = Some(i);
        }
        Strip { links: links.to_vec(), lane: lane.to_vec(), from: (0..lane.len()).collect(), out, next: lane.len() }
    }

    fn k(&self, i: usize) -> usize {
        usize::from(self.lane[i] == 1)
    }

    /// A lane without branches: every slot reads the one before it.
    pub fn is_line(&self, lane: u8) -> bool {
        let mut last = Node::Source;
        for i in (0..self.lane.len()).filter(|i| self.lane[*i] == lane) {
            if self.links[i] != Links::line(last) {
                return false;
            }
            last = Node::Slot(i);
        }
        true
    }

    /// Who reads `node` (of `lane`): the slots and their sockets.
    pub fn readers(&self, lane: u8, node: Node) -> Vec<(usize, Socket)> {
        (0..self.links.len())
            .filter(|i| self.lane[*i] == lane)
            .flat_map(|i| self.links[i].each().map(move |(n, s)| (i, n, s)))
            .filter(|(_, n, _)| *n == node)
            .map(|(i, _, s)| (i, s))
            .collect()
    }

    /// The slots of `lane` that reach its Output.
    pub fn live(&self, lane: u8) -> Vec<bool> {
        let mut live = vec![false; self.links.len()];
        let mut todo: Vec<usize> = self.out[usize::from(lane == 1)].into_iter().collect();
        while let Some(i) = todo.pop() {
            if std::mem::replace(&mut live[i], true) {
                continue;
            }
            todo.extend(self.links[i].each().filter_map(|(n, _)| match n {
                Node::Slot(j) => Some(j),
                Node::Source => None,
            }));
        }
        live
    }

    fn push(&mut self, lane: u8, l: Links) -> usize {
        self.links.push(l);
        self.lane.push(lane);
        self.from.push(self.next);
        self.next += 1;
        self.links.len() - 1
    }

    fn out_node(&self, lane: u8) -> Node {
        self.out[usize::from(lane == 1)].map_or(Node::Source, Node::Slot)
    }

    /// A slot added at the end of the lane: it becomes the Output.
    pub fn append(&mut self, lane: u8) -> usize {
        let l = Links::line(self.out_node(lane));
        let i = self.push(lane, l);
        self.out[usize::from(lane == 1)] = Some(i);
        i
    }

    /// A new arm starting from `from`: the Output stays where it was. From the Output itself (or in
    /// an empty lane) there is nothing to branch from: the chain goes on.
    pub fn branch(&mut self, lane: u8, from: Node) -> usize {
        if from == self.out_node(lane) {
            return self.append(lane);
        }
        self.push(lane, Links::line(from))
    }

    /// A Merge before the Output: the Output becomes its Background, `from` its Foreground (none when
    /// `from` is the Output: the socket waits for its wire).
    pub fn merge(&mut self, lane: u8, from: Node) -> usize {
        let out = self.out_node(lane);
        let i = self.append(lane);
        if from != out {
            self.links[i].second = Some(from);
        }
        i
    }

    /// A block of `n` slots wired by `fx_in` relative to itself (a paste, a preset, design D6): after
    /// the slot `after`, taking its readers, or at the end of the lane. Returns its slots.
    pub fn block(&mut self, lane: u8, after: Option<usize>, fx_in: &[SlotInputs], n: usize) -> Vec<usize> {
        let base = self.links.len();
        if n == 0 {
            return Vec::new();
        }
        let src = after.map_or(self.out_node(lane), Node::Slot);
        for l in links_of(fx_in, n, base) {
            let mut l = l;
            l.map(|x| if x == Node::Source { src } else { x });
            self.push(lane, l);
        }
        let last = base + n - 1;
        match after {
            Some(j) if self.out[usize::from(lane == 1)] != Some(j) => {
                for (r, s) in self.readers(lane, Node::Slot(j)) {
                    if r < base {
                        self.links[r].set(s, Node::Slot(last));
                    }
                }
            }
            _ => self.out[usize::from(lane == 1)] = Some(last),
        }
        (base..base + n).collect()
    }

    /// A new slot on a wire or on an output.
    pub fn insert(&mut self, lane: u8, to: To) -> usize {
        let i = self.push(lane, Links::line(Node::Source));
        self.place(i, to);
        i
    }

    fn place(&mut self, i: usize, to: To) {
        let lane = self.lane[i];
        let k = self.k(i);
        match to {
            To::Before(j) => self.place(i, To::Wire(j, Socket::Main)),
            To::Wire(j, s) => {
                if let Some(n) = self.links[j].get(s) {
                    self.links[i].main = n;
                    self.links[j].set(s, Node::Slot(i));
                }
            }
            To::After(j) => {
                for (r, s) in self.readers(lane, Node::Slot(j)) {
                    if r != i {
                        self.links[r].set(s, Node::Slot(i));
                    }
                }
                self.links[i].main = Node::Slot(j);
                if self.out[k] == Some(j) {
                    self.out[k] = Some(i);
                }
            }
            To::Out => {
                if self.out[k] != Some(i) {
                    self.links[i].main = self.out_node(lane);
                    self.out[k] = Some(i);
                }
            }
        }
    }

    /// Take slot `i` out of its wires: its readers read what it read (the rule of every removal).
    fn take_out(&mut self, i: usize) {
        let (lane, k) = (self.lane[i], self.k(i));
        let main = self.links[i].main;
        for (r, s) in self.readers(lane, Node::Slot(i)) {
            self.links[r].set(s, main);
        }
        if self.out[k] == Some(i) {
            // the Output reads what the slot read when that slot can stand last; else the lane's
            // last other slot is the Output, as the model's order has it
            let others = || (0..self.lane.len()).rev().filter(|j| *j != i && self.lane[*j] == lane);
            let can = |j: usize| self.readers(lane, Node::Slot(j)).iter().all(|(r, _)| *r == i);
            self.out[k] = match main {
                Node::Slot(j) if can(j) => Some(j),
                _ => others().find(|j| can(*j)).or_else(|| others().next()),
            };
        }
    }

    /// Move slot `i`; its other inputs keep their wires.
    pub fn shift(&mut self, i: usize, to: To) {
        let same = match to {
            To::Before(j) | To::After(j) | To::Wire(j, _) => j == i,
            To::Out => self.out[self.k(i)] == Some(i),
        };
        // a wire the slot itself stands on is the place it already has
        if same || matches!(to, To::Wire(j, s) if self.links[j].get(s) == Some(Node::Slot(i))) {
            return;
        }
        self.take_out(i);
        self.place(i, to);
    }

    /// Remove slot `i`: its readers read its main input.
    pub fn remove(&mut self, i: usize) {
        self.take_out(i);
        self.links.remove(i);
        self.lane.remove(i);
        self.from.remove(i);
        let down = |n: Node| match n {
            Node::Slot(j) if j > i => Node::Slot(j - 1),
            other => other,
        };
        for l in &mut self.links {
            l.map(down);
        }
        for o in &mut self.out {
            *o = o.map(|j| if j > i { j - 1 } else { j });
        }
    }

    /// Wire `node` into a socket.
    pub fn connect(&mut self, i: usize, s: Socket, node: Node) {
        self.links[i].set(s, node);
    }

    /// Take the wire off a Foreground, a key or a Matte socket.
    pub fn disconnect(&mut self, i: usize, s: Socket) {
        match s {
            Socket::Main => {}
            Socket::Second => self.links[i].second = None,
            Socket::Matte => self.links[i].matte = None,
        }
    }

    /// The lane's Output reads `i`.
    pub fn set_out(&mut self, i: usize) {
        let k = self.k(i);
        self.out[k] = Some(i);
    }

    /// Put the slots into a processing order: stable (a valid order is returned as it is), each lane
    /// in the places it has, its Output last. Returns where each slot came from (the index before
    /// the call). A loop, or a slot that reads the Output, is refused and changes nothing.
    pub fn settle(&mut self) -> Result<Vec<usize>, GraphWhy> {
        let n = self.links.len();
        let mut order: Vec<usize> = (0..n).collect();
        for lane in [0u8, 1] {
            let mine: Vec<usize> = (0..n).filter(|i| self.lane[*i] == lane).collect();
            let at = |i: usize| mine.iter().position(|x| *x == i);
            let out = self.out[usize::from(lane == 1)];
            let edges = |q: usize| -> Vec<usize> {
                let mut e: Vec<usize> = self.links[mine[q]]
                    .each()
                    .filter_map(|(n, _)| match n {
                        Node::Slot(j) => at(j),
                        Node::Source => None,
                    })
                    .collect();
                if Some(mine[q]) == out {
                    e.extend((0..mine.len()).filter(|x| *x != q));
                }
                e.sort_unstable();
                e.dedup();
                e
            };
            let sorted = topo_order(mine.len(), edges).ok_or(GraphWhy::Forward)?;
            for (place, q) in sorted.into_iter().enumerate() {
                order[mine[place]] = mine[q];
            }
        }
        let mut to = vec![0usize; n];
        for (new, old) in order.iter().enumerate() {
            to[*old] = new;
        }
        let map = |x: Node| match x {
            Node::Slot(j) => Node::Slot(to[j]),
            Node::Source => Node::Source,
        };
        let links: Vec<Links> = order
            .iter()
            .map(|o| {
                let mut l = self.links[*o];
                l.map(map);
                l
            })
            .collect();
        self.links = links;
        self.from = order.iter().map(|o| self.from[*o]).collect();
        self.out = self.out.map(|o| o.map(|j| to[j]));
        Ok(order)
    }

    /// The wiring of one lane as its chain stores it (the strip settled).
    pub fn wiring(&self, lane: u8) -> Vec<SlotInputs> {
        wiring_of(&self.links, &self.lane, lane)
    }
}

/// The wiring of one lane as its chain stores it: canonical, so a line is the empty list.
pub fn wiring_of(links: &[Links], lanes: &[u8], lane: u8) -> Vec<SlotInputs> {
    let mine: Vec<usize> = (0..lanes.len()).filter(|i| lanes[*i] == lane).collect();
    let at = |n: Node| match n {
        Node::Source => Node::Source,
        Node::Slot(j) => mine.iter().position(|x| *x == j).map_or(Node::Source, Node::Slot),
    };
    let mut out: Vec<SlotInputs> = mine
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            let l = &links[i];
            SlotInputs {
                main: encode(k, at(l.main)),
                second: l.second.map(|n| encode(k, at(n))),
                matte: l.matte.map(|(n, by)| Matte { from: encode(k, at(n)), by }),
            }
        })
        .collect();
    canonical(&mut out, mine.len());
    out
}

/// The wiring of a chain whose leading Geometry node is the slot `lead` of it, without the node: its
/// readers read what it read. The lead stands before the first branch, or the edit is refused.
pub fn without_lead(fx_in: &[SlotInputs], n: usize, lead: usize) -> Result<Vec<SlotInputs>, GraphWhy> {
    if trunk_len(fx_in, n) <= lead || resolve(lead, crate::fx_graph::inputs(fx_in, lead).main) != before(lead) {
        return Err(GraphWhy::LeadInArm);
    }
    let mut w = fx_in.to_vec();
    crate::fx_graph::remove_slot(&mut w, n, lead);
    Ok(w)
}

fn before(i: usize) -> Node {
    if i == 0 { Node::Source } else { Node::Slot(i - 1) }
}
