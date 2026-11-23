// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `base::DictValue` (base/values.{h,cc}).

use std::collections::BTreeMap;
use std::fmt;

use crate::json;
use crate::value::{BlobStorage, Value, ValueView};
use crate::list::ListValue;
use crate::value_iterators::{DictIntoIter, DictIter, DictIterMut};

/// A dictionary of string keys to `Value`s, iterated in sorted key order.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct DictValue {
    storage: BTreeMap<String, Value>,
}

/// Splits a dotted path into components, as the C++ `PathSplitter` does.
///
/// At least one component is always returned, even for an empty path. A
/// trailing dot does not add an empty component (`"a."` gives `["a"]`), but a
/// leading or doubled dot does (`".a"` gives `["", "a"]`).
fn split_path(path: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut pos = 0;
    loop {
        match path[pos..].find('.') {
            Some(dot) => {
                parts.push(&path[pos..pos + dot]);
                pos += dot + 1;
            }
            None => {
                parts.push(&path[pos..]);
                break;
            }
        }
        if pos >= path.len() {
            break;
        }
    }
    parts
}

impl DictValue {
    /// Creates an empty dictionary.
    pub fn new() -> DictValue {
        DictValue::default()
    }

    /// `empty()`.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// `size()`.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// `begin()`/`end()`: iterates over `(key, value)` pairs in key order.
    pub fn iter(&self) -> DictIter<'_> {
        DictIter::new(self.storage.iter())
    }

    /// `begin()`/`end()` with mutable values. Keys are shared, values are
    /// mutable.
    pub fn iter_mut(&mut self) -> DictIterMut<'_> {
        DictIterMut::new(self.storage.iter_mut())
    }

    /// `contains()`: `true` if `key` is an entry in this dictionary.
    pub fn contains_key(&self, key: &str) -> bool {
        self.storage.contains_key(key)
    }

    /// `clear()`: removes all entries.
    pub fn clear(&mut self) {
        self.storage.clear();
    }

    /// `Merge()`. Entries in `other` are merged into this dictionary. When both
    /// sides have a dictionary under the same key, the two are merged
    /// recursively. Otherwise the entry from `other` replaces this one.
    pub fn merge(&mut self, other: DictValue) {
        for (key, value) in other.storage {
            match self.storage.get_mut(&key) {
                Some(current) => {
                    if current.is_dict() && value.is_dict() {
                        current
                            .get_dict_mut()
                            .merge(value.take_dict());
                    } else {
                        *current = value;
                    }
                }
                None => {
                    self.storage.insert(key, value);
                }
            }
        }
    }

    /// `Find()`: the entry for `key`, or `None` if there is none.
    pub fn find(&self, key: &str) -> Option<&Value> {
        self.storage.get(key)
    }

    /// `Find()`, mutable.
    pub fn find_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.storage.get_mut(key)
    }

    /// `FindBool()`: `None` if the key is missing or the entry is not a bool.
    pub fn find_bool(&self, key: &str) -> Option<bool> {
        self.find(key)?.get_if_bool()
    }

    /// `FindInt()`: `None` if the key is missing or the entry is not an int.
    pub fn find_int(&self, key: &str) -> Option<i32> {
        self.find(key)?.get_if_int()
    }

    /// `FindDouble()`. Accepts both `Double` and `Int` entries, converting an
    /// int to `f64`.
    pub fn find_double(&self, key: &str) -> Option<f64> {
        self.find(key)?.get_if_double()
    }

    /// `FindString()`.
    pub fn find_string(&self, key: &str) -> Option<&str> {
        self.find(key)?.get_if_string()
    }

    /// `FindString()`, mutable.
    pub fn find_string_mut(&mut self, key: &str) -> Option<&mut String> {
        self.find_mut(key)?.get_if_string_mut()
    }

    /// `FindBlob()`.
    pub fn find_blob(&self, key: &str) -> Option<&[u8]> {
        self.find(key)?.get_if_blob()
    }

    /// `FindBlob()`, mutable.
    pub fn find_blob_mut(&mut self, key: &str) -> Option<&mut BlobStorage> {
        self.find_mut(key)?.get_if_blob_mut()
    }

    /// `FindDict()`.
    pub fn find_dict(&self, key: &str) -> Option<&DictValue> {
        self.find(key)?.get_if_dict()
    }

    /// `FindDict()`, mutable.
    pub fn find_dict_mut(&mut self, key: &str) -> Option<&mut DictValue> {
        self.find_mut(key)?.get_if_dict_mut()
    }

    /// `FindList()`.
    pub fn find_list(&self, key: &str) -> Option<&ListValue> {
        self.find(key)?.get_if_list()
    }

    /// `FindList()`, mutable.
    pub fn find_list_mut(&mut self, key: &str) -> Option<&mut ListValue> {
        self.find_mut(key)?.get_if_list_mut()
    }

    /// `EnsureDict()`: returns the dictionary at `key`. Creates an empty one if
    /// the key is missing. If the entry has another type, it is replaced.
    pub fn ensure_dict(&mut self, key: &str) -> &mut DictValue {
        if !self.find(key).is_some_and(Value::is_dict) {
            self.storage
                .insert(key.to_owned(), Value::from(DictValue::new()));
        }
        self.find_dict_mut(key)
            .expect("entry was just made a dictionary")
    }

    /// `EnsureList()`: as `ensure_dict`, but for lists.
    pub fn ensure_list(&mut self, key: &str) -> &mut ListValue {
        if !self.find(key).is_some_and(Value::is_list) {
            self.storage
                .insert(key.to_owned(), Value::from(ListValue::new()));
        }
        self.find_list_mut(key)
            .expect("entry was just made a list")
    }

    /// `Set()`: sets the entry at `key`, replacing any existing entry. Returns
    /// a mutable reference to the stored value.
    pub fn set<V: Into<Value>>(&mut self, key: &str, value: V) -> &mut Value {
        let slot = self.storage.entry(key.to_owned()).or_default();
        *slot = value.into();
        slot
    }

    /// `Set_HintAtEnd()`. The C++ hint is a performance hint only, so this is
    /// the same as `set`.
    pub fn set_hint_at_end<V: Into<Value>>(&mut self, key: &str, value: V) -> &mut Value {
        self.set(key, value)
    }

    /// Builder form of `set()`: consumes `self`, sets the entry, and returns
    /// `self`. It replaces the C++ `Set(...) &&` overloads.
    ///
    /// ```
    /// use values::DictValue;
    /// let dict = DictValue::new().with("a", 1).with("b", "text");
    /// assert_eq!(dict.find_int("a"), Some(1));
    /// ```
    pub fn with<V: Into<Value>>(mut self, key: &str, value: V) -> DictValue {
        self.set(key, value);
        self
    }

    /// `Remove()`: removes the entry for `key`. Returns `true` if an entry was
    /// removed.
    pub fn remove(&mut self, key: &str) -> bool {
        self.storage.remove(key).is_some()
    }

    /// `Extract()`: removes the entry for `key` and returns its value, or
    /// `None` if there is no such entry.
    pub fn extract(&mut self, key: &str) -> Option<Value> {
        self.storage.remove(key)
    }

    /// `EraseIf()`: removes every entry for which `predicate(key, value)` is
    /// `true`. Returns the number of entries removed.
    pub fn erase_if<F>(&mut self, mut predicate: F) -> usize
    where
        F: FnMut(&str, &Value) -> bool,
    {
        let before = self.storage.len();
        self.storage.retain(|key, value| !predicate(key, value));
        before - self.storage.len()
    }

    /// `FindByDottedPath()`: looks up a dotted path such as `"a.b.c"`. Each
    /// component except the last must name a dictionary.
    pub fn find_by_dotted_path(&self, path: &str) -> Option<&Value> {
        let parts = split_path(path);
        let (last, intermediate) = parts.split_last()?;
        let mut dict = self;
        for key in intermediate {
            dict = dict.find_dict(key)?;
        }
        dict.find(last)
    }

    /// `FindByDottedPath()`, mutable.
    pub fn find_by_dotted_path_mut(&mut self, path: &str) -> Option<&mut Value> {
        let parts = split_path(path);
        let (last, intermediate) = parts.split_last()?;
        let mut dict = self;
        for key in intermediate {
            dict = dict.find_dict_mut(key)?;
        }
        dict.find_mut(last)
    }

    /// `FindBoolByDottedPath()`.
    pub fn find_bool_by_dotted_path(&self, path: &str) -> Option<bool> {
        self.find_by_dotted_path(path)?.get_if_bool()
    }

    /// `FindIntByDottedPath()`.
    pub fn find_int_by_dotted_path(&self, path: &str) -> Option<i32> {
        self.find_by_dotted_path(path)?.get_if_int()
    }

    /// `FindDoubleByDottedPath()`. Accepts both `Double` and `Int` entries.
    pub fn find_double_by_dotted_path(&self, path: &str) -> Option<f64> {
        self.find_by_dotted_path(path)?.get_if_double()
    }

    /// `FindStringByDottedPath()`.
    pub fn find_string_by_dotted_path(&self, path: &str) -> Option<&str> {
        self.find_by_dotted_path(path)?.get_if_string()
    }

    /// `FindStringByDottedPath()`, mutable.
    pub fn find_string_by_dotted_path_mut(&mut self, path: &str) -> Option<&mut String> {
        self.find_by_dotted_path_mut(path)?.get_if_string_mut()
    }

    /// `FindBlobByDottedPath()`.
    pub fn find_blob_by_dotted_path(&self, path: &str) -> Option<&[u8]> {
        self.find_by_dotted_path(path)?.get_if_blob()
    }

    /// `FindBlobByDottedPath()`, mutable.
    pub fn find_blob_by_dotted_path_mut(&mut self, path: &str) -> Option<&mut BlobStorage> {
        self.find_by_dotted_path_mut(path)?.get_if_blob_mut()
    }

    /// `FindDictByDottedPath()`.
    pub fn find_dict_by_dotted_path(&self, path: &str) -> Option<&DictValue> {
        self.find_by_dotted_path(path)?.get_if_dict()
    }

    /// `FindDictByDottedPath()`, mutable.
    pub fn find_dict_by_dotted_path_mut(&mut self, path: &str) -> Option<&mut DictValue> {
        self.find_by_dotted_path_mut(path)?.get_if_dict_mut()
    }

    /// `FindListByDottedPath()`.
    pub fn find_list_by_dotted_path(&self, path: &str) -> Option<&ListValue> {
        self.find_by_dotted_path(path)?.get_if_list()
    }

    /// `FindListByDottedPath()`, mutable.
    pub fn find_list_by_dotted_path_mut(&mut self, path: &str) -> Option<&mut ListValue> {
        self.find_by_dotted_path_mut(path)?.get_if_list_mut()
    }

    /// `SetByDottedPath()`. Missing intermediate dictionaries are created. If
    /// an intermediate component exists but is not a dictionary, nothing is
    /// changed and `None` is returned. Otherwise returns the stored value.
    pub fn set_by_dotted_path<V: Into<Value>>(&mut self, path: &str, value: V) -> Option<&mut Value> {
        let parts = split_path(path);
        set_by_parts(self, &parts, value.into())
    }

    /// Builder form of `set_by_dotted_path()`. Consumes `self` and returns it.
    /// The result of the set is ignored, as with the C++ `&&` overloads.
    pub fn with_by_dotted_path<V: Into<Value>>(mut self, path: &str, value: V) -> DictValue {
        self.set_by_dotted_path(path, value);
        self
    }

    /// `RemoveByDottedPath()`. Returns `true` if a value was removed.
    pub fn remove_by_dotted_path(&mut self, path: &str) -> bool {
        self.extract_by_dotted_path(path).is_some()
    }

    /// `ExtractByDottedPath()`. Removes the value at `path` and returns it. A
    /// dictionary that becomes empty because of the removal is also removed.
    pub fn extract_by_dotted_path(&mut self, path: &str) -> Option<Value> {
        let Some(dot) = path.find('.') else {
            return self.extract(path);
        };
        let next_key = &path[..dot];
        let (extracted, next_is_empty) = {
            let next = self.find_dict_mut(next_key)?;
            let extracted = next.extract_by_dotted_path(&path[dot + 1..]);
            (extracted, next.is_empty())
        };
        if extracted.is_some() && next_is_empty {
            self.remove(next_key);
        }
        extracted
    }

    /// `DebugString()`: pretty-printed JSON with a trailing newline.
    pub fn debug_string(&self) -> String {
        json::debug_string(ValueView::Dict(self))
    }
}

