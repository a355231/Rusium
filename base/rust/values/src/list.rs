// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `base::ListValue` (base/values.{h,cc}).

use std::fmt;
use std::ops::{Index, IndexMut, Range};

use crate::dict::DictValue;
use crate::json;
use crate::value::{Value, ValueView};

/// A list of `Value`s.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct ListValue {
    storage: Vec<Value>,
}

impl ListValue {
    /// Creates an empty list.
    pub fn new() -> ListValue {
        ListValue::default()
    }

    /// `with_capacity()`: an empty list with room for `capacity` values.
    pub fn with_capacity(capacity: usize) -> ListValue {
        ListValue {
            storage: Vec::with_capacity(capacity),
        }
    }

    /// `reserve()`: makes room for `capacity` values without changing the size.
    pub fn reserve(&mut self, capacity: usize) {
        self.storage.reserve(capacity);
    }

    /// `empty()`.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// `size()`.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// `begin()`/`end()`.
    pub fn iter(&self) -> std::slice::Iter<'_, Value> {
        self.storage.iter()
    }

    /// `begin()`/`end()`, mutable.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Value> {
        self.storage.iter_mut()
    }

    /// `front()`: the first value, or `None` when the list is empty. The C++
    /// version `CHECK()`s instead.
    pub fn front(&self) -> Option<&Value> {
        self.storage.first()
    }

    /// `front()`, mutable.
    pub fn front_mut(&mut self) -> Option<&mut Value> {
        self.storage.first_mut()
    }

    /// `back()`: the last value, or `None` when the list is empty.
    pub fn back(&self) -> Option<&Value> {
        self.storage.last()
    }

    /// `back()`, mutable.
    pub fn back_mut(&mut self) -> Option<&mut Value> {
        self.storage.last_mut()
    }

    /// `resize()`. New values are `Value::None`, and extra values are removed
    /// from the end.
    pub fn resize(&mut self, new_size: usize) {
        self.storage.resize_with(new_size, Value::default);
    }

    /// `contains(bool)`: `true` if some element is a bool equal to `val`.
    pub fn contains_bool(&self, val: bool) -> bool {
        self.storage.iter().any(|v| v.get_if_bool() == Some(val))
    }

    /// `contains(int)`: `true` if some element is an int equal to `val`.
    pub fn contains_int(&self, val: i32) -> bool {
        self.storage.iter().any(|v| v.get_if_int() == Some(val))
    }

    /// `contains(double)`. Matches a `Double` element, and also an `Int`
    /// element with the same numeric value, because C++ uses `GetIfDouble()`.
    pub fn contains_double(&self, val: f64) -> bool {
        self.storage.iter().any(|v| v.get_if_double() == Some(val))
    }

    /// `contains(std::string_view)`: `true` if some element is a string equal
    /// to `val`.
    pub fn contains_str(&self, val: &str) -> bool {
        self.storage
            .iter()
            .any(|v| v.get_if_string() == Some(val))
    }

    /// `contains(BlobStorage)`: `true` if some element is a blob equal to
    /// `val`.
    pub fn contains_blob(&self, val: &[u8]) -> bool {
        self.storage
            .iter()
            .any(|v| v.get_if_blob() == Some(val))
    }

    /// `contains(DictValue)`.
    pub fn contains_dict(&self, val: &DictValue) -> bool {
        self.storage.iter().any(|v| v.get_if_dict() == Some(val))
    }

    /// `contains(ListValue)`.
    pub fn contains_list(&self, val: &ListValue) -> bool {
        self.storage.iter().any(|v| v.get_if_list() == Some(val))
    }

    /// `clear()`: removes all values.
    pub fn clear(&mut self) {
        self.storage.clear();
    }

    /// `erase(pos)`: removes the value at `index` and returns it. The C++
    /// version returns an iterator to the next value. In Rust, the following
    /// value is at `index` after the call. Panics if `index` is out of range.
    pub fn erase(&mut self, index: usize) -> Value {
        self.storage.remove(index)
    }

    /// `erase(first, last)`: removes the values in `range`. Returns the index
    /// of the first following value, which is `range.start`. Panics if the
    /// range is out of bounds.
    pub fn erase_range(&mut self, range: Range<usize>) -> usize {
        let start = range.start;
        self.storage.drain(range);
        start
    }

    /// `Append()`: appends `value` to the end of the list.
    pub fn append<V: Into<Value>>(&mut self, value: V) {
        self.storage.push(value.into());
    }

    /// Builder form of `append()`: consumes `self`, appends `value`, and
    /// returns `self`. It replaces the C++ `Append(...) &&` overloads.
    ///
    /// ```
    /// use values::ListValue;
    /// let list = ListValue::new().with(1).with("two").with(true);
    /// assert_eq!(list.len(), 3);
    /// ```
    pub fn with<V: Into<Value>>(mut self, value: V) -> ListValue {
        self.append(value);
        self
    }

    /// `Insert(pos, value)`: inserts `value` before `index`, and returns a
    /// mutable reference to the inserted value. `index` may equal `len()`.
    /// Panics if `index > len()`.
    pub fn insert<V: Into<Value>>(&mut self, index: usize, value: V) -> &mut Value {
        self.storage.insert(index, value.into());
        &mut self.storage[index]
    }

    /// `EraseValue()`: removes every value equal to `value`. Returns the
    /// number removed.
    pub fn erase_value(&mut self, value: &Value) -> usize {
        let before = self.storage.len();
        self.storage.retain(|v| v != value);
        before - self.storage.len()
    }

    /// `EraseIf()`: removes every value for which `predicate` is `true`.
    /// Returns the number removed.
    pub fn erase_if<F>(&mut self, mut predicate: F) -> usize
    where
        F: FnMut(&Value) -> bool,
    {
        let before = self.storage.len();
        self.storage.retain(|v| !predicate(v));
        before - self.storage.len()
    }

    /// `DebugString()`: pretty-printed JSON with a trailing newline.
    pub fn debug_string(&self) -> String {
        json::debug_string(ValueView::List(self))
    }
}

impl Index<usize> for ListValue {
    type Output = Value;

    /// `operator[]`. Panics if `index >= len()`, as the C++ `CHECK_LT` does.
    fn index(&self, index: usize) -> &Value {
        &self.storage[index]
    }
}

impl IndexMut<usize> for ListValue {
    fn index_mut(&mut self, index: usize) -> &mut Value {
        &mut self.storage[index]
    }
}

impl<'a> IntoIterator for &'a ListValue {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.iter()
    }
}

impl<'a> IntoIterator for &'a mut ListValue {
    type Item = &'a mut Value;
    type IntoIter = std::slice::IterMut<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.iter_mut()
    }
}

impl IntoIterator for ListValue {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.into_iter()
    }
}

impl FromIterator<Value> for ListValue {
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> ListValue {
        ListValue {
            storage: iter.into_iter().collect(),
        }
    }
}

impl fmt::Display for ListValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.debug_string())
    }
}
