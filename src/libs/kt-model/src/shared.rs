//! Event cells shared between project versions (shared-events D1, D1a). `Shared<T>` is an `Arc` with value
//! semantics: a clone copies a pointer, a write (`edit`) copies the value only while another version holds
//! it, equality is pointer-first, Debug and serde are the value's own, so every file byte, Debug-text key
//! and hash stays what it was. Its fingerprint is the hash of its Debug text, computed once per version of
//! the value: `edit` clears it. `Events<T>` is the vector of cells with a `Vec<T>`'s surface; there is no
//! `iter_mut` and no `&mut` iteration on purpose — a loop over every event that writes two must copy two.

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

// ---- the Debug-text hash (the builders' keys) -------------------------------------

/// A streaming hasher behind `fmt::Write`: the Debug text of a value hashed with no `String` built.
/// The digest equals `xxh3_64` of the text.
pub struct DebugHash(xxhash_rust::xxh3::Xxh3);

impl DebugHash {
    #[allow(clippy::new_without_default)]
    pub fn new() -> DebugHash {
        DebugHash(xxhash_rust::xxh3::Xxh3::new())
    }

    pub fn digest(&self) -> u64 {
        self.0.digest()
    }
}

impl std::fmt::Write for DebugHash {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.update(s.as_bytes());
        Ok(())
    }
}

/// `xxh3_64(format!("{v:?}"))` without the `String`.
pub fn hash_debug(v: &impl std::fmt::Debug) -> u64 {
    hash_fmt(format_args!("{v:?}"))
}

/// `xxh3_64(format!(…))` without the `String`.
pub fn hash_fmt(args: std::fmt::Arguments<'_>) -> u64 {
    let mut h = DebugHash::new();
    let _ = std::fmt::write(&mut h, args);
    h.digest()
}

// ---- the cell ---------------------------------------------------------------------

struct Cell<T> {
    value: T,
    /// `hash_debug(&value)`, asked at least once for THIS value.
    fp: OnceLock<u64>,
}

/// A value shared between versions, copied on write.
pub struct Shared<T>(Arc<Cell<T>>);

impl<T> Shared<T> {
    pub fn new(value: T) -> Shared<T> {
        Shared(Arc::new(Cell { value, fp: OnceLock::new() }))
    }

    /// Whether two cells are one (no value is looked at).
    pub fn same(a: &Shared<T>, b: &Shared<T>) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
}

impl<T: Clone> Shared<T> {
    /// The value for writing: copied first while another version holds it, its fingerprint cleared.
    pub fn edit(&mut self) -> &mut T {
        if Arc::get_mut(&mut self.0).is_none() {
            kt_time::cost::add(kt_time::cost::Cost::EventClone, 1);
            self.0 = Arc::new(Cell { value: self.0.value.clone(), fp: OnceLock::new() });
        }
        let cell = Arc::get_mut(&mut self.0).expect("a cell of its own");
        cell.fp = OnceLock::new();
        &mut cell.value
    }

    /// Write only where the test says so: a value that would not change is not copied.
    pub fn edit_if(&mut self, would: impl FnOnce(&T) -> bool, write: impl FnOnce(&mut T)) -> bool {
        let yes = would(&self.0.value);
        if yes {
            write(self.edit());
        }
        yes
    }

    pub fn into_inner(self) -> T {
        match Arc::try_unwrap(self.0) {
            Ok(c) => c.value,
            Err(a) => {
                kt_time::cost::add(kt_time::cost::Cost::EventClone, 1);
                a.value.clone()
            }
        }
    }
}

impl<T: std::fmt::Debug> Shared<T> {
    /// `hash_debug` of the value, computed once per version of it.
    pub fn fp(&self) -> u64 {
        *self.0.fp.get_or_init(|| {
            kt_time::cost::add(kt_time::cost::Cost::EventHash, 1);
            hash_debug(&self.0.value)
        })
    }
}

impl<T> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0.value
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T: PartialEq> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        Shared::same(self, other) || self.0.value == other.0.value
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.value.fmt(f)
    }
}

impl<T> From<T> for Shared<T> {
    fn from(v: T) -> Self {
        Shared::new(v)
    }
}

impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.value.serialize(s)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Shared<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Shared::new)
    }
}

// ---- the container ----------------------------------------------------------------

/// A vector of shared cells with a `Vec<T>`'s surface (D1a).
pub struct Events<T>(Vec<Shared<T>>);

/// Iteration by value reference.
pub struct Iter<'a, T>(std::slice::Iter<'a, Shared<T>>);

impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Iter(self.0.clone())
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        self.0.next().map(|c| &**c)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
    fn nth(&mut self, n: usize) -> Option<&'a T> {
        self.0.nth(n).map(|c| &**c)
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|c| &**c)
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

