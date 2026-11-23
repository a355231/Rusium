// Copyright 2017 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Dictionary iterator types (base/value_iterators.{h,cc}).
//!
//! The C++ `dict_iterator` and `const_dict_iterator` wrap a `flat_map` and
//! present `pair<const std::string&, Value&>` and `pair<const std::string&,
//! const Value&>` as references. These types wrap `BTreeMap` iterators and
//! yield `(&str, &Value)`, `(&str, &mut Value)` and `(String, Value)` tuples.
//!
//! Differences from the C++ version:
//! - Rust iterators have no separate `operator==` on iterator objects. Tests
//!   compare the sequences they produce.
//! - Iterators are `Clone` and `DoubleEndedIterator`, as the C++ bidirectional
//!   iterators are. `Clone` replaces the copy in `operator++(int)`.
//! - The mutable iterator yields `&mut Value` and a shared `&str` key. This is
//!   the same split as `pair<const std::string&, Value&>`. It is enforced by
//!   the types, so a key cannot be assigned through the iterator.

use std::collections::btree_map;
use std::iter::FusedIterator;

use crate::value::Value;

/// Iterator over `(key, value)` pairs of a `DictValue`, in key order.
#[derive(Clone, Debug)]
pub struct DictIter<'a> {
    inner: btree_map::Iter<'a, String, Value>,
}

impl<'a> DictIter<'a> {
    pub(crate) fn new(inner: btree_map::Iter<'a, String, Value>) -> DictIter<'a> {
        DictIter { inner }
    }
}

impl<'a> Iterator for DictIter<'a> {
    type Item = (&'a str, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(key, value)| (key.as_str(), value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for DictIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner
            .next_back()
            .map(|(key, value)| (key.as_str(), value))
    }
}

impl ExactSizeIterator for DictIter<'_> {}

impl FusedIterator for DictIter<'_> {}

/// Iterator over `(key, value)` pairs of a `DictValue`, with mutable values.
#[derive(Debug)]
pub struct DictIterMut<'a> {
    inner: btree_map::IterMut<'a, String, Value>,
}

impl<'a> DictIterMut<'a> {
    pub(crate) fn new(inner: btree_map::IterMut<'a, String, Value>) -> DictIterMut<'a> {
        DictIterMut { inner }
    }
}

impl<'a> Iterator for DictIterMut<'a> {
    type Item = (&'a str, &'a mut Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(key, value)| (key.as_str(), value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for DictIterMut<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner
            .next_back()
            .map(|(key, value)| (key.as_str(), value))
    }
}

impl ExactSizeIterator for DictIterMut<'_> {}

impl FusedIterator for DictIterMut<'_> {}

/// Owning iterator over `(key, value)` pairs of a `DictValue`, in key order.
#[derive(Debug)]
pub struct DictIntoIter {
    inner: btree_map::IntoIter<String, Value>,
}

impl DictIntoIter {
    pub(crate) fn new(inner: btree_map::IntoIter<String, Value>) -> DictIntoIter {
        DictIntoIter { inner }
    }
}

impl Iterator for DictIntoIter {
    type Item = (String, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for DictIntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl ExactSizeIterator for DictIntoIter {}

impl FusedIterator for DictIntoIter {}

#[cfg(test)]
mod tests {
    // Ports of base/value_iterators_unittest.cc. The C++ `dict_iterator` is
    // `DictIterMut` here, and `const_dict_iterator` is `DictIter`. C++ tests
    // that compare iterator objects with `operator==` compare the sequences
    // the Rust iterators yield.

    use crate::dict::DictValue;
    use crate::value::{Value, ValueView};

    /// Builds a dictionary from `(key, int)` pairs.
    fn dict_with(entries: &[(&str, i32)]) -> DictValue {
        let mut dict = DictValue::new();
        for (key, value) in entries {
            dict.set(key, *value);
        }
        dict
    }

    // `IsAssignable`: the key of an item is a shared `&str`, and the value is
    // a shared `&Value` from `DictIter`, or a `&mut Value` from `DictIterMut`.
    // The type annotations below are the Rust form of the C++ `static_assert`s.
    #[test]
    fn is_assignable() {
        let dict = dict_with(&[("0", 0)]);
        let item: (&str, &Value) = dict.iter().next().expect("one entry");
        assert_eq!(item.0, "0");
        assert_eq!(item.1, &Value::from(0));

        let mut dict = dict_with(&[("0", 0)]);
        let item: (&str, &mut Value) = dict.iter_mut().next().expect("one entry");
        *item.1 = Value::from(7);
        assert_eq!(dict.find("0"), Some(&Value::from(7)));
    }

    // `DictIteratorOperatorStar`: `*iter` gives `(key, value)`, and the value
    // can be changed through the reference.
    #[test]
    fn dict_iterator_operator_star() {
        let mut dict = dict_with(&[("0", 0)]);
        let (key, value) = dict.iter_mut().next().expect("one entry");
        assert_eq!(key, "0");
        assert_eq!(*value, Value::from(0));
        *value = Value::from(1);
        assert_eq!(dict.find("0"), Some(&Value::from(1)));
    }

    // `DictIteratorOperatorArrow`: the C++ `iter->first` and `iter->second`
    // are tuple fields in Rust.
    #[test]
    fn dict_iterator_operator_arrow() {
        let mut dict = dict_with(&[("0", 0)]);
        let mut iter = dict.iter_mut();
        let item = iter.next().expect("one entry");
        assert_eq!(item.0, "0");
        assert_eq!(*item.1, Value::from(0));
        *item.1 = Value::from(1);
        assert_eq!(dict.find("0"), Some(&Value::from(1)));
    }

    // `DictIteratorPreIncrement`: `next()` moves forward and yields the
    // element at the new position.
    #[test]
    fn dict_iterator_pre_increment() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let first = iter.next().expect("first");
        assert_eq!((first.0, first.1), ("0", &Value::from(0)));
        let second = iter.next().expect("second");
        assert_eq!((second.0, second.1), ("1", &Value::from(1)));
        assert!(iter.next().is_none());
    }

    // `DictIteratorPostIncrement`: `iter++` returns the old position and
    // advances `iter`. A `clone()` of the iterator keeps the old position.
    #[test]
    fn dict_iterator_post_increment() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let iter_old = iter.clone();
        let old_first = iter_old.clone().next().expect("old first");
        assert_eq!((old_first.0, old_first.1), ("0", &Value::from(0)));
        iter.next();
        let current = iter.next().expect("current");
        assert_eq!((current.0, current.1), ("1", &Value::from(1)));
    }

