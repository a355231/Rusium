// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::Value`, `base::DictValue`, `base::ListValue` and
//! `base::ValueView` (base/values.{h,cc}), plus the dictionary iterator types
//! from base/value_iterators.{h,cc}.
//!
//! A `Value` is one of: none, bool, int (`i32`), double (`f64`), string (UTF-8),
//! binary blob (`Vec<u8>`), dictionary, or list. `DictValue` is a map from
//! string keys to `Value`s, iterated in sorted key order. `ListValue` is a
//! vector of `Value`s.
//!
//! # Mapping from the C++ API
//!
//! - `Value::Type` is `ValueType`. `type()` is `value_type()`. Type names come
//!   from `ValueType::name()`, with the same strings as `GetTypeName()`.
//! - `Clone()` is the `Clone` trait. The C++ copy constructor is deleted, but
//!   Rust `Clone` is explicit at every call site, so the same guard applies.
//! - `GetIf*()` returns `Option<T>` or `Option<&T>`. `Get*()` panics when the
//!   type is wrong. That is the Rust form of the C++ `CHECK()`.
//! - `Find*()` on a dictionary returns `Option<...>`. The typed variants return
//!   `None` when the key is missing or the type is wrong.
//! - `Set(key, value)` takes `impl Into<Value>` and returns `&mut Value`.
//!   Builder-style `Set(...) &&` calls are `DictValue::with`. Builder-style
//!   `Append(...) &&` calls are `ListValue::with`.
//! - `ValueView` is `ValueView<'a>`, an enum of borrowed views. Use
//!   `Value::view()` to match on a value.
//! - Stream output (`operator<<`) is `Display`. `DebugString()` is
//!   `debug_string()`.
//! - `ValueSerializer` and `ValueDeserializer` are traits. `Deserialize`
//!   returns `Result<Value, DeserializeError>` in place of out-parameters.
//!
//! # Intentional differences
//!
//! - `Value` is an opaque struct around a private enum. Callers use the `get_*`
//!   and `view()` accessors instead of matching on variants. This keeps the
//!   invariant that a double is always finite.
//! - Non-finite doubles become `0.0` on construction, as in C++. The C++
//!   version also reports the error through `DUMP_WILL_BE_NOTREACHED()`, which
//!   is not ported.
//! - Strings are `&str` and `String`, so the C++ UTF-8 `DCHECK`s on keys and
//!   values can never fail. There is no death test for invalid UTF-8 input.
//!   `Value::from_utf16` converts UTF-16 input lossily, which is what
//!   `base::UTF16ToUTF8` does.
//! - `Value` and `DictValue` have no `Hash` impl. `f64` is not `Hash`, and C++
//!   has no hash for these types either. `PartialEq`, `Eq`-like equality and
//!   `PartialOrd` are provided. They order variants by declaration order, as
//!   `std::variant`'s `operator<=>` does. Ordering is partial because of
//!   doubles, as in C++.
//! - `ValueType` has `Ord` (declaration order), which the C++ enum class does
//!   not provide.
//! - `ListValue::front()` and `back()` return `Option`. The C++ versions
//!   `CHECK()` on an empty list.
//! - `ListValue::erase(index)` returns the removed `Value` (`Vec::remove`
//!   semantics). The C++ version returns an iterator to the following element.
//!   `erase_range` returns the index of the first following element.
//! - `ListValue::contains` is split into typed methods (`contains_bool`,
//!   `contains_int`, `contains_double`, `contains_str`, `contains_blob`,
//!   `contains_dict`, `contains_list`), because Rust has no overloading. Each
//!   method keeps the C++ rule for its type. For example `contains_double`
//!   matches an `Int` element with the same numeric value, because C++ uses
//!   `GetIfDouble()` there.
//! - `DictValue::contains` is `contains_key`. `DictValue::Remove` is `remove`
//!   and returns `bool`. `Extract` is `extract` and returns `Option<Value>`.
//! - `DictValue::Set_HintAtEnd` is `set_hint_at_end`. The hint is only a
//!   performance hint in C++, and `BTreeMap` has no equivalent, so it is
//!   plain `set`.
//! - Raw pointers are not `Value` constructor arguments. The C++ deleted
//!   `Value(const T*)` overloads, which block pointers from converting to
//!   `bool`, have no Rust equivalent, because `From` has no implementation for
//!   raw pointers.
//! - `DictValue` is a `BTreeMap<String, Value>`. Keys sort by UTF-8 bytes, the
//!   same order as `std::string` comparison. Entries are stored inline, so a
//!   reference to a value is not stable across insertions. C++ stores
//!   `unique_ptr<Value>`. Code that depends on pointer stability is not
//!   possible in safe Rust.
//! - The bulk constructor (`FromIterator<(String, Value)>` on `DictValue`)
//!   keeps the first occurrence of duplicate keys. This matches the C++
//!   `flat_map` constructor, which stable-sorts and then calls `std::unique`.
//! - An empty dotted path is a single empty key. The C++ code has
//!   `DCHECK(!path.empty())` and in release builds treats the path as the
//!   empty key. Path components are split as `PathSplitter` does, so `"a."`
//!   is `["a"]` for lookup and set, while `extract_by_dotted_path` treats it
//!   as `["a", ""]`. That matches the C++ code, which uses a different
//!   algorithm for extraction.
//! - `DictValue::reserve`, `ListValue::reserve` (on `Vec`) and the iterator
//!   `erase(iterator)` overloads are only partly ported. `reserve` is on
//!   `ListValue` and `with_capacity`. The dictionary has no `reserve`.
//!   Dictionary removal is `remove` and `erase_if`.
//! - `EstimateMemoryUsage()` and `WriteIntoTrace()` (perfetto tracing) are not
//!   ported.
//! - `DebugString()` writes pretty-printed JSON through a port of the
//!   `JSONWriter::WriteWithOptions(OPTIONS_PRETTY_PRINT)` path, including its
//!   quirks. A binary blob writes nothing after its key, and lists use
//!   `[ a, b ]` spacing. Output ends with a newline. The default depth limit
//!   of 200 applies. Other `JSONWriter` options are not ported.
//! - `Value::FromUniquePtrValue` and `ToUniquePtrValue` are not needed. Rust's
//!   `Box<Value>` and `*boxed` do the same job.
//! - `std::swap(v, v)` self-swap safety is not testable in Rust, which forbids
//!   aliasing `&mut`. The C++ `SelfSwap` test is not ported.
//! - `sizeof(Value)` and `noexcept` checks are not ported. They describe C++
//!   object layout and exceptions.
//! - `base::Value` has no `Clone` in the C++ sense of an implicit copy, so the
//!   C++ `Copy*` tests were ported as `Clone` tests.

#![forbid(unsafe_code)]

pub mod dict;
pub mod json;
pub mod list;
pub mod value;
pub mod value_iterators;

#[cfg(test)]
mod values_unittest;

pub use dict::DictValue;
pub use list::ListValue;
pub use value::{
    error_code_is_data_error, BlobStorage, DeserializeError, Value, ValueDeserializer,
    ValueSerializer, ValueType, ValueView,
};
pub use value_iterators::{DictIntoIter, DictIter, DictIterMut};