impl<T> Events<T> {
    pub const fn new() -> Events<T> {
        Events(Vec::new())
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.0.iter())
    }

    /// The cells themselves (the builders' fingerprints, a pass that hands an untouched event on).
    pub fn cells(&self) -> &[Shared<T>] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn first(&self) -> Option<&T> {
        self.0.first().map(|c| &**c)
    }

    pub fn last(&self) -> Option<&T> {
        self.0.last().map(|c| &**c)
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        self.0.get(i).map(|c| &**c)
    }

    pub fn push(&mut self, v: T) {
        self.0.push(Shared::new(v));
    }

    /// The same cell in another vector: nothing is copied.
    pub fn push_shared(&mut self, c: Shared<T>) {
        self.0.push(c);
    }

    pub fn insert(&mut self, i: usize, v: T) {
        self.0.insert(i, Shared::new(v));
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.0.retain(|c| keep(c));
    }

    pub fn truncate(&mut self, n: usize) {
        self.0.truncate(n);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.0.swap(a, b);
    }

    pub fn sort_by_key<K: Ord>(&mut self, mut key: impl FnMut(&T) -> K) {
        self.0.sort_by_key(|c| key(c));
    }

    pub fn sort_by(&mut self, mut cmp: impl FnMut(&T, &T) -> std::cmp::Ordering) {
        self.0.sort_by(|a, b| cmp(a, b));
    }

    /// Append another vector's cells, emptying it (no value is copied).
    pub fn append(&mut self, other: &mut Events<T>) {
        self.0.append(&mut other.0);
    }
}

impl<T: Clone> Events<T> {
    /// The event `i` for writing.
    pub fn edit(&mut self, i: usize) -> &mut T {
        self.0[i].edit()
    }

    /// The first event the test takes, for writing; no other is copied.
    pub fn edit_where(&mut self, mut is: impl FnMut(&T) -> bool) -> Option<&mut T> {
        let i = self.0.iter().position(|c| is(c))?;
        Some(self.0[i].edit())
    }

    /// Write every event the test takes; the others are not copied. How many were written.
    pub fn edit_if(&mut self, mut would: impl FnMut(&T) -> bool, mut write: impl FnMut(&mut T)) -> usize {
        let mut n = 0;
        for c in &mut self.0 {
            if would(c) {
                write(c.edit());
                n += 1;
            }
        }
        n
    }

    /// EVERY event for writing: each shared cell the iteration reaches is copied. For loaders and builders
    /// of a project nobody else holds yet.
    pub fn edit_all(&mut self) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        self.0.iter_mut().map(Shared::edit)
    }

    pub fn remove(&mut self, i: usize) -> T {
        self.0.remove(i).into_inner()
    }

    pub fn pop(&mut self) -> Option<T> {
        self.0.pop().map(Shared::into_inner)
    }

    pub fn drain(&mut self, r: impl std::ops::RangeBounds<usize>) -> impl Iterator<Item = T> + '_ {
        self.0.drain(r).map(Shared::into_inner)
    }

    pub fn to_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }
}

impl<T> std::ops::Index<usize> for Events<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        &self.0[i]
    }
}

impl<T: Clone> std::ops::IndexMut<usize> for Events<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        self.0[i].edit()
    }
}

impl<'a, T> IntoIterator for &'a Events<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<T: Clone> IntoIterator for Events<T> {
    type Item = T;
    type IntoIter = std::iter::Map<std::vec::IntoIter<Shared<T>>, fn(Shared<T>) -> T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().map(Shared::into_inner)
    }
}

impl<T> Extend<T> for Events<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, it: I) {
        self.0.extend(it.into_iter().map(Shared::new));
    }
}

impl<T> FromIterator<T> for Events<T> {
    fn from_iter<I: IntoIterator<Item = T>>(it: I) -> Self {
        Events(it.into_iter().map(Shared::new).collect())
    }
}

impl<T> From<Vec<T>> for Events<T> {
    fn from(v: Vec<T>) -> Self {
        v.into_iter().collect()
    }
}

impl<T, const N: usize> From<[T; N]> for Events<T> {
    fn from(v: [T; N]) -> Self {
        v.into_iter().collect()
    }
}

impl<T> Default for Events<T> {
    fn default() -> Self {
        Events::new()
    }
}

impl<T> Clone for Events<T> {
    fn clone(&self) -> Self {
        Events(self.0.clone())
    }
}