    // `DictIteratorPreDecrement`: `next_back()` moves backward from the end.
    #[test]
    fn dict_iterator_pre_decrement() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let last = iter.next_back().expect("last");
        assert_eq!((last.0, last.1), ("1", &Value::from(1)));
        let prev = iter.next_back().expect("prev");
        assert_eq!((prev.0, prev.1), ("0", &Value::from(0)));
        assert!(iter.next_back().is_none());
    }

    // `DictIteratorPostDecrement`: `iter--` returns the old position and moves
    // `iter` back. A `clone()` made before the step keeps the old position.
    #[test]
    fn dict_iterator_post_decrement() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let iter_old = iter.clone();
        let old_last = iter_old.clone().next_back().expect("old last");
        assert_eq!((old_last.0, old_last.1), ("1", &Value::from(1)));
        let back = iter.next_back().expect("back");
        assert_eq!((back.0, back.1), ("1", &Value::from(1)));
        let current = iter.next_back().expect("current");
        assert_eq!((current.0, current.1), ("0", &Value::from(0)));
    }

    // `DictIteratorOperatorEQ`: two begin iterators of an empty dictionary
    // both produce nothing, so they are equal at the end.
    #[test]
    fn dict_iterator_operator_eq() {
        let dict = DictValue::new();
        let mut a = dict.iter();
        let mut b = dict.iter();
        assert!(a.next().is_none());
        assert!(b.next().is_none());
    }

    // `DictIteratorOperatorNE`: a begin iterator of a non-empty dictionary
    // yields an element, so it is not at the end.
    #[test]
    fn dict_iterator_operator_ne() {
        let dict = dict_with(&[("0", 0)]);
        let mut iter = dict.iter();
        assert!(iter.next().is_some());
        assert!(iter.next().is_none());
    }

    // `ConstDictIteratorOperatorStar`: the shared iterator yields `(&str,
    // &Value)`.
    #[test]
    fn const_dict_iterator_operator_star() {
        let dict = dict_with(&[("0", 0)]);
        let (key, value) = dict.iter().next().expect("one entry");
        assert_eq!(key, "0");
        assert_eq!(value, &Value::from(0));
        assert_eq!(value.view(), ValueView::Int(0));
    }

    // `ConstDictIteratorOperatorArrow`: same as the star test, with tuple
    // fields.
    #[test]
    fn const_dict_iterator_operator_arrow() {
        let dict = dict_with(&[("0", 0)]);
        let item = dict.iter().next().expect("one entry");
        assert_eq!(item.0, "0");
        assert_eq!(item.1, &Value::from(0));
    }

    // `ConstDictIteratorPreIncrement`.
    #[test]
    fn const_dict_iterator_pre_increment() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let first = iter.next().expect("first");
        assert_eq!((first.0, first.1), ("0", &Value::from(0)));
        let second = iter.next().expect("second");
        assert_eq!((second.0, second.1), ("1", &Value::from(1)));
    }

    // `ConstDictIteratorPostIncrement`.
    #[test]
    fn const_dict_iterator_post_increment() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let iter_old = iter.clone();
        let old_first = iter_old.clone().next().expect("old first");
        assert_eq!((old_first.0, old_first.1), ("0", &Value::from(0)));
        iter.next();
        let current = iter.next().expect("current");
        assert_eq!((current.0, current.1), ("1", &Value::from(1)));
    }

    // `ConstDictIteratorPreDecrement`.
    #[test]
    fn const_dict_iterator_pre_decrement() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let last = iter.next_back().expect("last");
        assert_eq!((last.0, last.1), ("1", &Value::from(1)));
        let prev = iter.next_back().expect("prev");
        assert_eq!((prev.0, prev.1), ("0", &Value::from(0)));
    }

    // `ConstDictIteratorPostDecrement`.
    #[test]
    fn const_dict_iterator_post_decrement() {
        let dict = dict_with(&[("0", 0), ("1", 1)]);
        let mut iter = dict.iter();
        let iter_old = iter.clone();
        let old_last = iter_old.clone().next_back().expect("old last");
        assert_eq!((old_last.0, old_last.1), ("1", &Value::from(1)));
        let back = iter.next_back().expect("back");
        assert_eq!((back.0, back.1), ("1", &Value::from(1)));
        let current = iter.next_back().expect("current");
        assert_eq!((current.0, current.1), ("0", &Value::from(0)));
    }

    // `ConstDictIteratorOperatorEQ`.
    #[test]
    fn const_dict_iterator_operator_eq() {
        let dict = DictValue::new();
        assert!(dict.iter().next().is_none());
        assert!(dict.iter().next_back().is_none());
    }

    // `ConstDictIteratorOperatorNE`.
    #[test]
    fn const_dict_iterator_operator_ne() {
        let dict = dict_with(&[("0", 0)]);
        assert!(dict.iter().next().is_some());
        let mut it = dict.iter();
        it.next();
        assert!(it.next().is_none());
    }
}
