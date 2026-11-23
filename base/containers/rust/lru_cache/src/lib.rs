// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base/containers/lru_cache.h`, `hashing_lru_cache.h` and
//! `lru_cache_internal.h`.
//!
//! A least-recently-used cache with constant-time access by key. Entries
//! iterate from most to least recently used (`iter()`, `.rev()` for least to
//! most). A "use" is a `put` or a `get`. `peek` does not count as a use.
//!
//! Types (the C++ names are in parentheses):
//! - [`LruCache<K, V>`] (`base::LRUCache`): keys ordered by `Ord`. `index()`
//!   iterates in key order.
//! - [`HashingLruCache<K, V, S>`] (`base::HashingLRUCache`): keys hashed. The
//!   index order is unspecified.
//! - [`LruCacheSet<T>`] and [`HashingLruCacheSet<T, S>`] (`base::LRUCacheSet`
//!   and `base::HashingLRUCacheSet`): a set where each value is its own key.
//!
//! Differences from the C++ version:
//! - Iterators are not exposed. `iter()` returns a double-ended iterator of
//!   `(&K, &V)` (or `&T` for sets). The C++ `Put`, `Get` and `Peek` return
//!   iterators into the cache. The Rust versions return references to the
//!   entry, or `None` where the C++ returns `end()`. Erase-by-iterator is not
//!   needed, because the borrow checker prevents erasing while iterating. Use
//!   `remove(&key)`, `pop_lru()` or `shrink_to_size()` instead.
//! - Lookups take `&Q` where `K: Borrow<Q>`. This is the transparent lookup
//!   that the C++ `std::less<>` variant provides. For the hashing variant the
//!   `Hash` and `Eq` of `Q` must match those of `K`.
//! - `K` must be `Clone`, because the key is stored in both the recency list
//!   and the index. The C++ comment makes the same requirement.
//! - `max_size()` panics when the cache was created with `NO_AUTO_EVICT`. The
//!   C++ `CHECK`s in this case.
//! - `UpdateMaxSize`, `ShrinkToSize`, `Erase` and `Clear` keep their names in
//!   snake case: `update_max_size`, `shrink_to_size`, `remove` and `clear`.
//!   `Erase` takes a key, and `pop_lru` removes the least recent entry.
//! - `Swap` is `swap(&mut other)`. It exchanges everything, including the max
//!   size, as the C++ does.
//! - The hashing variants accept any `BuildHasher`. The C++ uses
//!   `absl::flat_hash_map`.
//! - The cache is move-only in both languages. Rust has no copy constructor,
//!   so this is the default.
//! - `Put` for a `max_size` of 0 returns `None` and inserts nothing. This
//!   matches the C++ `end()` return.
//!
//! Not ported:
//! - The `kLRUCacheMemoryConsumer` integration (`lru_cache_internal.cc`,
//!   `MemoryConsumerImpl`). The C++ scales `max_size()` by the process memory
//!   limit from the MemoryCoordinator, and this needs
//!   `base::SequencedTaskRunner` and `FeatureList`. Here `max_size()` is always
//!   the value given to the constructor or `update_max_size`. The
//!   `LRUCacheMemoryConsumerTest` cases are not ported for this reason.
//! - `EstimateMemoryUsage` (`trace_event::EstimateMemoryUsage`). The
//!   `LRUCacheTest.EstimateMemory` cases are not ported.
//! - The C++ `MoveAndSwap` and `UpdateMaxSize` memory-consumer cases, which
//!   depend on the same integration.
//!
//! Testing: `src/tests.rs` ports every `TYPED_TEST` and `TEST` from
//! `lru_cache_unittest.cc` that does not depend on the memory coordinator. It
//! also adds a randomized differential test against a simple `Vec` model, for
//! each cache type.
//!
//! No `unsafe` is used. The recency list is a slab in `recency.rs`.

#![forbid(unsafe_code)]

mod recency;

use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash};

use recency::Recency;