impl<T: PartialEq> PartialEq for Events<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: PartialEq> PartialEq<Vec<T>> for Events<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        self.len() == other.len() && self.iter().zip(other).all(|(a, b)| a == b)
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Events<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Serialize> Serialize for Events<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Events<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Vec::<T>::deserialize(d).map(Events::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kt_time::cost::{Cost, meter};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Ev {
        id: u32,
        name: String,
    }

    fn ev(id: u32) -> Ev {
        Ev { id, name: format!("event {id}") }
    }

    /// Test t3376 (shared-events D1, D1a; capability `project-format`): the cell and the container are
    /// invisible — Debug text (plain and pretty), serde bytes and equality are a `Vec<T>`'s; a clone copies
    /// pointers; a write copies the one cell it writes, and only while it is shared; `edit_if` copies what it
    /// writes; a removal of a shared event gives the value and leaves the other version whole.
    #[test]
    fn t3376_a_shared_vector_is_a_vector() {
        let plain: Vec<Ev> = (0..5).map(ev).collect();
        let events: Events<Ev> = plain.clone().into();
        assert_eq!(format!("{events:?}"), format!("{plain:?}"));
        assert_eq!(format!("{events:#?}"), format!("{plain:#?}"));
        let text = serde_json::to_string(&plain).unwrap();
        assert_eq!(serde_json::to_string(&events).unwrap(), text);
        assert_eq!(serde_json::from_str::<Events<Ev>>(&text).unwrap(), events);
        assert_eq!(events, plain);
        assert_eq!(events.iter().rev().map(|e| e.id).collect::<Vec<_>>(), [4, 3, 2, 1, 0]);
        assert_eq!((events.len(), events[2].id, events.first().unwrap().id, events.last().unwrap().id), (5, 2, 0, 4));

        let clones = |f: &mut dyn FnMut()| meter(f).1[Cost::EventClone as usize];
        let mut a = events.clone();
        assert_eq!(clones(&mut || drop(events.clone())), 0, "a clone copied events");
        assert_eq!(clones(&mut || a[1].name = "x".into()), 1, "a write copied more than its event");
        assert_eq!(clones(&mut || a[1].name = "y".into()), 0, "a cell of its own was copied again");
        assert_eq!((events[1].name.as_str(), a[1].name.as_str()), ("event 1", "y"), "the write reached the other version");
        assert!(a != events && a.cells()[0] == events.cells()[0] && Shared::same(&a.cells()[0], &events.cells()[0]));
        assert_eq!(clones(&mut || assert_eq!(a.edit_if(|e| e.id > 2, |e| e.id += 10), 2)), 2, "edit_if copied what it did not write");
        assert_eq!(clones(&mut || a.edit_where(|e| e.id == 13).unwrap().name.clear()), 0);
        assert_eq!(clones(&mut || assert!(a.edit_where(|e| e.id == 99).is_none())), 0);
        let mut b = events.clone();
        assert_eq!(clones(&mut || assert_eq!(b.remove(0), ev(0))), 1);
        assert_eq!((b.len(), events.len()), (4, 5));
        assert_eq!(clones(&mut || b.edit_all().for_each(|e| e.id += 1)), 4, "edit_all writes every event and says so");
        assert_eq!(b.iter().map(|e| e.id).collect::<Vec<_>>(), [2, 3, 4, 5]);
        b.push_shared(events.cells()[4].clone());
        assert!(Shared::same(b.cells().last().unwrap(), &events.cells()[4]), "a handed cell was copied");
        b.retain(|e| e.id % 2 == 0);
        b.insert(0, ev(7));
        b.extend([ev(8)]);
        assert_eq!(b.into_iter().map(|e| e.id).collect::<Vec<_>>(), [7, 2, 4, 4, 8]);
    }

    /// Test t3377 (shared-events D1, D3; capability `transport`): the fingerprint is the hash of the Debug text,
    /// computed once per version of the value whoever asks; a write clears it, a clone carries it.
    #[test]
    fn t3377_the_fingerprint_is_once_per_version() {
        let hashes = |f: &mut dyn FnMut()| meter(f).1[Cost::EventHash as usize];
        let a = Shared::new(ev(1));
        let want = hash_debug(&ev(1));
        assert_eq!(want, xxhash_rust::xxh3::xxh3_64(format!("{:?}", ev(1)).as_bytes()));
        assert_eq!(hashes(&mut || assert_eq!(a.fp(), want)), 1);
        let mut b = a.clone();
        assert_eq!(hashes(&mut || assert_eq!((a.fp(), b.fp()), (want, want))), 0, "a clone hashed again");
        b.edit().id = 2;
        assert_eq!(b.fp(), hash_debug(&ev_named(2, "event 1")), "a written cell kept a stale fingerprint");
        assert_eq!(a.fp(), want);
        // a cell of its own: written in place, the fingerprint still cleared
        b.edit().id = 3;
        assert_eq!(b.fp(), hash_debug(&ev_named(3, "event 1")));
        assert!(b.edit_if(|e| e.id == 3, |e| e.id = 4) && !b.edit_if(|e| e.id == 3, |e| e.id = 5));
        assert_eq!(b.fp(), hash_debug(&ev_named(4, "event 1")));
    }

    fn ev_named(id: u32, name: &str) -> Ev {
        Ev { id, name: name.into() }
    }
}