/// Recursive part of `set_by_dotted_path`. `parts` must not be empty.
fn set_by_parts<'a>(dict: &'a mut DictValue, parts: &[&str], value: Value) -> Option<&'a mut Value> {
    let (key, rest) = parts.split_first()?;
    if rest.is_empty() {
        return Some(dict.set(key, value));
    }
    if !dict.contains_key(key) {
        dict.set(key, DictValue::new());
    }
    let next = dict.find_dict_mut(key)?;
    set_by_parts(next, rest, value)
}

impl<'a> IntoIterator for &'a DictValue {
    type Item = (&'a str, &'a Value);
    type IntoIter = DictIter<'a>;

    fn into_iter(self) -> DictIter<'a> {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut DictValue {
    type Item = (&'a str, &'a mut Value);
    type IntoIter = DictIterMut<'a>;

    fn into_iter(self) -> DictIterMut<'a> {
        self.iter_mut()
    }
}

impl IntoIterator for DictValue {
    type Item = (String, Value);
    type IntoIter = DictIntoIter;

    fn into_iter(self) -> DictIntoIter {
        DictIntoIter::new(self.storage.into_iter())
    }
}

impl FromIterator<(String, Value)> for DictValue {
    /// Builds a dictionary from pairs. When a key appears more than once, the
    /// first occurrence wins, as with the C++ bulk `flat_map` constructor.
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> DictValue {
        let mut dict = DictValue::new();
        for (key, value) in iter {
            dict.storage.entry(key).or_insert(value);
        }
        dict
    }
}

impl fmt::Display for DictValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.debug_string())
    }
}