/// Maps keys to slots in the recency list. Implemented for `BTreeMap` and
/// `HashMap`. Users do not implement this trait.
pub trait SlotIndex<K>: Default {
    /// Each key with its slot, in the index's order.
    fn entries<'a>(&'a self) -> impl Iterator<Item = (&'a K, usize)>
    where
        K: 'a;

    /// Records `key` at `slot`, replacing any earlier entry for `key`.
    fn insert_slot(&mut self, key: K, slot: usize);

    /// Removes every entry.
    fn clear_slots(&mut self);
}

/// Lookups in the index by a borrowed form `Q` of the key.
pub trait SlotLookup<K, Q: ?Sized>: SlotIndex<K> {
    /// The slot for `key`, if present.
    fn find_slot(&self, key: &Q) -> Option<usize>;

    /// Removes `key` and returns its slot, if present.
    fn take_slot(&mut self, key: &Q) -> Option<usize>;
}

impl<K: Ord> SlotIndex<K> for BTreeMap<K, usize> {
    fn entries<'a>(&'a self) -> impl Iterator<Item = (&'a K, usize)>
    where
        K: 'a,
    {
        self.iter().map(|(key, &slot)| (key, slot))
    }

    fn insert_slot(&mut self, key: K, slot: usize) {
        self.insert(key, slot);
    }

    fn clear_slots(&mut self) {
        self.clear();
    }
}

impl<K, Q> SlotLookup<K, Q> for BTreeMap<K, usize>
where
    K: Ord + Borrow<Q>,
    Q: Ord + ?Sized,
{
    fn find_slot(&self, key: &Q) -> Option<usize> {
        self.get(key).copied()
    }

    fn take_slot(&mut self, key: &Q) -> Option<usize> {
        self.remove(key)
    }
}

impl<K: Eq + Hash, S: BuildHasher + Default> SlotIndex<K> for HashMap<K, usize, S> {
    fn entries<'a>(&'a self) -> impl Iterator<Item = (&'a K, usize)>
    where
        K: 'a,
    {
        self.iter().map(|(key, &slot)| (key, slot))
    }

    fn insert_slot(&mut self, key: K, slot: usize) {
        self.insert(key, slot);
    }

    fn clear_slots(&mut self) {
        self.clear();
    }
}

impl<K, Q, S> SlotLookup<K, Q> for HashMap<K, usize, S>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash + ?Sized,
    S: BuildHasher + Default,
{
    fn find_slot(&self, key: &Q) -> Option<usize> {
        self.get(key).copied()
    }

    fn take_slot(&mut self, key: &Q) -> Option<usize> {
        self.remove(key)
    }
}

/// An LRU cache of `(K, V)` pairs, with the index type `M`.
///
/// Use [`LruCache`] or [`HashingLruCache`] instead of naming this type.
pub struct LruCacheBase<K, V, M> {
    ordering: Recency<(K, V)>,
    index: M,
    /// `None` means no automatic eviction (`NO_AUTO_EVICT`).
    max_size: Option<usize>,
}

/// Map from `K` to `V` with an ordered index (`base::LRUCache`).
pub type LruCache<K, V> = LruCacheBase<K, V, BTreeMap<K, usize>>;

/// Map from `K` to `V` with a hashed index (`base::HashingLRUCache`).
pub type HashingLruCache<K, V, S = RandomState> = LruCacheBase<K, V, HashMap<K, usize, S>>;

impl<K, V, M: Default> LruCacheBase<K, V, M> {
    /// Pass as `max_size` to disable automatic eviction.
    pub const NO_AUTO_EVICT: Option<usize> = None;

    /// Creates an empty cache. When `max_size` is `Some(n)`, inserting into a
    /// full cache evicts the least recently used entries until there is room.
    pub fn new(max_size: Option<usize>) -> Self {
        Self {
            ordering: Recency::new(),
            index: M::default(),
            max_size,
        }
    }
}

impl<K, V, M: SlotIndex<K>> LruCacheBase<K, V, M> {
    /// The number of entries.
    pub fn len(&self) -> usize {
        self.ordering.len()
    }

