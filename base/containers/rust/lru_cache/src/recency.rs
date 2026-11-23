// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! The recency list used by the LRU caches. It is a doubly linked list stored
//! in a `Vec` (a slab) and linked by slot index, so it needs no `unsafe`. The
//! front is the most recently used entry and the back is the least recently
//! used.

/// Marks "no neighbor" in `prev` and `next`.
const NIL: usize = usize::MAX;

struct Node<T> {
    value: T,
    prev: usize,
    next: usize,
}

/// A list of `T` ordered from most to least recently used. Each entry has a
/// stable slot index until it is removed. A removed slot may be reused by a
/// later insertion.
pub(crate) struct Recency<T> {
    nodes: Vec<Option<Node<T>>>,
    free: Vec<usize>,
    front: usize,
    back: usize,
    len: usize,
}

impl<T> Recency<T> {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            free: Vec::new(),
            front: NIL,
            back: NIL,
            len: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    fn node(&self, slot: usize) -> &Node<T> {
        self.nodes[slot].as_ref().expect("slot is live")
    }

    fn node_mut(&mut self, slot: usize) -> &mut Node<T> {
        self.nodes[slot].as_mut().expect("slot is live")
    }

    /// Inserts `value` at the front and returns its slot.
    pub(crate) fn push_front(&mut self, value: T) -> usize {
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None => {
                self.nodes.push(None);
                self.nodes.len() - 1
            }
        };
        self.nodes[slot] = Some(Node {
            value,
            prev: NIL,
            next: self.front,
        });
        if self.front == NIL {
            self.back = slot;
        } else {
            let old_front = self.front;
            self.node_mut(old_front).prev = slot;
        }
        self.front = slot;
        self.len += 1;
        slot
    }

    /// Unlinks `slot` from the list without freeing it.
    fn unlink(&mut self, slot: usize) {
        let (prev, next) = {
            let node = self.node(slot);
            (node.prev, node.next)
        };
        if prev == NIL {
            self.front = next;
        } else {
            self.node_mut(prev).next = next;
        }
        if next == NIL {
            self.back = prev;
        } else {
            self.node_mut(next).prev = prev;
        }
    }

    /// Moves a live `slot` to the front.
    pub(crate) fn move_to_front(&mut self, slot: usize) {
        if self.front == slot {
            return;
        }
        self.unlink(slot);
        let old_front = self.front;
        {
            let node = self.node_mut(slot);
            node.prev = NIL;
            node.next = old_front;
        }
        self.node_mut(old_front).prev = slot;
        self.front = slot;
    }

    /// Removes a live `slot` and returns its value.
    pub(crate) fn remove(&mut self, slot: usize) -> T {
        self.unlink(slot);
        let node = self.nodes[slot].take().expect("slot is live");
        self.free.push(slot);
        self.len -= 1;
        node.value
    }

    pub(crate) fn get(&self, slot: usize) -> &T {
        &self.node(slot).value
    }

    pub(crate) fn get_mut(&mut self, slot: usize) -> &mut T {
        &mut self.node_mut(slot).value
    }

    /// The slot of the least recently used entry, if any.
    pub(crate) fn back(&self) -> Option<usize> {
        (self.back != NIL).then_some(self.back)
    }

    pub(crate) fn clear(&mut self) {
        self.nodes.clear();
        self.free.clear();
        self.front = NIL;
        self.back = NIL;
        self.len = 0;
    }

    /// Iterates from the most recently used entry to the least recently used.
    pub(crate) fn iter(&self) -> Iter<'_, T> {
        Iter {
            list: self,
            front: self.front,
            back: self.back,
            remaining: self.len,
        }
    }
}

/// Iterator over a [`Recency`] list, from most to least recently used.
pub(crate) struct Iter<'a, T> {
    list: &'a Recency<T>,
    front: usize,
    back: usize,
    remaining: usize,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if self.remaining == 0 {
            return None;
        }
        let list: &'a Recency<T> = self.list;
        let node = list.node(self.front);
        self.front = node.next;
        self.remaining -= 1;
        Some(&node.value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let list: &Recency<T> = self.list;
        let node = list.node(self.back);
        self.back = node.prev;
        self.remaining -= 1;
        Some(&node.value)
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}