    /// Whether the cache has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The maximum number of entries. Panics if the cache was created with
    /// `NO_AUTO_EVICT`, as the C++ `max_size()` `CHECK`s.
    pub fn max_size(&self) -> usize {
        self.max_size
            .expect("max_size() is not valid for a cache with NO_AUTO_EVICT")
    }

    /// Iterates from the most recently used entry to the least recently used.
    /// Use `.rev()` to go from least to most recent.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.ordering.iter().map(|(key, value)| (key, value))
    }

    /// Iterates the index. For [`LruCache`] this is in key order. Each item is
    /// an entry, and the order is the index's order.
    pub fn index(&self) -> impl Iterator<Item = (&K, &V)> {
        self.index.entries().map(move |(_, slot)| {
            let (key, value) = self.ordering.get(slot);
            (key, value)
        })
    }

    /// Removes every entry. The max size does not change.
    pub fn clear(&mut self) {
        self.index.clear_slots();
        self.ordering.clear();
    }

    /// Exchanges the contents of `self` and `other`, including the max size.
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(self, other);
    }
}

impl<K, V, M> LruCacheBase<K, V, M>
where
    M: SlotLookup<K, K>,
{
    /// Removes the least recently used entry and returns it.
    pub fn pop_lru(&mut self) -> Option<(K, V)> {
        let slot = self.ordering.back()?;
        let (key, value) = self.ordering.remove(slot);
        self.index.take_slot(&key);
        Some((key, value))
    }

    /// Evicts least recently used entries until at most `new_size` remain.
    pub fn shrink_to_size(&mut self, new_size: usize) {
        while self.len() > new_size {
            self.pop_lru();
        }
    }

    /// Changes the max size and evicts entries to fit it. Passing 0 evicts
    /// everything, and later inserts are then dropped.
    pub fn update_max_size(&mut self, new_max_size: usize) {
        self.max_size = Some(new_max_size);
        self.shrink_to_size(new_max_size);
    }
}

impl<K, V, M> LruCacheBase<K, V, M>
where
    K: Clone,
    M: SlotLookup<K, K>,
{
    /// Inserts `key` with `value` as the most recent entry. Any existing entry
    /// with an equal key is removed first. If the cache is full, the least
    /// recently used entries are evicted to make room.
    ///
    /// Returns the new entry, or `None` if the max size is 0 and nothing was
    /// inserted.
    pub fn put(&mut self, key: K, value: V) -> Option<(&K, &V)> {
        if let Some(slot) = self.index.take_slot(&key) {
            self.ordering.remove(slot);
        }

        if let Some(max_size) = self.max_size {
            if max_size == 0 {
                return None;
            }
            // Make room for the new entry.
            self.shrink_to_size(max_size - 1);
        }

        let slot = self.ordering.push_front((key.clone(), value));
        self.index.insert_slot(key, slot);
        let (key, value) = self.ordering.get(slot);
        Some((key, value))
    }
}

impl<K, V, M> LruCacheBase<K, V, M> {
    /// Looks up `key` and marks it as the most recent entry. Returns `None` if
    /// it is not present.
    pub fn get<Q: ?Sized>(&mut self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        M: SlotLookup<K, Q>,
    {
        let slot = self.index.find_slot(key)?;
        self.ordering.move_to_front(slot);
        let (key, value) = self.ordering.get(slot);
        Some((key, value))
    }

    /// Like [`get`](Self::get), but returns a mutable value.
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<(&K, &mut V)>
    where
        K: Borrow<Q>,
        M: SlotLookup<K, Q>,
    {
        let slot = self.index.find_slot(key)?;
        self.ordering.move_to_front(slot);
        let (key, value) = self.ordering.get_mut(slot);
        Some((&*key, value))
    }

    /// Looks up `key` without changing the recency order.
    pub fn peek<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        M: SlotLookup<K, Q>,
    {
        let slot = self.index.find_slot(key)?;
        let (key, value) = self.ordering.get(slot);
        Some((key, value))
    }

    /// Like [`peek`](Self::peek), but returns a mutable value.
    pub fn peek_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<(&K, &mut V)>
    where
        K: Borrow<Q>,
        M: SlotLookup<K, Q>,
    {
        let slot = self.index.find_slot(key)?;
        let (key, value) = self.ordering.get_mut(slot);
        Some((&*key, value))
    }

    /// Removes the entry for `key` and returns it.
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        M: SlotLookup<K, Q>,
    {
        let slot = self.index.take_slot(key)?;
        Some(self.ordering.remove(slot))
    }
}

/// Set of `T` with an ordered index (`base::LRUCacheSet`). Each value is its
/// own key.
pub struct LruCacheSetBase<T, M>(LruCacheBase<T, (), M>);

/// Set with an ordered index (`base::LRUCacheSet`).
pub type LruCacheSet<T> = LruCacheSetBase<T, BTreeMap<T, usize>>;

/// Set with a hashed index (`base::HashingLRUCacheSet`).
pub type HashingLruCacheSet<T, S = RandomState> = LruCacheSetBase<T, HashMap<T, usize, S>>;

impl<T, M: Default> LruCacheSetBase<T, M> {
    /// Pass as `max_size` to disable automatic eviction.
    pub const NO_AUTO_EVICT: Option<usize> = None;

    /// Creates an empty set. See [`LruCacheBase::new`].
    pub fn new(max_size: Option<usize>) -> Self {
        Self(LruCacheBase::new(max_size))
    }
}

impl<T, M: SlotIndex<T>> LruCacheSetBase<T, M> {
    /// The number of values.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The max size. Panics if the set was created with `NO_AUTO_EVICT`.
    pub fn max_size(&self) -> usize {
        self.0.max_size()
    }

    /// Iterates from the most recently used value to the least recently used.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.0.iter().map(|(value, _)| value)
    }

    /// Iterates the index. See [`LruCacheBase::index`].
    pub fn index(&self) -> impl Iterator<Item = &T> {
        self.0.index().map(|(value, _)| value)
    }

    /// Removes every value. The max size does not change.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Exchanges the contents of `self` and `other`, including the max size.
    pub fn swap(&mut self, other: &mut Self) {
        self.0.swap(&mut other.0);
    }
}

impl<T, M> LruCacheSetBase<T, M>
where
    M: SlotLookup<T, T>,
{
    /// Removes the least recently used value and returns it.
    pub fn pop_lru(&mut self) -> Option<T> {
        self.0.pop_lru().map(|(value, _)| value)
    }

    /// Evicts least recently used values until at most `new_size` remain.
    pub fn shrink_to_size(&mut self, new_size: usize) {
        self.0.shrink_to_size(new_size);
    }

    /// Changes the max size and evicts values to fit it.
    pub fn update_max_size(&mut self, new_max_size: usize) {
        self.0.update_max_size(new_max_size);
    }
}

impl<T, M> LruCacheSetBase<T, M>
where
    T: Clone,
    M: SlotLookup<T, T>,
{
    /// Inserts `value` as the most recent entry. An equal existing value is
    /// replaced by the new one. Returns the stored value, or `None` if the max
    /// size is 0.
    pub fn put(&mut self, value: T) -> Option<&T> {
        self.0.put(value, ()).map(|(value, _)| value)
    }
}

impl<T, M> LruCacheSetBase<T, M> {
    /// Looks up `value` and marks it as the most recent entry.
    pub fn get<Q: ?Sized>(&mut self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        M: SlotLookup<T, Q>,
    {
        self.0.get(value).map(|(value, _)| value)
    }

    /// Looks up `value` without changing the recency order.
    pub fn peek<Q: ?Sized>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        M: SlotLookup<T, Q>,
    {
        self.0.peek(value).map(|(value, _)| value)
    }

    /// Removes `value` and returns it.
    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        T: Borrow<Q>,
        M: SlotLookup<T, Q>,
    {
        self.0.remove(value).map(|(value, _)| value)
    }
}

#[cfg(test)]
mod tests;
